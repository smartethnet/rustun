use crate::client::http::server;
use crate::client::p2p::peer::{NewPeersTx, PeerHandler, PeerHandlerApi, SendFrame, SendFrameTx};
use crate::client::p2p::stun::StunClient;
use crate::client::prettylog::{get_status, log_startup_banner};
use crate::client::relay::{RelayHandler, new_relay_handler};
use crate::client::{Args, P2P_HOLE_PUNCH_PORT, P2P_UDP_PORT};
use crate::codec::frame::{DataFrame, Frame, HandshakeReplyFrame};
use crate::crypto::{self, Block};
use crate::utils::device::DeviceHandler;
use crate::utils::{self, StunAddr};
use clap::Parser;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;

pub async fn run_client() -> anyhow::Result<()> {
    let args = Args::parse();

    if let Err(e) = utils::init_tracing() {
        anyhow::bail!("Failed to initialize logging: {}", e);
    }

    log_startup_banner(&args);

    // parse crypto configuration
    let crypto_config = match crypto::parse_crypto_config(&args.crypto) {
        Ok(cfg) => cfg,
        Err(e) => {
            anyhow::bail!("Invalid crypto configuration: {}", e);
        }
    };
    let block = crypto::new_block(&crypto_config);
    let crypto_block: Arc<Box<dyn Block>> = Arc::new(block);

    let ipv6 = utils::get_ipv6().await;
    let stun_result = StunClient::new().discover(P2P_HOLE_PUNCH_PORT).await;
    let stun = match stun_result {
        Ok(result) => Some(StunAddr {
            ip: result.public_ip.to_string(),
            port: result.public_port,
        }),
        Err(_) => None,
    };

    // create relay handler
    let (mut relay_handler, device_config) =
        match new_relay_handler(&args, crypto_block.clone(), ipv6, P2P_UDP_PORT, stun).await {
            Ok(result) => result,
            Err(e) => {
                anyhow::bail!("Failed to setup client: {}", e);
            }
        };

    // Initialize P2P handler if enabled (wrapped in Arc<RwLock<>> for sharing with device packet task)
    let p2p_handler = if args.enable_p2p {
        tracing::info!("P2P mode enabled");

        let handler = PeerHandler::start_peer_service(
            crypto_block.clone(),
            args.identity.clone(),
            device_config.peer_details.clone(),
        );
        Some(handler)
    } else {
        tracing::info!("P2P mode disabled, using relay only");
        None
    };

    // Check iptables availability if masq is enabled (Linux only)
    #[cfg(target_os = "linux")]
    {
        if args.masq
            && let Err(e) = crate::utils::sys_route::SysRoute::check_iptables_available()
        {
            anyhow::bail!("❌ Error: {e}\nPlease install iptables or run without --masq option.");
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
            anyhow::bail!("Failed to initialize device: {}", e);
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
    run_event_loop(&mut relay_handler, p2p_handler, &mut dev).await;
    Ok(())
}

async fn init_device(
    device_config: &HandshakeReplyFrame,
    enable_masq: bool,
) -> anyhow::Result<DeviceHandler> {
    tracing::info!("Initializing device with config: {:?}", device_config);
    let mut dev = DeviceHandler::new();
    let tun_index = dev.run(device_config, enable_masq).await?;

    // Log TUN index (Windows only)
    if let Some(idx) = tun_index {
        tracing::info!("TUN interface index: {}", idx);
    }

    dev.reload_route(device_config.peer_details.clone()).await;

    // Setup CIDR mapping DNAT rules
    if !device_config.cider_mapping.is_empty()
        && let Err(e) = dev.setup_cidr_mapping(&device_config.cider_mapping)
    {
        tracing::error!("Failed to setup CIDR mapping DNAT rules: {}", e);
        // Don't fail initialization, just log the error
        // This allows the client to continue even if DNAT setup fails
    }

    Ok(dev)
}

async fn run_event_loop(
    client_handler: &mut RelayHandler,
    p2p_handler: Option<PeerHandlerApi>,
    dev: &mut DeviceHandler,
) {
    let (
        p2p_handler_new_peers,
        mut p2p_handler_recv_frame,
        p2p_handler_get_status,
        p2p_handler_send_frame,
    ) = match p2p_handler {
        Some(p) => (
            Some(p.new_peers),
            Some(p.new_frame),
            Some(p.get_status),
            Some(p.send_frame),
        ),
        None => (None, None, None, None),
    };
    let mut refresh_ticker = interval(Duration::from_secs(30));
    let relay_outbound = match client_handler.get_outbound_tx() {
        Some(tx) => tx,
        None => return,
    };

    let mut dev_inbound = match dev.get_dev_inbound() {
        Some(dev) => dev,
        None => return,
    };

    tokio::spawn(async move {
        loop {
            let packet = dev_inbound.recv().await;
            if let Some(packet) = packet {
                handle_device_packet(
                    relay_outbound.clone(),
                    p2p_handler_send_frame.as_ref(),
                    packet,
                )
                .await;
            }
        }
    });

    loop {
        tokio::select! {
            // Server -> TUN device or route update
            frame = client_handler.recv_frame() => {
                if let Ok(frame) = frame {
                    handle_relay_frame(frame, p2p_handler_new_peers.as_ref(), dev).await;
                }
            }

            // P2P -> TUN device (only if P2P enabled)
            Some(frame) = async {
                match p2p_handler_recv_frame.as_mut() {
                    Some(rx) => rx.0.recv().await,
                    None => std::future::pending().await, // Never resolves if no P2P
                }
            } => {
                let Frame::Data(data_frame) = frame else {
                    continue;
                };
                tracing::debug!("P2P -> Device: {} bytes", data_frame.payload.len());
                if let Err(e) = dev.send(data_frame.payload).await {
                    tracing::error!("Failed to write to device: {}", e);
                }
            }

            // refresh config and status
            _ = refresh_ticker.tick() => {
                let peer_status = match p2p_handler_get_status.as_ref() {
                    None => None,
                    Some(p) => match p.get().await {
                        Ok(s) => Some(s),
                        Err(e) => {
                            tracing::error!("failed to get peer status: {e}");
                            None
                        }
                    },
                };
                get_status(client_handler, peer_status.as_deref(), dev).await;
            }
        }
    }
}

/// Handle outbound packet from TUN device: try P2P first if available, then fallback to relay.
async fn handle_device_packet(
    relay_outbound: mpsc::Sender<Frame>,
    p2p_handler: Option<&SendFrameTx>,
    packet: Vec<u8>,
) {
    let data_frame = DataFrame {
        payload: packet.clone(),
    };

    // Try P2P first if available
    if let Some(tx) = &p2p_handler {
        let dst = data_frame.dst();
        let frame = SendFrame {
            frame: Frame::Data(data_frame.clone()),
            dst,
        };

        match tx.0.send(frame).await {
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
    if let Err(e) = RelayHandler::send_frame(relay_outbound, frame).await {
        tracing::error!("Failed to send via relay: {}", e);
    }
}

/// Handle frame received from relay server
async fn handle_relay_frame(
    frame: Frame,
    p2p_handler: Option<&NewPeersTx>,
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
            tracing::debug!(
                "Received keepalive with {:?} peer details",
                keepalive.peer_details
            );

            // Update routes in device handler
            dev.reload_route(keepalive.peer_details.clone()).await;

            // Update P2P peer information if P2P is enabled
            if let Some(tx) = p2p_handler {
                let _ = tx.0.send(keepalive.peer_details).await;
            }
        }
        _ => {}
    }
}
