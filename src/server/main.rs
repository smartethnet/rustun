use crate::network::connection_manager::ConnectionManager;
use crate::server::client_manager::ClientManager;
use crate::server::conf_agent::ConfAgent;
use crate::server::config;
use crate::server::config_watcher::ConfigWatcher;
use crate::server::handler::Server;
use crate::{crypto, utils};
use std::sync::Arc;

pub async fn run_server() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let cfg = config::load_main(args.get(1).unwrap_or(&"server.toml".to_string())).unwrap();

    if let Err(e) = utils::init_tracing() {
        anyhow::bail!("Failed to initialize logging: {e}");
    }

    let routes_file = cfg.route_config.routes_file.clone();
    let client_routes = config::load_routes(routes_file.as_str()).unwrap();
    tracing::debug!("config: {cfg:?}, routes: {client_routes:?}");

    let client_manager = Arc::new(ClientManager::new());
    client_manager.add_clients_config(client_routes.clone());

    // load dynamic client configurations
    let watcher = ConfigWatcher::new(client_manager.clone(), routes_file.clone());
    watcher.reload();

    let block = crypto::new_block(&cfg.crypto_config);

    // Create connection manager
    let connection_manager = Arc::new(ConnectionManager::new());

    // Create conf-agent if configured
    if let Some(ref conf_agent_config) = cfg.conf_agent {
        let agent = Arc::new(ConfAgent::new(
            conf_agent_config.clone(),
            client_manager.clone(),
            connection_manager.clone(),
            routes_file.clone(),
        ));

        // Start conf-agent background task
        let agent_clone = agent.clone();
        tokio::spawn(async move {
            if let Err(e) = agent_clone.start().await {
                tracing::error!("Conf-agent error: {e:?}");
            }
        });
    }

    let mut server = Server::new(
        cfg.server_config.clone(),
        client_manager,
        connection_manager.clone(),
        Arc::new(block),
    );
    if let Err(e) = server.run().await {
        anyhow::bail!("Server error: {e}");
    }
    Ok(())
}
