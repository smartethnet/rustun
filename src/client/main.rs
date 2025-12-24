use clap::Parser;
use std::sync::Arc;

use crate::client::relay::{ClientHandler, new_relay_handler};
use crate::client::peer::{PeerHandler};
use crate::client::prettylog::{log_startup_banner};
use crate::codec::frame::{DataFrame, Frame, HandshakeReplyFrame};
use crate::crypto::{self, Block};
use crate::utils;
use crate::utils::device::{DeviceConfig, DeviceHandler};
use crate::utils::sys_route::SysRoute;

const DEFAULT_MTU: u16 = 1430;

/// Rustun VPN Client
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Server address (e.g., 127.0.0.1:8080)
    #[arg(short, long)]
    pub server: String,

    /// Client identity/name
    #[arg(short, long)]
    pub identity: String,

    /// Encryption method: plain, aes256:<key>, chacha20:<key>, or xor:<key>
    #[arg(short, long, default_value = "chacha20:rustun")]
    pub crypto: String,

    /// Keep-alive interval in seconds
    #[arg(long, default_value = "10")]
    pub keepalive_interval: u64,

    /// Keep-alive threshold (reconnect after this many failures)
    #[arg(long, default_value = "5")]
    pub keepalive_threshold: u8,

    /// Enable P2P direct connection (disabled by default, uses relay only)
    #[arg(long)]
    pub enable_p2p: bool,
}

pub async fn run_client() {
    let args = Args::parse();

    if let Err(e) = utils::init_tracing() {
        eprintln!("Failed to initialize logging: {}", e);
        return;
    }

    log_startup_banner(&args);

    // parse crypto configuration
    let crypto_config = match crypto::parse_crypto_config(&args.crypto) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("Invalid crypto configuration: {}", e);
            return;
        }
    };
    let block = crypto::new_block(&crypto_config);
    let crypto_block: Arc<Box<dyn Block>> = Arc::new(block);

    // create relay handler
    let (mut relay_handler, device_config) = match new_relay_handler(&args, crypto_block.clone()).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to setup client: {}", e);
            return;
        }
    };

    // Initialize P2P handler if enabled
    let mut peer_handler = if args.enable_p2p {
        tracing::info!("P2P mode enabled");
        let mut handler = PeerHandler::new(crypto_block.clone());
        handler.run_peer(51258);
        handler.add_peers(device_config.others.clone()).await;
        handler.start_keepalive_timer().await;
        Some(handler)
    } else {
        tracing::info!("P2P mode disabled, using relay only");
        None
    };

    // initialize TUN device
    let mut dev = match init_device(&device_config) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to initialize device: {}", e);
            return;
        }
    };

    // Run main event loop
    run_event_loop(&mut relay_handler, &mut peer_handler, &mut dev).await;
}

fn init_device(device_config: &HandshakeReplyFrame) -> crate::Result<DeviceHandler> {
    let mut dev = DeviceHandler::new();
    dev.run(DeviceConfig {
        ip: device_config.private_ip.clone(),
        mask: device_config.mask.clone(),
        gateway: device_config.gateway.clone(),
        mtu: DEFAULT_MTU,
    })?;

    // Add system routes for peers
    let sys_route = SysRoute::default();
    for route_item in &device_config.others {
        if let Err(e) = sys_route.add(route_item.ciders.clone(), device_config.private_ip.clone()) {
            tracing::error!("Failed to add route for {:?}: {}", route_item, e);
        }
    }
    
    Ok(dev)
}

async fn run_event_loop(
    client_handler: &mut ClientHandler,
    peer_handler: &mut Option<PeerHandler>,
    dev: &mut DeviceHandler,
) {
    loop {
        // Build select branches based on whether P2P is enabled
        if let Some(peer_handler) = peer_handler {
            // P2P enabled: try P2P first, fallback to relay
            tokio::select! {
                // TUN device -> P2P or Server
                packet = dev.recv() => {
                    if let Some(packet) = packet {
                        let data_frame = DataFrame{ payload: packet.clone() };
                        let dst = data_frame.dst();
                        let frame = Frame::Data(data_frame);
                        
                        // Try P2P first
                        match peer_handler.send_frame(frame, dst.as_str()).await {
                            Ok(_) => {
                                tracing::debug!("Device -> P2P: {} bytes", packet.len());
                                continue;
                            }
                            Err(e) => {
                                tracing::debug!("P2P send failed: {}, fallback to relay", e);
                            }
                        }

                        // Fallback to relay
                        let frame = Frame::Data(DataFrame { payload: packet });
                        if let Err(e) = client_handler.send_frame(frame).await {
                            tracing::error!("Failed to send via relay: {}", e);
                        }
                    }
                }

                // Server -> TUN device
                frame = client_handler.recv_frame() => {
                    if let Ok(Frame::Data(data_frame)) = frame {
                        tracing::debug!("Relay -> Device: {} bytes", data_frame.payload.len());
                        if let Err(e) = dev.send(data_frame.payload).await {
                            tracing::error!("Failed to write to device: {}", e);
                        }
                    }
                }

                // Peers -> TUN device
                frame = peer_handler.recv_frame() => {
                    if let Ok(Frame::Data(data_frame)) = frame {
                        tracing::debug!("P2P -> Device: {} bytes", data_frame.payload.len());
                        if let Err(e) = dev.send(data_frame.payload).await {
                            tracing::error!("Failed to write to device: {}", e);
                        }
                    }
                }
            }
        } else {
            // P2P disabled: relay only
            tokio::select! {
                // TUN device -> Server (relay only)
                packet = dev.recv() => {
                    if let Some(packet) = packet {
                        let frame = Frame::Data(DataFrame { payload: packet });
                        if let Err(e) = client_handler.send_frame(frame).await {
                            tracing::error!("Failed to send via relay: {}", e);
                        }
                    }
                }

                // Server -> TUN device
                frame = client_handler.recv_frame() => {
                    if let Ok(Frame::Data(data_frame)) = frame {
                        tracing::debug!("Relay -> Device: {} bytes", data_frame.payload.len());
                        if let Err(e) = dev.send(data_frame.payload).await {
                            tracing::error!("Failed to write to device: {}", e);
                        }
                    }
                }
            }
        }
    }
}
