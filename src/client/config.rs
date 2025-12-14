use std::fs;
use serde::{Deserialize, Serialize};
use crate::crypto::CryptoConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub client_config: ClientConfig,
    pub device_config: DeviceConfig,
    pub crypto_config: CryptoConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    // server protocol: eg: tcp
    #[serde(default = "default_server_protocol")]
    pub server_protocol: String,

    // server address: eg: 127.0.0.1:8080
    pub server_addr: String,

    // connect timeout between client and server
    #[serde(default = "default_connect_timeout_secs")]
    pub connect_timeout_secs: u16,

    // will reconnect once keepalive not received >= keep_alive_thresh
    #[serde(default = "default_keep_alive_thresh")]
    pub keep_alive_thresh: u8,

    // heartbeat interval
    #[serde(default = "default_keep_alive_interval")]
    pub keep_alive_interval: u64,

    pub identity: String,
}

fn default_server_protocol() -> String {
    "tcp".to_string()
}

fn default_connect_timeout_secs() -> u16 {
    5
}

fn default_keep_alive_thresh() -> u8 {
    5
}

fn default_keep_alive_interval() -> u64 {
    10
}

/// tunnel device configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceConfig {
    // my vpn local ip
    // considering config in server side
    pub private_ip: String,

    // mask
    // considering config in server side
    pub mask: String,

    // gateway
    pub gateway: String,

    // local routes cidr, server will route these ciders here
    // considering config in server side
    #[serde(default)]
    pub routes_to_me: Vec<String>,

    // mtu, 1500 - ip_overhead - tcp_overhead - rustun_overhead
    #[serde(default = "default_mtu")]
    pub mtu: u16,

    #[serde(default = "default_masquerade")]
    pub masquerade: bool,
}

fn default_mtu() -> u16 {
    1430
}

fn default_masquerade() -> bool {
    #[cfg(target_os = "linux")]
    return true;
    #[cfg(target_os = "windows")]
    return false;
    #[cfg(target_os = "macos")]
    return false;
}

pub fn load(path: &str) -> anyhow::Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
