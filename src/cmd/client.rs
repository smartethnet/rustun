use std::sync::Arc;
use std::time::Duration;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use rustun::client::client::{ClientConfig, ClientHandler};
use rustun::client::device::{ DeviceConfig, DeviceHandler};
use rustun::codec::frame::{DataFrame, Frame};
use rustun::crypto::xor::XorBlock;
use rustun::client::config;

#[tokio::main]
async fn main() {
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
    let private_ip = config.device_config.private_ip;
    tracing::info!("Starting client, connecting to: {}", server_addr);

    // initialize client handler
    let mut handler = ClientHandler::new(ClientConfig{
        server_addr: server_addr.clone(),
        private_ip: private_ip.clone(),
        cidr: config.device_config.routes,
        keepalive_interval: Duration::from_secs(config.client_config.keep_alive_interval),
        outbound_buffer_size: 1000,
        keep_alive_thresh: config.client_config.keep_alive_thresh,
    },Arc::new(Box::new(XorBlock::from_string("rustun"))));
    handler.run_client();

    // initialize device handler
    let mut dev = DeviceHandler::new();
    match dev.run(DeviceConfig {
        name: "tun.0".to_string(),
        ip: private_ip.clone(),
        mask: config.device_config.mask,
        gateway: config.device_config.gateway,
        mtu: config.device_config.mtu,
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
