use std::collections::HashMap;
use std::sync::RwLock;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ClientConfig {
    pub identity: String,
    pub private_ip: String,
    pub mask: String,
    pub gateway: String,
    pub ciders: Vec<String>
}


pub struct ClientManager {
    clients: RwLock<HashMap<String, ClientConfig>>
}

impl ClientManager {
    pub fn new() -> Self {
        Self{
            clients: RwLock::new(HashMap::new())
        }
    }

    pub fn add_clients_config(&self, clients: Vec<ClientConfig>) {
        let mut clients_map = self.clients.write()
            .unwrap_or_else(|e| { e.into_inner() });
        
        for client in clients {
            tracing::info!("add client config {:?}", client);
            clients_map.insert(client.identity.clone(), client);
        }
    }

    #[allow(unused)]
    pub fn del_client(&self, identity: &String) {
        self.clients.write().unwrap_or_else(|e| {e.into_inner()})
            .remove(identity);
    }

    pub fn get_clients_exclude(&self, identity: &String) -> Vec<ClientConfig> {
        self.clients.read().unwrap_or_else(|e| {e.into_inner()})
            .values()
            .filter(|item| item.identity != *identity)
            .cloned()
            .collect()
    }

    #[allow(unused)]
    pub fn get_all_clients(&self) -> Vec<ClientConfig> {
        self.clients.read().unwrap_or_else(|e| {e.into_inner()})
            .values()
            .cloned()
            .collect()
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