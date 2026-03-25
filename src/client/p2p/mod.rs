use std::net::SocketAddr;
use std::time::{Duration, Instant};

pub mod peer;
pub mod stun;
mod udp_server;

/// Buffer size for outbound/inbound channels (2KB)
const OUTBOUND_BUFFER_SIZE: usize = 2048;

/// Keepalive interval: how often to send keepalive packets to peers (10 seconds)
///
/// Peers will receive a keepalive every 10 seconds to maintain connection health
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

/// Connection timeout: maximum time allowed since last received packet (15 seconds)
///
/// This is 1.5x the keepalive interval. If a peer hasn't responded within this time,
/// the connection is considered invalid and data sending will be rejected.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
struct PeerMeta {
    name: String,
    /// Unique identifier of the peer (e.g., client name)
    identity: String,

    /// Private VPN IP address assigned to this peer (e.g., "10.0.1.2")
    private_ip: String,

    /// CIDR ranges accessible through this peer (e.g., ["192.168.1.0/24"])
    ///
    /// Traffic destined for these ranges will be routed to this peer
    ciders: Vec<String>,

    /// Resolved socket address combining IPv6 and port ([ipv6]:port)
    remote_addr: LastActive<Option<SocketAddr>>,

    /// Stun socket address
    stun_addr: LastActive<Option<SocketAddr>>,
}

#[derive(Debug, Clone)]
struct LastActive<T> {
    value: T,
    last_active: Option<Instant>,
}
impl<T> LastActive<T> {
    pub fn dormant(value: T) -> Self {
        Self {
            value,
            last_active: None,
        }
    }
    pub fn restart(&mut self) {
        self.last_active = Some(Instant::now());
    }
    pub fn activate(&mut self, value: T) {
        self.value = value;
        self.last_active = Some(Instant::now());
    }
    pub fn get(&self) -> &T {
        &self.value
    }
    pub fn last_active(&self) -> Option<Instant> {
        self.last_active
    }
}

#[derive(Debug)]
pub struct PeerStatus {
    pub name: String,
    /// Unique identifier of the peer
    pub identity: String,

    /// IPv6 direct connection info
    pub ipv6_addr: Option<SocketAddr>,
    pub ipv6_last_active: Option<Instant>,

    /// STUN hole-punched connection info
    pub stun_addr: Option<SocketAddr>,
    pub stun_last_active: Option<Instant>,
}
