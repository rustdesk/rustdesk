#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
clang++ -std=c++17 tests/microphone/macos_route.mm src/microphone_forwarding/macos.mm -framework CoreAudio -framework CoreFoundation -o "$TMP_DIR/route-test"
"$TMP_DIR/route-test"
