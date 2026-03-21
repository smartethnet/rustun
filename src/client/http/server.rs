//! HTTP server setup and management

use super::handlers::{AppState, health, status};
use axum::{Router, routing::get};

/// Start the HTTP server
pub async fn start(port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app_state = AppState::new();

    let app = Router::new()
        .route("/status", get(status))
        .route("/health", get(health))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    tracing::info!(
        "HTTP status server listening on http://127.0.0.1:{}/status",
        port
    );

    axum::serve(listener, app).await?;
    Ok(())
}
