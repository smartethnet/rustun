use clap::Parser;
use std::sync::{Arc};
use std::time::Duration;
use tokio::time::interval;
use crate::client::{Args, P2P_HOLE_PUNCH_PORT, P2P_UDP_PORT};
use crate::client::relay::{RelayHandler, new_relay_handler};
use crate::client::p2p::peer::{PeerHandler};
use crate::client::prettylog::{get_status, log_startup_banner};
use crate::client::p2p::stun::StunClient;
use crate::client::http::server;
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
    let (mut relay_handler, device_config) = match new_relay_handler(&args,
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
    let mut p2p_handler = if args.enable_p2p {
        tracing::info!("P2P mode enabled");

        let mut handler = PeerHandler::new(
            crypto_block.clone(),
            args.identity.clone(),
        );
        handler.run_peer_service();
        handler.rewrite_peers(device_config.peer_details.clone()).await;
        handler.start_probe_timer().await;
        Some(handler)
    } else {
        tracing::info!("P2P mode disabled, using relay only");
        None
    };

    // Check iptables availability if masq is enabled (Linux only)
    #[cfg(target_os = "linux")]
    {
        if args.masq {
            if let Err(e) = crate::utils::sys_route::SysRoute::check_iptables_available() {
                eprintln!("❌ Error: {}", e);
                eprintln!("\nPlease install iptables or run without --masq option.");
                std::process::exit(1);
            }
        }
    }

    // initialize TUN device
    #[cfg(target_os = "linux")]
    let enable_masq = args.masq;
    #[cfg(not(target_os = "linux"))]
    let enable_masq = false;
    
    let mut dev = match init_device(&device_config, enable_masq).await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to initialize device: {}", e);
            return;
        }
    };

    // Start HTTP server if port is specified
    if let Some(http_port) = args.http_port {
        tokio::spawn(async move {
            if let Err(e) = server::start(http_port).await {
                tracing::error!("HTTP server error: {}", e);
            }
        });
    }

    // Run main event loop
    run_event_loop(&mut relay_handler, &mut p2p_handler, &mut dev).await;
}

async fn init_device(device_config: &HandshakeReplyFrame, enable_masq: bool) -> crate::Result<DeviceHandler> {
    tracing::info!("Initializing device with config: {:?}", device_config);
    let mut dev = DeviceHandler::new();
    let tun_index = dev.run(device_config, enable_masq).await?;

    // Log TUN index (Windows only)
    if let Some(idx) = tun_index {
        tracing::info!("TUN interface index: {}", idx);
    }

    dev.reload_route(device_config.peer_details.clone()).await;
    
    // Setup CIDR mapping DNAT rules
    if !device_config.cider_mapping.is_empty() {
        if let Err(e) = dev.setup_cidr_mapping(&device_config.cider_mapping) {
            tracing::error!("Failed to setup CIDR mapping DNAT rules: {}", e);
            // Don't fail initialization, just log the error
            // This allows the client to continue even if DNAT setup fails
        }
    }
    
    Ok(dev)
}

async fn run_event_loop(
    client_handler: &mut RelayHandler,
    p2p_handler: &mut Option<PeerHandler>,
    dev: &mut DeviceHandler,
) {
    let mut refresh_ticker = interval(Duration::from_secs(30));
    
    loop {
        tokio::select! {
            // TUN device -> Network (P2P or Relay)
            packet = dev.recv() => {
                if let Some(packet) = packet {
                    handle_device_packet(client_handler, p2p_handler, packet).await;
                }
            }

            // Server -> TUN device or route update
            frame = client_handler.recv_frame() => {
                if let Ok(frame) = frame {
                    handle_relay_frame(frame, p2p_handler, dev).await;
                }
            }

            // P2P -> TUN device (only if P2P enabled)
            frame = async {
                match p2p_handler {
                    Some(handler) => handler.recv_frame().await,
                    None => std::future::pending().await, // Never resolves if no P2P
                }
            } => {
                if let Ok(Frame::Data(data_frame)) = frame {
                    tracing::debug!("P2P -> Device: {} bytes", data_frame.payload.len());
                    if let Err(e) = dev.send(data_frame.payload).await {
                        tracing::error!("Failed to write to device: {}", e);
                    }
                }
            }

            // refresh config and status
            _ = refresh_ticker.tick() => {
                get_status(client_handler, p2p_handler.as_ref(), dev).await;

            }
        }
    }
}

/// Handle outbound packet from TUN device
async fn handle_device_packet(
    client_handler: &mut RelayHandler,
    p2p_handler: &mut Option<PeerHandler>,
    packet: Vec<u8>,
) {
    let data_frame = DataFrame { payload: packet.clone() };
    
    // Try P2P first if available
    if let Some(p2p) = p2p_handler {
        let dst = data_frame.dst();
        match p2p.send_frame(Frame::Data(data_frame), dst.as_str()).await {
            Ok(_) => {
                tracing::debug!("Device -> P2P: {} bytes", packet.len());
                return;
            }
            Err(e) => {
                tracing::debug!("P2P send failed: {}, fallback to relay", e);
            }
        }
    }
    
    // Fallback to relay (or direct if no P2P)
    let frame = Frame::Data(DataFrame { payload: packet });
    if let Err(e) = client_handler.send_frame(frame).await {
        tracing::error!("Failed to send via relay: {}", e);
    }
}

/// Handle frame received from relay server
async fn handle_relay_frame(
    frame: Frame,
    p2p_handler: &mut Option<PeerHandler>,
    dev: &mut DeviceHandler,
) {
    match frame {
        Frame::Data(data_frame) => {
            tracing::debug!("Relay -> Device: {} bytes", data_frame.payload.len());
            if let Err(e) = dev.send(data_frame.payload).await {
                tracing::error!("Failed to write to device: {}", e);
            }
        }
        Frame::KeepAlive(keepalive) => {
            tracing::debug!("Received keepalive with {:?} peer details", keepalive.peer_details);
            
            // Update routes in device handler
            dev.reload_route(keepalive.peer_details.clone()).await;
            
            // Update P2P peer information if P2P is enabled
            if let Some(p2p) = p2p_handler {
                p2p.insert_or_update(keepalive.peer_details).await;
            }
        }
        _ => {}
    }
}
