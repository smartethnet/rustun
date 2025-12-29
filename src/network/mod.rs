pub mod connection_manager;
pub mod tcp_connection;
pub mod tcp_listener;

use crate::codec::frame::Frame;
use crate::crypto::Block;
use crate::network::tcp_connection::TcpConnection;
use crate::network::tcp_listener::TCPListener;
use crate::network::ListenerConfig::TCP;
use async_trait::async_trait;
use ipnet::IpNet;
use std::fmt::Display;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Default timeout for TCP connection establishment
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Network connection abstraction for reading/writing frames
///
/// This trait provides a protocol-agnostic interface for connection operations.
/// Implementations handle the underlying transport (TCP, UDP, etc.) and frame
/// marshaling/unmarshaling with encryption/decryption.
#[async_trait]
pub trait Connection: Send + Sync {
    /// Read a frame from the connection
    ///
    /// Blocks until a complete frame is received and decoded.
    ///
    /// # Returns
    /// - `Ok(Frame)` - Successfully received and decoded frame
    /// - `Err` - Connection error or frame parsing failure
    async fn read_frame(&mut self) -> crate::Result<Frame>;

    /// Write a frame to the connection
    ///
    /// Encodes and sends the frame over the connection.
    ///
    /// # Arguments
    /// - `frame` - The frame to send
    ///
    /// # Returns
    /// - `Ok(())` - Frame sent successfully
    /// - `Err` - Connection error or encoding failure
    async fn write_frame(&mut self, frame: Frame) -> crate::Result<()>;

    // async fn send_frame_to(&mut self, frame: &Frame, to: SocketAddr) -> crate::Result<()>;

    /// Close the connection gracefully
    async fn close(&mut self);

    /// Get the peer's socket address
    ///
    /// # Returns
    /// - `Ok(SocketAddr)` - Peer's address
    /// - `Err` - Connection not established or closed
    fn peer_addr(&mut self) -> io::Result<SocketAddr>;
}

/// Network listener abstraction for accepting connections
///
/// This trait provides a protocol-agnostic interface for server-side operations.
/// Implementations handle binding to addresses and accepting new connections.
#[async_trait]
pub trait Listener: Send + Sync {
    /// Start listening and serving connections
    ///
    /// Binds to the configured address and begins accepting connections.
    /// This is a blocking operation that runs until the listener is closed.
    ///
    /// # Returns
    /// - `Ok(())` - Listener closed gracefully
    /// - `Err` - Failed to bind or accept connections
    async fn listen_and_serve(&mut self) -> crate::Result<()>;

    /// Subscribe to new connections
    ///
    /// Returns a channel receiver for newly accepted connections.
    /// Each accepted connection is sent as a boxed Connection trait object.
    ///
    /// # Returns
    /// - `Ok(Receiver)` - Channel for receiving new connections
    /// - `Err` - Failed to create subscription channel
    async fn subscribe_on_conn(&mut self) -> crate::Result<mpsc::Receiver<Box<dyn Connection>>>;

    /// Close the listener
    ///
    /// Stops accepting new connections and releases the bound address.
    ///
    /// # Returns
    /// - `Ok(())` - Listener closed successfully
    /// - `Err` - Error during shutdown
    async fn close(&mut self) -> crate::Result<()>;
}

/// Metadata for a client connection
///
/// Contains routing information and configuration for a connected client,
/// including cluster membership, network addresses, and routing CIDRs.
#[derive(Debug, Clone)]
pub struct ConnectionMeta {
    /// Cluster/tenant name for multi-tenancy
    pub cluster: String,
    /// Unique client identifier
    pub identity: String,
    /// Client's private VPN IP address
    pub private_ip: String,
    /// Network mask for the VPN subnet
    pub mask: String,
    /// Gateway IP address
    pub gateway: String,
    /// CIDR ranges routed through this client
    pub ciders: Vec<String>,
    /// Channel for sending outbound frames to this client
    pub(crate) outbound_tx: mpsc::Sender<Frame>,
    /// ipv6
    pub ipv6: String,
    pub port: u16,
    // hole punch address
    pub stun_ip: String,
    pub stun_port: u16,
}

impl PartialEq<ConnectionMeta> for &ConnectionMeta {
    fn eq(&self, other: &ConnectionMeta) -> bool {
        self.identity == other.identity
    }
}

impl ConnectionMeta {
    /// Check if a destination IP matches this connection's routing rules
    ///
    /// Returns true if the destination matches the private IP or falls
    /// within any of the configured CIDR ranges.
    ///
    /// # Arguments
    /// - `dst` - Destination IP address as string
    ///
    /// # Returns
    /// - `true` if destination should be routed through this connection
    /// - `false` otherwise
    pub fn match_dst(&self, dst: String) -> bool {
        if self.private_ip == dst {
            return true;
        }

        let dst_ip = match dst.parse::<IpAddr>() {
            Ok(ip) => ip,
            Err(_) => return false,
        };

        for cidr in &self.ciders {
            if let Ok(network) = cidr.parse::<IpNet>()
                && network.contains(&dst_ip) {
                return true;
            }
        }

        false
    }
}

impl Display for ConnectionMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.identity, self.private_ip)
    }
}

/// Configuration for TCP listener
pub struct TCPListenerConfig {
    /// Address to bind the listener to (e.g., "0.0.0.0:8080")
    pub(crate) listen_addr: String,
}

/// Configuration for network listener
pub enum ListenerConfig {
    TCP(TCPListenerConfig),
}

/// Create a listener based on protocol type
///
/// # Arguments
/// - `config` - Listener configuration (TCP or UDP)
/// - `block` - Crypto block for encryption/decryption
///
/// # Returns
/// - `Ok(Box<dyn Listener>)` - Created listener
/// - `Err` - Unsupported protocol or configuration error
pub fn create_listener(
    config: ListenerConfig,
    block: Arc<Box<dyn Block>>,
) -> crate::Result<Box<dyn Listener>> {
    match config {
        TCP(config) => Ok(Box::new(TCPListener::new(config.listen_addr, block))),
    }
}

pub struct TCPConnectionConfig {
    pub(crate) server_addr: String
}

pub enum ConnectionConfig {
    TCP(TCPConnectionConfig),
}

pub async fn create_connection(config: ConnectionConfig,
                               block: Arc<Box<dyn Block>>,
) -> crate::Result<Box<dyn Connection>> {
    match config {
        ConnectionConfig::TCP(config) => {
            // Connect with timeout
            let connect_result = timeout(
                DEFAULT_CONNECT_TIMEOUT,
                TcpStream::connect(&config.server_addr)
            ).await;

            match connect_result {
                Ok(Ok(stream)) => {
                    let conn = TcpConnection::new(stream, block.clone());
                    Ok(Box::new(conn))
                }
                Ok(Err(e)) => Err(e.into()),
                Err(_) => Err("connection timeout".into()),
            }
        }
    }
}