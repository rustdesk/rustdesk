#!/bin/bash
set -e

# RustDesk Clean Build Script
# Removes all build artifacts and generated files for a fresh build

echo "==========================================="
echo "RustDesk Clean Build Script"
echo "==========================================="

cd $(dirname $0)/..
WORKSPACE=$(pwd)

echo "Workspace: $WORKSPACE"
echo ""

# Parse command line arguments
CLEAN_ALL=false
CLEAN_RUST=false
CLEAN_FLUTTER=false
CLEAN_BRIDGE=false
CLEAN_VCPKG=false

if [ $# -eq 0 ]; then
    CLEAN_ALL=true
else
    for arg in "$@"; do
        case $arg in
            --all)
                CLEAN_ALL=true
                ;;
            --rust)
                CLEAN_RUST=true
                ;;
            --flutter)
                CLEAN_FLUTTER=true
                ;;
            --bridge)
                CLEAN_BRIDGE=true
                ;;
            --vcpkg)
                CLEAN_VCPKG=true
                ;;
            --help)
                echo "Usage: $0 [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --all         Clean everything (default if no options specified)"
                echo "  --rust        Clean Rust build artifacts (target/)"
                echo "  --flutter     Clean Flutter build artifacts"
                echo "  --bridge      Clean generated bridge files"
                echo "  --vcpkg       Clean vcpkg build artifacts"
                echo "  --help        Show this help message"
                echo ""
                echo "Examples:"
                echo "  $0                    # Clean everything"
                echo "  $0 --rust --flutter   # Clean only Rust and Flutter"
                echo "  $0 --bridge           # Clean only bridge files"
                exit 0
                ;;
            *)
                echo "Unknown option: $arg"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done
fi

# Clean Rust build artifacts
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_RUST" = true ]; then
    echo "Cleaning Rust build artifacts..."
    if [ -d "target" ]; then
        rm -rf target
        echo "✓ Removed target/"
    else
        echo "✓ target/ already clean"
    fi

    # Clean Cargo.lock if it exists (optional, uncomment if needed)
    # if [ -f "Cargo.lock" ]; then
    #     rm -f Cargo.lock
    #     echo "✓ Removed Cargo.lock"
    # fi
fi

# Clean Flutter build artifacts
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_FLUTTER" = true ]; then
    echo ""
    echo "Cleaning Flutter build artifacts..."

    if [ -d "flutter/build" ]; then
        rm -rf flutter/build
        echo "✓ Removed flutter/build/"
    else
        echo "✓ flutter/build/ already clean"
    fi

    if [ -d "flutter/.dart_tool" ]; then
        rm -rf flutter/.dart_tool
        echo "✓ Removed flutter/.dart_tool/"
    else
        echo "✓ flutter/.dart_tool/ already clean"
    fi

    if [ -d "flutter/linux/flutter/ephemeral" ]; then
        rm -rf flutter/linux/flutter/ephemeral
        echo "✓ Removed flutter/linux/flutter/ephemeral/"
    fi

    # Clean Flutter pub cache for this project
    if [ -f "flutter/pubspec.lock" ]; then
        cd flutter
        flutter clean > /dev/null 2>&1 || true
        cd ..
        echo "✓ Ran flutter clean"
    fi
fi

# Clean generated bridge files
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_BRIDGE" = true ]; then
    echo ""
    echo "Cleaning generated bridge files..."

    if [ -f "flutter/lib/generated_bridge.dart" ]; then
        rm -f flutter/lib/generated_bridge.dart
        echo "✓ Removed flutter/lib/generated_bridge.dart"
    else
        echo "✓ generated_bridge.dart already clean"
    fi

    if [ -f "src/bridge_generated.rs" ]; then
        rm -f src/bridge_generated.rs
        echo "✓ Removed src/bridge_generated.rs"
    else
        echo "✓ bridge_generated.rs already clean"
    fi

    if [ -f "flutter/lib/generated_bridge.freezed.dart" ]; then
        rm -f flutter/lib/generated_bridge.freezed.dart
        echo "✓ Removed flutter/lib/generated_bridge.freezed.dart"
    fi

    if [ -f "flutter/lib/generated_bridge.g.dart" ]; then
        rm -f flutter/lib/generated_bridge.g.dart
        echo "✓ Removed flutter/lib/generated_bridge.g.dart"
    fi
fi

# Clean vcpkg build artifacts
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_VCPKG" = true ]; then
    echo ""
    echo "Cleaning vcpkg build artifacts..."

    if [ -n "$VCPKG_ROOT" ] && [ -d "$VCPKG_ROOT/buildtrees" ]; then
        echo "Cleaning $VCPKG_ROOT/buildtrees/..."
        rm -rf "$VCPKG_ROOT/buildtrees"
        echo "✓ Removed vcpkg buildtrees/"
    else
        echo "✓ vcpkg buildtrees already clean or VCPKG_ROOT not set"
    fi

    if [ -n "$VCPKG_ROOT" ] && [ -d "$VCPKG_ROOT/downloads" ]; then
        echo "Cleaning $VCPKG_ROOT/downloads/..."
        rm -rf "$VCPKG_ROOT/downloads"
        echo "✓ Removed vcpkg downloads/"
    fi
fi

# Clean build output packages
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_RUST" = true ] || [ "$CLEAN_FLUTTER" = true ]; then
    echo ""
    echo "Cleaning build output packages..."

    rm -f rustdesk*.deb 2>/dev/null || true
    rm -f rustdesk*.rpm 2>/dev/null || true
    rm -f rustdesk*.pkg.tar.zst 2>/dev/null || true
    rm -f rustdesk*.dmg 2>/dev/null || true
    rm -f rustdesk*-install.exe 2>/dev/null || true
    rm -f rustdesk_portable.exe 2>/dev/null || true

    echo "✓ Removed build output packages"
fi

echo ""
echo "==========================================="
echo "Clean complete!"
echo "==========================================="
echo ""
echo "To rebuild, run:"
echo "  ./ci_build.sh           # Full CI build"
echo "  ./rust_build.sh         # Rust only"
echo "  ./flutter_build.sh      # Flutter only (requires Rust build first)"
echo ""
