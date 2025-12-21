# Build Scripts

This directory contains build scripts for cross-platform compilation.

## Prerequisites

Install build tools for cross-compilation:

**For Linux/macOS/Windows (non-MSVC):**
```bash
cargo install cross --git https://github.com/cross-rs/cross
```

**For Windows MSVC:**
```bash
cargo install cargo-xwin
```

Note: 
- Docker is required for `cross` compilation
- `cargo-xwin` allows building Windows MSVC targets from Linux/macOS without Docker

## Binaries

This project builds two binaries:
- **server** - VPN server
- **client** - VPN client

## Scripts

### `build.sh` - Full Cross-Platform Build

Builds **both server and client** for all supported platforms:

```bash
./build.sh [version]
```

**Supported targets:**
- Linux x86_64 (glibc)
- Linux ARM64 (glibc)
- Linux x86_64 (musl, static)
- Linux ARM64 (musl, static)
- macOS Intel
- macOS Apple Silicon
- Windows x86_64 (MSVC)

**Output:**
- `build/` - Unpacked binaries with examples
- `dist/` - Compressed archives (`.tar.gz` / `.zip`)
- `dist/SHA256SUMS` - Checksums for verification

**Example:**
```bash
./build.sh v1.0.0
```

### `build-quick.sh` - Current Platform Build (Both Binaries)

Quick build for **both server and client** on current platform:

```bash
./build-quick.sh [version]
```

### `build-single.sh` - Single Binary Build

Build only server or client for current platform:

```bash
./build-single.sh [version] <server|client>
```

**Examples:**
```bash
./build-single.sh v1.0.0 server  # Build only server
./build-single.sh v1.0.0 client  # Build only client
```

**Use cases:**
- Local testing
- Fast iteration
- CI builds for specific platforms

## Build Artifacts

Each archive contains:
- `server` - Server binary
- `client` - Client binary
- `README.md` - Documentation
- `server.toml.example` - Server configuration example
- `routes.json.example` - Client routes example

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Release Build
on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: ./build.sh ${{ github.ref_name }}
      - uses: actions/upload-artifact@v3
        with:
          name: binaries
          path: dist/*
```

## Manual Build for Specific Target

**Linux/macOS targets:**
```bash
# Install target
rustup target add x86_64-unknown-linux-musl

# Build with cross
cross build --release --target x86_64-unknown-linux-musl
```

**Windows MSVC target:**
```bash
# Install target
rustup target add x86_64-pc-windows-msvc

# Build with cargo-xwin
cargo xwin build --release --target x86_64-pc-windows-msvc
```

## Troubleshooting

**Docker errors with cross:**
- Ensure Docker is running
- Try: `docker pull ghcr.io/cross-rs/x86_64-unknown-linux-gnu:latest`

**cargo-xwin errors:**
- Ensure cargo-xwin is installed: `cargo install cargo-xwin`
- The tool automatically downloads Windows SDK headers

**macOS targets on Linux:**
- macOS cross-compilation requires osxcross or macOS host
- Consider building macOS binaries on macOS runners in CI

**Missing dependencies:**
```bash
# Install all required tools
cargo install cross --git https://github.com/cross-rs/cross
cargo install cargo-xwin
```

