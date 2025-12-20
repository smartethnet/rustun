use crate::network::ConnectionMeta;
use std::collections::HashMap;
use std::sync::RwLock;

pub struct ConnectionManager {
    /// Cluster-based connections map (tenant isolation)
    /// key: cluster name -> value: connections in this cluster
    cluster_connections: RwLock<HashMap<String, Vec<ConnectionMeta>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            cluster_connections: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_connection(&self, meta: ConnectionMeta) {
        let cluster = meta.cluster.clone();

        tracing::debug!(
            "Add connection: cluster={}, identity={}",
            meta.identity,
            meta.cluster
        );

        self.cluster_connections
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .entry(cluster)
            .or_insert_with(Vec::new)
            .push(meta);
    }

    pub fn del_connection(&self, identity: String) {
        let mut cluster_map = self
            .cluster_connections
            .write()
            .unwrap_or_else(|e| e.into_inner());

        let mut cluster_to_remove = None;

        for (cluster, connections) in cluster_map.iter_mut() {
            if let Some(pos) = connections.iter().position(|c| c.identity == identity) {
                connections.remove(pos);
                tracing::debug!(
                    "Removed connection: cluster={}, identity={}",
                    cluster,
                    identity
                );

                if connections.is_empty() {
                    cluster_to_remove = Some(cluster.clone());
                }
                break;
            }
        }

        if let Some(cluster) = cluster_to_remove {
            cluster_map.remove(&cluster);
        }
    }

    pub fn get_connection(&self, cluster: &str, dst: &String) -> Option<ConnectionMeta> {
        let guard = self
            .cluster_connections
            .read()
            .unwrap_or_else(|e| e.into_inner());
        guard.get(cluster).and_then(|connections| {
            connections
                .iter()
                .find(|conn| conn.match_dst(dst.clone()))
                .cloned()
        })
    }
}
