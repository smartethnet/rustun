use std::sync::Arc;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use crate::crypto;
use crate::server::config;
use crate::server::server::Server;

pub async fn run_server() {
    let args = std::env::args().collect::<Vec<String>>();
    let cfg = config::load(args.get(1).unwrap_or(&"server.toml".to_string())).unwrap();

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .from_env_lossy(),
            )
            .with_line_number(true)
            .with_file(true)
            .finish(),
    ).unwrap();

    let addr = cfg.server_config.listen_addr;
    tracing::info!("Starting server on: {}", addr);

    let block = crypto::new_block(&cfg.crypto_config);
    let mut server = Server::new(addr, Arc::new(block));

    if let Err(e) = server.listen_and_serve().await {
        tracing::error!("Server error: {}", e);
    }
}