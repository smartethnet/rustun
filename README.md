# Rustun - A Modern VPN Tunnel in Rust

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

Another high-performance VPN tunnel implementation written in Rust.

**Status: Actively Developing** 🚧

![Architecture](./doc/arch.png)

## ✨ Features

- 🌍 **Multi-Platform Support** - Linux, macOS, Windows
- 🏢 **Multi-Tenancy** - Cluster-based isolation for different organizations
- ⚡ **High Performance** - Asynchronous I/O with Tokio runtime
- 🔐 **Multiple Encryption Methods**
  - **ChaCha20-Poly1305** (Default, Recommended)
  - **AES-256-GCM** (Hardware accelerated)
  - **XOR** (Lightweight, for testing)
  - **Plain** (No encryption, for debugging)

## 📋 Table of Contents

- [Quick Start](#quick-start)
- [Download](#download)
- [Configuration](#configuration)
- [Usage](#usage)
- [Build from Source](#build-from-source)
- [Architecture](#architecture)
- [Security](#security)
- [Contributing](#contributing)

## 🚀 Quick Start

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
wget https://github.com/smartethnet/rustun/releases/download/v1.0.0/rustun-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
tar xzf rustun-v1.0.0-x86_64-unknown-linux-gnu.tar.gz
cd rustun-v1.0.0-x86_64-unknown-linux-gnu

# Make binaries executable
chmod +x server client
```

**Windows:**
```powershell
# 1. Download rustun-v1.0.0-x86_64-pc-windows-msvc.zip from releases
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

## 🏗️ Architecture

### Network Topology

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│  Client A   │◄───────►│   Server    │◄───────►│  Client B   │
│  (Beijing)  │         │  (Central)  │         │  (Shanghai) │
└─────────────┘         └─────────────┘         └─────────────┘
      │                                                 │
      │ Virtual IP: 10.0.1.1                Virtual IP: 10.0.2.1
      │                                                 │
   ┌──▼──────────────┐                      ┌───────────▼──────┐
   │ LAN: 192.168.1.0│                      │ LAN: 192.168.10.0│
   └─────────────────┘                      └──────────────────┘
```

### Components

- **Server**: Central relay handling all client connections
- **Client**: Edge node connecting to the server
- **TUN Device**: Virtual network interface for packet tunneling
- **Crypto Layer**: Encryption/decryption of all traffic
- **Route Manager**: Dynamic routing table management

### Frame Protocol

```
Frame Header (8 bytes):
┌──────────────┬─────────┬──────┬─────────────────┐
│ Magic (4B)   │ Ver (1B)│ Type │  Payload Len    │
│ 0x91929394   │  0x01   │ (1B) │     (2B)        │
└──────────────┴─────────┴──────┴─────────────────┘
                                  │
                                  ▼
                          Encrypted Payload
```

**Frame Types:**
- `0x01`: Handshake (client authentication)
- `0x02`: KeepAlive (connection health check)
- `0x03`: Data (tunneled IP packets)
- `0x04`: HandshakeReply (server configuration response)

## 🔒 Security

### Encryption Algorithms

| Algorithm | Key Size | Nonce | Tag | Performance | Security |
|-----------|----------|-------|-----|-------------|----------|
| ChaCha20-Poly1305 | 256-bit | 96-bit | 128-bit | ⚡⚡⚡ | 🔒🔒🔒 |
| AES-256-GCM | 256-bit | 96-bit | 128-bit | ⚡⚡⚡* | 🔒🔒🔒 |
| XOR | Variable | N/A | N/A | ⚡⚡⚡⚡ | 🔓 |
| Plain | N/A | N/A | N/A | ⚡⚡⚡⚡ | ⛔ |

\* Requires AES-NI hardware support for optimal performance

### Security Features

✅ **AEAD Encryption** - Authenticated Encryption with Associated Data  
✅ **Perfect Forward Secrecy** - Each session uses unique keys  
✅ **Replay Protection** - Nonce-based protection against replay attacks  
✅ **Cluster Isolation** - Multi-tenant security with no cross-cluster access  
✅ **Connection Authentication** - Identity-based client authentication  

### Security Best Practices

1. **Use Strong Encryption**: Always use ChaCha20-Poly1305 or AES-256-GCM in production
2. **Long Keys**: Use at least 32 characters for encryption keys
3. **Regular Key Rotation**: Change encryption keys periodically
4. **Firewall Rules**: Restrict server port access to known client IPs
5. **Monitor Logs**: Enable logging and monitor for suspicious activity

## 🛠️ Troubleshooting

### Common Issues

**Issue: "Failed to initialize TUN device"**
```bash
# Linux/macOS: Run with elevated privileges
sudo ./client -s SERVER:8080 -i client-001

# Or configure TUN permissions (Linux)
sudo setcap cap_net_admin=eip ./client
```

**Windows: "Wintun driver not found"**
- Download Wintun from https://www.wintun.net/
- Extract `wintun.dll` to the same directory as `client.exe`
- Run as Administrator

**Issue: "Connection failed: Connection refused"**
```bash
# Check server is running
netstat -tuln | grep 8080

# Check firewall rules
sudo ufw allow 8080/tcp
```

**Issue: "Handshake failed"**
- Verify client identity is configured in `routes.json`
- Ensure encryption method matches server configuration
- Check server logs for authentication errors

## 📊 Performance

### Benchmarks (Preliminary)

- **Throughput**: ~900 Mbps (ChaCha20-Poly1305, single connection)
- **Latency**: < 1ms additional latency (local network)
- **CPU Usage**: ~5% per 100 Mbps throughput (Intel i7)
- **Memory**: ~20 MB per client connection

## 🔨 Build from Source

> **Note**: For most users, we recommend downloading pre-built binaries from [Releases](https://github.com/smartethnet/rustun/releases). Only build from source if you need to modify the code or target an unsupported platform.

### Prerequisites

- Rust 1.70 or higher
- Git

### Build Steps

```bash
# Clone the repository
git clone https://github.com/smartethnet/rustun.git
cd rustun

# Build release binaries
cargo build --release

# Binaries will be in target/release/
# - server
# - client
```

### Cross-Compilation

For cross-platform builds, see [BUILD.md](BUILD.md) for detailed instructions.

## 🗺️ Roadmap

- [ ] IPv6 support
- [ ] P2P direct connection
- [ ] Windows service support
- [ ] systemd integration for Linux
- [ ] Web-based management dashboard
- [ ] Dynamic route updates without restart
- [ ] UDP transport support
- [ ] QUIC protocol support
- [ ] Mobile clients (iOS/Android)
- [ ] Docker container images
- [ ] Kubernetes operator
- [ ] Pre-built binary releases
- [ ] Auto-update mechanism

## 📦 Download

Pre-built binaries are available from [GitHub Releases](https://github.com/smartethnet/rustun/releases):
- Linux (x86_64, ARM64, static musl builds)
- macOS (Intel, Apple Silicon)
- Windows (x86_64 MSVC)

**Windows users**: Remember to download [Wintun driver](https://www.wintun.net/) separately.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/smartethnet/rustun.git
cd rustun

# Install development dependencies
cargo install cargo-watch cargo-edit

# Run in development mode with auto-reload
cargo watch -x 'run --bin server'
```

### Code Style

```bash
# Format code
cargo fmt

# Lint code
cargo clippy -- -D warnings
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Tokio](https://tokio.rs/) async runtime
- Encryption by [RustCrypto](https://github.com/RustCrypto)
- TUN/TAP interface via [tun-rs](https://github.com/meh/rust-tun)

## 📞 Contact

- Issues: [GitHub Issues](https://github.com/smartethnet/rustun/issues)
- Discussions: [GitHub Discussions](https://github.com/smartethnet/rustun/discussions)

---

**Note**: This is an experimental project. Use at your own risk in production environments.
