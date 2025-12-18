
// ============================================================================
// Log Output Functions
// ============================================================================

use crate::client::main::Args;
use crate::codec::frame::HandshakeReplyFrame;

pub fn log_startup_banner(args: &Args) {
    println!("====================================");
    println!("  Rustun VPN Client Starting");
    println!("====================================");
    println!("Server address: {}", args.server);
    println!("Client identity: {}", args.identity);
    println!("Encryption: {}", args.crypto);
    println!("------------------------------------");
}

pub fn log_handshake_success(config: &HandshakeReplyFrame) {
    println!("Virtual IP address: {}", config.private_ip);
    println!("Network mask: {}", config.mask);
    println!("Gateway: {}", config.gateway);
    println!("IPv6: {}", config.ipv6);
    println!("Peer nodes: {}", config.others.len());
    
    if !config.others.is_empty() {
        for (idx, peer) in config.others.iter().enumerate() {
            println!("  [{}] Identity: {}", idx + 1, peer.identity);
            println!("      Private IP: {}", peer.private_ip);
            println!("      CIDR ranges: {}", peer.ciders.join(", "));
        }
    }
    println!("====================================");
    println!("Ready to forward traffic");
}
