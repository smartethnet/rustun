<div align="center">

<h1>🌐 Rustun</h1>

<h3>A Modern VPN Tunnel in Rust</h3>

<br/>

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Build Status](https://img.shields.io/github/actions/workflow/status/smartethnet/rustun/rust.yml?branch=main)](https://github.com/smartethnet/rustun/actions)
[![Release](https://img.shields.io/github/v/release/smartethnet/rustun)](https://github.com/smartethnet/rustun/releases)
[![Downloads](https://img.shields.io/github/downloads/smartethnet/rustun/total)](https://github.com/smartethnet/rustun/releases)
[![Stars](https://img.shields.io/github/stars/smartethnet/rustun?style=social)](https://github.com/smartethnet/rustun)

[🌐 Website](https://smartethnet.github.io) · [📖 Documentation](https://smartethnet.github.io) · [中文文档](./doc/README_CN.md) · [🐛 Report Bug](https://github.com/smartethnet/rustun/issues) · [✨ Request Feature](https://github.com/smartethnet/rustun/issues)

</div>

---

A high-performance VPN tunnel implementation written in Rust.

**Status: Actively Developing** 🚧

![Architecture](./doc/arch.png)

## ✨ Key Features

- 🔓 **Open Source** - MIT License, completely free and transparent
- ⚡ **Simple & Fast** - One command to start: `./client -s SERVER:8080 -i client-001`
- 🏢 **Multi-Tenant** - Cluster-based isolation for multiple teams or business units
- 🔐 **Secure Encryption** - ChaCha20-Poly1305 (default), AES-256-GCM, XOR/Plain options
- 🚀 **Dual-Path P2P** - IPv6 direct connection + STUN hole punching with auto-fallback to relay
- 🌐 **Smart Routing** - Automatic path selection: IPv6 (lowest latency) → STUN (NAT traversal) → Relay
- 🌍 **Cross-Platform** - Linux, macOS, Windows with pre-built binaries

## 📋 Table of Contents

- [Quick Start](#quick-start)
  - [Prerequisites](#prerequisites)
  - [Download Pre-built Binaries](#download-pre-built-binaries)
  - [Installation](#installation)
  - [Quick Test](#quick-test)
- [Configuration](#configuration)
  - [Server Configuration](#server-configuration)
  - [Client Routes Configuration](#client-routes-configuration)
- [Usage](#usage)
  - [Starting the Server](#starting-the-server)
  - [Starting a Client](#starting-a-client)
  - [Client Command-Line Options](#client-command-line-options)
  - [Encryption Options](#encryption-options)
  - [P2P Direct Connection](#p2p-direct-connection)
  - [Example: Multi-Tenant Setup](#example-multi-tenant-setup)
- [Roadmap](#roadmap)

## 🚀 Quick Start

> **💡 Tip:** Visit our [website](https://smartethnet.github.io) for an interactive demo and visual guide!

### Prerequisites

**All Platforms:**
- TUN/TAP driver support

**Windows:**
- Download [Wintun driver](https://www.wintun.net/) (required for TUN device)
- Administrator privileges

**Linux/macOS:**
- Root/sudo privileges (or set capabilities on Linux)

### Download Pre-built Binaries

**Download from [GitHub Releases](https://github.com/smartethnet/rustun/releases/latest)**

Available for:
- **Linux** - x86_64 (glibc/musl), ARM64 (glibc/musl)
- **macOS** - Intel (x86_64), Apple Silicon (ARM64)
- **Windows** - x86_64 (MSVC)

Each release includes:
- `server` - VPN server binary
- `client` - VPN client binary
- `server.toml.example` - Configuration example
- `routes.json.example` - Routes example

### Installation

**Linux/macOS:**
```bash
# Download and extract (example for Linux x86_64)
wget https://github.com/smartethnet/rustun/releases/download/0.0.1/rustun-0.0.1-x86_64-unknown-linux-gnu.tar.gz
tar xzf rustun-0.0.1-x86_64-unknown-linux-gnu.tar.gz
cd rustun-0.0.1-x86_64-unknown-linux-gnu

# Make binaries executable
chmod +x server client
```

**Windows:**
```powershell
# 1. Download rustun-0.0.1-x86_64-pc-windows-msvc.zip from releases
# 2. Extract to a directory
# 3. Download Wintun from https://www.wintun.net/
# 4. Extract wintun.dll to the same directory as client.exe
```

### Quick Test

**Start Server:**
```bash
# Linux/macOS
sudo ./server server.toml.example

# Windows (as Administrator)
.\server.exe server.toml.example
```

**Connect Client:**
```bash
# Linux/macOS
sudo ./client -s SERVER_IP:8080 -i client-001

# Windows (as Administrator)
.\client.exe -s SERVER_IP:8080 -i client-001
```

## ⚙️ Configuration

### Server Configuration

Create `server.toml`:

```toml
[server_config]
listen_addr = "0.0.0.0:8080"

[crypto_config]
# ChaCha20-Poly1305 (Recommended)
chacha20poly1305 = "your-secret-key-here"

# Or use AES-256-GCM
# aes256 = "your-secret-key-here"

# Or XOR (lightweight)
# xor = "rustun"

# Or no encryption
# crypto_config = plain

[route_config]
routes_file = "./etc/routes.json"
```

### Client Routes Configuration

Create `routes.json`:

```json
[
  {
    "cluster": "beijing",
    "identity": "bj-office-gw",
    "private_ip": "10.0.1.1",
    "mask": "255.255.255.0",
    "gateway": "10.0.1.254",
    "ciders": ["192.168.1.0/24", "192.168.2.0/24"]
  },
  {
    "cluster": "beijing",
    "identity": "bj-dev-server",
    "private_ip": "10.0.1.2",
    "mask": "255.255.255.0",
    "gateway": "10.0.1.254",
    "ciders": []
  },
  {
    "cluster": "shanghai",
    "identity": "sh-office-gw",
    "private_ip": "10.0.2.1",
    "mask": "255.255.255.0",
    "gateway": "10.0.2.254",
    "ciders": ["192.168.10.0/24"]
  },
  {
    "cluster": "shanghai",
    "identity": "sh-db-server",
    "private_ip": "10.0.2.2",
    "mask": "255.255.255.0",
    "gateway": "10.0.2.254",
    "ciders": []
  }
]
```

**Configuration Explained:**

- `cluster`: Logical group for multi-tenancy isolation
- `identity`: Unique client identifier
- `private_ip`: Virtual IP assigned to the client
- `mask`: Subnet mask for the VPN network
- `gateway`: Gateway IP for routing
- `ciders`: CIDR ranges accessible through this client

## 📖 Usage

### Starting the Server

```bash
# With default configuration file
./server etc/server.toml

# Server will:
# - Listen on 0.0.0.0:8080
# - Use ChaCha20-Poly1305 encryption
# - Load client routes from routes.json
```

### Starting a Client

```bash
# Basic usage (uses default ChaCha20 encryption)
./client -s SERVER_IP:8080 -i CLIENT_IDENTITY

# Example: Beijing office gateway
./client -s 192.168.1.100:8080 -i bj-office-gw

# Example: Shanghai database server
./client -s 192.168.1.100:8080 -i sh-db-server
```

### Client Command-Line Options

```bash
./client --help
```

```
Rustun VPN Client

Usage: client [OPTIONS] --server <SERVER> --identity <IDENTITY>

Options:
  -s, --server <SERVER>
          Server address (e.g., 127.0.0.1:8080)

  -i, --identity <IDENTITY>
          Client identity/name

  -c, --crypto <CRYPTO>
          Encryption method: plain, aes256:<key>, chacha20:<key>, or xor:<key>
          [default: chacha20:rustun]

      --enable-p2p
          Enable P2P direct connection via IPv6
          (disabled by default, uses relay only)

      --keepalive-interval <KEEPALIVE_INTERVAL>
          Keep-alive interval in seconds
          [default: 10]

      --keepalive-threshold <KEEPALIVE_THRESHOLD>
          Keep-alive threshold (reconnect after this many failures)
          [default: 5]

  -h, --help
          Print help

  -V, --version
          Print version
```

### Encryption Options

```bash
# ChaCha20-Poly1305 (Default, Recommended)
./client -s SERVER:8080 -i client-001 -c chacha20:my-secret-key

# AES-256-GCM (Hardware accelerated on supported CPUs)
./client -s SERVER:8080 -i client-001 -c aes256:my-secret-key

# XOR (Lightweight, for testing only)
./client -s SERVER:8080 -i client-001 -c xor:test-key

# Plain (No encryption, debugging only)
./client -s SERVER:8080 -i client-001 -c plain
```

### P2P Direct Connection

Rustun supports **dual-path P2P networking** for optimal performance and connectivity:

#### 🌟 Connection Modes

```bash
# Enable P2P with dual-path support
./client -s SERVER:8080 -i client-001 --enable-p2p
```

**Three-tier connection strategy:**

1. **🌐 IPv6 Direct Connection** (Primary Path)
   - Lowest latency, highest throughput
   - Works when both peers have global IPv6 addresses
   - UDP port 51258 (configurable via `P2P_UDP_PORT`)

2. **📡 STUN Hole Punching** (Secondary Path)
   - NAT traversal for IPv4 networks
   - Automatic public IP/port discovery
   - Works behind most NAT types
   - UDP port 51259 (configurable via `P2P_HOLE_PUNCH_PORT`)

3. **🔄 Relay Mode** (Fallback)
   - Guaranteed connectivity via central server
   - Automatic fallback when P2P fails
   - Works in all network conditions

#### ✨ Key Benefits

- **🎯 Smart Routing**: Automatically selects the best available path
  - IPv6 available & active (< 15s) → Use IPv6
  - IPv6 expired, STUN active → Use STUN
  - Both expired → Use Relay
- **⚡ Zero Configuration**: Addresses exchanged automatically via server
- **🔄 Dynamic Failover**: Seamless fallback between paths
- **📊 Real-time Status**: Monitor connection health with status command

#### 🔧 How It Works

```
Initial Setup:
  Client A ←--Handshake--→ Server ←--Handshake--→ Client B
                 ↓
         Exchange addresses:
         - IPv6: [2001:db8::1]:51258
         - STUN: 203.0.113.45:51259

Ongoing Communication:
  1. Try IPv6:    [2001:db8::1]:51258  → [2001:db8::2]:51258
     └─ If active (< 15s) → ✅ Use IPv6 (fastest)
     
  2. Try STUN:    203.0.113.45:51259   → 198.51.100.89:51259
     └─ If active (< 15s) → ✅ Use STUN (NAT traversal)
     
  3. Fallback:    Client A ──→ Server ──→ Client B
     └─ Always available → ✅ Use Relay (guaranteed)

Health Monitoring:
  - Keepalive probes every 10 seconds (both IPv6 & STUN)
  - Connection timeout: 15 seconds
  - Automatic path re-selection on failure
```

#### 📋 Requirements

**For IPv6 Direct Connection:**
- Both clients have global IPv6 addresses
- UDP port 51258 accessible
- Both clients use `--enable-p2p` flag

**For STUN Hole Punching:**
- IPv4 connectivity
- UDP port 51259 accessible
- Compatible NAT type (works with most routers)
- Both clients use `--enable-p2p` flag

**Note:** If neither IPv6 nor STUN works, traffic automatically falls back to relay mode.

#### 💡 Usage Examples

```bash
# Basic P2P (IPv6 + STUN)
./client -s SERVER:8080 -i client-001 --enable-p2p

# With custom encryption
./client -s SERVER:8080 -i client-001 --enable-p2p -c chacha20:my-key

# Check connection status (press 's' in interactive mode)
# Shows IPv6 and STUN connection health for each peer
```

#### 📊 Connection Status Output

```
╔══════════════════════════════════════════════════════════════════════╗
║                        CONNECTION STATUS                             ║
╚══════════════════════════════════════════════════════════════════════╝

📡 Relay Connection (TCP)
   ├─ RX Frames:  1234 (Errors: 0)
   └─ TX Frames:  5678 (Errors: 1)

🔗 P2P Connections (UDP): 2 peers
   ├─ Peer: client-002
   │    ├─ IPv6:  ✅ Active (5s ago, [2001:db8::2]:51258)
   │    └─ STUN:  ✅ Active (8s ago, 203.0.113.45:60001)
   └─ Peer: client-003
        ├─ IPv6:  ⚠️  Inactive (20s ago, [2001:db8::3]:51258)
        └─ STUN:  ✅ Active (3s ago, 198.51.100.89:60002)
```

#### 🔍 Troubleshooting

**IPv6 connection not working?**
- Check if both clients have IPv6: `curl -6 ifconfig.me`
- Verify firewall allows UDP 51258

**STUN connection not working?**
- Check NAT type: Some symmetric NATs may not work
- Verify firewall allows UDP 51259
- Check if STUN server is reachable

**Both failing?**
- Relay mode will automatically activate
- Check server connectivity
- Verify encryption keys match

### Example: Multi-Tenant Setup

#### Scenario: Two Offices (Beijing & Shanghai)

**Beijing Cluster:**
- Office Gateway: `bj-office-gw` (10.0.1.1)
- Dev Server: `bj-dev-server` (10.0.1.2)

**Shanghai Cluster:**
- Office Gateway: `sh-office-gw` (10.0.2.1)
- DB Server: `sh-db-server` (10.0.2.2)

**Start Server:**
```bash
./server etc/server.toml
```

**Connect Beijing Clients:**
```bash
# Terminal 1: Beijing Office Gateway
./client -s 192.168.1.100:8080 -i bj-office-gw

# Terminal 2: Beijing Dev Server
./client -s 192.168.1.100:8080 -i bj-dev-server
```

**Connect Shanghai Clients:**
```bash
# Terminal 3: Shanghai Office Gateway
./client -s 192.168.1.100:8080 -i sh-office-gw

# Terminal 4: Shanghai DB Server
./client -s 192.168.1.100:8080 -i sh-db-server
```

**Test Connectivity:**

```bash
# Beijing clients can communicate
ping 10.0.1.2  # From bj-office-gw to bj-dev-server

# Shanghai clients can communicate
ping 10.0.2.2  # From sh-office-gw to sh-db-server

# Cross-cluster communication is isolated
# Beijing cannot reach Shanghai and vice versa
```

## 🗺️ Roadmap

- [x] **IPv6 P2P support** - ✅ Completed (IPv6 direct connection)
- [x] **STUN hole punching** - ✅ Completed (NAT traversal for IPv4)
- [x] **Dual-path networking** - ✅ Completed (IPv6 + STUN with intelligent failover)
- [x] **Real-time connection monitoring** - ✅ Completed (Per-path health status)
- [ ] Windows service support
- [ ] systemd integration for Linux
- [ ] Web-based management dashboard
- [ ] Dynamic route updates without restart
- [ ] QUIC protocol support
- [ ] Mobile clients (iOS/Android)
- [ ] Docker container images
- [ ] Kubernetes operator
- [ ] Auto-update mechanism

## 🙏 Acknowledgments

- Built with [Tokio](https://tokio.rs/) async runtime
- Encryption by [RustCrypto](https://github.com/RustCrypto)
- TUN/TAP interface via [tun-rs](https://github.com/meh/rust-tun)

## 📞 Contact

- Issues: [GitHub Issues](https://github.com/smartethnet/rustun/issues)
- Discussions: [GitHub Discussions](https://github.com/smartethnet/rustun/discussions)

---

**Note**: This is an experimental project. Use at your own risk in production environments.
