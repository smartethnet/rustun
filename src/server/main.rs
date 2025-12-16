use std::sync::Arc;
use crate::{crypto, utils};
use crate::server::config;
use crate::server::client_manager::ClientManager;
use crate::server::server::Server;

pub async fn run_server() {
    let args = std::env::args().collect::<Vec<String>>();
    let cfg = config::load_main(args.get(1).unwrap_or(&"server.toml".to_string())).unwrap();

    if let Err(e) = utils::init_tracing() {
        eprintln!("Failed to initialize logging: {}", e);
        return;
    }

    let client_routes = config::load_routes(cfg.route_config.routes_file.as_str()).unwrap();
    tracing::info!("config: {:?}, routes: {:?}", cfg, client_routes);

    let client_manager = Arc::new(ClientManager::new());
    client_manager.add_clients_config(client_routes);

    // TODO: load dynamic client configurations

    let addr = cfg.server_config.listen_addr;
    let block = crypto::new_block(&cfg.crypto_config);
    let mut server = Server::new(addr, client_manager, Arc::new(block));

    if let Err(e) = server.listen_and_serve().await {
        tracing::error!("Server error: {}", e);
    }
}