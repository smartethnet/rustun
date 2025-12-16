use std::collections::HashMap;
use std::sync::RwLock;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ClientConfig {
    pub cluster: String,
    pub identity: String,
    pub private_ip: String,
    pub mask: String,
    pub gateway: String,
    pub ciders: Vec<String>
}

pub struct ClientManager {
    /// clients
    /// - key: client identity
    /// - value: client config
    clients: RwLock<HashMap<String, ClientConfig>>,

    /// cluster clients
    /// - key: cluster
    /// - value: clients at the same cluster
    cluster_clients: RwLock<HashMap<String, Vec<ClientConfig>>>
}

impl ClientManager {
    pub fn new() -> Self {
        Self{
            clients: RwLock::new(HashMap::new()),
            cluster_clients: RwLock::new(HashMap::new())
        }
    }

    pub fn add_clients_config(&self, clients: Vec<ClientConfig>) {
        let mut clients_map = self.clients.write()
            .unwrap_or_else(|e| { e.into_inner() });

        let mut cluster_map = self.cluster_clients.write()
            .unwrap_or_else(|e| { e.into_inner() });
        
        for client in clients {
            tracing::info!("add client config {:?}", client);
            clients_map.insert(client.identity.clone(), client.clone());
            cluster_map
                .entry(client.cluster.clone())
                .or_insert_with(Vec::new)
                .push(client);
        }
    }

    #[allow(unused)]
    pub fn del_client(&self, identity: &String) {
        let removed = self.clients.write()
            .unwrap_or_else(|e| {e.into_inner()})
            .remove(identity);

        // Also remove from cluster map
        if let Some(client) = removed {
            let mut cluster_map = self.cluster_clients.write()
                .unwrap_or_else(|e| {e.into_inner()});
            
            if let Some(clients) = cluster_map.get_mut(&client.cluster) {
                clients.retain(|c| c.identity != *identity);
                // Remove cluster if empty
                if clients.is_empty() {
                    cluster_map.remove(&client.cluster);
                }
            }
        }
    }

    pub fn get_cluster_clients_exclude(&self, identity: &String) -> Vec<ClientConfig> {
        let clients_map = self.clients.read().unwrap_or_else(|e| {e.into_inner()});

        let cluster = match clients_map.get(identity) {
            Some(client) => &client.cluster,
            None => return Vec::new(),
        };

        let cluster_map = self.cluster_clients.read().unwrap_or_else(|e| {e.into_inner()});

        cluster_map.get(cluster)
            .map(|clients| {
                clients.iter()
                    .filter(|c| c.identity != *identity)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    #[allow(unused)]
    pub fn get_cluster_clients(&self, cluster: &str) -> Vec<ClientConfig> {
        self.cluster_clients.read()
            .unwrap_or_else(|e| {e.into_inner()})
            .get(cluster)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_client(&self, identity: &String) -> Option<ClientConfig> {
        self.clients.read().unwrap_or_else(|e| {e.into_inner()})
            .get(identity)
            .cloned()
    }
}

impl Default for ClientManager {
    fn default() -> Self {
        Self::new()
    }
}