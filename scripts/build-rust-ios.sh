#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Required to avoid __chkstk_darwin linker errors when rustc defaults to iOS 10.0
export IPHONEOS_DEPLOYMENT_TARGET="13.0"
export VCPKG_ROOT="${VCPKG_ROOT:-$HOME/vcpkg}"
export VCPKG_INSTALLED_ROOT="$REPO_ROOT/vcpkg_installed"

echo "Building Rust static library for aarch64-apple-ios..."
echo "  VCPKG_ROOT=$VCPKG_ROOT"
echo "  Output: $REPO_ROOT/target/aarch64-apple-ios/release/liblibrustdesk.a"
echo ""

cd "$REPO_ROOT"
cargo build --features flutter,hwcodec --release --target aarch64-apple-ios --lib

echo ""
echo "Done: $REPO_ROOT/target/aarch64-apple-ios/release/liblibrustdesk.a"
