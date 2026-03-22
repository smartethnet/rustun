use crate::client::p2p::udp_server::UDPServer;
use crate::client::p2p::{
    CONNECTION_TIMEOUT, KEEPALIVE_INTERVAL, LastActive, OUTBOUND_BUFFER_SIZE, PeerMeta, PeerStatus,
};
use crate::client::{P2P_HOLE_PUNCH_PORT, P2P_UDP_PORT};
use crate::codec::frame::{Frame, PeerDetail, ProbeHolePunchFrame, ProbeIPv6Frame};
use crate::codec::parser::Parser;
use crate::crypto::Block;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

pub struct PeerHandlerApi {
    pub new_peers: NewPeersTx,
    pub new_frame: NewFrameRx,
    pub send_frame: SendFrameTx,
    pub get_status: GetStatusTx,
}

struct PeerHandlerPrivateRxApi {
    pub new_peers: NewPeersRx,
    pub send_frame: SendFrameRx,
    pub get_status: GetStatusRx,
    pub inbound_rx: mpsc::Receiver<(Vec<u8>, SocketAddr)>,
}
struct PeerHandlerPrivateTxApi {
    pub new_frame: NewFrameTx,
    pub outbound_tx: mpsc::Sender<(Vec<u8>, Vec<SocketAddr>)>,
}

#[derive(Debug)]
pub struct NewPeersTx(pub mpsc::Sender<Vec<PeerDetail>>);
#[derive(Debug)]
pub struct NewPeersRx(mpsc::Receiver<Vec<PeerDetail>>);
#[derive(Debug)]
pub struct NewFrameTx(mpsc::Sender<Frame>);
#[derive(Debug)]
pub struct NewFrameRx(pub mpsc::Receiver<Frame>);
#[derive(Debug)]
pub struct SendFrameTx(pub mpsc::Sender<SendFrame>);
#[derive(Debug)]
pub struct SendFrameRx(mpsc::Receiver<SendFrame>);
#[derive(Debug)]
pub struct SendFrame {
    pub frame: Frame,
    pub dst: String,
}
#[derive(Debug)]
pub struct GetStatusTx(mpsc::Sender<oneshot::Sender<Vec<PeerStatus>>>);
impl GetStatusTx {
    pub async fn get(&self) -> anyhow::Result<Vec<PeerStatus>> {
        let (a, b) = oneshot::channel::<Vec<PeerStatus>>();
        self.0.send(a).await?;
        Ok(b.await?)
    }
}
#[derive(Debug)]
pub struct GetStatusRx(mpsc::Receiver<oneshot::Sender<Vec<PeerStatus>>>);

#[derive(Debug)]
struct PeerSet {
    peers: HashMap<String, PeerMeta>,
}
impl PeerSet {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    pub fn all_peer_addrs(&self, protocol: Protocol) -> Vec<SocketAddr> {
        self.peers
            .values()
            .filter_map(|p| match protocol {
                Protocol::Ipv6 => *p.remote_addr.get(),
                Protocol::Stun => *p.stun_addr.get(),
            })
            .collect()
    }

    pub fn update_peer_active(&mut self, identity: &str, addr: SocketAddr, protocol: Protocol) {
        let Some(peer) = self.peers.get_mut(identity) else {
            return;
        };
        match protocol {
            Protocol::Stun => peer.stun_addr.activate(Some(addr)),
            Protocol::Ipv6 => peer.remote_addr.activate(Some(addr)),
        }
    }
    pub fn update_peer_active_by_addr(&mut self, remote_addr: SocketAddr) {
        for peer in self.peers.values_mut() {
            // Check if this is from IPv6 address
            if *peer.remote_addr.get() == Some(remote_addr) {
                peer.remote_addr.restart();
                tracing::debug!("Updated IPv6 last_active for peer: {}", peer.identity);
                return;
            }
            // Check if this is from STUN address
            if *peer.stun_addr.get() == Some(remote_addr) {
                peer.stun_addr.restart();
                tracing::debug!("Updated STUN last_active for peer: {}", peer.identity);
                return;
            }
        }
        tracing::warn!("Received packet from unknown peer address: {}", remote_addr);
    }

