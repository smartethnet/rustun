use std::sync::Arc;
use std::time::Duration;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use rustun::client::client::{ClientConfig, ClientHandler};
use rustun::client::device::{ DeviceConfig, DeviceHandler};
use rustun::codec::frame::{DataFrame, Frame};
use rustun::crypto::aes256::Aes256Block;
use rustun::crypto::plain;

#[tokio::main]
async fn main() {
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

    let server_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.8:8080".to_string());
    let local_addr = std::env::args().nth(2).unwrap_or_else(|| "10.0.0.100".to_string());

    tracing::info!("Starting client, connecting to: {}", server_addr);

    // initialize client handler
    let mut handler = ClientHandler::new(ClientConfig{
        server_addr: server_addr.clone(),
        private_ip: local_addr.clone(),
        cidr: vec![],
        keepalive_interval: Duration::from_secs(3),
        outbound_buffer_size: 1000,
        keep_alive_thresh: 5,
    },Arc::new(Box::new(Aes256Block::from_string("rustun"))));
    handler.run_client();

    // initialize device handler
    let mut dev = DeviceHandler::new();
    match dev.run(DeviceConfig {
        name: "tun.0".to_string(),
        ip: local_addr.clone(),
        mask: "255.255.255.0".to_string(),
        gateway: "10.0.0.1".to_string(),
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
