use clap::Parser;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use crate::client::{Args, P2P_HOLE_PUNCH_PORT, P2P_UDP_PORT};
use crate::client::relay::{RelayHandler, new_relay_handler};
use crate::client::p2p::peer::{PeerHandler};
use crate::client::prettylog::{get_status, log_startup_banner};
use crate::client::p2p::stun::StunClient;
use crate::codec::frame::{DataFrame, Frame, HandshakeReplyFrame};
use crate::crypto::{self, Block};
use crate::utils;
use crate::utils::device::{DeviceHandler};

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

    let ipv6 = utils::get_ipv6().unwrap_or(String::new());
    let stun_result = StunClient::new().discover(P2P_HOLE_PUNCH_PORT).await;
    let (stun_ip, stun_port) = match stun_result {
        Ok(result) => (result.public_ip.to_string(), result.public_port),
        Err(_) => {
            ("".to_string(), 0)
        }
    };

    // create relay handler
    let (mut relay_handler,
        device_config,
        config_update_signal) = match new_relay_handler(&args,
                                                        crypto_block.clone(),
                                                        ipv6, P2P_UDP_PORT,
                                                        stun_ip,
                                                        stun_port).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to setup client: {}", e);
            return;
        }
    };

    // Initialize P2P handler if enabled
    let mut peer_handler = if args.enable_p2p {
        tracing::info!("P2P mode enabled");

        let mut handler = PeerHandler::new(
            crypto_block.clone(),
            args.identity.clone(),
        );
        handler.run_peer();
        handler.add_peers(device_config.others.clone()).await;
        handler.start_probe_timer().await;
        Some(handler)
    } else {
        tracing::info!("P2P mode disabled, using relay only");
        None
    };

    // initialize TUN device
    let mut dev = match init_device(&device_config).await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to initialize device: {}", e);
            return;
        }
    };

    // Run main event loop
    run_event_loop(&mut relay_handler, &mut peer_handler, &mut dev, config_update_signal).await;
}

async fn init_device(device_config: &HandshakeReplyFrame) -> crate::Result<DeviceHandler> {
    tracing::info!("Initializing device with config: {:?}", device_config);
    let mut dev = DeviceHandler::new();
    let tun_index = dev.run(device_config).await?;

    // Log TUN index (Windows only)
    if let Some(idx) = tun_index {
        tracing::info!("TUN interface index: {}", idx);
    }

    dev.reload_route(device_config.others.clone()).await;
    
    Ok(dev)
}

async fn run_event_loop(
    client_handler: &mut RelayHandler,
    peer_handler: &mut Option<PeerHandler>,
    dev: &mut DeviceHandler,
    mut config_update_signal: mpsc::Receiver<HandshakeReplyFrame>,
) {
    let mut exporter_ticker = interval(Duration::from_secs(30));
    loop {
        // Build select branches based on whether P2P is enabled
        if let Some(peer_handler) = peer_handler {
            // P2P enabled: try P2P first, fallback to relay
            tokio::select! {
                config = config_update_signal.recv() => {
                    dev.reload_route(config.unwrap().others.clone()).await;
                }
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

                // Server -> TUN device or peer update
                frame = client_handler.recv_frame() => {
                    match frame {
                        Ok(Frame::Data(data_frame)) => {
                            tracing::debug!("Relay -> Device: {} bytes", data_frame.payload.len());
                            if let Err(e) = dev.send(data_frame.payload).await {
                                tracing::error!("Failed to write to device: {}", e);
                            }
                        }
                        Ok(Frame::PeerUpdate(peer_update)) => {
                            tracing::info!(
                                "Peer update received: {} -> {}:{}\n stun: {}:{}",
                                peer_update.identity,
                                peer_update.ipv6,
                                peer_update.port,
                                peer_update.stun_ip,
                                peer_update.stun_port,
                            );
                            peer_handler.update_peer(
                                peer_update.identity,
                                peer_update.ipv6,
                                peer_update.port,
                                peer_update.stun_ip,
                                peer_update.stun_port,
                            ).await;
                        }
                        _ => {}
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
                _ = exporter_ticker.tick() => {
                    get_status(client_handler, Some(peer_handler), dev).await;
                }
            }
        } else {
            // P2P disabled: relay only
            tokio::select! {
                config = config_update_signal.recv() => {
                    dev.reload_route(config.unwrap().others.clone()).await;
                }
                // TUN device -> Server (relay only)
                packet = dev.recv() => {
                    if let Some(packet) = packet {
                        let frame = Frame::Data(DataFrame { payload: packet });
                        if let Err(e) = client_handler.send_frame(frame).await {
                            tracing::error!("Failed to send via relay: {}", e);
                        }
                    }
                }

                // Server -> TUN device (relay only, ignore peer updates)
                frame = client_handler.recv_frame() => {
                    if let Ok(Frame::Data(data_frame)) = frame {
                        tracing::debug!("Relay -> Device: {} bytes", data_frame.payload.len());
                        if let Err(e) = dev.send(data_frame.payload).await {
                            tracing::error!("Failed to write to device: {}", e);
                        }
                    }
                }
                _ = exporter_ticker.tick() => {
                    get_status(client_handler, None, dev).await;
                }
            }
        }
    }
}
