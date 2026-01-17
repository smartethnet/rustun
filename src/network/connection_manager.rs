use crate::network::ConnectionMeta;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current Unix timestamp in seconds
#[inline]
fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

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
            .or_default()
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
                .find(|conn| conn.match_dst(dst.to_owned()))
                .cloned()
        })
    }

    pub fn get_connection_by_identity(&self, cluster: &str, identity: &String) -> Option<ConnectionMeta> {
        let guard = self
            .cluster_connections
            .read()
            .unwrap_or_else(|e| e.into_inner());
        guard.get(cluster).and_then(|connections| {
            connections
                .iter()
                .find(|conn| conn.identity ==*identity)
                .cloned()
        })
    }

    /// Update connection's IPv6 address and port (e.g., from keepalive)
    ///
    /// This is useful when a client's public IPv6 address changes dynamically.
    /// The updated information will be propagated to other clients in the same cluster.
    ///
    /// # Returns
    /// * `Some(Vec<ConnectionMeta>)` - List of other connections in the cluster if the address changed
    /// * `None` - If the address didn't change or the connection wasn't found
    pub fn update_connection_info(&self, cluster: &str, identity: &String,
                                  ciders: Vec<String>,
                                  ipv6: String, port: u16,
                                  stun_ip: String, stun_port: u16) -> Option<Vec<ConnectionMeta>> {
        let mut cluster_map = self
            .cluster_connections
            .write()
            .unwrap_or_else(|e| e.into_inner());

        if let Some(connections) = cluster_map.get_mut(cluster)
            && let Some(conn) = connections.iter_mut().find(|c| c.identity == *identity) {
            // Always update last_active timestamp on keepalive
            conn.last_active = now_timestamp();

            let mut changed = false;
            if conn.ipv6 != ipv6 || conn.port != port {
                conn.ipv6 = ipv6.clone();
                conn.port = port;
                changed = true;
            }

            if conn.stun_ip!= stun_ip || conn.stun_port != stun_port {
                changed = true;
                conn.stun_ip = stun_ip.clone();
                conn.stun_port = stun_port;
            }

            if conn.ciders != ciders {
                changed = true;
                conn.ciders = ciders;
            }

            if !changed {
                return None;
            }

            tracing::info!(
                "Updated connection info for {}: {}:{} -> {}:{} stun: {}:{} -> {}:{}",
                identity,
                conn.ipv6,
                conn.port,
                ipv6,
                port,
                conn.stun_ip,
                conn.stun_port,
                stun_ip,
                stun_port
            );
            // Return other connections in the cluster (excluding the updated one)
            let others: Vec<ConnectionMeta> = connections
                .iter()
                .filter(|c| c.identity != *identity)
                .cloned()
                .collect();
            return Some(others);
        }
        
        None
    }

    pub fn dump_connection_info(&self) -> Vec<ConnectionMeta> {
        let mut result = Vec::new();
        let guard = self.cluster_connections.read().unwrap_or_else(|e| e.into_inner());
        for (_, connections) in guard.iter() {
            for conn in connections {
                result.push(conn.clone());
            }
        }
        result
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}