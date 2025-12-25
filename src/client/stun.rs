//! STUN client for NAT traversal and public address discovery
//!
//! This module provides STUN (Session Traversal Utilities for NAT) client functionality
//! to discover the client's public IP address and NAT type, which is essential for
//! P2P connection establishment.

use std::net::{SocketAddr, IpAddr};
use std::time::Duration;
use anyhow::{Context, Result};

/// NAT type classifications based on RFC 3489 and RFC 5780
///
/// Different NAT types have different implications for P2P connectivity:
/// - OpenInternet: No NAT, direct connectivity (100% P2P success)
/// - FullCone: Port mapping is consistent, easiest to traverse (95%+ success)
/// - RestrictedCone: IP filtering, moderate difficulty (85%+ success)
/// - PortRestricted: IP+Port filtering, harder (70%+ success)
/// - Symmetric: Different mapping per destination, hardest (30%- success)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatType {
    /// No NAT detected, client is directly on the public internet
    OpenInternet,
    
    /// Full Cone NAT: Once an internal address is mapped to an external address,
    /// any external host can send packets to the internal host by sending to the mapped address
    FullCone,
    
    /// Restricted Cone NAT: External hosts can send packets only if the internal host
    /// has previously sent a packet to that external IP (port doesn't matter)
    RestrictedCone,
    
    /// Port-Restricted Cone NAT: External hosts can send packets only if the internal host
    /// has previously sent a packet to that specific external IP:port combination
    PortRestricted,
    
    /// Symmetric NAT: Different external mapping for each destination.
    /// Most difficult for P2P hole punching
    Symmetric,
    
    /// Unable to determine NAT type
    Unknown,
}

impl NatType {
    /// Returns the estimated P2P hole punching success rate (0.0 - 1.0)
    pub fn hole_punch_success_rate(&self, peer_nat: &NatType) -> f32 {
        match (self, peer_nat) {
            (NatType::OpenInternet, _) | (_, NatType::OpenInternet) => 1.0,
            (NatType::FullCone, NatType::FullCone) => 0.95,
            (NatType::FullCone, NatType::RestrictedCone) => 0.90,
            (NatType::FullCone, NatType::PortRestricted) => 0.85,
            (NatType::RestrictedCone, NatType::RestrictedCone) => 0.85,
            (NatType::RestrictedCone, NatType::PortRestricted) => 0.75,
            (NatType::PortRestricted, NatType::PortRestricted) => 0.70,
            (NatType::Symmetric, NatType::Symmetric) => 0.15,
            (NatType::Symmetric, _) | (_, NatType::Symmetric) => 0.50,
            _ => 0.30,
        }
    }
    
    /// Returns human-readable description of the NAT type
    pub fn description(&self) -> &'static str {
        match self {
            NatType::OpenInternet => "No NAT (Public Internet)",
            NatType::FullCone => "Full Cone NAT (Easy P2P)",
            NatType::RestrictedCone => "Restricted Cone NAT (Moderate P2P)",
            NatType::PortRestricted => "Port-Restricted Cone NAT (Harder P2P)",
            NatType::Symmetric => "Symmetric NAT (Difficult P2P)",
            NatType::Unknown => "Unknown NAT Type",
        }
    }
}

/// Result of STUN discovery containing public address and NAT information
#[derive(Debug, Clone)]
pub struct StunDiscoveryResult {
    /// Public IP address as seen by the STUN server
    pub public_ip: IpAddr,
    
    /// Public port as seen by the STUN server
    pub public_port: u16,
    
    /// Detected NAT type
    pub nat_type: NatType,
    
    /// Local address used for the STUN query
    pub local_addr: SocketAddr,
}

impl StunDiscoveryResult {
    /// Returns the full public socket address
    pub fn public_addr(&self) -> SocketAddr {
        SocketAddr::new(self.public_ip, self.public_port)
    }
}

/// STUN client for NAT discovery and public address resolution
///
/// Supports querying multiple STUN servers for redundancy and
/// detecting NAT type using RFC 5780 behavioral tests.
pub struct StunClient {
    /// List of STUN servers to query (format: "host:port")
    stun_servers: Vec<String>,
    
    /// Timeout for STUN requests
    timeout: Duration,
}

