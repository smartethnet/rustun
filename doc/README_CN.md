<div align="center">

<h1>🌐 Rustun</h1>

<h3>基于 Rust 的现代 VPN 隧道</h3>

<br/>

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![构建状态](https://img.shields.io/github/actions/workflow/status/smartethnet/rustun/rust.yml?branch=main)](https://github.com/smartethnet/rustun/actions)
[![最新版本](https://img.shields.io/github/v/release/smartethnet/rustun)](https://github.com/smartethnet/rustun/releases)
[![下载量](https://img.shields.io/github/downloads/smartethnet/rustun/total)](https://github.com/smartethnet/rustun/releases)
[![Stars](https://img.shields.io/github/stars/smartethnet/rustun?style=social)](https://github.com/smartethnet/rustun)

[🌐 官网](https://smartethnet.github.io) · [📖 文档](https://smartethnet.github.io) · [English](../README.md) · [🐛 报告问题](https://github.com/smartethnet/rustun/issues) · [✨ 功能建议](https://github.com/smartethnet/rustun/issues)

</div>

---

Rust 编写的高性能 VPN 隧道，用于实现设备互联，异地组网。

**状态：疯狂开发中** 🚧

![架构图](./arch.png)

## ✨ 核心特性

- 🔓 **开源免费** - MIT 许可证，完全免费透明
- ⚡ **简洁高效** - 一行命令启动：`./client -s SERVER:8080 -i client-001`
- 🏢 **多租户** - 基于集群的隔离，支持多团队或多业务单元
- 🔐 **安全加密** - ChaCha20-Poly1305（默认）、AES-256-GCM、XOR/Plain 可选
- 🚀 **高性能** - P2P 直连，自动降级到中继模式
- 🌍 **跨平台** - Linux、macOS、Windows 预编译二进制文件

## 📋 目录

- [快速开始](#快速开始)
  - [前置要求](#前置要求)
  - [下载预编译二进制文件](#下载预编译二进制文件)
  - [安装](#安装)
  - [快速测试](#快速测试)
- [配置](#配置)
  - [服务端配置](#服务端配置)
  - [客户端路由配置](#客户端路由配置)
- [使用说明](#使用说明)
  - [启动服务端](#启动服务端)
  - [启动客户端](#启动客户端)
  - [客户端命令行选项](#客户端命令行选项)
  - [加密选项](#加密选项)
  - [P2P 直连](#p2p-直连)
  - [示例：多租户设置](#示例多租户设置)
- [路线图](#路线图)

## 🚀 快速开始

> **💡 提示：** 访问我们的[官网](https://smartethnet.github.io)查看交互式演示和可视化指南！

### 前置要求

**所有平台：**
- TUN/TAP 驱动支持

**Windows：**
- 下载 [Wintun 驱动](https://www.wintun.net/)（TUN 设备必需）
- 管理员权限

**Linux/macOS：**
- Root/sudo 权限（或在 Linux 上设置 capabilities）

### 下载预编译二进制文件

**从 [GitHub Releases](https://github.com/smartethnet/rustun/releases/latest) 下载**

可用平台：
- **Linux** - x86_64 (glibc/musl), ARM64 (glibc/musl)
- **macOS** - Intel (x86_64), Apple Silicon (ARM64)
- **Windows** - x86_64 (MSVC)

每个发布包包含：
- `server` - VPN 服务端二进制文件
- `client` - VPN 客户端二进制文件
- `server.toml.example` - 配置示例
- `routes.json.example` - 路由示例

### 安装

**Linux/macOS：**
```bash
# 下载并解压（以 Linux x86_64 为例）
wget https://github.com/smartethnet/rustun/releases/download/0.0.1/rustun-v0.0.1-x86_64-unknown-linux-gnu.tar.gz
tar xzf rustun-v0.0.1-x86_64-unknown-linux-gnu.tar.gz
cd rustun-v0.0.1-x86_64-unknown-linux-gnu

# 添加可执行权限
chmod +x server client
```

**Windows：**
```powershell
# 1. 从 releases 下载 rustun-0.0.1-x86_64-pc-windows-msvc.zip
# 2. 解压到目录
# 3. 从 https://www.wintun.net/ 下载 Wintun
# 4. 将 wintun.dll 解压到与 client.exe 相同的目录
```

### 快速测试

**启动服务端：**
```bash
# Linux/macOS
sudo ./server server.toml.example

# Windows (以管理员身份)
.\server.exe server.toml.example
```

**连接客户端：**
```bash
# Linux/macOS
sudo ./client -s SERVER_IP:8080 -i client-001

# Windows (以管理员身份)
.\client.exe -s SERVER_IP:8080 -i client-001
```

## ⚙️ 配置

### 服务端配置

创建 `server.toml`：

```toml
[server_config]
listen_addr = "0.0.0.0:8080"

[crypto_config]
# ChaCha20-Poly1305（推荐）
chacha20poly1305 = "your-secret-key-here"

# 或使用 AES-256-GCM
# aes256 = "your-secret-key-here"

# 或 XOR（轻量级）
# xor = "rustun"

# 或无加密
# crypto_config = plain

[route_config]
routes_file = "./etc/routes.json"
```

### 客户端路由配置

创建 `routes.json`：

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

**配置说明：**

- `cluster`：多租户隔离的逻辑分组
- `identity`：唯一的客户端标识符
- `private_ip`：分配给客户端的虚拟 IP
- `mask`：VPN 网络的子网掩码
- `gateway`：路由的网关 IP
- `ciders`：通过此客户端可访问的 CIDR 范围

## 📖 使用说明

### 启动服务端

```bash
# 使用默认配置文件
./server etc/server.toml

# 服务端将：
# - 监听 0.0.0.0:8080
# - 使用 ChaCha20-Poly1305 加密
# - 从 routes.json 加载客户端路由
```

### 启动客户端

```bash
# 基本用法（使用默认 ChaCha20 加密）
./client -s SERVER_IP:8080 -i CLIENT_IDENTITY

# 示例：北京办公网关
./client -s 192.168.1.100:8080 -i bj-office-gw

# 示例：上海数据库服务器
./client -s 192.168.1.100:8080 -i sh-db-server
```

### 客户端命令行选项

```bash
./client --help
```

```
Rustun VPN Client

用法: client [OPTIONS] --server <SERVER> --identity <IDENTITY>

选项:
  -s, --server <SERVER>
          服务器地址 (例如, 127.0.0.1:8080)

  -i, --identity <IDENTITY>
          客户端标识/名称

  -c, --crypto <CRYPTO>
          加密方式: plain, aes256:<key>, chacha20:<key>, 或 xor:<key>
          [默认: chacha20:rustun]

      --enable-p2p
          启用 P2P IPv6 直连
          (默认禁用，仅使用中继)

      --keepalive-interval <KEEPALIVE_INTERVAL>
          保活间隔（秒）
          [默认: 10]

      --keepalive-threshold <KEEPALIVE_THRESHOLD>
          保活阈值（失败多少次后重连）
          [默认: 5]

  -h, --help
          显示帮助信息

  -V, --version
          显示版本
```

### 加密选项

```bash
# ChaCha20-Poly1305（默认，推荐）
./client -s SERVER:8080 -i client-001 -c chacha20:my-secret-key

# AES-256-GCM（支持的 CPU 上硬件加速）
./client -s SERVER:8080 -i client-001 -c aes256:my-secret-key

# XOR（轻量级，仅用于测试）
./client -s SERVER:8080 -i client-001 -c xor:test-key

# Plain（无加密，仅用于调试）
./client -s SERVER:8080 -i client-001 -c plain
```

### P2P 直连

默认情况下，所有流量都通过中继服务器。启用 P2P 可实现客户端之间的 IPv6 直连：

```bash
# 启用 P2P 直连
./client -s SERVER:8080 -i client-001 --enable-p2p
```

**P2P 优势：**
- 🚀 更低延迟（点对点直连）
- 📉 减少服务器带宽消耗
- ⚡ P2P 失败时自动降级到中继

**要求：**
- 双方客户端都需要 IPv6 连接
- 双方客户端都需要使用 `--enable-p2p` 标志
- UDP 端口 51258 需要可访问

**工作原理：**
1. 客户端通过服务器交换 IPv6 地址
2. Keepalive 数据包建立直连
3. 连接激活时通过 P2P 发送数据
4. P2P 失败时自动降级到中继

### 示例：多租户设置

#### 场景：两个办公室（北京和上海）

**北京集群：**
- 办公网关：`bj-office-gw` (10.0.1.1)
- 开发服务器：`bj-dev-server` (10.0.1.2)

**上海集群：**
- 办公网关：`sh-office-gw` (10.0.2.1)
- 数据库服务器：`sh-db-server` (10.0.2.2)

**启动服务端：**
```bash
./server etc/server.toml
```

**连接北京客户端：**
```bash
# 终端 1：北京办公网关
./client -s 192.168.1.100:8080 -i bj-office-gw

# 终端 2：北京开发服务器
./client -s 192.168.1.100:8080 -i bj-dev-server
```

**连接上海客户端：**
```bash
# 终端 3：上海办公网关
./client -s 192.168.1.100:8080 -i sh-office-gw

# 终端 4：上海数据库服务器
./client -s 192.168.1.100:8080 -i sh-db-server
```

**测试连通性：**

```bash
# 北京客户端可以通信
ping 10.0.1.2  # 从 bj-office-gw 到 bj-dev-server

# 上海客户端可以通信
ping 10.0.2.2  # 从 sh-office-gw 到 sh-db-server

# 跨集群通信被隔离
# 北京无法访问上海，反之亦然
```

## 🏗️ 架构

### 组件

- **服务端**：处理所有客户端连接的中心中继
- **客户端**：连接到服务端的边缘节点
- **TUN 设备**：用于数据包隧道的虚拟网络接口
- **加密层**：所有流量的加密/解密
- **路由管理器**：动态路由表管理

### 帧协议

```
帧头 (8 字节)：
┌──────────────┬─────────┬──────┬─────────────────┐
│ Magic (4B)   │ Ver (1B)│ Type │  Payload Len    │
│ 0x91929394   │  0x01   │ (1B) │     (2B)        │
└──────────────┴─────────┴──────┴─────────────────┘
                                  │
                                  ▼
                          加密的负载数据
```

**帧类型：**
- `0x01`：Handshake（客户端认证）
- `0x02`：KeepAlive（连接健康检查）
- `0x03`：Data（隧道化的 IP 数据包）
- `0x04`：HandshakeReply（服务端配置响应）

## 🗺️ 路线图

- [x] **IPv6 支持** - ✅ 已完成
- [x] **P2P 直连** - ✅ 已完成（IPv6 P2P 带自动降级）
- [ ] Windows 服务支持
- [ ] Linux systemd 集成
- [ ] 基于 Web 的管理面板
- [ ] 无需重启的动态路由更新
- [ ] QUIC 协议支持
- [ ] 移动客户端（iOS/Android）
- [ ] Docker 容器镜像
- [ ] Kubernetes operator
- [ ] 自动更新机制

## 🙏 致谢

- 使用 [Tokio](https://tokio.rs/) 异步运行时构建
- 加密由 [RustCrypto](https://github.com/RustCrypto) 提供
- TUN/TAP 接口通过 [tun-rs](https://github.com/meh/rust-tun) 实现

## 📞 联系方式

- 问题反馈：[GitHub Issues](https://github.com/smartethnet/rustun/issues)
- 讨论：[GitHub Discussions](https://github.com/smartethnet/rustun/discussions)

---

**注意**：这是一个实验性项目。在生产环境中使用需自行承担风险。
