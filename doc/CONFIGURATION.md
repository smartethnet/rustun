# Configuration

## Server (`/etc/rustun/server.toml`)

```toml
[server_config]
listen_addr = "0.0.0.0:8080"

[crypto_config]
# ChaCha20-Poly1305 (recommended)
chacha20poly1305 = "your-secret-key-here"

# AES-256-GCM (hardware accelerated)
# aes256 = "your-secret-key-here"

# XOR (testing only)
# xor = "test-key"

# Plain (debugging only)
# crypto_config = plain

[route_config]
routes_file = "/etc/rustun/routes.json"
```

## Routes (`/etc/rustun/routes.json`)

```json
[
  {
    "name": "Production Gateway 01",
    "cluster": "production",
    "identity": "prod-gateway-01",
    "private_ip": "10.0.1.1",
    "mask": "255.255.255.0",
    "gateway": "10.0.1.254",
    "ciders": ["192.168.100.0/24"],
    "cider_mapping": {}
  },
  {
    "name": "Production App Server 01",
    "cluster": "production",
    "identity": "prod-app-server-01",
    "private_ip": "10.0.1.2",
    "mask": "255.255.255.0",
    "gateway": "10.0.1.254",
    "ciders": [],
    "cider_mapping": {}
  }
]
```

| Field | Description | Example |
|-------|-------------|---------|
| `name` | Human-readable label (optional) | `"Production Gateway"` |
| `cluster` | Logical group for multi-tenancy isolation | `"production"` |
| `identity` | Unique client identifier | `"prod-app-01"` |
| `private_ip` | Virtual IP assigned to this client | `"10.0.1.1"` |
| `mask` | Subnet mask for the VPN network | `"255.255.255.0"` |
| `gateway` | Gateway IP for routing | `"10.0.1.254"` |
| `ciders` | CIDR ranges routable through this client | `["192.168.1.0/24"]` |
| `cider_mapping` | Map `ciders` to real CIDRs to resolve conflicts (Linux only) | `{"192.168.11.0/24": "192.168.10.0/24"}` |

## Multi-Tenant Isolation

Clients in different clusters are completely isolated — they can only communicate within their own cluster. Use separate `cluster` values for production, staging, and development:

```json
[
  { "cluster": "production", "identity": "prod-gw",  "private_ip": "10.0.1.1", ... },
  { "cluster": "production", "identity": "prod-app", "private_ip": "10.0.1.2", ... },
  { "cluster": "development","identity": "dev-ws-01", "private_ip": "10.0.2.1", ... }
]
```

Result: `10.0.1.0/24` (production) and `10.0.2.0/24` (development) are fully isolated.
