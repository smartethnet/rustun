// ============================================================================
// Log Output Functions
// ============================================================================

use crate::client::Args;
use crate::client::p2p::peer::PeerHandler;
use crate::client::relay::RelayHandler;
use crate::codec::frame::HandshakeReplyFrame;
use crate::utils::device::DeviceHandler;

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
    println!("Peer nodes: {}", config.peer_details.len());
    if !config.peer_details.is_empty() {
        for (idx, peer) in config.peer_details.iter().enumerate() {
            println!("  [{}] Identity: {}", idx + 1, peer.identity);
            println!("      Private IP: {}", peer.private_ip);
            println!("      IPv6: {}", peer.ipv6);
            println!("      CIDR ranges: {}", peer.ciders.join(", "));
        }
    }
    println!("====================================");
    println!("Ready to forward traffic");
}


pub async fn get_status(relay: &RelayHandler, peer: Option<&PeerHandler>, dev: &DeviceHandler) {
    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                        CONNECTION STATUS                             ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");

    // traffic status
    // device receive is the traffic outbound
    println!("Receive Bytes: {}MB", dev.tx_bytes/1024/1024);
    println!("Send Bytes: {}MB", dev.rx_bytes/1024/1024);

    // Relay Status
    let relay_status = relay.get_status();
    println!("\n📡 Relay Connection (TCP)");
    println!("   ├─ RX Frames:  {} (Errors: {})", relay_status.rx_frame, relay_status.rx_error);
    println!("   └─ TX Frames:  {} (Errors: {})", relay_status.tx_frame, relay_status.tx_error);
    
    // P2P Status
    if let Some(peer_handler) = peer {
        let peer_status = peer_handler.get_status().await;
        
        if peer_status.is_empty() {
            println!("\n🔗 P2P Connections (UDP)");
            println!("   └─ No peers configured");
        } else {
            println!("\n🔗 P2P Connections (UDP): {} peers", peer_status.len());
            
            for (idx, status) in peer_status.iter().enumerate() {
                let is_last = idx == peer_status.len() - 1;
                let prefix = if is_last { "└─" } else { "├─" };
                let continuation = if is_last { " " } else { "│" };
                
                println!("   {} Peer: {}", prefix, status.identity);
                
                // IPv6 Direct Connection
                let ipv6_state = match (&status.ipv6_addr, &status.ipv6_last_active) {
                    (None, _) => "❌ No Address".to_string(),
                    (Some(addr), None) => format!("⏳ Connecting... ({})", addr),
                    (Some(addr), Some(last)) => {
                        let elapsed = last.elapsed().as_secs();
                        if elapsed < 15 {
                            format!("✅ Active ({}s ago, {})", elapsed, addr)
                        } else {
                            format!("⚠️  Inactive ({}s ago, {})", elapsed, addr)
                        }
                    }
                };
                println!("   {}    ├─ IPv6:  {}", continuation, ipv6_state);
                
                // STUN Hole-Punched Connection
                let stun_state = match (&status.stun_addr, &status.stun_last_active) {
                    (None, _) => "❌ No Address".to_string(),
                    (Some(addr), None) => format!("⏳ Connecting... ({})", addr),
                    (Some(addr), Some(last)) => {
                        let elapsed = last.elapsed().as_secs();
                        if elapsed < 15 {
                            format!("✅ Active ({}s ago, {})", elapsed, addr)
                        } else {
                            format!("⚠️  Inactive ({}s ago, {})", elapsed, addr)
                        }
                    }
                };
                println!("   {}    └─ STUN:  {}", continuation, stun_state);
            }
        }
    } else {
        println!("\n🔗 P2P Mode: Disabled");
    }
    
    // Cluster Peers Status (from device handler)
    let others = dev.get_peer_details();
    if !others.is_empty() {
        println!("\n👥 Cluster Peers: {} total", others.len());
        
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        for (idx, peer) in others.iter().enumerate() {
            let is_last = idx == others.len() - 1;
            let prefix = if is_last { "└─" } else { "├─" };
            let continuation = if is_last { " " } else { "│" };
            
            // Online/Offline status
            let status_icon = if peer.last_active == 0 {
                "⚪"  // Offline
            } else {
                let elapsed = now.saturating_sub(peer.last_active);
                if elapsed < 30 {
                    "🟢"  // Online
                } else if elapsed < 120 {
                    "🟡"  // Warning
                } else {
                    "🔴"  // Inactive
                }
            };
            
            let online_info = if peer.last_active == 0 {
                "Offline".to_string()
            } else {
                let elapsed = now.saturating_sub(peer.last_active);
                format!("{}s ago", elapsed)
            };
            
            println!("   {} {} {} ({})", prefix, status_icon, peer.identity, online_info);
            println!("   {}    ├─ Private IP: {}", continuation, peer.private_ip);
            
            if !peer.ciders.is_empty() {
                println!("   {}    ├─ Routes: {}", continuation, peer.ciders.join(", "));
            }
            
            if !peer.ipv6.is_empty() {
                println!("   {}    ├─ IPv6: {}:{}", continuation, peer.ipv6, peer.port);
            }
            
            if !peer.stun_ip.is_empty() {
                println!("   {}    └─ STUN: {}:{}", continuation, peer.stun_ip, peer.stun_port);
            } else {
                // Adjust last item if no stun_ip
                println!("   {}    └─ STUN: Not configured", continuation);
            }
        }
    }
    
    println!();
}