    pub fn find_peer_by_ip_locked<'a>(&'a self, dest_ip: &str) -> Option<&'a PeerMeta> {
        use ipnet::IpNet;
        use std::net::IpAddr;

        let dest_ip_addr = match dest_ip.parse::<IpAddr>() {
            Ok(ip) => ip,
            Err(_) => return None,
        };

        for peer in self.peers.values() {
            // Check exact match with peer's private IP
            if peer.private_ip == dest_ip {
                return Some(peer);
            }

            // Check if destination falls within peer's CIDR ranges
            for cidr in &peer.ciders {
                if let Ok(network) = cidr.parse::<IpNet>()
                    && network.contains(&dest_ip_addr)
                {
                    return Some(peer);
                }
            }
        }

        None
    }

    /// insert or update peers
    ///
    /// if peer exist, and the ipv6/stun_ip changed,
    /// update peer and set last_active/stun_last_active to None,
    /// this will disable p2p temporary, if the new address reply probe, p2p will enable
    ///
    /// if peer not exist, add it.
    ///
    pub fn insert_or_update_dormant(&mut self, peer_details: Vec<PeerDetail>) {
        let peers = &mut self.peers;
        for peer in peer_details {
            match peers.get_mut(&peer.identity) {
                Some(existing_peer) => {
                    if !peer.ipv6.is_empty()
                        && let Some(addr) = parse_address(&peer.identity, &peer.ipv6, peer.port)
                    {
                        update_address(existing_peer, addr, Protocol::Ipv6);
                    }

                    if !peer.stun_ip.is_empty()
                        && let Some(addr) =
                            parse_address(&peer.identity, &peer.stun_ip, peer.stun_port)
                    {
                        update_address(existing_peer, addr, Protocol::Stun);
                    }
                }
                None => {
                    let ipv6_remote = parse_address(&peer.identity, &peer.ipv6, peer.port);
                    if ipv6_remote.is_some() {
                        tracing::info!(
                            "Added IPv6 peer: {} at {}:{}",
                            peer.identity,
                            peer.ipv6,
                            peer.port
                        );
                    }

                    let stun_remote = parse_address(&peer.identity, &peer.stun_ip, peer.stun_port);
                    if stun_remote.is_some() {
                        tracing::info!(
                            "Added Hole Punch peer: {} at {}:{}",
                            peer.identity,
                            peer.ipv6,
                            peer.port
                        );
                    }

                    // Add or update peer in the map
                    peers.insert(
                        peer.identity.clone(),
                        PeerMeta {
                            name: peer.name.clone(),
                            identity: peer.identity.clone(),
                            private_ip: peer.private_ip.clone(),
                            ciders: peer.ciders.clone(),
                            remote_addr: LastActive::dormant(ipv6_remote),
                            stun_addr: LastActive::dormant(stun_remote),
                        },
                    );
                }
            }
        }
    }

    pub fn add_peer(&mut self, p: PeerDetail) {
        let ipv6_remote = parse_address(&p.identity, &p.ipv6, p.port);
        if ipv6_remote.is_some() {
            tracing::info!("Added IPv6 peer: {} at {}:{}", p.identity, p.ipv6, p.port);
        }

        let stun_remote = parse_address(&p.identity, &p.stun_ip, p.stun_port);
        if stun_remote.is_some() {
            tracing::info!(
                "Added Hole Punch peer: {} at {}:{}",
                p.identity,
                p.ipv6,
                p.port
            );
        }

        // Add or update peer in the map
        let peers = &mut self.peers;
        peers.insert(
            p.identity.clone(),
            PeerMeta {
                name: p.name.clone(),
                identity: p.identity.clone(),
                private_ip: p.private_ip.clone(),
                ciders: p.ciders.clone(),
                remote_addr: LastActive::dormant(ipv6_remote),
                stun_addr: LastActive::dormant(stun_remote),
            },
        );
    }

    pub fn get_status(&self) -> Vec<PeerStatus> {
        let mut result: Vec<PeerStatus> = Vec::new();
        for peer in self.peers.values() {
            let status = PeerStatus {
                name: peer.name.clone(),
                identity: peer.identity.clone(),
                ipv6_addr: *peer.remote_addr.get(),
                ipv6_last_active: peer.remote_addr.last_active(),
                stun_addr: *peer.stun_addr.get(),
                stun_last_active: peer.stun_addr.last_active(),
            };
            result.push(status);
        }
        result
    }
}

