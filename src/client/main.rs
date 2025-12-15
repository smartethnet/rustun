use std::sync::{ Arc};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use crate::client::client::{ClientConfig, ClientHandler};
use crate::client::config;
use crate::client::device::{DeviceConfig, DeviceHandler};
use crate::codec::frame::{DataFrame, Frame};
use crate::crypto::xor::XorBlock;

pub async fn run_client() {
    let args = std::env::args().collect::<Vec<String>>();
    let config = config::load(args.get(1).unwrap_or(&"client.toml".to_string())).unwrap();

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .from_env_lossy(),
            )
            .with_line_number(true)
            .with_file(true)
            .finish(),
    ).unwrap();

    let server_addr = config.client_config.server_addr;
    tracing::info!("Starting client, connecting to: {}", server_addr);

    // initialize client handler
    let mut handler = ClientHandler::new(ClientConfig{
        server_addr: server_addr.clone(),
        keepalive_interval: Duration::from_secs(config.client_config.keep_alive_interval),
        outbound_buffer_size: 1000,
        keep_alive_thresh: config.client_config.keep_alive_thresh,
        identity: config.client_config.identity.clone(),
    },Arc::new(Box::new(XorBlock::from_string("rustun"))));

    let (config_ready_tx, mut config_ready_rx) = mpsc::channel(10);
    handler.run_client(config_ready_tx);

    let device_config = match config_ready_rx.recv().await {
        Some(handshake_config) =>handshake_config,
        None =>return,
    };
    tracing::info!("got config: {:?}", device_config);

    // initialize device handler
    let mut dev = DeviceHandler::new();
    match dev.run(DeviceConfig {
        name: "rustun".to_string(),
        ip: device_config.private_ip.clone(),
        mask: device_config.mask.clone(),
        gateway: device_config.gateway,
        mtu: 1430,
        routes: vec![],
    }) {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("{}", e);
            return;
        }
    };

    loop {
        tokio::select! {
            packet = dev.recv() => {
                let packet = match packet {
                    Some(packet) => packet,
                    None => continue,
                };
                tracing::info!("receive {} bytes from device", packet.len());
                let _ = handler.send_frame(Frame::Data(DataFrame{
                    payload: packet,
                })).await;
            }
            frame = handler.recv_frame() => {
                match frame {
                    Ok(frame) => {
                        match frame {
                            Frame::Data(frame) => {
                                tracing::info!("receive {} bytes from server", frame.payload.len());
                                if let Err(e) = dev.send(frame.payload).await {
                                    tracing::error!("write device fail {}", e);
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        tracing::error!("receive from server error: {:?}", e);
                        break;
                    }
                }
            }
        }
    }
}