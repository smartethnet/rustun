//! Peer-to-Peer connection management module
//!
//! This module provides P2P communication capabilities between VPN clients.
//! It implements a connection health monitoring mechanism using keepalive packets
//! and validates peer connectivity before sending data.
//!
//! # Architecture
//!
//! The module is split into two components:
//! - `PeerService`: Handles actual UDP socket I/O operations
//! - `PeerHandler`: Manages peer metadata and connection logic
//!
//! # Connection Lifecycle
//!
//! 1. **Discovery**: Peers are added via `add_peers()` from server handshake
//! 2. **Probing**: Initial keepalive sent immediately, `last_active` set to None
//! 3. **Active**: Peer responds to keepalive, `last_active` updated with timestamp
//! 4. **Validation**: Before sending data, check if peer responded within 15 seconds
//! 5. **Timeout**: Peer considered dead if no response for extended period

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use crate::codec::frame::{Frame, KeepAliveFrame, RouteItem};
use crate::codec::parser::Parser;
use crate::crypto::Block;

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

/// PeerService handles actual UDP socket I/O operations
///
/// This service runs in a separate tokio task and owns the UDP socket.
/// It communicates with PeerHandler via channels:
/// - Receives packets from socket and forwards to PeerHandler via `input_tx`
/// - Receives outbound packets from PeerHandler via `output_rx` and sends to socket
///
/// # Design Rationale
///
/// Splitting I/O operations into a separate service solves the ownership problem:
/// - `PeerHandler` can remain accessible for method calls (add_peers, send_frame, etc.)
/// - `PeerService` owns the socket and runs in a background task
pub struct PeerService {
    /// UDP port to bind the socket to
    listen_port: u16,
    /// Channel sender to forward received packets to PeerHandler
    input_tx: mpsc::Sender<(Vec<u8>, SocketAddr)>,
    /// Channel receiver to get outbound packets from PeerHandler
    output_rx: mpsc::Receiver<(Vec<u8>, SocketAddr)>,
}

impl PeerService {
    /// Create a new PeerService instance
    ///
    /// # Arguments
    /// * `listen_port` - UDP port to bind
    /// * `input_tx` - Channel to send received packets to PeerHandler
    /// * `output_rx` - Channel to receive outbound packets from PeerHandler
    fn new(
        listen_port: u16,
        input_tx: mpsc::Sender<(Vec<u8>, SocketAddr)>,
        output_rx: mpsc::Receiver<(Vec<u8>, SocketAddr)>,
    ) -> Self {
        PeerService {
            listen_port,
            input_tx,
            output_rx,
        }
    }

    /// Start the UDP service loop
    ///
    /// Binds to the specified port and enters an event loop that:
    /// - Sends outbound packets received from `output_rx` channel
    /// - Receives packets from UDP socket and forwards to `input_tx` channel
    ///
    /// # Returns
    /// * `Ok(())` - Never returns in normal operation
    /// * `Err` - Socket bind failure or fatal I/O error
    pub async fn serve(&mut self) -> crate::Result<()> {
        let socket = UdpSocket::bind(format!("[::]:{}", self.listen_port)).await?;
        tracing::info!("P2P UDP listening on port {}", self.listen_port);

        let mut buf = vec![0u8; 2048];

        loop {
            tokio::select! {
                // Handle outbound packets: PeerHandler -> Network
                Some((data, remote)) = self.output_rx.recv() => {
                    if let Err(e) = socket.send_to(&data, remote).await {
                        tracing::error!("Failed to send to {}: {:?}", remote, e);
                    }
                }

                // Handle inbound packets: Network -> PeerHandler
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((len, remote)) => {
                            let packet = buf[..len].to_vec();
                            if let Err(e) = self.input_tx.send((packet, remote)).await {
                                tracing::error!("Failed to forward received packet: {:?}", e);
                            }
                            buf = vec![0u8; 2048]; // Reset buffer for next packet
                        }
                        Err(e) => {
                            tracing::error!("UDP recv_from error: {}", e);
                            return Err(e.into());
                        }
                    }
                }
            }
        }
    }
}

