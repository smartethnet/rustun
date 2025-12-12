use crate::server::connection::ConnectionMeta;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub struct RouteManager {
    routes: Arc<RwLock<Vec<ConnectionMeta>>>,
}

impl RouteManager {
    pub fn new() -> RouteManager {
        RouteManager {
            routes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add_route(&self, meta: ConnectionMeta) {
        self.routes.write().unwrap().push(meta);
    }

    pub fn del_route(&self, meta: ConnectionMeta) {
        self.routes.write().unwrap().retain(|route| {
            route != meta
        })
    }

    pub fn route(&self, dst: String) -> Option<ConnectionMeta> {
        let guard = self.routes.read().unwrap_or_else(|e| e.into_inner());
        guard.iter()
            .find(|route| route.match_dst(dst.clone()))
            .cloned()
    }

    #[allow(dead_code)]
    pub fn print_route(&self) {
        let guard = self.routes.read().unwrap_or_else(|e| e.into_inner());
        guard.iter().for_each(|route| {
            tracing::info!("route: {:?}", route);
        })
    }
}