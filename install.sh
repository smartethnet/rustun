#!/bin/bash

# Rustun One-Click Installation Script
# Supports: Linux (Ubuntu, Debian, CentOS, Fedora, Arch)
# Usage: curl -fsSL https://raw.githubusercontent.com/smartethnet/rustun/main/install.sh | bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Installation directory
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/rustun"

# GitHub repository
GITHUB_REPO="smartethnet/rustun"
GITHUB_API="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"

# Print functions
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        print_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

# Get latest version from GitHub
get_latest_version() {
    # If version is set by environment variable, use it
    if [[ -n "${RUSTUN_VERSION}" ]]; then
        VERSION="${RUSTUN_VERSION}"
        print_info "Using specified version: ${VERSION}"
        return
    fi
    
    print_info "Fetching latest version from GitHub..."
    
    # Try to get latest version from GitHub API
    if command -v curl >/dev/null 2>&1; then
        VERSION=$(curl -fsSL "${GITHUB_API}" | grep '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/' | head -n 1)
    fi
    
    # Fallback to default version if API call fails
    if [[ -z "${VERSION}" ]]; then
        VERSION="0.0.2"
        print_warning "Could not fetch latest version, using default: ${VERSION}"
    else
        print_success "Latest version: ${VERSION}"
    fi
    
    # Set release URL
    RELEASE_URL="https://github.com/${GITHUB_REPO}/releases/download/${VERSION}"
}

# Detect OS and architecture
detect_system() {
    print_info "Detecting system information..."
    
    # Detect OS 
    if [[ -f /etc/os-release ]]; then
        # Read OS info without polluting our VERSION variable
        OS=$(grep '^ID=' /etc/os-release | cut -d= -f2 | tr -d '"')
        OS_VERSION=$(grep '^VERSION_ID=' /etc/os-release | cut -d= -f2 | tr -d '"')
    else
        print_error "Cannot detect OS"
        exit 1
    fi
    
    # Detect architecture
    ARCH=$(uname -m)
    case $ARCH in
        x86_64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        *)
            print_error "Unsupported architecture: $ARCH"
            exit 1
            ;;
    esac
    
    # Determine musl or gnu
    if command -v ldd >/dev/null 2>&1; then
        if ldd --version 2>&1 | grep -q musl; then
            LIBC="musl"
        else
            LIBC="gnu"
        fi
    else
        LIBC="musl"
    fi
    
    TARGET="${ARCH}-unknown-linux-${LIBC}"
    PACKAGE_NAME="rustun-${VERSION}-${TARGET}"
    
    print_success "Detected: ${OS} (${OS_VERSION}) on ${ARCH} with ${LIBC}"
}

# Install dependencies
install_dependencies() {
    print_info "Installing dependencies..."
    
    case $OS in
        ubuntu|debian)
            apt-get update -qq
            apt-get install -y curl tar
            ;;
        centos|rhel|fedora)
            yum install -y curl tar
            ;;
        arch)
            pacman -Sy --noconfirm curl tar
            ;;
        *)
            print_warning "Unknown OS, assuming curl and tar are available"
            ;;
    esac
}

# Download and extract rustun
download_rustun() {
    print_info "Downloading rustun ${VERSION} for ${TARGET}..."
    
    DOWNLOAD_URL="${RELEASE_URL}/${PACKAGE_NAME}.tar.gz"
    TMP_DIR=$(mktemp -d)
    
    cd "$TMP_DIR"
    
    if ! curl -fsSL "$DOWNLOAD_URL" -o "${PACKAGE_NAME}.tar.gz"; then
        print_error "Failed to download rustun from ${DOWNLOAD_URL}"
        print_error "Please check if the version ${VERSION} exists"
        exit 1
    fi
    
    print_info "Extracting files..."
    tar -xzf "${PACKAGE_NAME}.tar.gz"
    
    cd "${PACKAGE_NAME}"
}

# Install binaries
install_binaries() {
    print_info "Installing server binary..."
    
    # Install server only
    if [[ -f server ]]; then
        install -m 755 server "${INSTALL_DIR}/rustun-server"
        print_success "Installed rustun-server to ${INSTALL_DIR}"
    else
        print_error "Server binary not found in package"
        exit 1
    fi
}

# Setup configuration
setup_config() {
    print_info "Setting up configuration..."
    
    # Create config directory
    mkdir -p "$CONFIG_DIR"
    
    # Copy example configs if they don't exist
    if [[ -f server.toml.example ]] && [[ ! -f "${CONFIG_DIR}/server.toml" ]]; then
        cp server.toml.example "${CONFIG_DIR}/server.toml"
        print_success "Created ${CONFIG_DIR}/server.toml"
    fi
    
    if [[ -f routes.json.example ]] && [[ ! -f "${CONFIG_DIR}/routes.json" ]]; then
        cp routes.json.example "${CONFIG_DIR}/routes.json"
        print_success "Created ${CONFIG_DIR}/routes.json"
    fi
    
    # Set permissions
    chmod 644 "${CONFIG_DIR}"/*.{toml,json} 2>/dev/null || true
}

# Create systemd service
create_systemd_service() {
    print_info "Creating systemd service..."
    
    cat > /etc/systemd/system/rustun-server.service <<EOF
[Unit]
Description=Rustun VPN Server
After=network.target
Documentation=https://github.com/${GITHUB_REPO}

[Service]
Type=simple
User=root
ExecStart=${INSTALL_DIR}/rustun-server ${CONFIG_DIR}/server.toml
Restart=on-failure
RestartSec=5s
StandardOutput=journal
StandardError=journal
SyslogIdentifier=rustun-server

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=${CONFIG_DIR}

# Resource limits
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    print_success "Created systemd service: rustun-server.service"
}

# Cleanup
cleanup() {
    print_info "Cleaning up..."
    cd /
    rm -rf "$TMP_DIR"
}

# Main installation flow
main() {
    echo ""
    echo "╔══════════════════════════════════════════════════════════╗"
    echo "║         Rustun VPN Server Installation Script           ║"
    echo "║              Version: ${VERSION}                              ║"
    echo "╚══════════════════════════════════════════════════════════╝"
    echo ""
    
    check_root
    get_latest_version
    detect_system
    install_dependencies
    download_rustun
    install_binaries
    setup_config
    create_systemd_service
    cleanup
    
    echo ""
    print_success "Installation completed successfully!"
    echo ""
    echo -e "${BLUE}Next steps:${NC}"
    echo "  1. Edit server configuration: vim ${CONFIG_DIR}/server.toml"
    echo "  2. Edit routes configuration: vim ${CONFIG_DIR}/routes.json"
    echo "  3. Start server: systemctl start rustun-server"
    echo "  4. Enable auto-start: systemctl enable rustun-server"
    echo "  5. Check status: systemctl status rustun-server"
    echo "  6. View logs: journalctl -u rustun-server -f"
    echo ""
    echo -e "${YELLOW}Important:${NC}"
    echo "  - Configure your routes.json before starting the server"
    echo "  - Open port 8080 (or your configured port) in firewall"
    echo "  - Server requires root privileges to create TUN devices"
    echo ""
}

# Trap errors and cleanup
trap cleanup EXIT

# Run main installation
main "$@"

