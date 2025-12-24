//! Frame definitions for the VPN protocol
//!
//! This module defines the frame structure and types used in the VPN protocol.
//! All frames follow a common header format and may contain encrypted payloads.
//!
//! # Frame Header Format (8 bytes)
//! ```text
//! +--------+--------+--------+--------+--------+--------+--------+--------+
//! |      Magic (4 bytes)      |Version|  Type  |   Payload Length (2B)   |
//! +--------+--------+--------+--------+--------+--------+--------+--------+
//! ```
//!
//! - Magic: 0x91929394 (4 bytes) - Protocol identifier
//! - Version: 0x01 (1 byte) - Protocol version
//! - Type: Frame type identifier (1 byte)
//! - Payload Length: Length of the payload in bytes (2 bytes, big-endian)

pub(crate) use crate::codec::errors::FrameError;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Frame type identifiers
///
/// Each frame type serves a specific purpose in the VPN protocol lifecycle:
/// - Handshake: Initial client authentication and registration
/// - HandshakeReply: Server response with network configuration and peer routes
/// - KeepAlive: Connection health check
/// - Data: Encrypted IP packet tunnel data
pub(crate) enum FrameType {
    /// Client handshake request (Type 1)
    Handshake = 1,
    /// Connection keep-alive ping (Type 2)
    KeepAlive = 2,
    /// Tunneled data packet (Type 3)
    Data = 3,
    /// Server handshake response (Type 4)
    HandshakeReply = 4,
}

impl TryFrom<u8> for FrameType {
    type Error = FrameError;

    /// Converts a byte value to a FrameType
    ///
    /// # Arguments
    /// * `v` - Byte value to convert (1-4)
    ///
    /// # Returns
    /// * `Ok(FrameType)` if the value is valid
    /// * `Err(FrameError::Invalid)` if the value is unknown
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x01 => Ok(FrameType::Handshake),
            0x02 => Ok(FrameType::KeepAlive),
            0x03 => Ok(FrameType::Data),
            0x04 => Ok(FrameType::HandshakeReply),
            _ => Err(FrameError::Invalid),
        }
    }
}

/// Frame header length in bytes
///
/// Header format: Magic(4) + Version(1) + Type(1) + PayloadLen(2) = 8 bytes
pub(crate) const HDR_LEN: usize = 8;

/// Protocol frame enum
///
/// Represents all possible frame types in the VPN protocol. Each variant contains
/// the frame-specific data structure. Frames are serialized/deserialized using
/// the parser module and encrypted according to the configured cipher.
#[derive(Debug)]
pub enum Frame {
    /// Client handshake request containing identity
    Handshake(HandshakeFrame),
    /// Server handshake response with network config and peer routes
    HandshakeReply(HandshakeReplyFrame),
    /// Connection keep-alive heartbeat
    KeepAlive(KeepAliveFrame),
    /// Tunneled IP packet data
    Data(DataFrame),
}

impl Display for Frame {
    /// Formats the frame for logging and debugging
    ///
    /// Provides human-readable representation of each frame type with
    /// relevant summary information (identity, peer count, payload size, etc.)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Frame::Handshake(frame) => write!(f, "handshake with {}", frame.identity),
            Frame::HandshakeReply(frame) => {
                write!(f, "handshake reply with {} others", frame.others.len())
            }
            Frame::KeepAlive(_frame) => write!(f, "keepalive"),
            Frame::Data(frame) => write!(f, "data with payload size {}", frame.payload.len()),
        }
    }
}

/// Handshake frame sent by client during connection establishment
///
/// The client sends this frame as the first message after establishing a TCP/UDP
/// connection to the server. The identity is used for authentication and routing
/// configuration lookup.
///
/// # Flow
/// 1. Client connects to server
/// 2. Client sends Handshake with identity
/// 3. Server validates identity and sends HandshakeReply
/// 4. Connection established, data transfer begins
#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeFrame {
    /// Client identity (unique identifier)
    ///
    /// Used by the server to:
    /// - Authenticate the client
    /// - Look up network configuration (private IP, CIDR ranges)
    /// - Determine cluster membership for multi-tenancy
    pub identity: String,

    /// Public IPv6
    pub ipv6: String,
    pub port: u16,
}