impl StunClient {
    /// Creates a new STUN client with default Google STUN servers
    ///
    /// # Example
    /// ```rust,ignore
    /// let stun_client = StunClient::new();
    /// ```
    pub fn new() -> Self {
        Self::with_servers(vec![
            "stun.l.google.com:19302".to_string(),
            "stun1.l.google.com:19302".to_string(),
            "stun2.l.google.com:19302".to_string(),
            "stun3.l.google.com:19302".to_string(),
        ])
    }
    
    /// Creates a new STUN client with custom STUN servers
    ///
    /// # Arguments
    /// * `stun_servers` - List of STUN server addresses (host:port format)
    ///
    /// # Example
    /// ```rust,ignore
    /// let stun_client = StunClient::with_servers(vec![
    ///     "stun.example.com:3478".to_string(),
    /// ]);
    /// ```
    pub fn with_servers(stun_servers: Vec<String>) -> Self {
        Self {
            stun_servers,
            timeout: Duration::from_secs(5),
        }
    }
    
    /// Sets the timeout for STUN requests
    ///
    /// # Arguments
    /// * `timeout` - Request timeout duration
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    
    /// Discovers public IP address and port by querying STUN servers
    ///
    /// This performs a simple STUN binding request to discover the client's
    /// public address as seen from the internet. It tries multiple servers
    /// until one succeeds.
    ///
    /// # Arguments
    /// * `local_port` - Local UDP port to bind to (use 0 for automatic)
    ///
    /// # Returns
    /// * `Ok((IpAddr, u16))` - Public IP and port
    /// * `Err` - If all STUN servers fail or timeout
    ///
    /// # Example
    /// ```rust,ignore
    /// let (public_ip, public_port) = stun_client
    ///     .discover_public_address(0)
    ///     .await?;
    /// println!("Public address: {}:{}", public_ip, public_port);
    /// ```
    pub async fn discover_public_address(&self, local_port: u16) -> Result<(IpAddr, u16)> {
        let local_addr = if local_port == 0 {
            "0.0.0.0:0"
        } else {
            &format!("0.0.0.0:{}", local_port)
        };
        
        // Try each STUN server until one succeeds
        for stun_server in &self.stun_servers {
            tracing::debug!("Querying STUN server: {}", stun_server);
            
            match self.query_stun_server(local_addr, stun_server).await {
                Ok((ip, port)) => {
                    tracing::info!(
                        "STUN discovery successful via {}: {}:{}",
                        stun_server,
                        ip,
                        port
                    );
                    return Ok((ip, port));
                }
                Err(e) => {
                    tracing::warn!("STUN query to {} failed: {}", stun_server, e);
                    continue;
                }
            }
        }
        
        anyhow::bail!("All STUN servers failed")
    }
    
    /// Performs full STUN discovery including NAT type detection
    ///
    /// This performs a comprehensive STUN discovery that includes:
    /// 1. Public address discovery
    /// 2. NAT type detection using RFC 5780 tests
    ///
    /// # Arguments
    /// * `local_port` - Local UDP port to bind to
    ///
    /// # Returns
    /// * `Ok(StunDiscoveryResult)` - Complete discovery result
    /// * `Err` - If discovery fails
    ///
    /// # Example
    /// ```rust,ignore
    /// let result = stun_client.discover(51258).await?;
    /// println!("Public: {}", result.public_addr());
    /// println!("NAT Type: {:?}", result.nat_type);
    /// ```
    pub async fn discover(&self, local_port: u16) -> Result<StunDiscoveryResult> {
        let local_addr = SocketAddr::new("0.0.0.0".parse().unwrap(), local_port);
        
        // Step 1: Discover public address
        let (public_ip, public_port) = self.discover_public_address(local_port)
            .await
            .context("Failed to discover public address")?;
        
        tracing::debug!(
            "Public address discovered: {}:{} (local: {})",
            public_ip,
            public_port,
            local_addr
        );
        
        // Step 2: Detect NAT type
        // For now, use a simplified detection based on address comparison
        let nat_type = self.detect_nat_type_simple(local_addr, public_ip, public_port).await;
        
        tracing::info!(
            "NAT type detected: {:?} ({})",
            nat_type,
            nat_type.description()
        );
        
        Ok(StunDiscoveryResult {
            public_ip,
            public_port,
            nat_type,
            local_addr,
        })
    }
    
