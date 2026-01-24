use std::collections::HashMap;
use crate::server::client_manager::{ClientConfig, ClientManager};
use crate::server::config::ConfAgentConfig;
use crate::network::connection_manager::ConnectionManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::fs;
use tokio::time::{interval, Duration};

/// Connection update request for backend API
#[derive(Serialize, Debug)]
struct ConnectionUpdateRequest {
    cluster_id: u64,
    identity: String,
    last_active: Option<u64>,
}

/// Client config response from control plane API
#[derive(Deserialize, Debug)]
struct ClientConfigResponse {
    name: String,
    cluster: String, // Cluster ID as string
    identity: String,
    private_ip: String,
    mask: String,
    gateway: String,
    ciders: Vec<String>,
    #[serde(default)]
    cider_mapping: HashMap<String, String>,
}

pub struct ConfAgent {
    config: ConfAgentConfig,
    client_manager: Arc<ClientManager>,
    connection_manager: Arc<ConnectionManager>,
    routes_file: String,
}


impl ConfAgent {
    pub fn new(
        config: ConfAgentConfig,
        client_manager: Arc<ClientManager>,
        connection_manager: Arc<ConnectionManager>,
        routes_file: String,
    ) -> Self {
        Self {
            config,
            client_manager,
            connection_manager,
            routes_file,
        }
    }

    /// Start the conf-agent service
    pub async fn start(self: Arc<Self>) -> crate::Result<()> {
        tracing::info!("Starting conf-agent");
        tracing::info!("Control plane URL: {}", self.config.control_plane_url);
        tracing::info!("Routes file: {}", self.config.routes_file);
        tracing::info!("Poll interval: {}s", self.config.poll_interval);

        // Initial fetch and report
        if let Err(e) = self.fetch_and_update_routes().await {
            tracing::error!("Failed to fetch routes: {:?}", e);
        }
        if let Err(e) = self.report_connections().await {
            tracing::error!("Failed to report connections: {:?}", e);
        }

        // Periodic tasks: route fetching and connection reporting
        let mut route_ticker = interval(Duration::from_secs(self.config.poll_interval));
        let mut report_ticker = interval(Duration::from_secs(self.config.report_interval));

        loop {
            tokio::select! {
                _ = route_ticker.tick() => {
                    if let Err(e) = self.fetch_and_update_routes().await {
                        tracing::error!("Failed to fetch routes: {:?}", e);
                    }
                }
                _ = report_ticker.tick() => {
                    if let Err(e) = self.report_connections().await {
                        tracing::error!("Failed to report connections: {:?}", e);
                    }
                }
            }
        }
    }

    /// Report connections from connection manager
    async fn report_connections(&self) -> crate::Result<()> {
        // Get connections from connection manager
        let connections = self.connection_manager.dump_connection_info();

        if connections.is_empty() {
            return Ok(());
        }

        // Convert ConnectionMeta to ConnectionUpdateRequest
        let mut updates = Vec::new();
        for meta in &connections {
            // Parse cluster ID from string to u64
            let cluster_id: u64 = match meta.cluster.parse() {
                Ok(id) => id,
                Err(_) => {
                    tracing::warn!(
                        "Invalid cluster ID '{}' for identity {}, skipping",
                        meta.cluster,
                        meta.identity
                    );
                    continue;
                }
            };

            updates.push(ConnectionUpdateRequest {
                cluster_id,
                identity: meta.identity.clone(),
                last_active: Some(meta.last_active),
            });
        }

        if updates.is_empty() {
            return Ok(());
        }

        // Send batch update to backend
        let url = format!("{}/api/sync/connections", self.config.control_plane_url);
        Self::send_connection_updates(&url, self.config.api_token.as_deref(), &updates).await?;

        tracing::debug!("Reported {} connection updates", updates.len());
        Ok(())
    }

    /// Fetch routes from control plane and update local routes file
    async fn fetch_and_update_routes(&self) -> crate::Result<()> {
        tracing::debug!("Fetching routes from control plane...");

        let url = format!("{}/api/sync/clients", self.config.control_plane_url);
        let routes = Self::fetch_routes(&url, self.config.api_token.as_deref()).await?;

        tracing::info!("Fetched {} routes", routes.len());

        // Update client manager
        // self.client_manager.add_clients_config(routes.clone());
        self.client_manager.rewrite_clients_config(routes.clone());

        // Write to routes file
        Self::write_routes(&self.routes_file, &routes).await?;

        tracing::info!("Routes file updated successfully");
        Ok(())
    }

    /// Fetch routes from control plane API
    async fn fetch_routes(
        url: &str,
        token: Option<&str>,
    ) -> crate::Result<Vec<ClientConfig>> {
        let mut request = ureq::get(url).timeout(Duration::from_secs(30));

        if let Some(token) = token {
            request = request.set("Authorization", &format!("Bearer {}", token));
        }

        let response = request.call()?;

        let status = response.status();
        let body = response.into_string()?;
        
        if status != 200 {
            return Err(format!("Control plane returned error: {} - {}", status, body).into());
        }

        let routes: Vec<ClientConfigResponse> = serde_json::from_str(&body)?;

        // Convert to ClientConfig format
        let client_configs: Vec<ClientConfig> = routes
            .into_iter()
            .map(|r| ClientConfig {
                name: r.name,
                cluster: r.cluster,
                identity: r.identity,
                private_ip: r.private_ip,
                mask: r.mask,
                gateway: r.gateway,
                ciders: r.ciders,
                cider_mapping: r.cider_mapping,
            })
            .collect();

        Ok(client_configs)
    }

    /// Write routes to file atomically
    async fn write_routes(file_path: &str, routes: &[ClientConfig]) -> crate::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(file_path).parent() {
            fs::create_dir_all(parent).await?;
        }

        // Serialize to JSON with pretty formatting
        let json = serde_json::to_string_pretty(routes)?;

        // Write to temp file first, then rename (atomic write)
        let temp_path = format!("{}.tmp", file_path);
        fs::write(&temp_path, json).await?;
        fs::rename(&temp_path, file_path).await?;

        Ok(())
    }


    /// Send connection updates to control plane API
    async fn send_connection_updates(
        url: &str,
        token: Option<&str>,
        updates: &[ConnectionUpdateRequest],
    ) -> crate::Result<()> {
        let json_data = serde_json::to_string(updates)?;

        let mut request = ureq::post(url)
            .set("Content-Type", "application/json")
            .timeout(Duration::from_secs(30));

        if let Some(token) = token {
            request = request.set("Authorization", &format!("Bearer {}", token));
        }

        let response = request.send_string(&json_data)?;
        let status = response.status();
        let body = response.into_string()?;

        if status != 200 {
            return Err(format!("Backend returned error: {} - {}", status, body).into());
        }

        Ok(())
    }
}

