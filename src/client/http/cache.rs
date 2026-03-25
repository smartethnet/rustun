//! Status cache management

use super::models::StatusResponse;
use std::sync::Arc;
use std::sync::RwLock;

/// Global status cache (shared between HTTP server and event loop)
static STATUS_CACHE: once_cell::sync::Lazy<Arc<RwLock<Option<StatusResponse>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(None)));

/// Get a reference to the status cache
pub fn get_cache() -> Arc<RwLock<Option<StatusResponse>>> {
    STATUS_CACHE.clone()
}

/// Update the status cache (called from event loop)
pub fn update(status: StatusResponse) {
    let mut cache = STATUS_CACHE.write().unwrap();
    *cache = Some(status);
}

/// Get the current cached status
pub fn get() -> Option<StatusResponse> {
    let cache = STATUS_CACHE.read().unwrap();
    cache.clone()
}
