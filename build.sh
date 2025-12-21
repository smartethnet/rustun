#!/bin/bash

set -e

VERSION=${1:-$(git describe --tags --always --dirty 2>/dev/null || echo "dev")}
PROJECT_NAME="rustun"
BUILD_DIR="build"
DIST_DIR="dist"
BINARIES=("server" "client")

echo "Building ${PROJECT_NAME} version ${VERSION}"
echo "Binaries: ${BINARIES[@]}"

# Clean previous builds
rm -rf ${BUILD_DIR} ${DIST_DIR}
mkdir -p ${BUILD_DIR} ${DIST_DIR}

# Check if cross is installed
if ! command -v cross &> /dev/null; then
    echo "Installing cross..."
    cargo install cross --git https://github.com/cross-rs/cross
fi

# Check if cargo-xwin is installed (for Windows MSVC)
if ! command -v cargo-xwin &> /dev/null; then
    echo "Installing cargo-xwin..."
    cargo install cargo-xwin
fi

# Define build targets
TARGETS=(
    "x86_64-unknown-linux-gnu"      # Linux x86_64
    "aarch64-unknown-linux-gnu"     # Linux ARM64
    "x86_64-unknown-linux-musl"     # Linux x86_64 (static)
    "aarch64-unknown-linux-musl"    # Linux ARM64 (static)
    "x86_64-apple-darwin"           # macOS Intel
    "aarch64-apple-darwin"          # macOS Apple Silicon
    "x86_64-pc-windows-msvc"        # Windows x86_64
)

# Build for each target
for target in "${TARGETS[@]}"; do
    echo ""
    echo "========================================="
    echo "Building for ${target}"
    echo "========================================="
    
    # Use different build tools based on target
    if [[ "$target" == "x86_64-pc-windows-msvc" ]]; then
        # Use cargo-xwin for Windows MSVC
        cargo xwin build --release --target ${target}
    elif [[ "$OSTYPE" == "darwin"* ]] && [[ "$target" == *"darwin"* ]]; then
        # Use cargo for macOS targets on macOS
        cargo build --release --target ${target}
    else
        # Use cross for other targets
        cross build --release --target ${target}
    fi
    
    # Determine binary extension
    if [[ "$target" == *"windows"* ]]; then
        EXT=".exe"
    else
        EXT=""
    fi
    
    # Create target directory
    TARGET_DIR="${BUILD_DIR}/${PROJECT_NAME}-${VERSION}-${target}"
    mkdir -p ${TARGET_DIR}
    
    # Copy binaries
    for binary in "${BINARIES[@]}"; do
        if [ -f "target/${target}/release/${binary}${EXT}" ]; then
            cp target/${target}/release/${binary}${EXT} ${TARGET_DIR}/
            echo "  ✓ Copied ${binary}${EXT}"
        else
            echo "  ✗ Warning: ${binary}${EXT} not found"
        fi
    done
    
    # Copy additional files
    cp README.md ${TARGET_DIR}/ 2>/dev/null || true
    cp etc/server.toml ${TARGET_DIR}/server.toml.example 2>/dev/null || true
    cp etc/routes.json ${TARGET_DIR}/routes.json.example 2>/dev/null || true
    
    # Create archive
    echo "Creating archive for ${target}..."
    cd ${BUILD_DIR}
    if [[ "$target" == *"windows"* ]]; then
        zip -r ../${DIST_DIR}/${PROJECT_NAME}-${VERSION}-${target}.zip ${PROJECT_NAME}-${VERSION}-${target}
    else
        tar czf ../${DIST_DIR}/${PROJECT_NAME}-${VERSION}-${target}.tar.gz ${PROJECT_NAME}-${VERSION}-${target}
    fi
    cd ..
    
    echo "✓ Built ${target}"
done

echo ""
echo "========================================="
echo "Build complete!"
echo "========================================="
echo "Artifacts in ${DIST_DIR}:"
ls -lh ${DIST_DIR}

echo ""
echo "Checksums:"
cd ${DIST_DIR}
shasum -a 256 * > SHA256SUMS
cat SHA256SUMS
cd ..

echo ""
echo "Done!"

