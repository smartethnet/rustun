// ============================================================================
// Log Output Functions
// ============================================================================

use crate::client::Args;
use crate::client::p2p::peer::PeerHandler;
use crate::client::relay::RelayHandler;
use crate::client::http::{StatusResponse, TrafficStats, RelayStatusInfo, P2PStatus, P2PPeerInfo, IPv6ConnectionInfo, STUNConnectionInfo, ClusterPeerInfo};
use crate::client::http::cache;
use crate::codec::frame::HandshakeReplyFrame;
use crate::utils::device::DeviceHandler;
use std::time::{SystemTime, UNIX_EPOCH};

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
                
                println!("   {} Peer: {}", prefix, status.name);
                
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
            
            println!("   {} {} {} ({})", prefix, status_icon, peer.name, online_info);
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
    
    // Update HTTP cache
    let status = build_status_response(relay, peer, dev).await;
    cache::update(status).await;
}

/// Build status response for HTTP API
pub async fn build_status_response(
    relay: &RelayHandler,
    peer: Option<&PeerHandler>,
    dev: &DeviceHandler,
) -> StatusResponse {
    // Self information from relay
    let self_info = relay.get_self_info().await;
    
    // Traffic stats
    let traffic = TrafficStats {
        receive_bytes: dev.tx_bytes as u64,
        receive_bytes_mb: dev.tx_bytes as f64 / 1024.0 / 1024.0,
        send_bytes: dev.rx_bytes as u64,
        send_bytes_mb: dev.rx_bytes as f64 / 1024.0 / 1024.0,
    };

    // Relay status
    let relay_status = relay.get_status();
    let relay = RelayStatusInfo {
        rx_frames: relay_status.rx_frame,
        rx_errors: relay_status.rx_error,
        tx_frames: relay_status.tx_frame,
        tx_errors: relay_status.tx_error,
    };

    // P2P status
    let p2p = if let Some(peer_handler) = peer {
        let peer_statuses = peer_handler.get_status().await;
        let mut peers = Vec::new();
        
        for status in peer_statuses {
            let ipv6 = status.ipv6_addr.map(|addr| {
                let last_active_seconds = status.ipv6_last_active.map(|instant| {
                    instant.elapsed().as_secs()
                });
                IPv6ConnectionInfo {
                    address: addr.to_string(),
                    connected: last_active_seconds.is_some() && last_active_seconds.unwrap() < 30,
                    last_active_seconds_ago: last_active_seconds,
                }
            });

            let stun = status.stun_addr.map(|addr| {
                let last_active_seconds = status.stun_last_active.map(|instant| {
                    instant.elapsed().as_secs()
                });
                STUNConnectionInfo {
                    address: addr.to_string(),
                    connected: last_active_seconds.is_some(),
                    last_active_seconds_ago: last_active_seconds,
                }
            });

            peers.push(P2PPeerInfo {
                name: status.name.clone(),
                identity: status.identity.clone(),
                ipv6,
                stun,
            });
        }

        P2PStatus {
            enabled: true,
            peers,
        }
    } else {
        P2PStatus {
            enabled: false,
            peers: Vec::new(),
        }
    };

    // Cluster peers
    let others = dev.get_peer_details();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let cluster_peers = others
        .into_iter()
        .map(|peer| {
            let status = if peer.last_active == 0 {
                "offline".to_string()
            } else {
                let elapsed = now.saturating_sub(peer.last_active);
                if elapsed < 30 {
                    "online".to_string()
                } else if elapsed < 120 {
                    "warning".to_string()
                } else {
                    "inactive".to_string()
                }
            };

            ClusterPeerInfo {
                name: peer.name.clone(),
                identity: peer.identity,
                private_ip: peer.private_ip,
                ciders: peer.ciders,
                ipv6: if peer.ipv6.is_empty() {
                    None
                } else {
                    Some(peer.ipv6)
                },
                ipv6_port: if peer.port > 0 { Some(peer.port) } else { None },
                stun_ip: if peer.stun_ip.is_empty() {
                    None
                } else {
                    Some(peer.stun_ip)
                },
                stun_port: if peer.stun_port > 0 {
                    Some(peer.stun_port)
                } else {
                    None
                },
                last_active: peer.last_active,
                status,
            }
        })
        .collect();

    StatusResponse {
        self_info,
        traffic,
        relay,
        p2p,
        cluster_peers,
    }
}
