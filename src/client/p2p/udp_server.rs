use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

/// UDP packet buffer size
/// 
/// 2048 bytes is sufficient for:
/// - Typical VPN frames (MTU 1500 + headers)
/// - Control frames (handshake, keepalive, etc.)
const BUFFER_SIZE: usize = 2048;

/// Dual-stack UDP server for P2P communication
///
/// This server manages two UDP sockets simultaneously:
/// 1. IPv6 socket: For direct P2P connections using global IPv6 addresses
/// 2. IPv4 socket: For STUN-based NAT hole punching using discovered public IPv4 addresses
///
/// # Design
///
/// The server acts as a bidirectional packet forwarder:
/// - Inbound: Network -> Channel -> PeerHandler (for packet processing)
/// - Outbound: PeerHandler -> Channel -> Network (for packet transmission)
///
/// This separation of concerns allows:
/// - PeerHandler to focus on protocol logic without managing sockets
/// - Socket I/O to run in a dedicated task without blocking the main logic
/// - Automatic socket selection based on destination address type (IPv4 vs IPv6)
///
/// # Threading
///
/// This server runs in its own tokio task, communicating with PeerHandler via channels.
/// The `tokio::select!` macro is used to concurrently handle:
/// - Outbound packets from PeerHandler
/// - Inbound packets from IPv6 socket
/// - Inbound packets from IPv4 socket
pub struct UDPServer {
    /// IPv6 UDP port for P2P direct connections
    ///
    /// Used when both peers have global IPv6 addresses.
    /// This provides the lowest latency path.
    listen_port: u16,

    /// IPv4 UDP port for STUN hole punching
    ///
    /// Used when peers are behind NATs and need hole punching.
    /// This port is discovered via STUN and shared with other peers.
    stun_port: u16,

    /// Channel sender to forward received packets to PeerHandler
    ///
    /// All inbound packets (from both IPv4 and IPv6 sockets) are sent through
    /// this channel to PeerHandler for decryption and protocol processing.
    input_tx: mpsc::Sender<(Vec<u8>, SocketAddr)>,

    /// Channel receiver to get outbound packets from PeerHandler
    ///
    /// PeerHandler sends encrypted packets through this channel.
    /// The server selects the appropriate socket based on destination address type.
    output_rx: mpsc::Receiver<(Vec<u8>, SocketAddr)>,
}

impl UDPServer {
    /// Create a new UDP server for dual-stack P2P communication
    ///
    /// # Arguments
    /// * `listen_port` - IPv6 UDP port to bind (typically 51258)
    /// * `stun_port` - IPv4 UDP port for STUN hole punching (typically 51259)
    /// * `input_tx` - Channel to send received packets to PeerHandler
    /// * `output_rx` - Channel to receive outbound packets from PeerHandler
    ///
    /// # Example
    /// ```ignore
    /// let (inbound_tx, inbound_rx) = mpsc::channel(100);
    /// let (outbound_tx, outbound_rx) = mpsc::channel(100);
    /// let server = UDPServer::new(51258, 51259, inbound_tx, outbound_rx);
    /// tokio::spawn(async move { server.serve().await });
    /// ```
    pub(crate) fn new(
        listen_port: u16,
        stun_port: u16,
        input_tx: mpsc::Sender<(Vec<u8>, SocketAddr)>,
        output_rx: mpsc::Receiver<(Vec<u8>, SocketAddr)>,
    ) -> Self {
        UDPServer {
            listen_port,
            stun_port,
            input_tx,
            output_rx,
        }
    }

