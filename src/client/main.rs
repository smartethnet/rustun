use std::sync::Arc;
use std::time::Duration;
use clap::Parser;
use tokio::sync::mpsc;

use crate::client::client::{ClientConfig, ClientHandler};
use crate::client::prettylog::{log_handshake_success, log_startup_banner};
use crate::utils::device::{DeviceConfig, DeviceHandler};
use crate::utils::sys_route::SysRoute;
use crate::codec::frame::{DataFrame, Frame, HandshakeReplyFrame};
use crate::crypto::{self, Block, CryptoConfig};
use crate::utils;

const DEFAULT_MTU: u16 = 1430;
const OUTBOUND_BUFFER_SIZE: usize = 1000;
const CONFIG_CHANNEL_SIZE: usize = 10;

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
}

pub async fn run_client() {
    let args = Args::parse();

    if let Err(e) = utils::init_tracing() {
        eprintln!("Failed to initialize logging: {}", e);
        return;
    }

    log_startup_banner(&args);

    // Parse crypto configuration
    let crypto_config = match parse_crypto_config(&args.crypto) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("Invalid crypto configuration: {}", e);
            return;
        }
    };

    let mut handler = create_client_handler(&args, &crypto_config);
    let (config_ready_tx, mut config_ready_rx) = mpsc::channel(CONFIG_CHANNEL_SIZE);
    handler.run_client(config_ready_tx);

    let device_config = match config_ready_rx.recv().await {
        Some(cfg) => {
            log_handshake_success(&cfg);
            cfg
        }
        None => {
            tracing::error!("Failed to receive device config from server");
            return;
        }
    };

    let mut dev = match init_device(&device_config) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to initialize device: {}", e);
            return;
        }
    };
    
    run_event_loop(&mut handler, &mut dev).await;
}

fn parse_crypto_config(crypto_str: &str) -> anyhow::Result<CryptoConfig> {
    let parts: Vec<&str> = crypto_str.splitn(2, ':').collect();
    
    match parts[0].to_lowercase().as_str() {
        "plain" => Ok(CryptoConfig::Plain),
        "aes256" => {
            if parts.len() < 2 {
                anyhow::bail!("AES256 requires a key: aes256:<key>");
            }
            Ok(CryptoConfig::Aes256(parts[1].to_string()))
        }
        "chacha20" => {
            if parts.len() < 2 {
                anyhow::bail!("ChaCha20 requires a key: chacha20:<key>");
            }
            Ok(CryptoConfig::ChaCha20Poly1305(parts[1].to_string()))
        }
        "xor" => {
            if parts.len() < 2 {
                anyhow::bail!("XOR requires a key: xor:<key>");
            }
            Ok(CryptoConfig::Xor(parts[1].to_string()))
        }
        _ => anyhow::bail!("Unknown crypto method: {}. Use plain, aes256:<key>, chacha20:<key>, or xor:<key>", parts[0]),
    }
}

fn create_client_handler(args: &Args, crypto_config: &CryptoConfig) -> ClientHandler {
    let client_config = ClientConfig {
        server_addr: args.server.clone(),
        keepalive_interval: Duration::from_secs(args.keepalive_interval),
        outbound_buffer_size: OUTBOUND_BUFFER_SIZE,
        keep_alive_thresh: args.keepalive_threshold,
        identity: args.identity.clone(),
    };

    let block = crypto::new_block(crypto_config);
    let crypto_block: Arc<Box<dyn Block>> = Arc::new(block);
    ClientHandler::new(client_config, crypto_block)
}

fn init_device(device_config: &HandshakeReplyFrame) -> crate::Result<DeviceHandler> {
    let mut dev = DeviceHandler::new();
    dev.run(DeviceConfig {
        ip: device_config.private_ip.clone(),
        mask: device_config.mask.clone(),
        gateway: device_config.gateway.clone(),
        mtu: DEFAULT_MTU,
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