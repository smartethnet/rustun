# Rustun Protocol

## Frame Structure

### Header (8 bytes)
```
+--------+--------+--------+--------+--------+--------+--------+--------+
|      Magic (4B)           |Version|  Type  | Payload Length (2B)     |
+--------+--------+--------+--------+--------+--------+--------+--------+
```

- **Magic**: `0x91929394` (Fixed value for protocol identification)
- **Version**: `0x01` (Protocol version)
- **Type**: Frame type (1=Handshake, 2=KeepAlive, 3=Data, 4=HandshakeReply, 5=PeerUpdate)
- **Payload Length**: Payload size in bytes (Big-endian, max 65535 bytes)

### Encryption
- Non-Data frames: JSON serialization followed by encryption
- Data Frame: Direct encryption of raw IP packets
- Supported algorithms: ChaCha20-Poly1305 (default), AES-256-GCM, XOR, Plain

---

## Connection Flow

### Initial Connection (Client -> Server)

```
Client A                          Server                          Client B
   |                                 |                                 |
   |-- 1. TCP Connect -------------->|                                 |
   |                                 |<-- 1. TCP Connect --------------|
   |                                 |                                 |
   |-- 2. Handshake ---------------->|                                 |
   |    (identity, ipv6, port)       |                                 |
   |                                 |                                 |
   |<- 3. HandshakeReply ------------|                                 |
   |    (private_ip, mask,           |                                 |
   |     gateway, others[])          |                                 |
   |                                 |                                 |
   |    others[] contains:           |                                 |
   |    - Client B's identity        |                                 |
   |    - Client B's private_ip      |                                 |
   |    - Client B's ipv6            |                                 |
   |    - Client B's port            |                                 |
   |    - Client B's CIDRs           |                                 |
   |                                 |                                 |
```

**Explanation**:
- Client connects to Server via TCP on startup
- Sends Handshake with its identity and P2P address (ipv6:port)
- Server validates identity and returns network config plus list of other clients in the same cluster

---

## Relay Mode (Default)

All clients use Relay mode by default.

```
Client A                          Server                          Client B
   |                                 |                                 |
   |-- Data (dst: 10.0.1.3) -------->|                                 |
   |    Payload: IP Packet           |                                 |
   |                                 |-- Data ----------------------->|
   |                                 |    Payload: IP Packet          |
   |                                 |                                 |
   |                                 |<- Data (dst: 10.0.1.2) ---------|
   |<- Data -------------------------|                                 |
   |    Payload: IP Packet           |                                 |
```

**Features**:
- All traffic relayed through Server
- Server routes packets based on destination IP
- Works in all network environments (NAT-friendly)
- Higher latency, increased server load

---

## P2P Mode (Peer-to-Peer)

### Phase 1: P2P Connection Establishment

```
Client A (10.0.1.2)                                      Client B (10.0.1.3)
IPv6: [2001:db8::1]:51258                               IPv6: [2001:db8::2]:51258
   |                                                            |
   |-- 1. UDP Bind 51258 ---------------------------------------|
   |                                                            |-- UDP Bind 51258
   |                                                            |
   |-- 2. Send KeepAlive -------------------------------------->|
   |    To: [2001:db8::2]:51258                                 |
   |    Payload: {identity: "A", ipv6, port}                    |
   |                                                            |
   |                                                            |-- 3. Receive KeepAlive
   |                                                            |    Set last_active = now()
   |                                                            |
   |<------------------------------------- 4. Reply KeepAlive --|
   |                                                            |    To: [2001:db8::1]:51258
   |                                                            |    Payload: {identity: "B", ipv6, port}
   |-- 5. Receive KeepAlive                                     |
   |    Set last_active = now()                                 |
   |                                                            |
   |==================== P2P Connection Established =============|
```

**Key Points**:
1. Client gets peer's IPv6 address and port from HandshakeReply
2. Immediately sends KeepAlive probe to all peers (initially `last_active = 0`)
3. Upon receiving peer's KeepAlive, updates `last_active = now()`
4. **Connection is valid only if `last_active > 0` and `now() - last_active < 15s`**

### Phase 2: P2P Data Transmission

