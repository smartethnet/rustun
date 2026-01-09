//! HTTP request handlers

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde_json;
use super::models::StatusResponse;
use super::cache::get_cache;

/// Shared state for the HTTP server
#[derive(Clone)]
pub struct AppState {
    status_cache: std::sync::Arc<tokio::sync::RwLock<Option<StatusResponse>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            status_cache: get_cache(),
        }
    }
}

/// Health check endpoint
pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "rustun-client"
    }))
}

/// Status endpoint handler
pub async fn status(
    State(state): State<AppState>,
) -> Result<Json<StatusResponse>, StatusCode> {
    let cache = state.status_cache.read().await;
    match cache.as_ref() {
        Some(status) => Ok(Json(status.clone())),
        None => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

