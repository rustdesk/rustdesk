#!/bin/bash
set -e

# RustDesk CI-based Build Script
# Strictly follows .github/workflows/flutter-build.yml build-rustdesk-linux Docker run section

echo "==========================================="
echo "RustDesk CI Build Script (x86_64)"
echo "==========================================="

# Environment setup
export RUST_VERSION="1.75"
export FLUTTER_VERSION="3.24.5"
export VERSION="1.4.2"
export TARGET="x86_64-unknown-linux-gnu"
export VCPKG_TRIPLET="x64-linux"
export DEB_ARCH="amd64"
export ARCH="x86_64"
export VCPKG_ROOT=/opt/vcpkg
export JOBS=""  # empty for x86_64, "--jobs 3" for aarch64

cd $(dirname $0)
WORKSPACE=$(pwd)

echo "Workspace: $WORKSPACE"
echo "VCPKG_ROOT: $VCPKG_ROOT"
echo ""

# Verify Rust installation
RUST_TOOLCHAIN_VERSION=$(cargo --version | awk '{print $2}')
echo "Rust version: $RUST_TOOLCHAIN_VERSION"

# Step 1: Generate Flutter Rust Bridge (if not exists)
echo ""
echo "Step 1: Generating Flutter Rust Bridge..."
cd $WORKSPACE
./scripts/generate_bridge.sh

# Step 2: Build Rust library
echo ""
echo "Step 2: Building Rust library..."
cd $WORKSPACE
./scripts/rust_build.sh

# Step 3: Build Flutter application and packages
echo ""
echo "Step 3: Building Flutter application and packages..."
cd $WORKSPACE
./scripts/flutter_build.sh
