//! Status cache management

use std::sync::Arc;
use tokio::sync::RwLock;
use super::models::StatusResponse;

/// Global status cache (shared between HTTP server and event loop)
static STATUS_CACHE: once_cell::sync::Lazy<Arc<RwLock<Option<StatusResponse>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(None)));

/// Get a reference to the status cache
pub fn get_cache() -> Arc<RwLock<Option<StatusResponse>>> {
    STATUS_CACHE.clone()
}

/// Update the status cache (called from event loop)
pub async fn update(status: StatusResponse) {
    let mut cache = STATUS_CACHE.write().await;
    *cache = Some(status);
}

/// Get the current cached status
pub async fn get() -> Option<StatusResponse> {
    let cache = STATUS_CACHE.read().await;
    cache.clone()
}

