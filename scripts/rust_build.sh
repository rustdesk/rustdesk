#!/bin/bash
set -e

# RustDesk Rust Library Build Script
# Builds only the Rust library component

echo "==========================================="
echo "RustDesk Rust Library Build Script"
echo "==========================================="

# Environment setup
export RUST_VERSION="1.75"
export TARGET="x86_64-unknown-linux-gnu"
export VCPKG_ROOT=/opt/vcpkg
export JOBS=""  # empty for x86_64, "--jobs 3" for aarch64

cd $(dirname $0)/..
WORKSPACE=$(pwd)

echo "Workspace: $WORKSPACE"
echo "VCPKG_ROOT: $VCPKG_ROOT"
echo ""

# Verify environment
if [ ! -d "$VCPKG_ROOT" ]; then
    echo "ERROR: vcpkg not found at $VCPKG_ROOT"
    echo "Please run ./ci_setup.sh first"
    exit 1
fi

# Verify Rust installation
if ! command -v cargo &> /dev/null; then
    echo "ERROR: Rust/cargo not found"
    echo "Please run ./ci_setup.sh first"
    exit 1
fi

RUST_TOOLCHAIN_VERSION=$(cargo --version | awk '{print $2}')
echo "Rust version: $RUST_TOOLCHAIN_VERSION"

# Step 1: Configure git
echo ""
echo "Step 1: Configuring git..."
git config --global --add safe.directory "*"
echo "✓ Git configured"

# Step 2: Configure cargo
echo ""
echo "Step 2: Configuring cargo..."
mkdir -p ~/.cargo/
cat > ~/.cargo/config << 'EOF'
[source.crates-io]
registry = 'https://github.com/rust-lang/crates.io-index'
EOF
cat ~/.cargo/config
echo "✓ Cargo configured"

# Step 3: Build Rust library
echo ""
echo "Step 3: Building Rust library..."
echo "This will take 20-40 minutes on first build..."
cd $WORKSPACE
export PATH="$HOME/.cargo/bin:$PATH"

cargo build --lib $JOBS --features hwcodec,flutter,unix-file-copy-paste --release

echo "✓ Rust library built at: $WORKSPACE/target/release/liblibrustdesk.so"

# Clean up build artifacts to save space
echo ""
echo "Cleaning up build artifacts..."
rm -rf $WORKSPACE/target/release/deps target/release/build
echo "✓ Cleanup complete"

# Summary
echo ""
echo "==========================================="
echo "Rust Build Complete!"
echo "==========================================="
echo ""
echo "Built library:"
ls -lh $WORKSPACE/target/release/liblibrustdesk.so
echo ""
echo "Next step: Run ./flutter_build.sh to build the Flutter application"
echo ""