pub struct PeerHandler {
    peers: PeerSet,
    block: Arc<Box<dyn Block>>,
    identity: String,
    tx_api: PeerHandlerPrivateTxApi,
}

/// Result of attempting to send data via a specific address
enum SendResult {
    Success,
    Expired(Duration),
    NeverResponded,
    NoAddress,
}

impl PeerHandler {
    /// run peer service listen udp socket for p2p
    pub fn start_peer_service(
        block: Arc<Box<dyn Block>>,
        identity: String,
        peer_details: Vec<PeerDetail>,
    ) -> PeerHandlerApi {
        let (outbound_tx, output_rx) = mpsc::channel(OUTBOUND_BUFFER_SIZE);
        let (inbound_tx, inbound_rx) = mpsc::channel(OUTBOUND_BUFFER_SIZE);
        let mut udp_server =
            UDPServer::new(P2P_UDP_PORT, P2P_HOLE_PUNCH_PORT, inbound_tx, output_rx);
        tokio::spawn(async move {
            if let Err(e) = udp_server.serve().await {
                tracing::error!("PeerService error: {e}");
            }
        });
        let (new_pears_tx, new_pears_rx) = mpsc::channel(1024);
        let (new_frame_tx, new_frame_rx) = mpsc::channel(1024);
        let (send_frame_tx, send_frame_rx) = mpsc::channel(1024);
        let (get_status_tx, get_status_rx) = mpsc::channel(1024);
        let private_rx_api = PeerHandlerPrivateRxApi {
            new_peers: NewPeersRx(new_pears_rx),
            send_frame: SendFrameRx(send_frame_rx),
            get_status: GetStatusRx(get_status_rx),
            inbound_rx,
        };
        let mut this = Self {
            peers: PeerSet::new(),
            block,
            identity,
            tx_api: PeerHandlerPrivateTxApi {
                new_frame: NewFrameTx(new_frame_tx),
                outbound_tx,
            },
        };
        this.rewrite_peers(peer_details);
        tokio::spawn(async move {
            if let Err(e) = this.run_peer_service(private_rx_api).await {
                tracing::error!("peer service failed: {e}");
            }
        });
        tracing::info!("Running p2p peer service");
        PeerHandlerApi {
            new_peers: NewPeersTx(new_pears_tx),
            new_frame: NewFrameRx(new_frame_rx),
            send_frame: SendFrameTx(send_frame_tx),
            get_status: GetStatusTx(get_status_tx),
        }
    }
    async fn run_peer_service(mut self, rx_api: PeerHandlerPrivateRxApi) -> anyhow::Result<()> {
        let mut send_probes_interval = tokio::time::interval(KEEPALIVE_INTERVAL);
        let PeerHandlerPrivateRxApi {
            mut new_peers,
            mut send_frame,
            mut get_status,
            mut inbound_rx,
        } = rx_api;

        loop {
            tokio::select! {
                _ = send_probes_interval.tick() => {
                    self.send_probes().await;
                }
                Some(peer_details) = new_peers.0.recv() => {
                    self.insert_or_update(peer_details);
                }
                Some(sf) = send_frame.0.recv() => {
                    if let Err(e) = self.send_frame(sf.frame, &sf.dst).await {
                        tracing::warn!("send_frame failed: {e}");
                    }
                }
                Some(reply_tx) = get_status.0.recv() => {
                    let _ = reply_tx.send(self.get_status());
                }
                opt = inbound_rx.recv() => {
                    let opt = opt.ok_or(anyhow::anyhow!("recv from peers channel closed"))?;
                    if let Err(e) = self.recv_frame(opt).await {
                        tracing::warn!("recv_frame failed: {e}");
                    };
                }
            }
        }
    }