/// Handshake reply frame sent by server in response to client handshake
///
/// Contains the network configuration for the client and information about
/// other peers in the same cluster. This enables the client to set up routes
/// and communicate with other VPN nodes.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HandshakeReplyFrame {
    /// Private IP address assigned to this client
    ///
    /// This is the client's virtual IP within the VPN network
    pub private_ip: String,

    /// Subnet mask for the VPN network
    ///
    /// Example: "255.255.255.0"
    pub mask: String,

    /// Gateway IP address for the VPN network
    ///
    /// Used for routing traffic within the VPN
    pub gateway: String,

    /// List of other peers in the same cluster
    ///
    /// Each RouteItem contains routing information for a peer node,
    /// allowing this client to establish routes to other VPN members
    pub others: Vec<RouteItem>,

    /// Public IPv6
    ///
    /// Others will try to connect IPv6 directly
    pub ipv6: String,
}

/// Routing information for a peer node
///
/// Describes a single peer in the VPN cluster, including its identity,
/// virtual IP address, and the CIDR ranges it can route to.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RouteItem {
    /// Unique identifier of the peer
    pub identity: String,

    /// Private IP address of the peer within the VPN
    pub private_ip: String,

    /// CIDR ranges accessible through this peer
    ///
    /// Example: ["192.168.1.0/24", "10.0.0.0/8"]
    /// Traffic destined for these ranges will be routed through this peer
    pub ciders: Vec<String>,

    /// IPv6 is public ip address of ther peer
    pub ipv6: String,

    pub port: u16,
}

/// Keep-alive frame for connection health monitoring
///
/// Sent periodically by both client and server to detect connection failures.
/// If no frames (including keep-alives) are received within the threshold period,
/// the connection is considered dead and will be closed.
///
/// # Purpose
/// - Detect network failures or peer crashes
/// - Prevent idle connection timeouts by firewalls/NAT devices
/// - Maintain connection state information
///
/// This frame has no payload data.
#[derive(Debug, Deserialize)]
pub struct KeepAliveFrame {}

/// Data frame containing tunneled IP packets
///
/// Encapsulates raw IP packets that are being tunneled through the VPN.
/// The payload is encrypted before transmission and decrypted upon receipt.
/// The frame provides helper methods to extract IP header information.
///
/// # Payload Format
/// Contains a complete IP packet (IPv4 or IPv6) including headers and data.
/// Minimum valid IPv4 packet size is 20 bytes (header only).
#[derive(Debug, Deserialize)]
pub struct DataFrame {
    /// Raw IP packet data (encrypted in transit)
    ///
    /// This contains the entire IP packet including:
    /// - IP header (20+ bytes for IPv4, 40+ bytes for IPv6)
    /// - Transport layer header (TCP/UDP/etc.)
    /// - Application data
    pub payload: Vec<u8>,
}

impl DataFrame {
    /// Checks if the IP packet is invalid (too short)
    ///
    /// A valid IPv4 packet must be at least 20 bytes (minimum header size).
    ///
    /// # Returns
    /// * `true` if payload is too short to be a valid IP packet
    /// * `false` if payload size is sufficient
    pub fn invalid(&self) -> bool {
        self.payload.len() < 20
    }

    /// Extracts the IP version from the packet header
    ///
    /// Reads the first 4 bits of the IP header which indicate the version.
    ///
    /// # Returns
    /// * `4` for IPv4
    /// * `6` for IPv6
    /// * Other values indicate malformed packets
    pub fn version(&self) -> i32 {
        (self.payload[0] >> 4) as i32
    }

    /// Extracts the destination IP address from the packet
    ///
    /// Reads bytes 16-19 of the IPv4 header (destination address field).
    ///
    /// # Returns
    /// Destination IP address as a string (e.g., "192.168.1.1")
    ///
    /// # Note
    /// This assumes IPv4 format. For IPv6, the destination address is at
    /// a different offset and is 16 bytes long.
    pub fn dst(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.payload[16], self.payload[17], self.payload[18], self.payload[19]
        )
    }

    /// Extracts the source IP address from the packet
    ///
    /// Reads bytes 12-15 of the IPv4 header (source address field).
    ///
    /// # Returns
    /// Source IP address as a string (e.g., "10.0.0.2")
    ///
    /// # Note
    /// This assumes IPv4 format. For IPv6, the source address is at
    /// a different offset and is 16 bytes long.
    pub fn src(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.payload[12], self.payload[13], self.payload[14], self.payload[15]
        )
    }
}
