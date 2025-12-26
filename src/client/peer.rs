
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use crate::client::{P2P_HOLE_PUNCH_PORT, P2P_UDP_PORT};
use crate::codec::frame::{Frame, ProbeHolePunchFrame, ProbeIPv6Frame, RouteItem};
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

pub struct PeerService {
    /// UDP port to bind the socket to
    listen_port: u16,
    /// hole punch port
    hole_punch_port: u16,
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
        hole_punch_port: u16,
        input_tx: mpsc::Sender<(Vec<u8>, SocketAddr)>,
        output_rx: mpsc::Receiver<(Vec<u8>, SocketAddr)>,
    ) -> Self {
        PeerService {
            listen_port,
            hole_punch_port,
            input_tx,
            output_rx,
        }
    }

    pub async fn serve(&mut self) -> crate::Result<()> {
        let socket = UdpSocket::bind(format!("[::]:{}", self.listen_port)).await?;
        let socket4 = UdpSocket::bind(format!("0.0.0.0:{}", self.hole_punch_port)).await?;
        tracing::info!("P2P UDP listening on {}", socket.local_addr().unwrap().to_string());
        let mut buf = vec![0u8; 2048];
        let mut buf2 = vec![0u8; 2048];
        loop {
            tokio::select! {
                // Handle outbound packets: PeerHandler -> Network
                Some((data, remote)) = self.output_rx.recv() => {
                    if remote.is_ipv4() {
                         if let Err(e) = socket4.send_to(&data, remote).await {
                        tracing::error!("Failed to send to {}: {:?}", remote, e);
                    }
                    }else {
                        if let Err(e) = socket.send_to(&data, remote).await {
                            tracing::error!("Failed to send to {}: {:?}", remote, e);
                        }
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
                result = socket4.recv_from(&mut buf2) => {
                    match result {
                        Ok((len, remote)) => {
                            let packet = buf2[..len].to_vec();
                            if let Err(e) = self.input_tx.send((packet, remote)).await {
                                tracing::error!("Failed to forward received packet: {:?}", e);
                            }
                            buf2 = vec![0u8; 2048]; // Reset buffer for next packet
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
    remote_addr: Option<SocketAddr>,

    /// Stun socket address
    stun_addr: Option<SocketAddr>,

    /// Timestamp of last received packet from this peer
    ///
    /// - `None`: Never received any response (connection not established)
    /// - `Some(instant)`: Last successful communication time
    ///
    /// This is used to validate connection health before sending data.
    last_active: Option<Instant>,

    /// last_hole_punch_active
    stun_last_active: Option<Instant>,
}

#[derive(Debug)]
pub struct PeerStatus {
    /// Unique identifier of the peer
    pub identity: String,

    /// IPv6 direct connection info
    pub ipv6_addr: Option<SocketAddr>,
    pub ipv6_last_active: Option<Instant>,

    /// STUN hole-punched connection info
    pub stun_addr: Option<SocketAddr>,
    pub stun_last_active: Option<Instant>,
}

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

    /// Local peer identity
    identity: String,

    /// Local peer UDP port
    port: u16,

    // Hole punch UDP Port
    stun_port: u16,
}

impl PeerHandler {
    pub fn new(block: Arc<Box<dyn Block>>,
               identity: String) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel(OUTBOUND_BUFFER_SIZE);
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            outbound_tx: None,
            inbound_rx,
            inbound_tx,
            block,
            identity,
            port: P2P_UDP_PORT,
            stun_port: P2P_HOLE_PUNCH_PORT,
        }
    }

    /// Start the P2P UDP service in a background task
    ///
    /// Spawns a PeerService that handles actual socket I/O operations.
    /// After this call, the handler is ready to send/receive frames.
    /// The service binds to the port specified during `new()`.
    ///
    /// # Example
    /// ```rust,ignore
    /// use crate::client::P2P_UDP_PORT;
    ///
    /// let mut handler = PeerHandler::new(block, identity, ipv6, P2P_UDP_PORT);
    /// handler.run_peer();  // Bind to the port specified in new()
    /// ```
    pub fn run_peer(&mut self)  {
        let (output_tx, output_rx) = mpsc::channel(OUTBOUND_BUFFER_SIZE);
        let mut peer_service = PeerService::new(self.port, self.stun_port,
                                                self.inbound_tx.clone(), output_rx);

        // Spawn PeerService in background task
        tokio::spawn(async move {
            if let Err(e) = peer_service.serve().await {
                tracing::error!("PeerService error: {}", e);
            }
        });

        self.outbound_tx = Some(output_tx);
    }

    pub async fn add_peers(&mut self, routes: Vec<RouteItem>) {
        let mut peers = self.peers.write().await;

        for route in routes {
            // Save peer although ipv6 is not set
            // in order to update peer info if peer's ipv6 detected
            let ipv6_remote = if route.ipv6.is_empty() {
                None
            } else {
                // Parse remote address from IPv6 and port
                match format!("[{}]:{}", route.ipv6, route.port).parse::<SocketAddr>() {
                    Ok(addr) => Some(addr),
                    Err(e) => {
                        tracing::warn!("Invalid IPv6 address for peer {}: {}", route.identity, e);
                        None
                    }
                }
            };

            if ipv6_remote.is_some() {
                tracing::info!("Added IPv6 peer: {} at [{}]:{}", route.identity, route.ipv6, route.port);
                let _ = self.send_ipv6_probe(ipv6_remote.unwrap());
            }

            let stun_remote = if route.stun_ip.is_empty() {
                None
            } else {
                // Parse remote address from IPv6 and port
                match format!("{}:{}", route.stun_ip, route.stun_port).parse::<SocketAddr>() {
                    Ok(addr) => Some(addr),
                    Err(e) => {
                        tracing::warn!("Invalid IPv6 address for peer {}: {}", route.identity, e);
                        None
                    }
                }
            };

            if stun_remote.is_some() {
                tracing::info!("Added HolePunch peer: {} at {}:{}", route.identity, route.stun_ip, route.stun_port);
                let _ = self.send_hole_punch_probe(stun_remote.unwrap());
            }

            // Add or update peer in the map
            peers.insert(
                route.identity.clone(),
                PeerMeta {
                    identity: route.identity.clone(),
                    private_ip: route.private_ip.clone(),
                    ciders: route.ciders.clone(),
                    ipv6: route.ipv6.clone(),
                    port: route.port,
                    remote_addr: ipv6_remote,
                    stun_addr: stun_remote,
                    last_active: None,
                    stun_last_active: None,
                },
            );

        }
    }

    pub async fn update_peer(&mut self, identity: String,
                             ipv6: String, port: u16,
                             stun_ip: String, stun_port: u16) {
        let mut peers = self.peers.write().await;

        // Parse new remote address
        if let Some(peer) = peers.get_mut(&identity) {
            if !ipv6.is_empty() {
                self.update_ipv6(peer, ipv6, port);
            }

            if !stun_ip.is_empty() {
                self.update_stun(peer, stun_ip, stun_port);
            }
        }
    }

    fn update_ipv6(&self, peer: &mut PeerMeta, ipv6: String, port: u16) {
        let new_addr = match format!("[{}]:{}", ipv6, port).parse::<SocketAddr>() {
            Ok(addr) => addr,
            Err(e) => {
                tracing::warn!("Invalid new IPv6 address for peer {}: {}", peer.identity, e);
                return;
            }
        };
        let old = peer.remote_addr;
        if (old.is_some() && old.unwrap() != new_addr) || old.is_none() {
            tracing::info!("Update new ipv6 for peer {} {}", peer.identity, new_addr.to_string());
            peer.remote_addr = Some(new_addr);
            peer.last_active = None;
            let _ = self.send_ipv6_probe(new_addr);
        }
    }

    fn update_stun(&self, peer: &mut PeerMeta, stun_ip: String, stun_port: u16) {
        let new_addr = match format!("{}:{}", stun_ip, stun_port).parse::<SocketAddr>() {
            Ok(addr) => addr,
            Err(e) => {
                tracing::warn!("Invalid new stun address for peer {}: {}", peer.identity, e);
                return;
            }
        };
        let old = peer.stun_addr;
        if (old.is_some() && old.unwrap() != new_addr) || old.is_none() {
            tracing::info!("Update stun for peer {} {}", peer.identity, new_addr.to_string());
            peer.stun_addr = Some(new_addr);
            peer.stun_last_active = None;
            let _ = self.send_hole_punch_probe(new_addr);
        }
    }

    pub async fn start_probe_timer(&self) {
        let outbound_tx = match &self.outbound_tx {
            Some(tx) => tx.clone(),
            None => {
                tracing::error!("Cannot start keepalive: outbound_tx not initialized");
                return;
            }
        };

        let block = self.block.clone();
        let peers = self.peers.clone(); // Clone Arc, not the data
        let identity = self.identity.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(KEEPALIVE_INTERVAL);
            loop {
                interval.tick().await;

                let peer_addrs: Vec<SocketAddr> = {
                    let peers_guard = peers.read().await;
                    peers_guard.values()
                        .filter(|p| p.remote_addr.is_some())
                        .map(|p| p.remote_addr.unwrap()).collect()
                };

                let probe_ipv6 = Frame::ProbeIPv6(ProbeIPv6Frame {
                    identity: identity.clone(),
                });
                for remote_addr in &peer_addrs {
                    let probe_ipv6_data = Parser::marshal(probe_ipv6.clone(), block.as_ref());
                    if let Ok(data) = probe_ipv6_data {
                        if let Err(e) = outbound_tx.send((data, *remote_addr)).await {
                            tracing::warn!("Failed to send probe ipv6: {}", e);
                            continue;
                        }
                        tracing::info!("Sent probe ipv6 for {}", remote_addr);
                    }
                }

                let peer_addrs: Vec<SocketAddr> = {
                    let peers_guard = peers.read().await;
                    peers_guard.values()
                        .filter(|p| p.stun_addr.is_some())
                        .map(|p| p.stun_addr.unwrap()).collect()
                };

                let probe_hole = Frame::ProbeHolePunch(ProbeHolePunchFrame {
                    identity: identity.clone(),
                });
                for remote_addr in &peer_addrs {
                    let probe_hole_data = Parser::marshal(probe_hole.clone(), block.as_ref());
                    if let Ok(data) = probe_hole_data {
                        if let Err(e) = outbound_tx.send((data, *remote_addr)).await {
                            tracing::warn!("Failed to send probe ipv6: {}", e);
                            continue;
                        }
                        tracing::info!("Sent probe hole punch for {}", remote_addr);
                    }
                }

            }
        });
    }

    fn send_ipv6_probe(&self, remote_addr: SocketAddr) -> crate::Result<()> {
        let outbound_tx = self.outbound_tx.as_ref().ok_or("outbound_tx not initialized")?;
        let probe_ipv6 = Frame::ProbeIPv6(ProbeIPv6Frame {
            identity: self.identity.clone(),
        });
        let data = Parser::marshal(probe_ipv6, self.block.as_ref())?;

        // Non-blocking send - best effort
        let _ = outbound_tx.try_send((data, remote_addr));
        Ok(())
    }

    fn send_hole_punch_probe(&self, remote_addr: SocketAddr) -> crate::Result<()> {
        let outbound_tx = self.outbound_tx.as_ref().ok_or("outbound_tx not initialized")?;
        let probe_hole_punch = Frame::ProbeHolePunch(ProbeHolePunchFrame {
            identity: self.identity.clone(),
        });
        let data = Parser::marshal(probe_hole_punch, self.block.as_ref())?;
        let _ = outbound_tx.try_send((data, remote_addr));
        Ok(())
    }

    pub async fn recv_frame(&mut self) -> crate::Result<Frame> {
        loop {
            let (buf, remote) = self
                .inbound_rx
                .recv()
                .await
                .ok_or("recv from peers channel closed")?;

            let (frame, _) = Parser::unmarshal(&buf, self.block.as_ref())?;

            match frame {
                Frame::ProbeIPv6(probe) => {
                    tracing::info!("Received probe ipv6 from peer {} at {}", probe.identity, remote);

                    let mut peers = self.peers.write().await;
                    if let Some(peer) = peers.get_mut(&probe.identity) {
                        peer.remote_addr = Some(remote);
                        peer.last_active = Some(Instant::now());
                    }
                }
                Frame::ProbeHolePunch(probe) => {
                    tracing::info!("Received probe hole punch from peer {} at {}", probe.identity, remote);
                    let mut peers = self.peers.write().await;
                    if let Some(peer) = peers.get_mut(&probe.identity) {
                        peer.stun_addr = Some(remote);
                        peer.stun_last_active = Some(Instant::now());
                    }
                }
                _ => {
                    self.update_peer_active(remote).await;
                    return Ok(frame);
                }
            }
        }
    }

    pub async fn send_frame(&self, frame: Frame, dest_ip: &str) -> crate::Result<()> {
        // Find peer responsible for this destination
        let peers = self.peers.read().await;
        let peer = self.find_peer_by_ip_locked(&peers, dest_ip)
            .ok_or("No peer found for destination")?;

        // Check if we have any address at all
        if peer.remote_addr.is_none() && peer.stun_addr.is_none() {
            return Err(format!("Peer {} has no available address (IPv6 or STUN)", peer.identity).into());
        }

        let peer_identity = peer.identity.clone();
        let remote_addr = peer.remote_addr;
        let stun_addr = peer.stun_addr;
        let ipv6_last_active = peer.last_active;      // IPv6 last active
        let stun_last_active = peer.stun_last_active;  // STUN last active
        
        // Release lock before async operations
        drop(peers);

        // Marshal frame once for potential multiple attempts
        let data = Parser::marshal(frame, self.block.as_ref())?;
        let outbound_tx = self.outbound_tx.as_ref().ok_or("outbound_tx not initialized")?;

        // Strategy: Try IPv6 first, fallback to STUN
        
        // Attempt 1: Try IPv6 direct connection
        if let Some(ipv6_addr) = remote_addr {
            // Validate IPv6 connection is active
            match ipv6_last_active {
                Some(last_active_time) => {
                    let elapsed = Instant::now().duration_since(last_active_time);
                    if elapsed <= CONNECTION_TIMEOUT {
                        // IPv6 connection is valid, use it
                        outbound_tx
                            .send((data, ipv6_addr))
                            .await
                            .map_err(|e| format!("Failed to send via IPv6: {}", e))?;

                        tracing::debug!("Sent frame to peer {} via IPv6: {}", peer_identity, ipv6_addr);
                        return Ok(());
                    } else {
                        tracing::debug!(
                            "IPv6 connection to {} expired ({:?} ago), trying STUN",
                            peer_identity, elapsed
                        );
                    }
                }
                None => {
                    tracing::debug!("Peer {} IPv6 never responded, trying STUN", peer_identity);
                }
            }
        }

        // Attempt 2: Try STUN address
        if let Some(stun_address) = stun_addr {
            // Validate STUN connection is active
            match stun_last_active {
                Some(last_active_time) => {
                    let elapsed = Instant::now().duration_since(last_active_time);
                    if elapsed <= CONNECTION_TIMEOUT {
                        // STUN connection is valid, use it
                        outbound_tx
                            .send((data, stun_address))
                            .await
                            .map_err(|e| format!("Failed to send via STUN: {}", e))?;
                        
                        tracing::debug!("Sent frame to peer {} via STUN: {}", peer_identity, stun_address);
                        return Ok(());
                    } else {
                        return Err(format!(
                            "Peer {} STUN connection also expired ({:?} ago)",
                            peer_identity, elapsed
                        ).into());
                    }
                }
                None => {
                    return Err(format!("Peer {} STUN address never responded", peer_identity).into());
                }
            }
        }

        // Both attempts failed
        Err(format!(
            "Failed to send to peer {}: IPv6 unavailable/expired, STUN unavailable/expired",
            peer_identity
        ).into())
    }

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
                if let Ok(network) = cidr.parse::<IpNet>()
                    && network.contains(&dest_ip_addr) {
                    return Some(peer);
                }
            }
        }

        None
    }

    async fn update_peer_active(&mut self, remote_addr: SocketAddr) {
        let mut peers = self.peers.write().await;
        for peer in peers.values_mut() {
            match peer.remote_addr {
                Some(r) => {
                    if r == remote_addr {
                        peer.last_active = Some(Instant::now());
                        tracing::debug!("Updated last_active for peer: {}", peer.identity);
                        break;
                    }
                }
                None => continue,
            }
        }
    }

    pub async fn get_status(&self) -> Vec<PeerStatus> {
        let guard = self.peers.read().await;
        let mut result: Vec<PeerStatus> = Vec::new();
        for peer in guard.values() {
            let status = PeerStatus {
                identity: peer.identity.clone(),
                ipv6_addr: peer.remote_addr,
                ipv6_last_active: peer.last_active,
                stun_addr: peer.stun_addr,
                stun_last_active: peer.stun_last_active,
            };
            result.push(status);
        }
        result
    }
}