    /// Start the UDP server loop
    ///
    /// This method binds both IPv4 and IPv6 sockets and enters an infinite loop
    /// to handle bidirectional packet forwarding.
    ///
    /// # Behavior
    ///
    /// 1. Binds IPv6 socket on `[::]:<listen_port>` (all IPv6 interfaces)
    /// 2. Binds IPv4 socket on `0.0.0.0:<stun_port>` (all IPv4 interfaces)
    /// 3. Concurrently handles:
    ///    - Outbound packets: Routes to IPv4 or IPv6 socket based on destination
    ///    - IPv6 inbound packets: Forwards to PeerHandler via channel
    ///    - IPv4 inbound packets: Forwards to PeerHandler via channel
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Socket binding fails (port already in use, permission denied, etc.)
    /// - Socket receive operation fails (network error, etc.)
    ///
    /// # Note
    ///
    /// This method never returns under normal operation. It only exits on error.
    pub async fn serve(&mut self) -> crate::Result<()> {
        // Bind IPv6 socket for direct connections
        // [::] means all IPv6 interfaces (equivalent to 0.0.0.0 for IPv4)
        let socket_ipv6 = UdpSocket::bind(format!("[::]:{}", self.listen_port)).await?;
        tracing::info!("P2P IPv6 UDP listening on {}", socket_ipv6.local_addr()?);

        // Bind IPv4 socket for STUN hole punching
        // This socket uses the port discovered by STUN client
        let socket_ipv4 = UdpSocket::bind(format!("0.0.0.0:{}", self.stun_port)).await?;
        tracing::info!("P2P IPv4 UDP (STUN) listening on {}", socket_ipv4.local_addr()?);

        // Separate buffers for each socket to avoid data races
        let mut buf_ipv6 = vec![0u8; BUFFER_SIZE];
        let mut buf_ipv4 = vec![0u8; BUFFER_SIZE];

        loop {
            tokio::select! {
                // Handle outbound packets: PeerHandler -> Network
                // PeerHandler decides the destination, we just route to the right socket
                Some((data, remote)) = self.output_rx.recv() => {
                    self.handle_outbound(&socket_ipv6, &socket_ipv4, &data, remote).await;
                }

                // Handle IPv6 inbound packets: Network -> PeerHandler
                // Direct P2P connections or responses to our keepalives
                result = socket_ipv6.recv_from(&mut buf_ipv6) => {
                    if let Err(e) = self.handle_inbound(result, &mut buf_ipv6, "IPv6").await {
                        return Err(e);
                    }
                }

                // Handle IPv4 inbound packets: Network -> PeerHandler
                // STUN-hole-punched connections or responses
                result = socket_ipv4.recv_from(&mut buf_ipv4) => {
                    if let Err(e) = self.handle_inbound(result, &mut buf_ipv4, "IPv4").await {
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Handle outbound packet by selecting appropriate socket based on destination address type
    ///
    /// # Strategy
    ///
    /// - IPv4 destination -> Use IPv4 socket (STUN port)
    /// - IPv6 destination -> Use IPv6 socket (direct connection port)
    ///
    /// # Arguments
    ///
    /// * `socket_ipv6` - IPv6 UDP socket reference
    /// * `socket_ipv4` - IPv4 UDP socket reference
    /// * `data` - Encrypted packet payload to send
    /// * `remote` - Destination address (can be IPv4 or IPv6)
    ///
    /// # Error Handling
    ///
    /// Errors are logged but don't cause the server to stop.
    /// This is intentional because:
    /// - Network failures might be transient
    /// - One failed send shouldn't affect other connections
    /// - PeerHandler will detect connection failure via keepalive timeout
    async fn handle_outbound(
        &self,
        socket_ipv6: &UdpSocket,
        socket_ipv4: &UdpSocket,
        data: &[u8],
        remote: SocketAddr,
    ) {
        // Select socket based on destination address family
        let (socket, protocol) = if remote.is_ipv4() {
            (socket_ipv4, "IPv4")
        } else {
            (socket_ipv6, "IPv6")
        };

        if let Err(e) = socket.send_to(data, remote).await {
            tracing::error!("Failed to send {} packet to {}: {:?}", protocol, remote, e);
        }
    }

    /// Handle inbound packet by forwarding it to PeerHandler
    ///
    /// # Processing Flow
    ///
    /// 1. Extract packet data from buffer (only the received bytes)
    /// 2. Forward packet + source address to PeerHandler via channel
    /// 3. Reset buffer for next packet
    ///
    /// # Arguments
    ///
    /// * `result` - Result from `socket.recv_from()` call
    /// * `buffer` - Buffer that received the packet data
    /// * `protocol` - Protocol name ("IPv4" or "IPv6") for logging
    ///
    /// # Return Value
    ///
    /// - `Ok(())` - Packet successfully forwarded to PeerHandler
    /// - `Err(_)` - Socket error occurred, server should stop
    ///
    /// # Note on Buffer Reset
    ///
    /// We use `buffer.fill(0)` instead of reallocating because:
    /// - More efficient (no memory allocation)
    /// - Buffer is reused in the next loop iteration
    /// - Only the `len` bytes are used, so zeroing is not strictly necessary,
    ///   but helps prevent potential bugs from stale data
    async fn handle_inbound(
        &self,
        result: std::io::Result<(usize, SocketAddr)>,
        buffer: &mut Vec<u8>,
        protocol: &str,
    ) -> crate::Result<()> {
        match result {
            Ok((len, remote)) => {
                // Copy only the received bytes (not the entire buffer)
                let packet = buffer[..len].to_vec();

                // Forward to PeerHandler for decryption and protocol processing
                if let Err(e) = self.input_tx.send((packet, remote)).await {
                    tracing::error!("Failed to forward {} packet from {}: {:?}", protocol, remote, e);
                }

                // Reset buffer for next packet (optional but good practice)
                buffer.fill(0);
                Ok(())
            }
            Err(e) => {
                // Socket errors are fatal - we can't recover from a broken socket
                tracing::error!("UDP {} recv_from error: {}", protocol, e);
                Err(e.into())
            }
        }
    }
}
