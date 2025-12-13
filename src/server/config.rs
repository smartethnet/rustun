use std::fs;
use serde::Deserialize;
use crate::crypto::CryptoConfig;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server_config: ServerConfig,
    pub crypto_config: CryptoConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub listen_addr: String,
}

pub fn load(path: &str) -> anyhow::Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