    /// rewrite peers with new peers details for route
    ///
    /// this function will update peer's ipv6 and stun address
    ///
    fn rewrite_peers(&mut self, peer_details: Vec<PeerDetail>) {
        self.peers = PeerSet::new();
        for p in peer_details {
            self.peers.add_peer(p);
        }
    }

    fn insert_or_update(&mut self, peer_details: Vec<PeerDetail>) {
        self.peers.insert_or_update_dormant(peer_details);
    }

    /// recv_frame to recv from local p2p socket to get peers frame
    ///
    /// only support for ProbeIPv6, ProbeStun, Data
    ///
    /// **ProbeIPv6**
    /// - update last_active, this is for p2p send_frame healthy checker
    /// - remote address, most of the time this is not changed.
    async fn recv_frame(&mut self, msg: (Vec<u8>, SocketAddr)) -> anyhow::Result<()> {
        let (buf, remote) = msg;

        let (frame, _) = Parser::unmarshal(&buf, self.block.as_ref().as_ref())?;

        match frame {
            Frame::ProbeIPv6(probe) => {
                tracing::info!(
                    "Received probe ipv6 from peer {} at {remote}",
                    probe.identity
                );
                self.peers
                    .update_peer_active(&probe.identity, remote, Protocol::Ipv6);
            }
            Frame::ProbeHolePunch(probe) => {
                tracing::info!(
                    "Received probe hole punch from peer {} at {remote}",
                    probe.identity
                );
                self.peers
                    .update_peer_active(&probe.identity, remote, Protocol::Stun);
            }
            _ => {
                self.peers.update_peer_active_by_addr(remote);
                let _ = self.tx_api.new_frame.0.send(frame).await;
            }
        }
        Ok(())
    }

    /// send_frame tries to get peers that contains dest_ip in ciders or private_ip
    ///
    /// firstly try ipv6 direct, if peers is healthy(base on last_active)
    ///
    /// secondary try p2p hole punch, if peers is healthy(base on stun_last_active)
    ///
    async fn send_frame(&self, frame: Frame, dest_ip: &str) -> anyhow::Result<()> {
        let peer = self
            .peers
            .find_peer_by_ip_locked(dest_ip)
            .ok_or_else(|| anyhow::anyhow!("No peer found for destination"))?;

        if peer.remote_addr.get().is_none() && peer.stun_addr.get().is_none() {
            return Err(anyhow::anyhow!(
                "Peer {} has no available address (IPv6 or STUN)",
                peer.identity
            ));
        }
        let peer_identity = peer.identity.clone();

        // Marshal frame once for potential multiple attempts
        let data = Parser::marshal(frame, self.block.as_ref().as_ref())?;
        let outbound_tx = &self.tx_api.outbound_tx;

        // Attempt 1: Try IPv6 direct connection
        match self
            .try_send_via(
                outbound_tx,
                &data,
                *peer.remote_addr.get(),
                peer.remote_addr.last_active(),
                &peer_identity,
                "IPv6",
            )
            .await
        {
            SendResult::Success => return Ok(()),
            SendResult::Expired(elapsed) => {
                tracing::debug!(
                    "IPv6 connection to {peer_identity} expired ({elapsed:?} ago), trying STUN"
                );
            }
            SendResult::NeverResponded => {
                tracing::debug!("Peer {peer_identity} IPv6 never responded, trying STUN");
            }
            SendResult::NoAddress => {
                // No IPv6 address, try STUN
            }
        }

        // Attempt 2: Try STUN address
        match self
            .try_send_via(
                outbound_tx,
                &data,
                *peer.stun_addr.get(),
                peer.stun_addr.last_active(),
                &peer_identity,
                "STUN",
            )
            .await
        {
            SendResult::Success => Ok(()),
            SendResult::Expired(elapsed) => Err(anyhow::anyhow!(
                "Peer {peer_identity} STUN connection also expired ({elapsed:?} ago)"
            )),
            SendResult::NeverResponded => Err(anyhow::anyhow!(
                "Peer {peer_identity} STUN address never responded"
            )),
            SendResult::NoAddress => Err(anyhow::anyhow!(
                "Failed to send to peer {peer_identity}: IPv6 unavailable/expired, STUN unavailable/expired"
            )),
        }
    }

