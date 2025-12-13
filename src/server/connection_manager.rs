use crate::server::connection::ConnectionMeta;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub struct ConnectionManager {
    connections: Arc<RwLock<Vec<ConnectionMeta>>>,
}

impl ConnectionManager {
    pub fn new() -> ConnectionManager {
        ConnectionManager {
            connections: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add_connection(&self, meta: ConnectionMeta) {
        self.connections.write().unwrap().push(meta);
    }

    pub fn del_connection(&self, key: String) {
        self.connections.write().unwrap().retain(|route| {
            route.key != key
        })
    }

    pub fn get_connection(&self, dst: &String) -> Option<ConnectionMeta> {
        let guard = self.connections.read().unwrap_or_else(|e| e.into_inner());
        guard.iter()
            .find(|client| client.match_dst(dst.clone()))
            .cloned()
    }

    #[allow(dead_code)]
    pub fn print_connections(&self) {
        let guard = self.connections.read().unwrap_or_else(|e| e.into_inner());
        guard.iter().for_each(|route| {
            tracing::info!("route: {:?}", route);
        })
    }
}