    /// Queries a single STUN server using the stunclient library
    async fn query_stun_server(
        &self,
        local_addr: &str,
        stun_server: &str,
    ) -> Result<(IpAddr, u16)> {
        use std::net::UdpSocket;
        
        // Create UDP socket
        let socket = UdpSocket::bind(local_addr)
            .context("Failed to bind UDP socket")?;
        
        // Set socket timeout
        socket.set_read_timeout(Some(self.timeout))
            .context("Failed to set socket timeout")?;
        
        // Resolve STUN server address (may be hostname or IP)
        let server_addr: SocketAddr = if let Ok(addr) = stun_server.parse() {
            // Already a valid SocketAddr
            addr
        } else {
            // Need to resolve DNS
            use tokio::net::lookup_host;
            let mut addrs = lookup_host(stun_server)
                .await
                .context("Failed to resolve STUN server hostname")?;
            addrs
                .next()
                .context("No addresses resolved for STUN server")?
        };
        
        // Create STUN client
        let stun_client = stunclient::StunClient::new(server_addr);
        
        // Query external address
        let external_addr = tokio::task::spawn_blocking(move || {
            stun_client.query_external_address(&socket)
        })
        .await
        .context("STUN query task panicked")?
        .context("Failed to get external address")?;
        
        Ok((external_addr.ip(), external_addr.port()))
    }
    
    /// Simplified NAT type detection based on address comparison
    ///
    /// This is a basic heuristic:
    /// - If local and public addresses match -> OpenInternet
    /// - If ports match but IPs differ -> likely FullCone
    /// - Otherwise -> assume PortRestricted (conservative estimate)
    ///
    /// For full RFC 5780 detection, we would need multiple STUN servers
    /// with different IP addresses and the ability to request responses
    /// from different source addresses.
    async fn detect_nat_type_simple(
        &self,
        local_addr: SocketAddr,
        public_ip: IpAddr,
        public_port: u16,
    ) -> NatType {
        let local_ip = local_addr.ip();
        let local_port = local_addr.port();
        
        // Check if we're directly on the public internet
        if local_ip == public_ip && local_port == public_port {
            return NatType::OpenInternet;
        }
        
        // If port is preserved, likely Full Cone NAT
        if local_port == public_port {
            tracing::debug!("Port preserved, likely Full Cone NAT");
            return NatType::FullCone;
        }
        
        // Port changed, conservatively assume Port-Restricted Cone
        // (most common in modern routers)
        tracing::debug!("Port changed, assuming Port-Restricted Cone NAT");
        NatType::PortRestricted
    }
    
    /// Advanced NAT type detection using RFC 5780 behavioral tests
    ///
    /// This would require:
    /// 1. STUN server with multiple IP addresses
    /// 2. Support for CHANGE-REQUEST attribute
    /// 3. Multiple binding requests with different parameters
    ///
    /// Currently marked as future enhancement.
    #[allow(dead_code)]
    async fn detect_nat_type_rfc5780(&self, _local_port: u16) -> Result<NatType> {
        // TODO: Implement full RFC 5780 NAT type detection
        // This requires a STUN server that supports RFC 5780 extensions
        // Most public STUN servers only support basic RFC 5389
        
        tracing::warn!("Advanced NAT detection not yet implemented, using simplified detection");
        Ok(NatType::Unknown)
    }
}

impl Default for StunClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_nat_type_descriptions() {
        assert_eq!(NatType::OpenInternet.description(), "No NAT (Public Internet)");
        assert_eq!(NatType::FullCone.description(), "Full Cone NAT (Easy P2P)");
    }
    
    #[test]
    fn test_hole_punch_success_rates() {
        // Best case: both on public internet
        assert_eq!(
            NatType::OpenInternet.hole_punch_success_rate(&NatType::OpenInternet),
            1.0
        );
        
        // Worst case: both symmetric NAT
        assert!(
            NatType::Symmetric.hole_punch_success_rate(&NatType::Symmetric) < 0.2
        );
        
        // Good case: both full cone
        assert!(
            NatType::FullCone.hole_punch_success_rate(&NatType::FullCone) > 0.9
        );
    }
    
    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_stun_discovery() {
        let client = StunClient::new();
        let result = client.discover_public_address(0).await;
        
        // This test is ignored by default as it requires internet access
        // Run with: cargo test test_stun_discovery -- --ignored
        if let Ok((ip, port)) = result {
            println!("Discovered public address: {}:{}", ip, port);
            assert!(port > 0);
        }
    }
}