/// Peer metadata for tracking P2P connection state
///
/// Each peer represents a remote VPN client that we can communicate with directly.
/// The `last_active` field is crucial for connection validation:
/// - `None`: Peer never responded to our keepalive packets
/// - `Some(instant)`: Last time we received ANY packet from this peer
struct PeerMeta {
    /// Unique identifier of the peer (e.g., client name)
    identity: String,
    
    /// Private VPN IP address assigned to this peer (e.g., "10.0.1.2")
    private_ip: String,
    
    /// CIDR ranges accessible through this peer (e.g., ["192.168.1.0/24"])
    ///
    /// Traffic destined for these ranges will be routed to this peer
    ciders: Vec<String>,

    /// Public IPv6 address of the peer for P2P connection
    #[allow(unused)]
    ipv6: String,

    /// UDP port number for P2P communication
    #[allow(unused)]
    port: u16,

    /// Resolved socket address combining IPv6 and port ([ipv6]:port)
    remote_addr: SocketAddr,
    
    /// Timestamp of last received packet from this peer
    ///
    /// - `None`: Never received any response (connection not established)
    /// - `Some(instant)`: Last successful communication time
    ///
    /// This is used to validate connection health before sending data.
    last_active: Option<Instant>,
}

/// Peer connection manager for P2P communication
///
/// Manages a collection of peer connections and handles:
/// - Peer discovery and registration
/// - Keepalive probing (sent every 10 seconds)
/// - Connection validation (requires response within 15 seconds)
/// - Routing decisions based on destination IP
///
/// # Workflow
///
/// 1. Create PeerHandler with `new()`
/// 2. Start UDP service with `run_peer()`
/// 3. Add peers from server with `add_peers()`
/// 4. Start periodic keepalive with `start_keepalive_timer()`
/// 5. Send/receive data with `send_frame()` and `recv_frame()`
pub struct PeerHandler {
    /// Map of peer identity to peer metadata (shared with keepalive task)
    ///
    /// Wrapped in Arc<RwLock<>> to allow dynamic peer updates while
    /// keepalive timer is running.
    peers: Arc<RwLock<HashMap<String, PeerMeta>>>,

    /// Channel sender to PeerService for outbound packets
    ///
    /// Initialized when `run_peer()` is called. None before initialization.
    outbound_tx: Option<mpsc::Sender<(Vec<u8>, SocketAddr)>>,

    /// Channel receiver for inbound packets from PeerService
    inbound_rx: mpsc::Receiver<(Vec<u8>, SocketAddr)>,
    
    /// Channel sender for inbound packets (cloned to PeerService)
    inbound_tx: mpsc::Sender<(Vec<u8>, SocketAddr)>,

    /// Encryption/decryption block for frame marshaling
    block: Arc<Box<dyn Block>>,
}

