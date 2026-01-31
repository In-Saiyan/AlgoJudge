#!/bin/bash
# Build all benchmark container images

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IMAGE_PREFIX="algojudge"

echo "Building AlgoJudge benchmark images..."

# Build C image
echo "Building C image..."
docker build -t ${IMAGE_PREFIX}/c:latest "$SCRIPT_DIR/c"

# Build C++ image
echo "Building C++ image..."
docker build -t ${IMAGE_PREFIX}/cpp:latest "$SCRIPT_DIR/cpp"

# Build Rust image
echo "Building Rust image..."
docker build -t ${IMAGE_PREFIX}/rust:latest "$SCRIPT_DIR/rust"

# Build Go image
echo "Building Go image..."
docker build -t ${IMAGE_PREFIX}/go:latest "$SCRIPT_DIR/go"

# Build Zig image
echo "Building Zig image..."
docker build -t ${IMAGE_PREFIX}/zig:latest "$SCRIPT_DIR/zig"

# Build Python image
echo "Building Python image..."
docker build -t ${IMAGE_PREFIX}/python:latest "$SCRIPT_DIR/python"

echo ""
echo "All images built successfully!"
echo ""
echo "Images:"
docker images | grep ${IMAGE_PREFIX}
