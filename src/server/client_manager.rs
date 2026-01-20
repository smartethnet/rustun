use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClientConfig {
    #[serde(default)]
    pub name: String,
    pub cluster: String,
    pub identity: String,
    pub private_ip: String,
    pub mask: String,
    pub gateway: String,
    pub ciders: Vec<String>,
}

pub struct ClientManager {
    /// clients
    /// - key: client identity
    /// - value: client config
    clients: RwLock<HashMap<String, ClientConfig>>,

    /// cluster clients
    /// - key: cluster
    /// - value: clients at the same cluster
    cluster_clients: RwLock<HashMap<String, Vec<ClientConfig>>>,
}

impl ClientManager {
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
            cluster_clients: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_clients_config(&self, clients: Vec<ClientConfig>) {
        let mut clients_map = self.clients.write().unwrap_or_else(|e| e.into_inner());

        let mut cluster_map = self
            .cluster_clients
            .write()
            .unwrap_or_else(|e| e.into_inner());

        for client in clients {
            tracing::debug!("add client config {:?}", client);
            clients_map.insert(client.identity.clone(), client.clone());
            cluster_map
                .entry(client.cluster.clone())
                .or_default()
                .push(client);
        }
    }

    pub fn rewrite_clients_config(&self, clients: Vec<ClientConfig>) {
        let mut clients_map = self.clients.write().unwrap_or_else(|e| e.into_inner());
        let mut cluster_map = self.cluster_clients.write().unwrap_or_else(|e| e.into_inner());

        let mut new_clients_map = HashMap::new();
        let mut new_cluster_map: HashMap<String, Vec<ClientConfig>> = HashMap::new();
        for client in clients {
            tracing::debug!("add client config {:?}", client);
            new_clients_map.insert(client.identity.clone(), client.clone());
            new_cluster_map.entry(client.cluster.clone()).or_default().push(client.clone());
        }

        *clients_map = new_clients_map;
        *cluster_map = new_cluster_map;
    }

    pub fn get_cluster_clients_exclude(&self, identity: &String) -> Vec<ClientConfig> {
        let clients_map = self.clients.read().unwrap_or_else(|e| e.into_inner());

        let cluster = match clients_map.get(identity) {
            Some(client) => &client.cluster,
            None => return Vec::new(),
        };

        let cluster_map = self
            .cluster_clients
            .read()
            .unwrap_or_else(|e| e.into_inner());

        cluster_map
            .get(cluster)
            .map(|clients| {
                clients
                    .iter()
                    .filter(|c| c.identity != *identity)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }


    pub fn get_client(&self, identity: &String) -> Option<ClientConfig> {
        self.clients
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(identity)
            .cloned()
    }
}

impl Default for ClientManager {
    fn default() -> Self {
        Self::new()
    }
}
