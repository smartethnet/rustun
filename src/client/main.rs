use clap::Parser;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use crate::client::{Args, DEFAULT_MTU, P2P_UDP_PORT};
use crate::client::relay::{RelayHandler, new_relay_handler};
use crate::client::peer::{PeerHandler};
use crate::client::prettylog::{log_startup_banner};
use crate::codec::frame::{DataFrame, Frame, HandshakeReplyFrame};
use crate::crypto::{self, Block};
use crate::utils;
use crate::utils::device::{DeviceConfig, DeviceHandler};
use crate::utils::sys_route::SysRoute;

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
        let mut handler = PeerHandler::new(
            crypto_block.clone(),
            args.identity.clone(),
            device_config.ipv6.clone(),
            P2P_UDP_PORT,
        );
        handler.run_peer();
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
    let sys_route = SysRoute::new();
    for route_item in &device_config.others {
        if let Err(e) = sys_route.add(route_item.ciders.clone(), device_config.private_ip.clone()) {
            tracing::error!("Failed to add route for {:?}: {}", route_item, e);
        }
    }
    
    Ok(dev)
}

async fn run_event_loop(
    client_handler: &mut RelayHandler,
    peer_handler: &mut Option<PeerHandler>,
    dev: &mut DeviceHandler,
) {
    let mut exporter_ticker = interval(Duration::from_secs(30));
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
                                "Peer update received: {} -> {}:{}",
                                peer_update.identity,
                                peer_update.ipv6,
                                peer_update.port
                            );
                            peer_handler.update_peer(
                                peer_update.identity,
                                peer_update.ipv6,
                                peer_update.port
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
                    get_status(client_handler, Some(peer_handler)).await;
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
                    get_status(client_handler, None).await;
                }
            }
        }
    }
}

async fn get_status(relay: &RelayHandler, peer: Option<&PeerHandler>) {
    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                        CONNECTION STATUS                             ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    
    // Relay Status
    let relay_status = relay.get_status();
    println!("\n📡 Relay Connection (TCP)");
    println!("   ├─ RX Frames:  {} (Errors: {})", relay_status.rx_frame, relay_status.rx_error);
    println!("   └─ TX Frames:  {} (Errors: {})", relay_status.tx_frame, relay_status.tx_error);
    
    // P2P Status
    if let Some(peer_handler) = peer {
        let peer_status = peer_handler.get_status().await;
        
        if peer_status.is_empty() {
            println!("\n🔗 P2P Connections (UDP)");
            println!("   └─ No peers configured");
        } else {
            println!("\n🔗 P2P Connections (UDP): {} peers", peer_status.len());
            
            for (idx, status) in peer_status.iter().enumerate() {
                let is_last = idx == peer_status.len() - 1;
                let prefix = if is_last { "└─" } else { "├─" };
                
                // Format connection state
                let state = match (&status.addr, &status.last_active) {
                    (None, _) => "❌ Unknown Address".to_string(),
                    (Some(_), None) => "⏳ Connecting...".to_string(),
                    (Some(_), Some(last)) => {
                        let elapsed = last.elapsed().as_secs();
                        if elapsed < 15 {
                            format!("✅ Active ({}s ago)", elapsed)
                        } else {
                            format!("⚠️  Inactive ({}s ago)", elapsed)
                        }
                    }
                };
                
                // Format address
                let addr_str = status.addr
                    .map(|a| format!("{}", a))
                    .unwrap_or_else(|| "N/A".to_string());
                
                println!("   {} Peer: {}", prefix, status.identity);
                println!("   {}    Address: {}", if is_last { " " } else { "│" }, addr_str);
                println!("   {}    Status:  {}", if is_last { " " } else { "│" }, state);
            }
        }
    } else {
        println!("\n🔗 P2P Mode: Disabled");
    }
    
    println!();
}