    async fn try_send_via(
        &self,
        outbound_tx: &mpsc::Sender<(Vec<u8>, Vec<SocketAddr>)>,
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
        match outbound_tx.send((data.to_vec(), vec![addr])).await {
            Ok(_) => {
                tracing::debug!("Sent frame to peer {peer_identity} via {protocol}: {addr}");
                SendResult::Success
            }
            Err(e) => {
                tracing::error!("Failed to send via {protocol}: {e}");
                SendResult::NeverResponded // Treat send error as connection problem
            }
        }
    }

    fn get_status(&self) -> Vec<PeerStatus> {
        self.peers.get_status()
    }

    async fn send_probes(&self) {
        let outbound_tx = &self.tx_api.outbound_tx;

        let block = &self.block;
        let identity = &self.identity;

        // Send IPv6 probes
        send_probes(&self.peers, outbound_tx, block, identity, Protocol::Ipv6).await;

        // Send STUN hole punch probes
        send_probes(&self.peers, outbound_tx, block, identity, Protocol::Stun).await;
    }
}

async fn send_probes(
    peers: &PeerSet,
    outbound_tx: &mpsc::Sender<(Vec<u8>, Vec<SocketAddr>)>,
    block: &Arc<Box<dyn Block>>,
    identity: &str,
    protocol: Protocol,
) {
    let peer_addrs = peers.all_peer_addrs(protocol);

    // Skip if no peers have this type of address
    if peer_addrs.is_empty() {
        return;
    }

    // Create appropriate probe frame
    let probe_frame = match protocol {
        Protocol::Ipv6 => Frame::ProbeIPv6(ProbeIPv6Frame {
            identity: identity.to_string(),
        }),
        Protocol::Stun => Frame::ProbeHolePunch(ProbeHolePunchFrame {
            identity: identity.to_string(),
        }),
    };

    // Marshal once, reuse for all peers
    let probe_data = match Parser::marshal(probe_frame, block.as_ref().as_ref()) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("Failed to marshal {protocol} probe: {e}");
            return;
        }
    };

    // Send to all peers
    let peer_addrs_display = format!("{peer_addrs:?}");
    if let Err(e) = outbound_tx.send((probe_data.clone(), peer_addrs)).await {
        tracing::warn!("Failed to send {protocol} probe to {peer_addrs_display}: {e:?}");
    } else {
        tracing::info!("Sent {protocol} probe to {peer_addrs_display:?}");
    }
}

fn parse_address(identity: &str, ip: &str, port: u16) -> Option<SocketAddr> {
    if ip.is_empty() {
        return None;
    }
    let ip = match ip.parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(e) => {
            tracing::warn!("Invalid address for peer {identity}: {e}");
            return None;
        }
    };

    let addr = SocketAddr::new(ip, port);
    Some(addr)
}

fn update_address(peer: &mut PeerMeta, new_addr: SocketAddr, protocol: Protocol) {
    let old_addr = match protocol {
        Protocol::Stun => *peer.stun_addr.get(),
        Protocol::Ipv6 => *peer.remote_addr.get(),
    };

    if old_addr != Some(new_addr) {
        tracing::info!(
            "Update {protocol} address for peer {}: {} -> {new_addr}",
            peer.identity,
            old_addr
                .map(|a| a.to_string())
                .unwrap_or_else(|| "None".to_string()),
        );

        let new_addr = LastActive::dormant(Some(new_addr));
        match protocol {
            Protocol::Stun => peer.stun_addr = new_addr,
            Protocol::Ipv6 => peer.remote_addr = new_addr,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Protocol {
    Stun,
    Ipv6,
}
impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Stun => write!(f, "STUN"),
            Protocol::Ipv6 => write!(f, "IPv6"),
        }
    }
}
