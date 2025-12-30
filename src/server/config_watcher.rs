use std::sync::Arc;
use std::time::Duration;
use crate::server::client_manager::{ClientManager};
use crate::server::config;

const RELOAD_INTERVAL: Duration = Duration::from_secs(10);

pub struct ConfigWatcher {
    client_manager: Arc<ClientManager>,
    routes_file: String,
}

impl ConfigWatcher {
    pub fn new(client_manager: Arc<ClientManager>, routes_file: String) -> Self {
        Self {
            client_manager,
            routes_file,
        }
    }

    pub fn reload(&self) {
        let client_manager = self.client_manager.clone();
        let routes_file = self.routes_file.clone();
        tokio::spawn(async move {
            loop {
                tracing::info!("Reloading clients configuration");
                let client_routes = config::load_routes(routes_file.as_str());
                match client_routes {
                    Ok(client_routes) => {
                        tracing::info!("Loaded {} clients configuration", client_routes.len());
                        client_manager.rewrite_clients_config(client_routes);
                    }
                    Err(e) => {
                        tracing::error!("load client routes error: {}", e);
                    }
                }
                tokio::time::sleep(RELOAD_INTERVAL).await;
            }
        });
    }
}