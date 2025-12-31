<div align="center">

<h1>🌐 Rustun</h1>

<h3>AI驱动的智能VPN隧道</h3>

<br/>

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![构建状态](https://img.shields.io/github/actions/workflow/status/smartethnet/rustun/rust.yml?branch=main)](https://github.com/smartethnet/rustun/actions)
[![最新版本](https://img.shields.io/github/v/release/smartethnet/rustun)](https://github.com/smartethnet/rustun/releases)
[![下载量](https://img.shields.io/github/downloads/smartethnet/rustun/total)](https://github.com/smartethnet/rustun/releases)
[![Stars](https://img.shields.io/github/stars/smartethnet/rustun?style=social)](https://github.com/smartethnet/rustun)

[🌐 官网](https://smartethnet.github.io) · [📖 文档](https://smartethnet.github.io) · [English](../README.md) · [🐛 报告问题](https://github.com/smartethnet/rustun/issues) · [✨ 功能建议](https://github.com/smartethnet/rustun/issues)

**平台客户端:**
[📱 iOS](https://github.com/smartethnet/rustun-ios) · [🤖 Android](https://github.com/smartethnet/rustun-android) · [🪟 Windows](https://github.com/smartethnet/rustun) · [🍎 macOS](https://github.com/smartethnet/rustun) · [🐧 Linux](https://github.com/smartethnet/rustun)

</div>

---

基于 Rust 构建的 AI 驱动智能 VPN 隧道，具备自动路径选择和智能路由能力。

**状态：积极开发中** 🚧

![架构图](./arch.png)

## ✨ 核心特性

- 🔓 **开源免费** - MIT 许可证，完全免费透明
- ⚡ **简洁高效** - 一行命令启动：`./client -s SERVER:8080 -i client-001`
- 🏢 **多租户** - 基于集群的隔离，支持多团队或多业务单元
- 🔐 **安全加密** - ChaCha20-Poly1305（默认）、AES-256-GCM、XOR/Plain 可选
- 🚀 **双路径 P2P** - IPv6 直连 + STUN 打洞，自动降级到中继模式
- 🌐 **智能路由** - 自动选择最佳路径：IPv6（最低延迟）→ STUN（NAT穿透）→ 中继
- 🌍 **跨平台** - Linux、macOS、Windows 预编译二进制文件

## 📋 目录

### 面向用户
- [快速开始](#-快速开始)
- [安装](#-安装)
- [配置](#-配置)
- [使用](#-使用)
- [P2P 连接](#-p2p-直连)
- [多租户](#-多租户隔离)
- [使用场景](#-使用场景)

### 面向开发者
- [从源码构建](#-从源码构建)
- [贡献代码](#-贡献代码)
- [架构文档](#-架构文档)

### 路线图
- [路线图](#-路线图)

## 🚀 快速开始

### 一键安装（推荐）

**服务端安装：**

```bash
# 自动安装最新版本
curl -fsSL https://raw.githubusercontent.com/smartethnet/rustun/main/install.sh | sudo bash

# 配置
sudo vim /etc/rustun/server.toml
sudo vim /etc/rustun/routes.json

# 启动服务
sudo systemctl start rustun-server
sudo systemctl enable rustun-server
```

### 下载预编译二进制

从 [GitHub Releases](https://github.com/smartethnet/rustun/releases/latest) 下载

**支持平台：**
- **Linux**：x86_64 (glibc/musl)、ARM64 (glibc/musl)
- **macOS**：Intel (x86_64)、Apple Silicon (ARM64)
- **Windows**：x86_64 (MSVC)

**每个发布包包含：**
- `server` - VPN 服务端二进制文件
- `client` - VPN 客户端二进制文件
- `server.toml.example` - 服务端配置模板
- `routes.json.example` - 路由配置模板

### 系统要求

**所有平台：**
- Root/管理员权限（创建 TUN 设备和路由表所需）

**仅 Windows：**
- [Wintun 驱动](https://www.wintun.net/) - 将 `wintun.dll` 解压到二进制文件同目录

**Linux/macOS：**
- TUN/TAP 驱动支持（通常已预装）

## 📦 安装

### 方法一：一键脚本（仅服务端）

```bash
# 安装最新版本
curl -fsSL https://raw.githubusercontent.com/smartethnet/rustun/main/install.sh | sudo bash
```

**脚本功能：**
- ✅ 自动检测系统（Ubuntu/Debian/CentOS/Fedora/Arch）
- ✅ 下载适合您架构的正确二进制文件
- ✅ 安装到 `/usr/local/bin/rustun-server`
- ✅ 创建配置目录 `/etc/rustun/`
- ✅ 设置 systemd 服务实现自动启动
- ✅ 配置失败自动重启

**安装后：**

```bash
# 编辑服务端配置
sudo vim /etc/rustun/server.toml

# 编辑路由配置
sudo vim /etc/rustun/routes.json

# 启动服务
sudo systemctl start rustun-server

# 开机自启
sudo systemctl enable rustun-server

# 查看状态
sudo systemctl status rustun-server

# 查看日志
sudo journalctl -u rustun-server -f
```

### 方法二：手动安装（客户端和服务端）

**步骤 1：下载**

```bash
# 前往 releases 页面下载适合您平台的版本
# https://github.com/smartethnet/rustun/releases/latest

# 示例：Linux x86_64
wget https://github.com/smartethnet/rustun/releases/latest/download/rustun-x86_64-unknown-linux-gnu.tar.gz
tar xzf rustun-x86_64-unknown-linux-gnu.tar.gz
cd rustun-*
```

**步骤 2：运行**

```bash
# 启动服务端（Linux/macOS）
sudo ./server server.toml.example

# 启动客户端（Linux/macOS）
sudo ./client -s SERVER_IP:8080 -i client-001
```

**Windows：**

```powershell
# 1. 下载 rustun-x86_64-pc-windows-msvc.zip
# 2. 解压到文件夹
# 3. 从 https://www.wintun.net/ 下载 Wintun
# 4. 将 wintun.dll 解压到同一文件夹
# 5. 以管理员身份运行：

.\server.exe server.toml.example
# 或
.\client.exe -s SERVER_IP:8080 -i client-001
```

## ⚙️ 配置

### 服务端配置

创建或编辑 `/etc/rustun/server.toml`：

```toml
[server_config]
# 服务器监听地址
listen_addr = "0.0.0.0:8080"

[crypto_config]
# 加密方式（选择其一）：

# ChaCha20-Poly1305（推荐 - 高安全性，性能优秀）
chacha20poly1305 = "your-secret-key-here"

# AES-256-GCM（现代 CPU 硬件加速）
# aes256gcm = "your-secret-key-here"

# XOR（轻量级，仅用于测试）
# xor = "test-key"

# Plain（无加密，仅用于调试）
# crypto_config=plain

[route_config]
# 路由配置文件路径
routes_file = "/etc/rustun/routes.json"
```

**生成安全密钥：**

```bash
# 生成随机 32 字符密钥
openssl rand -base64 32
```

### 路由配置

创建或编辑 `/etc/rustun/routes.json`：

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

**字段说明：**

| 字段 | 说明 | 示例 |
|------|------|------|
| `cluster` | 多租户隔离的逻辑组 | `"production"` |
| `identity` | 客户端唯一标识符 | `"prod-app-01"` |
| `private_ip` | 分配给客户端的虚拟 IP | `"10.0.1.1"` |
| `mask` | VPN 网络的子网掩码 | `"255.255.255.0"` |
| `gateway` | 路由网关 IP | `"10.0.1.254"` |
| `ciders` | 可通过此客户端路由的 CIDR 范围 | `["192.168.1.0/24"]` |

**💡 动态路由重载：**

服务端自动监控 `routes.json` 变化。只需编辑保存即可 - 无需重启！

```bash
# 编辑路由
sudo vim /etc/rustun/routes.json

# 服务端日志会显示：
# Routes file changed, reloading...
# Added new client: prod-db-01
# Route reload complete: 5 total clients
```

## 📖 使用

### 启动服务端

**使用 systemd（如果通过脚本安装）：**

```bash
sudo systemctl start rustun-server
sudo systemctl status rustun-server
sudo journalctl -u rustun-server -f
```

**手动运行：**

```bash
# Linux/macOS
sudo ./server /etc/rustun/server.toml

# Windows（以管理员身份）
.\server.exe server.toml
```

### 连接客户端

**基本连接：**

```bash
# Linux/macOS
sudo ./client -s SERVER_IP:8080 -i client-identity

# Windows（以管理员身份）
.\client.exe -s SERVER_IP:8080 -i client-identity
```

**示例：**

```bash
# 生产网关
./client -s 192.168.1.100:8080 -i prod-gateway-01

# 开发工作站
./client -s vpn.example.com:8080 -i dev-workstation-01

# 自定义加密
./client -s SERVER:8080 -i client-001 -c chacha20:my-secret-key
```

### 客户端选项

```bash
./client --help
```

**常用选项：**

| 选项 | 说明 | 示例 |
|------|------|------|
| `-s, --server` | 服务器地址 | `-s 192.168.1.100:8080` |
| `-i, --identity` | 客户端标识 | `-i prod-app-01` |
| `-c, --crypto` | 加密方式 | `-c chacha20:my-key` |
| `--enable-p2p` | 启用 P2P 模式 | `--enable-p2p` |
| `--keepalive-interval` | 心跳间隔（秒） | `--keepalive-interval 10` |

### 加密选项

```bash
# ChaCha20-Poly1305（默认，推荐）
./client -s SERVER:8080 -i client-001 -c chacha20:my-secret-key

# AES-256-GCM（硬件加速）
./client -s SERVER:8080 -i client-001 -c aes256:my-secret-key

# XOR（轻量级，仅测试用）
./client -s SERVER:8080 -i client-001 -c xor:test-key

# Plain（无加密，仅调试用）
./client -s SERVER:8080 -i client-001 -c plain
```

## 🚀 P2P 直连

启用 P2P 实现点对点直连，具备自动智能路径选择：

```bash
./client -s SERVER:8080 -i client-001 --enable-p2p
```

### 连接策略

Rustun 使用三层智能路由策略：

1. **🌐 IPv6 直连**（主要路径）
   - 最低延迟，最高吞吐量
   - 当双方都有全局 IPv6 地址时工作
   - 自动建立连接

2. **🔄 STUN 打洞**（次要路径）
   - IPv4 网络的 NAT 穿透
   - 适用于大多数 NAT 类型
   - IPv6 不可用时自动回退

3. **📡 中继模式**（兜底）
   - P2P 失败时通过服务器中继
   - 保证连通性
   - 自动故障转移

### 性能对比

| 模式 | 延迟 | 吞吐量 | NAT 支持 |
|------|------|--------|----------|
| IPv6 直连 | ~1ms | 1000+ Mbps | N/A |
| STUN P2P | ~5ms | 500+ Mbps | 大多数 NAT |
| 中继 | ~20ms | 100+ Mbps | 所有 |

## 🏢 多租户隔离

Rustun 支持基于集群的多租户，实现不同团队或业务单元之间的完全网络隔离。

### 工作原理

- 每个客户端属于一个**集群**
- 客户端只能与同集群的对等节点通信
- 不同集群使用独立的 IP 范围
- 非常适合隔离生产、预发布和开发环境

### 配置示例

**routes.json：**

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

### 隔离效果

- ✅ 生产客户端只能在 `10.0.1.0/24` 网络内通信
- ✅ 开发客户端在 `10.0.2.0/24` 网络内隔离
- ✅ 跨集群通信不可能
- ✅ 每个团队拥有完全的网络独立性

## 💼 使用场景

Rustun 适用于各种网络场景。以下是常见使用场景：

| 使用场景 | 说明 | 主要优势 | 典型部署 |
|----------|------|----------|----------|
| **🏢 远程办公室连接** | 通过站点到站点 VPN 连接多个办公地点 | • 无缝资源共享<br>• P2P 优化降低延迟<br>• 多租户支持部门隔离 | 一个服务器 + 每个办公室一个网关客户端 |
| **👨‍💻 安全远程办公** | 为在家办公的员工提供安全远程访问 | • 随处加密连接<br>• P2P 降低服务器负载<br>• 通过 routes.json 轻松管理用户 | 一个服务器 + 每个员工一个客户端 |
| **🔀 多环境隔离** | 分离生产、预发布和开发网络 | • 零跨环境访问风险<br>• 所有环境相同基础设施<br>• 易于配置复制 | 一个服务器 + 每个环境独立集群 |
| **🤖 IoT 设备管理** | 跨地点安全连接和管理 IoT 设备 | • 加密设备通信<br>• 直接 P2P 实现低延迟控制<br>• 可扩展至数千设备 | 一个服务器 + 每个网关一个轻量级客户端 |
| **🎮 游戏服务器网络** | 跨区域游戏服务器低延迟网络 | • P2P 确保 10ms 以下延迟<br>• 安全的服务器间通信<br>• 易于区域扩展 | 一个服务器 + 每个游戏服务器区域一个客户端 |
| **☁️ 混合云连接** | 连接本地基础设施和云资源 | • 安全的云到数据中心桥接<br>• 自动路径优化<br>• 支持多云场景 | 一个服务器 + 每个数据中心/云区域一个客户端 |
| **🔐 零信任网络** | 构建具有对等节点隔离的零信任网络 | • 基于身份的每节点认证<br>• CIDR 细粒度访问控制<br>• 完全流量加密 | 一个服务器 + 严格的集群配置 |

## 🛠️ 从源码构建

### 前置要求

- **Rust 1.70+**：[安装 Rust](https://www.rust-lang.org/tools/install)
- **构建工具**：
  - Linux：`build-essential` 或等效工具
  - macOS：Xcode 命令行工具
  - Windows：MSVC 构建工具

### 快速构建

```bash
# 克隆仓库
git clone https://github.com/smartethnet/rustun.git
cd rustun

# 构建 release 版本
cargo build --release

# 二进制文件位于 target/release/
./target/release/server --help
./target/release/client --help
```

### 跨平台构建

```bash
# 安装交叉编译工具
cargo install cross

# 为 Linux x86_64 构建（musl, 静态链接）
cross build --release --target x86_64-unknown-linux-musl

# 为 ARM64 Linux 构建
cross build --release --target aarch64-unknown-linux-gnu

# 为 Windows 构建
cross build --release --target x86_64-pc-windows-msvc

# 为 macOS 构建（需要 macOS 主机）
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

### 构建脚本

使用提供的构建脚本进行多平台构建：

```bash
# 为所有平台构建
./build.sh

# 构建产物位于 build/ 目录
# 压缩包位于 dist/ 目录
```

## 🤝 贡献代码

我们欢迎贡献！详见 [贡献指南](CONTRIBUTING.md)，包括：

- 开发环境设置和工作流程
- 代码风格和规范
- 测试要求
- Pull Request 流程
- 项目结构

**贡献者快速开始：**

```bash
# Fork、克隆并创建分支
git clone https://github.com/YOUR_USERNAME/rustun.git
cd rustun
git checkout -b feature/your-feature

# 进行更改并测试
cargo test
cargo fmt
cargo clippy

# 提交并推送
git commit -m "feat: your feature"
git push origin feature/your-feature
```

如有问题和讨论，请访问 [GitHub Discussions](https://github.com/smartethnet/rustun/discussions)。

## 📚 架构文档

详细的协议和架构文档，请参阅：
- [协议文档](PROTOCOL.md) / [English](PROTOCOL_EN.md)
- [构建文档](BUILD.md)
- [贡献指南](CONTRIBUTING.md)

## 🗺️ 路线图

- [x] **IPv6 P2P 支持** - ✅ 已完成（IPv6 直连）
- [x] **STUN 打洞** - ✅ 已完成（IPv4 NAT 穿透）
- [x] **双路径网络** - ✅ 已完成（IPv6 + STUN 智能故障转移）
- [x] **实时连接监控** - ✅ 已完成（每条路径健康状态）
- [x] **动态路由更新** - ✅ 已完成（通过 KeepAlive 实时同步，无需重启）
- [ ] Linux systemd 集成
- [ ] 基于 Web 的管理仪表板
- [ ] 移动端和桌面客户端（Android/iOS/Windows/MacOS）
- [ ] QUIC 协议支持
- [ ] Docker 容器镜像
- [ ] Kubernetes operator
- [ ] 自动更新机制
- [ ] Windows 服务支持

## 🙏 致谢

- 基于 [Tokio](https://tokio.rs/) 异步运行时构建
- 加密由 [RustCrypto](https://github.com/RustCrypto) 提供
- TUN/TAP 接口通过 [tun-rs](https://github.com/meh/rust-tun) 实现

## 📞 联系

- Issues：[GitHub Issues](https://github.com/smartethnet/rustun/issues)
- 讨论：[GitHub Discussions](https://github.com/smartethnet/rustun/discussions)

---

**注意**：这是一个实验性项目。在生产环境中使用需自行承担风险。