```
Client A                                                   Client B
   |                                                            |
   |-- 1. Receive IP packet from TUN (dst: 10.0.1.3) ----------|
   |                                                            |
   |-- 2. Route lookup: 10.0.1.3 -> Peer B -------------------|
   |                                                            |
   |-- 3. Check connection status ------------------------------|
   |    if (now() - last_active < 15s)                         |
   |      Connection valid, use P2P                            |
   |    else                                                    |
   |      Connection invalid, fallback to Relay                |
   |                                                            |
   |-- 4. UDP Send Data ---------------------------------------->|
   |    To: [2001:db8::2]:51258                                 |
   |    Payload: Encrypted IP Packet                            |
   |                                                            |
   |                                                            |-- 5. Receive Data
   |                                                            |    Decrypt, write to TUN
   |                                                            |    Update last_active = now()
   |                                                            |
   |<----------------------------------------- 6. UDP Send Data --|
   |    Payload: Encrypted IP Packet                            |
   |-- 7. Receive Data, update last_active --------------------|
```

**Send Decision**:
```
if P2P enabled && peer exists:
    if now() - last_active < 15s:
        Send via UDP P2P
    else:
        Fallback to Relay (TCP)
else:
    Relay (TCP)
```

### Phase 3: P2P Keep-Alive

```
Client A                                                   Client B
   |                                                            |
   |-- Timer triggers every 10 seconds -----------------------------------------|
   |                                                            |
   |-- Iterate all peers -------------------------------------------|
   |                                                            |
   |-- Send KeepAlive ----------------------------------------->|
   |    To: [2001:db8::2]:51258                                 |
   |                                                            |
   |                                                            |-- Receive KeepAlive
   |                                                            |    Update last_active = now()
   |                                                            |
   |<---------------------------------------- Reply KeepAlive --|
   |                                                            |
   |-- Receive KeepAlive, update last_active --------------------------|
```

**Keep-Alive Strategy**:
- Timer sends KeepAlive every 10 seconds
- Receiving KeepAlive automatically updates `last_active`
- Connection considered dead if no packets (Data or KeepAlive) received within 15 seconds
- Next send automatically falls back to Relay

---

## Dynamic Peer Address Update

When a client's public address changes (e.g., network switch):

```
Client A                          Server                          Client B
   |                                 |                                 |
   |-- KeepAlive ------------------->|                                 |
   |    (new ipv6: 2001:db8::99)     |                                 |
   |                                 |                                 |
   |                                 |-- Detect address change        |
   |                                 |    2001:db8::1 -> 2001:db8::99 |
   |                                 |                                 |
   |                                 |-- PeerUpdate ------------------>|
   |                                 |    (A's identity, new ipv6)    |
   |                                 |                                 |
   |                                 |                                 |-- Receive PeerUpdate
   |                                 |                                 |    Update A's address
   |                                 |                                 |    Reset last_active = 0
   |                                 |                                 |    Send new KeepAlive probe
   |                                 |                                 |
   |<----------------------------------------------------- KeepAlive --|
   |    To: [2001:db8::99]:51258                                       |
   |                                                                   |
   |-- Reply KeepAlive ----------------------------------------------->|
   |                                                                   |
   |==================== P2P Connection Restored ==================================|
```

**Flow**:
1. Client A periodically (every 5 minutes) checks its public address
2. If address changes, includes new address in next KeepAlive to Server
3. Server detects change and broadcasts PeerUpdate to all other clients in the same cluster
4. Client B receives PeerUpdate, updates A's address, and re-initiates P2P probe

---

## Hybrid P2P and Relay

In practice, P2P and Relay coexist:

```
Client A ----[P2P UDP]----> Client B  (Direct connection successful)
Client A ----[Relay TCP]---> Server ---> Client C  (P2P failed, using Relay)
Client B ----[P2P UDP]----> Client C  (Direct connection successful)
```

**Auto Fallback**:
- Try P2P first
- Automatically use Relay when P2P connection is invalid
- Transparent to upper layers, no manual switching required

---

## Summary

**Relay Mode**:
- ✅ Stable and reliable, works in all networks
- ❌ Higher latency, increased server load

**P2P Mode**:
- ✅ Low latency, direct connection
- ✅ Reduces server load
- ❌ Requires IPv6 or STUN support
- ✅ Automatic fallback to Relay on failure

