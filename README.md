<div align="center">

<h1>🌐 Rustun</h1>

<h3>AI-Driven Intelligent VPN Tunnel</h3>

<br/>

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Build Status](https://img.shields.io/github/actions/workflow/status/smartethnet/rustun/rust.yml?branch=main)](https://github.com/smartethnet/rustun/actions)
[![Release](https://img.shields.io/github/v/release/smartethnet/rustun)](https://github.com/smartethnet/rustun/releases)
[![Downloads](https://img.shields.io/github/downloads/smartethnet/rustun/total)](https://github.com/smartethnet/rustun/releases)
[![Stars](https://img.shields.io/github/stars/smartethnet/rustun?style=social)](https://github.com/smartethnet/rustun)

[🌐 Website](https://smartethnet.github.io) · [📖 Documentation](https://smartethnet.github.io) · [中文文档](./doc/README_CN.md) · [🐛 Report Bug](https://github.com/smartethnet/rustun/issues) · [✨ Request Feature](https://github.com/smartethnet/rustun/issues)

**Platform Clients:**
[📱 iOS](https://github.com/smartethnet/rustun-apple) · [🤖 Android](https://github.com/smartethnet/rustun-android) · [🪟 Windows](https://github.com/smartethnet/rustun-desktop) · [🍎 macOS](https://github.com/smartethnet/rustun-apple) · [🐧 Linux](https://github.com/smartethnet/rustun)

</div>

---

An AI-driven intelligent VPN tunnel built with Rust, featuring automatic path selection and smart routing capabilities.

**Status: Beta** 🚧

**Welcome to Rustun!** 🎉 Download our native apps for the best experience:

![](screenshot.png)

- 📱 [iOS App](https://testflight.apple.com/join/2zf3dwxH) - Available on TestFlight
- 🍎 [macOS App](https://testflight.apple.com/join/2zf3dwxH) - Native macOS TestFlight

![Architecture](./doc/ai.png)

## ✨ Key Features

- 🔓 **Open Source** - MIT License, completely free and transparent
- ⚡ **Simple & Fast** - One command to start: `./client -s SERVER:8080 -i client-001`
- 🏢 **Multi-Tenant** - Cluster-based isolation for multiple teams or business units
- 🔐 **Secure Encryption** - ChaCha20-Poly1305 (default), AES-256-GCM, XOR/Plain options
- 🚀 **Dual-Path P2P** - IPv6 direct connection + STUN hole punching with auto-fallback to relay
- 🌐 **Smart Routing** - Automatic path selection: IPv6 (lowest latency) → STUN (NAT traversal) → Relay
- 🌍 **Cross-Platform** - Linux, macOS, Windows with pre-built binaries

## 📋 Table of Contents

### For Users
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
- [P2P Connections](#-p2p-direct-connection)
- [Multi Tenant](#-multi-tenant-isolation)
- [Use Cases](#-use-cases)

### For Developers
- [Build from Source](#build-from-source)
- [Architecture](#architecture)
- [Contributing](#contributing)

### Roadmap
- [Roadmap](#roadmap)

## 🚀 Quick Start

### One-Click Installation (Recommended)

**Server Installation:**

```bash
# Automatically installs the latest version
curl -fsSL https://raw.githubusercontent.com/smartethnet/rustun/main/install.sh | sudo bash

# Configure
sudo vim /etc/rustun/server.toml
sudo vim /etc/rustun/routes.json

# Start service
sudo systemctl start rustun-server
sudo systemctl enable rustun-server
```

### Download Pre-built Binaries

Download from [GitHub Releases](https://github.com/smartethnet/rustun/releases/latest)

**Available Platforms:**
- **Linux**: x86_64 (glibc/musl), ARM64 (glibc/musl)
- **macOS**: Intel (x86_64), Apple Silicon (ARM64)
- **Windows**: x86_64 (MSVC)

**Each release includes:**
- `server` - VPN server binary
- `client` - VPN client binary
- `server.toml.example` - Server configuration template
- `routes.json.example` - Routes configuration template

### Prerequisites

**All Platforms:**
- Root/Administrator privileges (required for TUN device and routing)

**Windows Only:**
- [Wintun driver](https://www.wintun.net/) - extract `wintun.dll` to the same directory as binaries

**Linux/macOS:**
- TUN/TAP driver support (usually pre-installed)

## 📦 Installation

### Method 1: One-Click Script (Server Only)

```bash
# Install latest version
curl -fsSL https://raw.githubusercontent.com/smartethnet/rustun/main/install.sh | sudo bash
```

**What it does:**
- ✅ Detects your system automatically (Ubuntu/Debian/CentOS/Fedora/Arch)
- ✅ Downloads the correct binary for your architecture
- ✅ Installs to `/usr/local/bin/rustun-server`
- ✅ Creates configuration directory `/etc/rustun/`
- ✅ Sets up systemd service for auto-start
- ✅ Configures automatic restart on failure

**Post-installation:**

```bash
# Edit server configuration
sudo vim /etc/rustun/server.toml

# Edit routes configuration  
sudo vim /etc/rustun/routes.json

# Start server
sudo systemctl start rustun-server

# Enable auto-start on boot
sudo systemctl enable rustun-server

# Check status
sudo systemctl status rustun-server

# View logs
sudo journalctl -u rustun-server -f
```

### Method 2: Manual Installation (Client & Server)

**Step 1: Download**

```bash
# Go to releases page and download for your platform
# https://github.com/smartethnet/rustun/releases/latest

# Example for Linux x86_64
wget https://github.com/smartethnet/rustun/releases/latest/download/rustun-x86_64-unknown-linux-gnu.tar.gz
tar xzf rustun-x86_64-unknown-linux-gnu.tar.gz
cd rustun-*
```

**Step 2: Run**

```bash
# Start server (Linux/macOS)
sudo ./server server.toml.example

# Start client (Linux/macOS)
sudo ./client -s SERVER_IP:8080 -i client-001
```

**Windows:**

```powershell
# 1. Download rustun-x86_64-pc-windows-msvc.zip
# 2. Extract to a folder
# 3. Download Wintun from https://www.wintun.net/
# 4. Extract wintun.dll to the same folder
# 5. Run as Administrator:

.\server.exe server.toml.example
# or
.\client.exe -s SERVER_IP:8080 -i client-001
```

## ⚙️ Configuration

### Server Configuration

Create or edit `/etc/rustun/server.toml`:

```toml
[server_config]
# Server listening address
listen_addr = "0.0.0.0:8080"

[crypto_config]
# Encryption method (choose one):

# ChaCha20-Poly1305 (Recommended - high security, great performance)
chacha20poly1305 = "your-secret-key-here"

# AES-256-GCM (Hardware accelerated on modern CPUs)
# aes256 = "your-secret-key-here"

# XOR (Lightweight, for testing only)
# xor = "test-key"

# Plain (No encryption, debugging only)
# crypto_config=plain

[route_config]
# Path to routes configuration file
routes_file = "/etc/rustun/routes.json"
```

### Routes Configuration

Create or edit `/etc/rustun/routes.json`:

```json
[
  {
    "cluster": "production",
    "identity": "prod-gateway-01",
    "private_ip": "10.0.1.1",
    "mask": "255.255.255.0",
    "gateway": "10.0.1.254",
    "ciders": ["192.168.100.0/24", "192.168.101.0/24"]
  },
  {
    "cluster": "production",
    "identity": "prod-app-server-01",
    "private_ip": "10.0.1.2",
    "mask": "255.255.255.0",
    "gateway": "10.0.1.254",
    "ciders": []
  }
]
```

**Field Descriptions:**

| Field | Description | Example |
|-------|-------------|---------|
| `cluster` | Logical group for multi-tenancy isolation | `"production"` |
| `identity` | Unique client identifier | `"prod-app-01"` |
| `private_ip` | Virtual IP assigned to the client | `"10.0.1.1"` |
| `mask` | Subnet mask for the VPN network | `"255.255.255.0"` |
| `gateway` | Gateway IP for routing | `"10.0.1.254"` |
| `ciders` | CIDR ranges routable through this client | `["192.168.1.0/24"]` |

## 📖 Usage

### Starting the Server

**Using systemd (if installed with script):**

```bash
sudo systemctl start rustun-server
sudo systemctl status rustun-server
sudo journalctl -u rustun-server -f
```

**Running manually:**

```bash
# Linux/macOS
sudo ./server /etc/rustun/server.toml

# Windows (as Administrator)
.\server.exe server.toml
```

### Connecting Clients

**Basic Connection:**

```bash
# Linux/macOS
sudo ./client -s SERVER_IP:8080 -i client-identity

# Windows (as Administrator)
.\client.exe -s SERVER_IP:8080 -i client-identity
```

**Examples:**

```bash
# Production gateway
./client -s 192.168.1.100:8080 -i prod-gateway-01

# Development workstation
./client -s vpn.example.com:8080 -i dev-workstation-01

# With custom encryption
./client -s SERVER:8080 -i client-001 -c chacha20:my-secret-key
```

### Client Options

```bash
./client --help
```

**Common Options:**

| Option | Description | Example |
|--------|-------------|---------|
| `-s, --server` | Server address | `-s 192.168.1.100:8080` |
| `-i, --identity` | Client identity | `-i prod-app-01` |
| `-c, --crypto` | Encryption method | `-c chacha20:my-key` |
| `--enable-p2p` | Enable P2P mode | `--enable-p2p` |
| `--keepalive-interval` | Keepalive interval (seconds) | `--keepalive-interval 10` |

### Encryption Options

```bash
# ChaCha20-Poly1305 (Default, Recommended)
./client -s SERVER:8080 -i client-001 -c chacha20:my-secret-key

# AES-256-GCM (Hardware accelerated)
./client -s SERVER:8080 -i client-001 -c aes256:my-secret-key

# XOR (Lightweight, testing only)
./client -s SERVER:8080 -i client-001 -c xor:test-key

# Plain (No encryption, debugging only)
./client -s SERVER:8080 -i client-001 -c plain
```

## 🚀 P2P Direct Connection

Enable P2P for direct peer-to-peer connections with automatic intelligent path selection:

```bash
./client -s SERVER:8080 -i client-001 --enable-p2p
```

### Connection Strategy

Rustun uses a three-tier intelligent routing strategy:

1. **🌐 IPv6 Direct Connection** (Primary Path)
   - Lowest latency, highest throughput
   - Works when both peers have global IPv6 addresses
   - Automatic connection establishment

2. **🔄 STUN Hole Punching** (Secondary Path)
   - NAT traversal for IPv4 networks
   - Works across most NAT types
   - Automatic fallback when IPv6 unavailable

3. **📡 Relay Mode** (Fallback)
   - Via server when P2P fails
   - Guaranteed connectivity
   - Automatic failover

## 🏢 Multi-Tenant Isolation

Rustun supports cluster-based multi-tenancy for complete network isolation between different teams or business units.

### How It Works

- Each client belongs to a **cluster**
- Clients can only communicate with peers in the same cluster
- Different clusters use separate IP ranges
- Perfect for isolating production, staging, and development environments

### Configuration Example

**routes.json:**

```json
[
  {
    "cluster": "production",
    "identity": "prod-gateway",
    "private_ip": "10.0.1.1",
    "mask": "255.255.255.0",
    "gateway": "10.0.1.254",
    "ciders": ["192.168.100.0/24"]
  },
  {
    "cluster": "production",
    "identity": "prod-app-01",
    "private_ip": "10.0.1.2",
    "mask": "255.255.255.0",
    "gateway": "10.0.1.254",
    "ciders": []
  },
  {
    "cluster": "development",
    "identity": "dev-workstation-01",
    "private_ip": "10.0.2.1",
    "mask": "255.255.255.0",
    "gateway": "10.0.2.254",
    "ciders": []
  },
  {
    "cluster": "development",
    "identity": "dev-workstation-02",
    "private_ip": "10.0.2.2",
    "mask": "255.255.255.0",
    "gateway": "10.0.2.254",
    "ciders": []
  }
]
```

### Result

- ✅ Production clients can only communicate within `10.0.1.0/24` network
- ✅ Development clients are isolated in `10.0.2.0/24` network
- ✅ No cross-cluster communication possible
- ✅ Each team has complete network independence

## 💼 Use Cases

Rustun is designed for various networking scenarios. Here are common use cases:

| Use Case | Description | Key Benefits | Typical Setup |
|----------|-------------|--------------|---------------|
| **🏢 Remote Office Connectivity** | Connect multiple office locations with site-to-site VPN | • Seamless resource sharing<br>• P2P optimization reduces latency<br>• Multi-tenant support for departments | One server + gateway client per office |
| **👨‍💻 Secure Remote Work** | Enable secure remote access for employees working from home | • Encrypted connections from anywhere<br>• P2P reduces server load<br>• Easy user management via routes.json | One server + client per employee |
| **🔀 Multi-Environment Isolation** | Separate networks for production, staging, and development | • Zero risk of cross-environment access<br>• Same infrastructure for all envs<br>• Easy configuration replication | One server + separate cluster per environment |
| **🤖 IoT Device Management** | Securely connect and manage IoT devices across locations | • Encrypted device communication<br>• Direct P2P for low-latency control<br>• Scalable to thousands of devices | One server + lightweight client per gateway |
| **🎮 Gaming Server Network** | Low-latency network for game servers across regions | • P2P ensures sub-10ms latency<br>• Secure server-to-server comms<br>• Easy regional expansion | One server + client per game server region |
| **☁️ Hybrid Cloud Connectivity** | Connect on-premise infrastructure with cloud resources | • Secure cloud-to-datacenter bridge<br>• Automatic path optimization<br>• Support for multi-cloud scenarios | One server + client per datacenter/cloud region |
| **🔐 Zero Trust Network** | Build a zero-trust network with peer isolation | • Per-peer authentication via identity<br>• Fine-grained access control with CIDRs<br>• Complete traffic encryption | One server + strict cluster configuration |

## 🛠️ Build from Source

### Prerequisites

- **Rust 1.70+**: [Install Rust](https://www.rust-lang.org/tools/install)
- **Build Tools**: 
  - Linux: `build-essential` or equivalent
  - macOS: Xcode Command Line Tools
  - Windows: MSVC Build Tools

### Quick Build

```bash
# Clone repository
git clone https://github.com/smartethnet/rustun.git
cd rustun

# Build release binaries
cargo build --release

# Binaries will be in target/release/
./target/release/server --help
./target/release/client --help
```

### Cross-Platform Build

```bash
# Install cross-compilation tool
cargo install cross

# Build for Linux x86_64 (musl, static)
cross build --release --target x86_64-unknown-linux-musl

# Build for ARM64 Linux
cross build --release --target aarch64-unknown-linux-gnu

# Build for Windows
cross build --release --target x86_64-pc-windows-msvc

# Build for macOS (requires macOS host)
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

### Build Script

Use the provided build script for multi-platform builds:

```bash
# Build for all platforms
./build.sh

# Builds will be in build/ directory
# Archives will be in dist/ directory
```

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details on:

- Development setup and workflow
- Code style and conventions
- Testing requirements
- Pull request process
- Project structure

**Quick Start for Contributors:**

```bash
# Fork, clone and create a branch
git clone https://github.com/YOUR_USERNAME/rustun.git
cd rustun
git checkout -b feature/your-feature

# Make changes and test
cargo test
cargo fmt
cargo clippy

# Commit and push
git commit -m "feat: your feature"
git push origin feature/your-feature
```

For questions and discussions, visit [GitHub Discussions](https://github.com/smartethnet/rustun/discussions).

## 📚 Architecture

For detailed protocol and architecture documentation, see:
- [Protocol Documentation](PROTOCOL.md) / [中文版本](PROTOCOL_CN.md)
- [Build Documentation](BUILD.md)
- [Contributing Guide](CONTRIBUTING.md)

## 🗺️ Roadmap

- [x] **IPv6 P2P support** - ✅ Completed (IPv6 direct connection)
- [x] **STUN hole punching** - ✅ Completed (NAT traversal for IPv4)
- [x] **Dual-path networking** - ✅ Completed (IPv6 + STUN with intelligent failover)
- [x] **Real-time connection monitoring** - ✅ Completed (Per-path health status)
- [x] **Dynamic route updates** - ✅ Completed (Real-time sync via KeepAlive, no restart needed)
- [ ] systemd integration for Linux
- [ ] Web-based management dashboard
- [ ] Mobile & Desktop clients (Android/iOS/Windows/MacOS)
- [ ] QUIC protocol support
- [ ] Docker container images
- [ ] Kubernetes operator
- [ ] Auto-update mechanism
- [ ] Windows service support

## 🙏 Acknowledgments

- Built with [Tokio](https://tokio.rs/) async runtime
- Encryption by [RustCrypto](https://github.com/RustCrypto)
- TUN/TAP interface via [tun-rs](https://github.com/meh/rust-tun)

## 📞 Contact

- Issues: [GitHub Issues](https://github.com/smartethnet/rustun/issues)
- Discussions: [GitHub Discussions](https://github.com/smartethnet/rustun/discussions)

---

**Note**: This is an experimental project. Use at your own risk in production environments.
