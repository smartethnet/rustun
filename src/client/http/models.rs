//! HTTP API response models

use serde::Serialize;

/// Complete status response structure
#[derive(Serialize, Debug, Clone)]
pub struct StatusResponse {
    pub self_info: Option<SelfInfo>,
    pub traffic: TrafficStats,
    pub relay: RelayStatusInfo,
    pub p2p: P2PStatus,
    pub cluster_peers: Vec<ClusterPeerInfo>,
}

/// Self/client information
#[derive(Serialize, Debug, Clone)]
pub struct SelfInfo {
    pub identity: String,
    pub private_ip: String,
    pub mask: String,
    pub gateway: String,
    pub ciders: Vec<String>,
    pub ipv6: String,
    pub port: u16,
    pub stun_ip: String,
    pub stun_port: u16,
}

/// Traffic statistics
#[derive(Serialize, Debug, Clone)]
pub struct TrafficStats {
    /// Receive bytes (outbound traffic from device)
    pub receive_bytes: u64,
    pub receive_bytes_mb: f64,
    /// Send bytes (inbound traffic to device)
    pub send_bytes: u64,
    pub send_bytes_mb: f64,
}

/// Relay connection status
#[derive(Serialize, Debug, Clone)]
pub struct RelayStatusInfo {
    pub rx_frames: u64,
    pub rx_errors: u64,
    pub tx_frames: u64,
    pub tx_errors: u64,
}

/// P2P connection status
#[derive(Serialize, Debug, Clone)]
pub struct P2PStatus {
    pub enabled: bool,
    pub peers: Vec<P2PPeerInfo>,
}

/// P2P peer connection information
#[derive(Serialize, Debug, Clone)]
pub struct P2PPeerInfo {
    pub name: String,
    pub identity: String,
    pub ipv6: Option<IPv6ConnectionInfo>,
    pub stun: Option<STUNConnectionInfo>,
}

/// IPv6 direct connection information
#[derive(Serialize, Debug, Clone)]
pub struct IPv6ConnectionInfo {
    pub address: String,
    pub connected: bool,
    pub last_active_seconds_ago: Option<u64>,
}

/// STUN hole-punched connection information
#[derive(Serialize, Debug, Clone)]
pub struct STUNConnectionInfo {
    pub address: String,
    pub connected: bool,
    pub last_active_seconds_ago: Option<u64>,
}

/// Cluster peer information
#[derive(Serialize, Debug, Clone)]
pub struct ClusterPeerInfo {
    pub name: String,
    pub identity: String,
    pub private_ip: String,
    pub ciders: Vec<String>,
    pub ipv6: Option<String>,
    pub ipv6_port: Option<u16>,
    pub stun_ip: Option<String>,
    pub stun_port: Option<u16>,
    pub last_active: u64,
    pub status: String, // "online", "warning", "inactive", "offline"
}

