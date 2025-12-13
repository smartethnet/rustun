use std::sync::Arc;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use rustun::crypto::xor::XorBlock;
use rustun::server::server::Server;

#[tokio::main]
async fn main() {
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
    
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());
    
    tracing::info!("Starting server on: {}", addr);
    
    let mut server = Server::new(addr, Arc::new(Box::new(XorBlock::from_string("rustun"))));
    if let Err(e) = server.listen_and_serve().await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}

