//! HTTP request handlers

use super::cache::get_cache;
use super::models::StatusResponse;
use axum::{extract::State, http::StatusCode, response::Json};
use serde_json;

/// Shared state for the HTTP server
#[derive(Clone)]
pub struct AppState {
    status_cache: std::sync::Arc<std::sync::RwLock<Option<StatusResponse>>>,
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
pub async fn status(State(state): State<AppState>) -> Result<Json<StatusResponse>, StatusCode> {
    let cache = state.status_cache.read().unwrap();
    match cache.as_ref() {
        Some(status) => Ok(Json(status.clone())),
        None => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}
