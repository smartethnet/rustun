
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use crate::client::{P2P_HOLE_PUNCH_PORT, P2P_UDP_PORT};
use crate::client::p2p::{PeerMeta, PeerStatus, CONNECTION_TIMEOUT, KEEPALIVE_INTERVAL, OUTBOUND_BUFFER_SIZE};
use crate::client::p2p::udp_server::UDPServer;
use crate::codec::frame::{Frame, ProbeHolePunchFrame, ProbeIPv6Frame, PeerDetail};
use crate::codec::parser::Parser;
use crate::crypto::Block;

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
    inbound_rx: Option<mpsc::Receiver<(Vec<u8>, SocketAddr)>>,

    /// Encryption/decryption block for frame marshaling
    block: Arc<Box<dyn Block>>,

    /// Local peer identity
    identity: String,

    /// Local peer UDP port
    port: u16,

    // Hole punch UDP Port
    stun_port: u16,
}

/// Result of attempting to send data via a specific address
enum SendResult {
    Success,
    Expired(Duration),
    NeverResponded,
    NoAddress,
}

impl PeerHandler {
    pub fn new(block: Arc<Box<dyn Block>>,
               identity: String) -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            outbound_tx: None,
            inbound_rx: None,
            block,
            identity,
            port: P2P_UDP_PORT,
            stun_port: P2P_HOLE_PUNCH_PORT,
        }
    }

    /// run peer service listen udp socket for p2p
    pub fn run_peer_service(&mut self)  {
        let (output_tx, output_rx) = mpsc::channel(OUTBOUND_BUFFER_SIZE);
        let (inbound_tx, inbound_rx) = mpsc::channel(OUTBOUND_BUFFER_SIZE);
        let mut udp_server = UDPServer::new(self.port, self.stun_port,
                                              inbound_tx, output_rx);

        tokio::spawn(async move {
            if let Err(e) = udp_server.serve().await {
                tracing::error!("PeerService error: {}", e);
            }
        });

        self.outbound_tx = Some(output_tx);
        self.inbound_rx = Some(inbound_rx);
        tracing::info!("Running p2p peer service");
    }

    /// rewrite peers with new peers details for route
    ///
    /// this function will update peer's ipv6 and stun address
    ///
    pub async fn rewrite_peers(&mut self, peer_details: Vec<PeerDetail>) {
        {
            let mut peers = self.peers.write().await;
            *peers = HashMap::new();
        }

        for p in peer_details {
            self.add_peer(p).await;
        }
    }

    async fn add_peer(&self, p: PeerDetail) {
        let mut peers = self.peers.write().await;
        let ipv6_remote = self.parse_address(
            &p.identity,
            &p.ipv6,
            p.port,
            true, // is_ipv6
        );
        if ipv6_remote.is_some() {
            tracing::info!("Added IPv6 peer: {} at {}:{}", p.identity, p.ipv6, p.port);
        }

        let stun_remote = self.parse_address(
            &p.identity,
            &p.stun_ip,
            p.stun_port,
            false, // is_ipv4
        );
        if stun_remote.is_some() {
            tracing::info!("Added Hole Punch peer: {} at {}:{}", p.identity, p.ipv6, p.port);
        }

        // Add or update peer in the map
        peers.insert(
            p.identity.clone(),
            PeerMeta {
                identity: p.identity.clone(),
                private_ip: p.private_ip.clone(),
                ciders: p.ciders.clone(),
                ipv6: p.ipv6.clone(),
                port: p.port,
                remote_addr: ipv6_remote,
                stun_addr: stun_remote,
                last_active: None,
                stun_last_active: None,
            },
        );
    }

    fn parse_address(
        &self,
        identity: &str,
        ip: &str,
        port: u16,
        is_ipv6: bool,
    ) -> Option<SocketAddr> {
        if ip.is_empty() {
            return None;
        }

        let addr_str = if is_ipv6 {
            format!("[{}]:{}", ip, port)
        } else {
            format!("{}:{}", ip, port)
        };

        let addr = match addr_str.parse::<SocketAddr>() {
            Ok(addr) => addr,
            Err(e) => {
                let protocol = if is_ipv6 { "IPv6" } else { "IPv4" };
                tracing::warn!("Invalid {} address for peer {}: {}", protocol, identity, e);
                return None;
            }
        };
        Some(addr)
    }

    /// insert or update peers
    ///
    /// if peer exist, and the ipv6/stun_ip changed,
    /// update peer and set last_active/stun_last_active to None,
    /// this will disable p2p temporary, if the new address reply probe, p2p will enable
    ///
    /// if peer not exist, add it.
    ///
    pub async fn insert_or_update(&mut self, peer_details: Vec<PeerDetail>) {
        let mut peers = self.peers.write().await;
        for peer in peer_details {
            match peers.get_mut(&peer.identity) {
                Some(existing_peer) => {
                    if !peer.ipv6.is_empty() {
                        // update ipv6 if changed
                        self.update_address(existing_peer, &peer.ipv6, peer.port, true);
                    }

                    if !peer.stun_ip.is_empty() {
                        // update stun_ip if changed
                        self.update_address(existing_peer, &peer.stun_ip, peer.stun_port, false);
                    }
                }
                None => {
                    self.add_peer(peer).await;
                }
            }

        }
    }

    fn update_address(&self, peer: &mut PeerMeta, ip: &str, port: u16, is_ipv6: bool) {
        // Format and parse address
        let addr_str = if is_ipv6 {
            format!("[{}]:{}", ip, port)
        } else {
            format!("{}:{}", ip, port)
        };

        let new_addr = match addr_str.parse::<SocketAddr>() {
            Ok(addr) => addr,
            Err(e) => {
                let protocol = if is_ipv6 { "IPv6" } else { "STUN" };
                tracing::warn!("Invalid new {} address for peer {}: {}", protocol, peer.identity, e);
                return;
            }
        };

        let (old_addr, protocol) = if is_ipv6 {
            (peer.remote_addr, "IPv6")
        } else {
            (peer.stun_addr, "STUN")
        };

        if old_addr != Some(new_addr) {
            tracing::info!(
                "Update {} address for peer {}: {} -> {}",
                protocol,
                peer.identity,
                old_addr.map(|a| a.to_string()).unwrap_or_else(|| "None".to_string()),
                new_addr
            );

            if is_ipv6 {
                peer.remote_addr = Some(new_addr);
                peer.last_active = None;
            } else {
                peer.stun_addr = Some(new_addr);
                peer.stun_last_active = None;
            }
        }
    }

    /// recv_frame to recv from local p2p socket to get peers frame
    ///
    /// only support for ProbeIPv6, ProbeStun, Data
    ///
    /// **ProbeIPv6**
    /// - update last_active, this is for p2p send_frame healthy checker
    /// - remote address, most of the time this is not changed.
    pub async fn recv_frame(&mut self) -> crate::Result<Frame> {
        let inbound_rx = self.inbound_rx.as_mut().ok_or("inbound_rx not initialized")?;
        
        loop {
            let (buf, remote) = inbound_rx
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

    /// send_frame tries to get peers that contains dest_ip in ciders or private_ip
    ///
    /// firstly try ipv6 direct, if peers is healthy(base on last_active)
    ///
    /// secondary try p2p hole punch, if peers is healthy(base on stun_last_active)
    ///
    pub async fn send_frame(&self, frame: Frame, dest_ip: &str) -> crate::Result<()> {
        let peers = self.peers.read().await;
        let peer = self.find_peer_by_ip_locked(&peers, dest_ip)
            .ok_or("No peer found for destination")?;

        if peer.remote_addr.is_none() && peer.stun_addr.is_none() {
            return Err(format!("Peer {} has no available address (IPv6 or STUN)", peer.identity).into());
        }

        let peer_identity = peer.identity.clone();
        let remote_addr = peer.remote_addr;
        let stun_addr = peer.stun_addr;
        let ipv6_last_active = peer.last_active;
        let stun_last_active = peer.stun_last_active;

        drop(peers);

        // Marshal frame once for potential multiple attempts
        let data = Parser::marshal(frame, self.block.as_ref())?;
        let outbound_tx = self.outbound_tx.as_ref().ok_or("outbound_tx not initialized")?;

        // Attempt 1: Try IPv6 direct connection
        match self.try_send_via(
            outbound_tx,
            &data,
            remote_addr,
            ipv6_last_active,
            &peer_identity,
            "IPv6"
        ).await {
            SendResult::Success => return Ok(()),
            SendResult::Expired(elapsed) => {
                tracing::debug!(
                    "IPv6 connection to {} expired ({:?} ago), trying STUN",
                    peer_identity, elapsed
                );
            }
            SendResult::NeverResponded => {
                tracing::debug!("Peer {} IPv6 never responded, trying STUN", peer_identity);
            }
            SendResult::NoAddress => {
                // No IPv6 address, try STUN
            }
        }

        // Attempt 2: Try STUN address
        match self.try_send_via(
            outbound_tx,
            &data,
            stun_addr,
            stun_last_active,
            &peer_identity,
            "STUN"
        ).await {
            SendResult::Success => Ok(()),
            SendResult::Expired(elapsed) => {
                Err(format!(
                    "Peer {} STUN connection also expired ({:?} ago)",
                    peer_identity, elapsed
                ).into())
            }
            SendResult::NeverResponded => {
                Err(format!("Peer {} STUN address never responded", peer_identity).into())
            }
            SendResult::NoAddress => {
                // Both attempts failed
                Err(format!(
                    "Failed to send to peer {}: IPv6 unavailable/expired, STUN unavailable/expired",
                    peer_identity
                ).into())
            }
        }
    }

    async fn try_send_via(
        &self,
        outbound_tx: &mpsc::Sender<(Vec<u8>, SocketAddr)>,
        data: &[u8],
        addr: Option<SocketAddr>,
        last_active: Option<Instant>,
        peer_identity: &str,
        protocol: &str,
    ) -> SendResult {
        // Check if address exists
        let addr = match addr {
            Some(a) => a,
            None => return SendResult::NoAddress,
        };

        // Check if connection is active
        let last_active_time = match last_active {
            Some(t) => t,
            None => return SendResult::NeverResponded,
        };

        let elapsed = Instant::now().duration_since(last_active_time);
        if elapsed > CONNECTION_TIMEOUT {
            return SendResult::Expired(elapsed);
        }

        // Connection is valid, send the packet
        match outbound_tx.send((data.to_vec(), addr)).await {
            Ok(_) => {
                tracing::debug!("Sent frame to peer {} via {}: {}", peer_identity, protocol, addr);
                SendResult::Success
            }
            Err(e) => {
                tracing::error!("Failed to send via {}: {}", protocol, e);
                SendResult::NeverResponded // Treat send error as connection problem
            }
        }
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
            // Check if this is from IPv6 address
            if let Some(ipv6_addr) = peer.remote_addr {
                if ipv6_addr == remote_addr {
                    peer.last_active = Some(Instant::now());
                    tracing::debug!("Updated IPv6 last_active for peer: {}", peer.identity);
                    return;
                }
            }
            
            // Check if this is from STUN address
            if let Some(stun_addr) = peer.stun_addr {
                if stun_addr == remote_addr {
                    peer.stun_last_active = Some(Instant::now());
                    tracing::debug!("Updated STUN last_active for peer: {}", peer.identity);
                    return;
                }
            }
        }
        
        tracing::warn!("Received packet from unknown peer address: {}", remote_addr);
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

impl PeerHandler {
    pub async fn start_probe_timer(&self) {
        let outbound_tx = match &self.outbound_tx {
            Some(tx) => tx.clone(),
            None => {
                tracing::error!("Cannot start probe timer: outbound_tx not initialized");
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

                // Send IPv6 probes
                Self::send_probes(
                    &peers,
                    &outbound_tx,
                    &block,
                    &identity,
                    true, // is_ipv6
                ).await;

                // Send STUN hole punch probes
                Self::send_probes(
                    &peers,
                    &outbound_tx,
                    &block,
                    &identity,
                    false, // is_ipv4/stun
                ).await;
            }
        });
    }

    async fn send_probes(
        peers: &Arc<RwLock<HashMap<String, PeerMeta>>>,
        outbound_tx: &mpsc::Sender<(Vec<u8>, SocketAddr)>,
        block: &Arc<Box<dyn Block>>,
        identity: &str,
        is_ipv6: bool,
    ) {
        let peer_addrs: Vec<SocketAddr> = {
            let peers_guard = peers.read().await;
            peers_guard
                .values()
                .filter_map(|p| {
                    if is_ipv6 {
                        p.remote_addr
                    } else {
                        p.stun_addr
                    }
                })
                .collect()
        };

        // Skip if no peers have this type of address
        if peer_addrs.is_empty() {
            return;
        }

        // Create appropriate probe frame
        let probe_frame = if is_ipv6 {
            Frame::ProbeIPv6(ProbeIPv6Frame {
                identity: identity.to_string(),
            })
        } else {
            Frame::ProbeHolePunch(ProbeHolePunchFrame {
                identity: identity.to_string(),
            })
        };

        // Marshal once, reuse for all peers
        let probe_data = match Parser::marshal(probe_frame, block.as_ref()) {
            Ok(data) => data,
            Err(e) => {
                let protocol = if is_ipv6 { "IPv6" } else { "STUN" };
                tracing::error!("Failed to marshal {} probe: {}", protocol, e);
                return;
            }
        };

        // Send to all peers
        let protocol = if is_ipv6 { "IPv6" } else { "hole punch" };
        for remote_addr in peer_addrs {
            if let Err(e) = outbound_tx.send((probe_data.clone(), remote_addr)).await {
                tracing::warn!("Failed to send {} probe to {}: {}", protocol, remote_addr, e);
            } else {
                tracing::info!("Sent {} probe to {}", protocol, remote_addr);
            }
        }
    }

}
