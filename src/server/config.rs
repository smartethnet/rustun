use crate::crypto::CryptoConfig;
use crate::server::client_manager::ClientConfig;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server_config: ServerConfig,
    pub crypto_config: CryptoConfig,
    pub route_config: RouteConfig,
    #[serde(default)]
    pub conf_agent: Option<ConfAgentConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub listen_addr: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConfAgentConfig {
    /// Control plane API URL
    pub control_plane_url: String,
    /// API token for authentication
    #[serde(default)]
    pub api_token: Option<String>,
    /// Routes file path to update
    pub routes_file: String,
    /// Poll interval in seconds for fetching routes
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u64,
    /// Connection reporting interval in seconds (default: 30)
    #[serde(default = "default_report_interval")]
    pub report_interval: u64,
}

fn default_poll_interval() -> u64 {
    60
}

fn default_report_interval() -> u64 {
    30
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
