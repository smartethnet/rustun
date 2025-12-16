use std::fs;
use serde::Deserialize;
use crate::crypto::CryptoConfig;
use crate::server::client_manager::ClientConfig;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server_config: ServerConfig,
    pub crypto_config: CryptoConfig,
    pub route_config: RouteConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub listen_addr: String,
}

#[derive(Debug, Deserialize)]
pub struct RouteConfig {
    pub routes_file: String,
}

pub fn load_main(path: &str) -> crate::Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

pub fn load_routes(path: &str) -> crate::Result<Vec<ClientConfig>> {
    let content = fs::read_to_string(path)?;
    let clients: Vec<ClientConfig> = serde_json::from_str(&content)?;
    Ok(clients)
}