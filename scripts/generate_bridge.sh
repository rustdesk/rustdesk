#!/bin/bash
set -e

# Flutter Rust Bridge Generation Script
# Installs flutter_rust_bridge_codegen if needed and generates bridge files

FLUTTER_RUST_BRIDGE_VERSION="1.80.1"
BRIDGE_OUTPUT="flutter/lib/generated_bridge.dart"

echo "==========================================="
echo "Flutter Rust Bridge Generator"
echo "==========================================="

cd $(dirname $0)/..
WORKSPACE=$(pwd)

# Check if bridge file already exists
if [ -f "$BRIDGE_OUTPUT" ]; then
    echo "✓ Bridge file already exists at $BRIDGE_OUTPUT"
    exit 0
fi

echo "Bridge file not found. Generating..."

# Step 1: Check if flutter_rust_bridge_codegen is installed
if ! command -v flutter_rust_bridge_codegen &> /dev/null; then
    echo "Step 1: Installing flutter_rust_bridge_codegen v${FLUTTER_RUST_BRIDGE_VERSION}..."
    cargo install flutter_rust_bridge_codegen --version ${FLUTTER_RUST_BRIDGE_VERSION} --features "uuid" --locked
    echo "✓ flutter_rust_bridge_codegen installed"
else
    INSTALLED_VERSION=$(flutter_rust_bridge_codegen --version 2>&1 | grep -oP 'flutter_rust_bridge_codegen \K[0-9.]+' || echo "unknown")
    echo "✓ flutter_rust_bridge_codegen already installed (version: $INSTALLED_VERSION)"
fi

# Step 2: Ensure Flutter is in PATH
if ! command -v flutter &> /dev/null; then
    if [ -d "/opt/flutter" ]; then
        echo "Adding Flutter to PATH..."
        export PATH=/opt/flutter/bin:$PATH
    else
        echo "ERROR: Flutter not found. Please install Flutter or set PATH."
        exit 1
    fi
fi

# Step 3: Ensure Flutter dependencies are installed
echo ""
echo "Step 2: Installing Flutter dependencies..."
cd $WORKSPACE/flutter
flutter pub get
cd $WORKSPACE

# Step 4: Generate bridge files
echo ""
echo "Step 3: Generating bridge files..."
cd $WORKSPACE

# Use flutter_rust_bridge_codegen from cargo bin
if [ -f "$HOME/.cargo/bin/flutter_rust_bridge_codegen" ]; then
    CODEGEN_BIN="$HOME/.cargo/bin/flutter_rust_bridge_codegen"
else
    CODEGEN_BIN="flutter_rust_bridge_codegen"
fi

$CODEGEN_BIN \
    --rust-input ./src/flutter_ffi.rs \
    --dart-output ./flutter/lib/generated_bridge.dart

echo "✓ Bridge files generated"

# Step 5: Apply FFI workaround if needed (for older flutter_rust_bridge versions)
# Note: This workaround is NOT needed for flutter_rust_bridge 1.80.1+
# The library now correctly handles the Bool type
echo ""
echo "Step 4: Checking if FFI workaround is needed..."
echo "✓ No FFI workaround needed for flutter_rust_bridge ${FLUTTER_RUST_BRIDGE_VERSION}"

echo ""
echo "==========================================="
echo "Bridge generation complete!"
echo "==========================================="
