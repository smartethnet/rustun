use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use crate::client::client::{ClientConfig, ClientHandler};
use crate::client::config;
use crate::client::device::{DeviceConfig, DeviceHandler};
use crate::client::sys_route::SysRoute;
use crate::codec::frame::{DataFrame, Frame, HandshakeReplyFrame};
use crate::crypto;
use crate::crypto::{Block};

const DEFAULT_CONFIG_PATH: &str = "client.toml";
const DEFAULT_DEVICE_NAME: &str = "rustun";
const DEFAULT_MTU: u16 = 1430;
const OUTBOUND_BUFFER_SIZE: usize = 1000;
const CONFIG_CHANNEL_SIZE: usize = 10;

pub async fn run_client() {
    if let Err(e) = init_tracing() {
        eprintln!("Failed to initialize logging: {}", e);
        return;
    }

    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("Failed to load config: {}", e);
            return;
        }
    };

    let mut handler = create_client_handler(&config);
    let (config_ready_tx, mut config_ready_rx) = mpsc::channel(CONFIG_CHANNEL_SIZE);
    handler.run_client(config_ready_tx);

    let device_config = match config_ready_rx.recv().await {
        Some(cfg) => {
            tracing::info!("Received device config from server: {:?}", cfg);
            cfg
        }
        None => {
            tracing::error!("Failed to receive device config from server");
            return;
        }
    };
    tracing::info!("Loaded device config: {:?}", device_config);

    let mut dev = match init_device(&device_config) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to initialize device: {}", e);
            return;
        }
    };

    run_event_loop(&mut handler, &mut dev).await;
}

fn init_tracing() -> Result<(), Box<dyn std::error::Error>> {
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
    )?;
    Ok(())
}

fn load_config() -> anyhow::Result<config::Config> {
    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).map(|s| s.as_str()).unwrap_or(DEFAULT_CONFIG_PATH);
    config::load(config_path)
}

fn create_client_handler(config: &config::Config) -> ClientHandler {
    let client_config = ClientConfig {
        server_addr: config.client_config.server_addr.clone(),
        keepalive_interval: Duration::from_secs(config.client_config.keep_alive_interval),
        outbound_buffer_size: OUTBOUND_BUFFER_SIZE,
        keep_alive_thresh: config.client_config.keep_alive_thresh,
        identity: config.client_config.identity.clone(),
    };

    let block = crypto::new_block(&config.crypto_config);
    let crypto_block: Arc<Box<dyn Block>> = Arc::new(block);
    ClientHandler::new(client_config, crypto_block)
}

fn init_device(device_config: &HandshakeReplyFrame) -> crate::Result<DeviceHandler> {
    let mut dev = DeviceHandler::new();
    dev.run(DeviceConfig {
        name: DEFAULT_DEVICE_NAME.to_string(),
        ip: device_config.private_ip.clone(),
        mask: device_config.mask.clone(),
        gateway: device_config.gateway.clone(),
        mtu: DEFAULT_MTU,
        routes: vec![],
    })?;

    // add sys route
    let sys_route = SysRoute::default();
    for route_item in &device_config.others {
        if let Err(e) = sys_route.add(route_item.ciders.clone(),
                                           device_config.private_ip.clone()) {
            tracing::error!("Failed to add route item {:?}: {}",route_item, e);
        }
    }
    Ok(dev)
}

async fn run_event_loop(handler: &mut ClientHandler, dev: &mut DeviceHandler) {
    loop {
        tokio::select! {
            packet = dev.recv() => {
                if let Some(packet) = packet {
                    handle_device_packet(handler, packet).await;
                }
            }

            frame = handler.recv_frame() => {
                if let Err(e) = handle_server_frame(dev, frame).await {
                    tracing::error!("Server connection error: {:?}", e);
                    break;
                }
            }
        }
    }
    
    tracing::info!("Event loop terminated, client shutting down");
}

async fn handle_device_packet(handler: &mut ClientHandler, packet: Vec<u8>) {
    tracing::debug!("Device -> Server: {} bytes", packet.len());
    
    if let Err(e) = handler.send_frame(Frame::Data(DataFrame { payload: packet })).await {
        tracing::error!("Failed to send packet to server: {}", e);
    }
}

async fn handle_server_frame(dev: &mut DeviceHandler, frame: crate::Result<Frame>) -> crate::Result<()> {
    let frame = frame?;
    
    if let Frame::Data(data_frame) = frame {
        tracing::debug!("Server -> Device: {} bytes", data_frame.payload.len());
        
        dev.send(data_frame.payload).await.map_err(|e| {
            tracing::error!("Failed to write to device: {}", e);
            e
        })?;
    }
    
    Ok(())
}