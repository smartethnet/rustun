//! STUN discovery example
//!
//! This example demonstrates how to use the STUN client to discover
//! your public IP address and NAT type.
//!
//! Usage:
//! ```bash
//! cargo run --example stun_discover
//! cargo run --example stun_discover -- --port 51258
//! ```

use rustun::client::stun::{StunClient, NatType};
use std::time::Duration;

#[derive(clap::Parser, Debug)]
#[command(name = "stun_discover")]
#[command(about = "Discover public IP and NAT type using STUN", long_about = None)]
struct Args {
    /// Local UDP port to bind (0 for automatic)
    #[arg(short, long, default_value = "0")]
    port: u16,
    
    /// Custom STUN server (can be specified multiple times)
    #[arg(short, long)]
    stun_server: Vec<String>,
    
    /// Request timeout in seconds
    #[arg(short, long, default_value = "5")]
    timeout: u64,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() {
    use clap::Parser;
    let args = Args::parse();
    
    // Setup logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .init();
    
    println!("🔍 STUN Discovery Tool");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Create STUN client
    let stun_client = if args.stun_server.is_empty() {
        println!("📡 Using default Google STUN servers");
        StunClient::new()
    } else {
        println!("📡 Using custom STUN servers: {:?}", args.stun_server);
        StunClient::with_servers(args.stun_server)
    };
    
    let stun_client = stun_client.with_timeout(Duration::from_secs(args.timeout));
    
    println!("🔌 Local port: {}", if args.port == 0 { "auto".to_string() } else { args.port.to_string() });
    println!();
    
    // Perform discovery
    println!("⏳ Discovering public address...");
    match stun_client.discover(args.port).await {
        Ok(result) => {
            println!("✅ STUN Discovery Successful!\n");
            
            println!("📍 Results:");
            println!("   Local Address:  {}", result.local_addr);
            println!("   Public Address: {}", result.public_addr());
            println!("   Public IP:      {}", result.public_ip);
            println!("   Public Port:    {}", result.public_port);
            println!();
            
            println!("🌐 NAT Information:");
            println!("   Type:           {:?}", result.nat_type);
            println!("   Description:    {}", result.nat_type.description());
            println!();
            
            // Show P2P compatibility
            println!("🔗 P2P Compatibility:");
            show_p2p_compatibility(&result.nat_type);
            println!();
            
            // Recommendations
            println!("💡 Recommendations:");
            match result.nat_type {
                NatType::OpenInternet => {
                    println!("   ✓ Your device is directly on the public internet");
                    println!("   ✓ P2P connections should work perfectly");
                    println!("   ✓ No NAT traversal needed");
                }
                NatType::FullCone => {
                    println!("   ✓ Your NAT type is very P2P-friendly");
                    println!("   ✓ Direct connections should work with most peers");
                    println!("   ✓ Hole punching success rate: 90%+");
                }
                NatType::RestrictedCone | NatType::PortRestricted => {
                    println!("   ~ Your NAT type has moderate P2P support");
                    println!("   ~ Direct connections may require hole punching");
                    println!("   ~ Success rate with similar NATs: 70-85%");
                    println!("   ~ Relay fallback recommended for reliability");
                }
                NatType::Symmetric => {
                    println!("   ⚠ Your NAT type is challenging for P2P");
                    println!("   ⚠ Direct connections may be difficult");
                    println!("   ⚠ Hole punching success rate: 15-50%");
                    println!("   ⚠ Strongly recommend relay fallback");
                }
                NatType::Unknown => {
                    println!("   ? Unable to determine NAT type precisely");
                    println!("   ? P2P may or may not work");
                    println!("   ? Recommend testing with actual peers");
                }
            }
        }
        Err(e) => {
            eprintln!("❌ STUN Discovery Failed: {}", e);
            eprintln!();
            eprintln!("Possible reasons:");
            eprintln!("  - No internet connection");
            eprintln!("  - STUN servers are unreachable");
            eprintln!("  - Firewall blocking UDP traffic");
            eprintln!("  - Port {} already in use", args.port);
            std::process::exit(1);
        }
    }
}

fn show_p2p_compatibility(nat_type: &NatType) {
    let scenarios = [
        ("Open Internet", NatType::OpenInternet),
        ("Full Cone NAT", NatType::FullCone),
        ("Restricted Cone", NatType::RestrictedCone),
        ("Port-Restricted", NatType::PortRestricted),
        ("Symmetric NAT", NatType::Symmetric),
    ];
    
    println!("   Success rates with different peer NAT types:");
    for (name, peer_nat) in &scenarios {
        let rate = nat_type.hole_punch_success_rate(peer_nat);
        let percentage = (rate * 100.0) as u32;
        let bar = "█".repeat((rate * 20.0) as usize);
        println!("   {:18} {:3}% {}", name, percentage, bar);
    }
}