impl PeerHandler {
    /// Create a new PeerHandler instance
    ///
    /// Initializes the handler with empty peer list and creates the inbound channel.
    /// Call `run_peer()` after creation to start the UDP service.
    ///
    /// # Arguments
    /// * `block` - Encryption/decryption block for frame processing
    ///
    /// # Returns
    /// A new PeerHandler ready to be started
    pub fn new(block: Arc<Box<dyn Block>>) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel(OUTBOUND_BUFFER_SIZE);
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            outbound_tx: None,
            inbound_rx,
            inbound_tx,
            block,
        }
    }

    /// Start the P2P UDP service in a background task
    ///
    /// Spawns a PeerService that handles actual socket I/O operations.
    /// After this call, the handler is ready to send/receive frames.
    ///
    /// # Arguments
    /// * `bind_port` - UDP port to bind for P2P communication
    ///
    /// # Example
    /// ```rust,ignore
    /// let mut handler = PeerHandler::new(block);
    /// handler.run_peer(51258);  // Bind to port 51258
    /// ```
    pub fn run_peer(&mut self, bind_port: u16)  {
        let (output_tx, output_rx) = mpsc::channel(OUTBOUND_BUFFER_SIZE);
        let mut peer_service = PeerService::new(bind_port, self.inbound_tx.clone(), output_rx);

        // Spawn PeerService in background task
        tokio::spawn(async move {
            if let Err(e) = peer_service.serve().await {
                tracing::error!("PeerService error: {}", e);
            }
        });

        self.outbound_tx = Some(output_tx);
    }

    /// Add or update peers from route information
    ///
    /// For each peer:
    /// 1. Parse IPv6 address and port into SocketAddr
    /// 2. Add to peers map with `last_active = None`
    /// 3. Send initial keepalive packet immediately
    ///
    /// The peer will remain in "unconfirmed" state (last_active = None) until
    /// we receive a response packet, at which point `last_active` will be updated.
    ///
    /// # Arguments
    /// * `routes` - List of peer route items from server handshake
    ///
    /// # Behavior
    /// - Skips peers with invalid IPv6 addresses
    /// - Updates existing peers if identity matches
    /// - Sends initial keepalive to probe connectivity
    pub async fn add_peers(&mut self, routes: Vec<RouteItem>) {
        let mut peers = self.peers.write().await;
        
        for route in routes {
            if route.ipv6.is_empty() {
                continue;
            }
            // Parse remote address from IPv6 and port
            let remote_addr = match format!("[{}]:{}", route.ipv6, route.port).parse::<SocketAddr>() {
                Ok(addr) => addr,
                Err(e) => {
                    tracing::warn!("Invalid IPv6 address for peer {}: {}", route.identity, e);
                    continue;
                }
            };

            // Add or update peer in the map
            peers.insert(
                route.identity.clone(),
                PeerMeta {
                    identity: route.identity.clone(),
                    private_ip: route.private_ip.clone(),
                    ciders: route.ciders.clone(),
                    ipv6: route.ipv6.clone(),
                    port: route.port,
                    remote_addr,
                    last_active: None, // Not confirmed yet - waiting for response
                },
            );

            tracing::info!("Added P2P peer: {} at [{}]:{}", route.identity, route.ipv6, route.port);

            // Send initial keepalive to probe connectivity
            let _ = self.send_keepalive_to(remote_addr);
        }
    }

    /// Start periodic keepalive timer
    ///
    /// Spawns a background task that sends keepalive packets to all peers
    /// every 10 seconds (KEEPALIVE_INTERVAL). This maintains connection health
    /// and allows detection of dead peers.
    ///
    /// **IMPORTANT**: Call this after `run_peer()` and `add_peers()`.
    ///
    /// # Behavior
    /// - Reads current peer list on each tick (supports dynamic peer updates)
    /// - Runs indefinitely in background task
    /// - Skips first immediate tick to avoid duplicate initial keepalive
    ///
    /// # Dynamic Updates
    /// New peers added via `add_peers()` after this is called will automatically
    /// receive keepalives in the next interval.
    pub async fn start_keepalive_timer(&self) {
        let outbound_tx = match &self.outbound_tx {
            Some(tx) => tx.clone(),
            None => {
                tracing::error!("Cannot start keepalive: outbound_tx not initialized");
                return;
            }
        };

        let block = self.block.clone();
        let peers = self.peers.clone(); // Clone Arc, not the data

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(KEEPALIVE_INTERVAL);
            loop {
                interval.tick().await;

                // Read current peer list (supports dynamic updates!)
                let peer_addrs: Vec<SocketAddr> = {
                    let peers_guard = peers.read().await;
                    peers_guard.values().map(|p| p.remote_addr).collect()
                };

                // Send keepalive to all current peers (every 10 seconds)
                let keepalive = Frame::KeepAlive(KeepAliveFrame {});
                if let Ok(data) = Parser::marshal(keepalive, block.as_ref()) {
                    for remote_addr in &peer_addrs {
                        if let Err(e) = outbound_tx.send((data.clone(), *remote_addr)).await {
                            tracing::warn!("Failed to send keepalive: {}", e);
                            break;
                        }
                    }
                    tracing::debug!("Sent keepalive to {} peers", peer_addrs.len());
                }
            }
        });
    }

    /// Send keepalive packet to a specific peer (non-blocking)
    ///
    /// Used for sending initial keepalive when a peer is added.
    /// Uses `try_send` to avoid blocking if the outbound channel is full.
    ///
    /// # Arguments
    /// * `remote_addr` - Socket address of the peer
    ///
    /// # Returns
    /// * `Ok(())` - Keepalive queued successfully
    /// * `Err` - Failed to marshal or send
    fn send_keepalive_to(&self, remote_addr: SocketAddr) -> crate::Result<()> {
        let outbound_tx = self.outbound_tx.as_ref().ok_or("outbound_tx not initialized")?;
        let keepalive = Frame::KeepAlive(KeepAliveFrame {});
        let data = Parser::marshal(keepalive, self.block.as_ref())?;

        // Non-blocking send - best effort
        let _ = outbound_tx.try_send((data, remote_addr));
        Ok(())
    }

    /// Receive and parse a frame from any peer
    ///
    /// Blocks until a data frame is received from the inbound channel.
    /// Automatically updates the sender peer's `last_active` timestamp,
    /// which is crucial for connection validation.
    ///
    /// Keepalive frames are handled internally and not returned to caller.
    /// This method will skip keepalive packets and continue receiving until
    /// a data frame arrives.
    ///
    /// # Returns
    /// * `Ok(Frame)` - Successfully received and parsed data frame
    /// * `Err` - Channel closed or parse error
    ///
    /// # Side Effects
    /// Updates `last_active` for the sending peer to `Some(Instant::now())`
    pub async fn recv_frame(&mut self) -> crate::Result<Frame> {
        loop {
            let (buf, remote) = self
                .inbound_rx
                .recv()
                .await
                .ok_or("recv from peers channel closed")?;

            self.update_peer_active(remote).await;
            let (frame, _) = Parser::unmarshal(&buf, self.block.as_ref())?;

            match frame {
                Frame::KeepAlive(_) => {
                    tracing::debug!("Received keepalive from peer at {}", remote);
                    continue; // Skip keepalive, receive next frame
                }
                _ => {
                    return Ok(frame);
                }
            }
        }
    }

    /// Send a frame to a peer determined by destination IP
    ///
    /// This method performs the following steps:
    /// 1. Find peer that can route to `dest_ip` (by private IP or CIDR match)
    /// 2. Validate peer connection is active (received packet within 15 seconds)
    /// 3. Marshal frame and send via outbound channel
    ///
    /// # Arguments
    /// * `frame` - Frame to send
    /// * `dest_ip` - Destination IP address as string (e.g., "10.0.1.2")
    ///
    /// # Returns
    /// * `Ok(())` - Frame sent successfully
    /// * `Err` - No peer found, connection expired, or send failed
    ///
    /// # Connection Validation
    /// - Peer with `last_active = None`: Rejected (never responded to keepalive)
    /// - Peer with `last_active > 15s ago`: Rejected (connection expired)
    /// - Peer with `last_active < 15s ago`: Accepted (connection valid)
    pub async fn send_frame(&self, frame: Frame, dest_ip: &str) -> crate::Result<()> {
        // Find peer responsible for this destination
        let (peer_identity, peer_remote_addr, peer_last_active) = {
            let peers = self.peers.read().await;
            let peer = self.find_peer_by_ip_locked(&peers, dest_ip)
                .ok_or("No peer found for destination")?;
            
            (peer.identity.clone(), peer.remote_addr, peer.last_active)
        };

        // Validate connection is active (within 15 seconds)
        match peer_last_active {
            Some(last_active) => {
                let elapsed = Instant::now().duration_since(last_active);
                if elapsed > CONNECTION_TIMEOUT {
                    return Err(format!(
                        "Peer {} connection expired (last seen {:?} ago)",
                        peer_identity, elapsed
                    )
                    .into());
                }
            }
            None => {
                return Err(format!("Peer {} never responded to keepalive", peer_identity).into());
            }
        }

        // Marshal frame and send to peer
        let outbound_tx = self.outbound_tx.as_ref().ok_or("outbound_tx not initialized")?;
        let data = Parser::marshal(frame, self.block.as_ref())?;

        outbound_tx
            .send((data, peer_remote_addr))
            .await
            .map_err(|e| format!("Failed to send: {}", e))?;

        tracing::debug!("Sent frame to peer {}", peer_identity);
        Ok(())
    }

    /// Find peer that can route to the given destination IP
    ///
    /// This is a helper method that requires the caller to already hold
    /// a read lock on the peers HashMap.
    ///
    /// Searches through all peers and checks if the destination matches:
    /// 1. Peer's private VPN IP (exact match)
    /// 2. Any of peer's CIDR ranges (subnet match)
    ///
    /// # Arguments
    /// * `peers` - Read-locked peers HashMap
    /// * `dest_ip` - Destination IP address as string
    ///
    /// # Returns
    /// * `Some(&PeerMeta)` - Peer that can route to this destination
    /// * `None` - No matching peer found
    ///
    /// # Example
    /// ```rust,ignore
    /// // Peer A has private_ip = "10.0.1.2" and ciders = ["192.168.1.0/24"]
    /// find_peer_by_ip_locked(&peers, "10.0.1.2")        // -> Some(Peer A)
    /// find_peer_by_ip_locked(&peers, "192.168.1.100")   // -> Some(Peer A)
    /// find_peer_by_ip_locked(&peers, "172.16.0.1")      // -> None
    /// ```
    fn find_peer_by_ip_locked<'a>(
        &self,
        peers: &'a HashMap<String, PeerMeta>,
        dest_ip: &str,
    ) -> Option<&'a PeerMeta> {
        use ipnet::IpNet;
        use std::net::IpAddr;

        let dest_ip_addr = match dest_ip.parse::<IpAddr>() {
            Ok(ip) => ip,
            Err(_) => return None,
        };

        for peer in peers.values() {
            // Check exact match with peer's private IP
            if peer.private_ip == dest_ip {
                return Some(peer);
            }

            // Check if destination falls within peer's CIDR ranges
            for cidr in &peer.ciders {
                if let Ok(network) = cidr.parse::<IpNet>() {
                    if network.contains(&dest_ip_addr) {
                        return Some(peer);
                    }
                }
            }
        }

        None
    }

    /// Update peer's last active timestamp when receiving a packet
    ///
    /// Finds the peer by remote address and updates its `last_active`
    /// to the current time. This marks the peer as alive and enables
    /// data sending to this peer.
    ///
    /// # Arguments
    /// * `remote_addr` - Socket address of the peer that sent the packet
    ///
    /// # Behavior
    /// - Sets `last_active` from `None` to `Some(now)` on first packet
    /// - Updates `last_active` to current time on subsequent packets
    /// - Logs debug message on successful update
    async fn update_peer_active(&mut self, remote_addr: SocketAddr) {
        let mut peers = self.peers.write().await;
        for peer in peers.values_mut() {
            if peer.remote_addr == remote_addr {
                peer.last_active = Some(Instant::now());
                tracing::debug!("Updated last_active for peer: {}", peer.identity);
                break;
            }
        }
    }
}
