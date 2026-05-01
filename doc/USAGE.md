# Usage

## Server

```bash
# systemd (installed via script)
sudo systemctl start rustun-server
sudo systemctl enable rustun-server
sudo journalctl -u rustun-server -f

# Manual
sudo ./server /etc/rustun/server.toml
```

## Client

```bash
# Basic connection
sudo ./client -s SERVER_IP:8080 -i client-identity

# With encryption
sudo ./client -s SERVER:8080 -i client-001 -c chacha20:my-secret-key

# P2P mode
sudo ./client -s SERVER:8080 -i client-001 --enable-p2p

# Demo server
sudo ./client -s rustun.demo.beyondnetwork.cn:18080 -i your-identity -c xor:rustun@smartethnet.github.io
```

## Client Options

| Option | Description | Example |
|--------|-------------|---------|
| `-s, --server` | Server address | `-s 192.168.1.100:8080` |
| `-i, --identity` | Client identity | `-i prod-app-01` |
| `-c, --crypto` | Encryption method | `-c chacha20:my-key` |
| `--enable-p2p` | Enable P2P mode | `--enable-p2p` |
| `--keepalive-interval` | Keepalive interval (seconds) | `--keepalive-interval 10` |
| `--masq` | Enable MASQUERADE/SNAT (Linux only, requires iptables) | `--masq` |

## Encryption Options

| Method | Flag | Notes |
|--------|------|-------|
| ChaCha20-Poly1305 | `-c chacha20:KEY` | Default, recommended |
| AES-256-GCM | `-c aes256:KEY` | Hardware accelerated |
| XOR | `-c xor:KEY` | Testing only |
| Plain | `-c plain` | Debugging only |

## P2P Connection Strategy

When `--enable-p2p` is set, Rustun uses a three-tier path selection:

1. **IPv6 Direct** — lowest latency, requires both peers to have global IPv6
2. **STUN Hole Punching** — NAT traversal for IPv4 networks
3. **Relay via server** — guaranteed fallback when P2P is unavailable

Path switching is automatic with no manual intervention required.

## Windows

```powershell
# 1. Download rustun-x86_64-pc-windows-msvc.zip from Releases
# 2. Extract and place wintun.dll (from https://www.wintun.net/) in the same folder
# 3. Run as Administrator
.\server.exe server.toml
.\client.exe -s SERVER_IP:8080 -i client-identity
```
