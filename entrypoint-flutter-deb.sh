#!/bin/sh
set -e
cd /home/user/rustdesk || exit 1

# shellcheck source=/dev/null
. /usr/local/cargo/env
export VCPKG_ROOT=/opt/vcpkg
export VCPKG_FORCE_SYSTEM_BINARIES=1
export PATH="/opt/flutter/bin:/usr/local/bin:${PATH}"
export CARGO_INCREMENTAL=0

git config --global --add safe.directory /home/user/rustdesk || true
git config --global --add safe.directory /opt/flutter || true

# Generated files are gitignored; CI uploads them as the bridge artifact.
(cd flutter && flutter pub get)
flutter_rust_bridge_codegen \
  --rust-input ./src/flutter_ffi.rs \
  --dart-output ./flutter/lib/generated_bridge.dart \
  --c-output ./flutter/macos/Runner/bridge_generated.h
cp ./flutter/macos/Runner/bridge_generated.h ./flutter/ios/Runner/bridge_generated.h

python3 ./build.py --flutter --hwcodec --unix-file-copy-paste "$@"

if [ -n "${PUID}" ]; then
  chown -R "${PUID}:${PGID:-${PUID}}" rustdesk*.deb target flutter/build 2>/dev/null || true
fi
ls -lh rustdesk*.deb
