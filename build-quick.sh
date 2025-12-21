#!/bin/bash

set -e

VERSION=${1:-$(git describe --tags --always --dirty 2>/dev/null || echo "dev")}
PROJECT_NAME="rustun"
DIST_DIR="dist"
BINARIES=("server" "client")

echo "Quick build for current platform"
echo "Binaries: ${BINARIES[@]}"

cargo build --release

mkdir -p ${DIST_DIR}

# Detect current platform
case "$OSTYPE" in
    linux*)   PLATFORM="linux" ;;
    darwin*)  PLATFORM="macos" ;;
    msys*|cygwin*|win32) PLATFORM="windows" ;;
    *) PLATFORM="unknown" ;;
esac

# Detect architecture
ARCH=$(uname -m)

if [[ "$PLATFORM" == "windows" ]]; then
    EXT=".exe"
else
    EXT=""
fi

OUTPUT_NAME="${PROJECT_NAME}-${VERSION}-${PLATFORM}-${ARCH}"
mkdir -p ${DIST_DIR}/${OUTPUT_NAME}

# Copy binaries
for binary in "${BINARIES[@]}"; do
    if [ -f "target/release/${binary}${EXT}" ]; then
        cp target/release/${binary}${EXT} ${DIST_DIR}/${OUTPUT_NAME}/
        echo "  ✓ Copied ${binary}${EXT}"
    else
        echo "  ✗ Warning: ${binary}${EXT} not found"
    fi
done
cp README.md ${DIST_DIR}/${OUTPUT_NAME}/ 2>/dev/null || true
cp etc/server.toml ${DIST_DIR}/${OUTPUT_NAME}/server.toml.example 2>/dev/null || true
cp etc/routes.json ${DIST_DIR}/${OUTPUT_NAME}/routes.json.example 2>/dev/null || true

cd ${DIST_DIR}
if [[ "$PLATFORM" == "windows" ]]; then
    zip -r ${OUTPUT_NAME}.zip ${OUTPUT_NAME}
else
    tar czf ${OUTPUT_NAME}.tar.gz ${OUTPUT_NAME}
fi
cd ..

echo "✓ Built for ${PLATFORM}-${ARCH}"
echo "Output: ${DIST_DIR}/${OUTPUT_NAME}.tar.gz"

