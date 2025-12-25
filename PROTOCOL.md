# Rustun Protocol

## Frame Structure

### Header (8 bytes)
```
+--------+--------+--------+--------+--------+--------+--------+--------+
|      Magic (4B)           |Version|  Type  | Payload Length (2B)     |
+--------+--------+--------+--------+--------+--------+--------+--------+
```

- **Magic**: `0x91929394` (固定值，用于识别协议)
- **Version**: `0x01` (协议版本)
- **Type**: Frame 类型 (1=Handshake, 2=KeepAlive, 3=Data, 4=HandshakeReply, 5=PeerUpdate)
- **Payload Length**: Payload 长度 (大端序，最大 65535 字节)

### Encryption
- 除 Data 外的 Frame：JSON 序列化后加密
- Data Frame：直接加密原始 IP 包
- 支持算法：ChaCha20-Poly1305 (默认)、AES-256-GCM、XOR、Plain

---

## Connection Flow

### 初始连接流程 (Client -> Server)

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
   |    others[] 包含:                |                                 |
   |    - Client B 的 identity       |                                 |
   |    - Client B 的 private_ip     |                                 |
   |    - Client B 的 ipv6           |                                 |
   |    - Client B 的 port           |                                 |
   |    - Client B 的网段ciders       |                                 |
```

**说明**：
- 客户端启动时先通过 TCP 连接到 Server
- 发送 Handshake 包含自己的 identity 和 P2P 地址（ipv6:port）
- Server 验证 identity，返回该客户端的网络配置和同 cluster 其他客户端列表

---

## Relay Mode (中继模式)

所有客户端默认使用 Relay 模式通信。

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

**特点**：
- 所有流量经过 Server 中转
- Server 根据目标 IP 查找对应客户端转发
- 适用于所有网络环境（NAT 友好）
- 延迟较高，服务器负载大

---

## P2P Mode (点对点模式)

### 阶段 1: P2P 连接建立

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
   |                                                            |    记录 last_active = now()
   |                                                            |
   |<------------------------------------- 4. Reply KeepAlive --|
   |                                                            |    To: [2001:db8::1]:51258
   |                                                            |    Payload: {identity: "B", ipv6, port}
   |-- 5. Receive KeepAlive                                     |
   |    记录 last_active = now()                                 |
   |                                                            |
   |==================== P2P 连接已建立 ==========================|
```

**关键点**：
1. 客户端从 HandshakeReply 获取其他 peer 的 IPv6 地址和端口
2. 立即向所有 peer 发送 KeepAlive 探测包（此时 `last_active = 0`）
3. 收到对方 KeepAlive 后更新 `last_active = now()`
4. **只有 `last_active > 0` 且 `now() - last_active < 15s` 才认为连接有效**

### 阶段 2: P2P 数据传输

```
Client A                                                   Client B
   |                                                            |
   |-- 1. 从 TUN 设备收到 IP 包 (dst: 10.0.1.3) ----------------|
   |                                                            |
   |-- 2. 查找路由: 10.0.1.3 -> Peer B -----------------------|
   |                                                            |
   |-- 3. 检查连接状态 ----------------------------------------|
   |    if (now() - last_active < 15s)                         |
   |      连接有效，使用 P2P                                    |
   |    else                                                    |
   |      连接无效，降级到 Relay                                |
   |                                                            |
   |-- 4. UDP Send Data ---------------------------------------->|
   |    To: [2001:db8::2]:51258                                 |
   |    Payload: Encrypted IP Packet                            |
   |                                                            |
   |                                                            |-- 5. 收到 Data
   |                                                            |    解密，写入 TUN 设备
   |                                                            |    更新 last_active = now()
   |                                                            |
   |<----------------------------------------- 6. UDP Send Data --|
   |    Payload: Encrypted IP Packet                            |
   |-- 7. 收到 Data，更新 last_active                          |
```

**发送决策**：
```
if P2P 已启用 && peer 存在:
    if now() - last_active < 15s:
        通过 UDP P2P 发送
    else:
        降级到 Relay (TCP)
else:
    Relay (TCP)
```

### 阶段 3: P2P 保活

```
Client A                                                   Client B
   |                                                            |
   |-- 每 10 秒定时器触发 -----------------------------------------|
   |                                                            |
   |-- 遍历所有 peers -------------------------------------------|
   |                                                            |
   |-- Send KeepAlive ----------------------------------------->|
   |    To: [2001:db8::2]:51258                                 |
   |                                                            |
   |                                                            |-- 收到 KeepAlive
   |                                                            |    更新 last_active = now()
   |                                                            |
   |<---------------------------------------- Reply KeepAlive --|
   |                                                            |
   |-- 收到 KeepAlive，更新 last_active --------------------------|
```

**保活策略**：
- 定时器每 10 秒发送一次 KeepAlive
- 收到 KeepAlive 自动更新 `last_active`
- 如果 15 秒内没有任何包（Data 或 KeepAlive），连接视为失效
- 下次发送时自动降级到 Relay

---

## 动态 Peer 地址更新

当客户端的公网地址变化时（如网络切换）：

```
Client A                          Server                          Client B
   |                                 |                                 |
   |-- KeepAlive ------------------->|                                 |
   |    (new ipv6: 2001:db8::99)     |                                 |
   |                                 |                                 |
   |                                 |-- 检测到地址变化                |
   |                                 |    2001:db8::1 -> 2001:db8::99 |
   |                                 |                                 |
   |                                 |-- PeerUpdate ------------------>|
   |                                 |    (A's identity, new ipv6)    |
   |                                 |                                 |
   |                                 |                                 |-- 收到 PeerUpdate
   |                                 |                                 |    更新 A 的地址
   |                                 |                                 |    重置 last_active = 0
   |                                 |                                 |    发送新 KeepAlive 探测
   |                                 |                                 |
   |<----------------------------------------------------- KeepAlive --|
   |    To: [2001:db8::99]:51258                                       |
   |                                                                   |
   |-- Reply KeepAlive ----------------------------------------------->|
   |                                                                   |
   |==================== P2P 连接恢复 ==================================|
```

**流程**：
1. Client A 定期（每 5 分钟）检查自己的公网地址
2. 如果地址变化，在下次 KeepAlive 时携带新地址发给 Server
3. Server 检测到变化，向同 cluster 的所有其他客户端广播 PeerUpdate
4. Client B 收到 PeerUpdate，更新 A 的地址，并重新发起 P2P 探测

---

## P2P vs Relay 混合使用

实际使用中，P2P 和 Relay 会同时存在：

```
Client A ----[P2P UDP]----> Client B  (直连成功)
Client A ----[Relay TCP]---> Server ---> Client C  (P2P 失败，使用 Relay)
Client B ----[P2P UDP]----> Client C  (直连成功)
```

**自动降级**：
- 优先尝试 P2P
- P2P 连接无效时自动使用 Relay
- 对上层透明，无需手动切换

---

## 总结

**Relay 模式**：
- ✅ 稳定可靠，适用所有网络
- ❌ 延迟高，服务器负载大

**P2P 模式**：
- ✅ 低延迟，直接连接
- ✅ 减轻服务器负载
- ❌ 需要 IPv6 或 STUN 支持
- ✅ 失败自动降级到 Relay

