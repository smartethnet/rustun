use std::fs;
use serde::{Deserialize, Serialize};
use crate::crypto::CryptoConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub client_config: ClientConfig,
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

pub fn load(path: &str) -> anyhow::Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
