use clap::Parser;

mod relay;
pub mod main;
mod prettylog;
pub mod stun;
mod p2p;

/// Default P2P UDP port for client-to-client direct connections
///
/// This port is used for P2P communication between VPN clients.
/// It should match the port configured in keepalive frames and handshake.
pub const P2P_UDP_PORT: u16 = 51258;

pub const P2P_HOLE_PUNCH_PORT: u16 = 51259;

const DEFAULT_MTU: u16 = 1430;

/// Rustun VPN Client
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Server address (e.g., 127.0.0.1:8080)
    #[arg(short, long)]
    pub server: String,

    /// Client identity/name
    #[arg(short, long)]
    pub identity: String,

    /// Encryption method: plain, aes256:<key>, chacha20:<key>, or xor:<key>
    #[arg(short, long, default_value = "chacha20:rustun")]
    pub crypto: String,

    /// Keep-alive interval in seconds
    #[arg(long, default_value = "10")]
    pub keepalive_interval: u64,

    /// Keep-alive threshold (reconnect after this many failures)
    #[arg(long, default_value = "5")]
    pub keepalive_threshold: u8,

    /// Enable P2P direct connection (disabled by default, uses relay only)
    #[arg(long)]
    pub enable_p2p: bool,
}