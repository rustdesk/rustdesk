#!/usr/bin/env bash
#
# build-ios.sh — clean → unsigned iOS IPA, single command.
#
# Encodes the eight-step pipeline proved by the Phase 0 spike (see
# SPIKE_NOTES.md §"Day 2 build invariants"). Designed to be re-runnable
# and idempotent: if a step's output already exists, the step is skipped.
#
# Output: build/ios/iphoneos/Runner.app (unsigned). Open Runner.xcworkspace
# in Xcode to sign and deploy, or run `flutter build ipa --release` after
# this completes for a signed IPA (requires ExportOptions.plist).
#
# Environment overrides:
#   FLUTTER_VERSION       (default: read from .fvmrc)
#   IOS_DEPLOYMENT_TARGET (default: 13.0 — required for cargo link step)
#   FRB_VERSION           (default: 1.80.1 — match flutter/pubspec.yaml)
#   SKIP_VCPKG=1          skip vcpkg install (assume already built)
#   SKIP_CODEGEN=1        skip flutter_rust_bridge codegen
#   SKIP_RUST=1           skip cargo build
#   SKIP_FLUTTER=1        skip flutter build ios

set -euo pipefail

cd "$(dirname "$0")/.."
REPO_ROOT="$(pwd)"

IOS_DEPLOYMENT_TARGET="${IOS_DEPLOYMENT_TARGET:-13.0}"
FRB_VERSION="${FRB_VERSION:-1.80.1}"
VCPKG_DIR="${VCPKG_ROOT:-$HOME/vcpkg}"

log() { printf '\033[1;34m[build-ios]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[build-ios] %s\033[0m\n' "$*" >&2; exit 1; }

# ── 1. Submodules ────────────────────────────────────────────────────────────
log "Step 1/8 — submodules"
if [ ! -f libs/hbb_common/Cargo.toml ]; then
  git submodule update --init --recursive
else
  log "submodules already initialized"
fi

# ── 2. Flutter SDK + patch ───────────────────────────────────────────────────
log "Step 2/8 — Flutter SDK (fvm) + dropdown_menu patch"
command -v fvm >/dev/null || die "fvm not installed (brew install fvm)"
[ -f .fvmrc ] || die ".fvmrc missing — cannot determine pinned Flutter version"
FLUTTER_VERSION="${FLUTTER_VERSION:-$(grep -oE '"flutter"[[:space:]]*:[[:space:]]*"[^"]*"' .fvmrc | sed -E 's/.*"([^"]+)"$/\1/')}"
[ -n "$FLUTTER_VERSION" ] || die "could not parse Flutter version from .fvmrc"

if [ ! -d "$HOME/fvm/versions/$FLUTTER_VERSION" ]; then
  fvm install "$FLUTTER_VERSION"
fi

PATCH_PATH="$REPO_ROOT/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff"
PATCHED_MARKER="$HOME/fvm/versions/$FLUTTER_VERSION/.tabby-patched"
if [ -f "$PATCH_PATH" ] && [ ! -f "$PATCHED_MARKER" ]; then
  if ( cd "$HOME/fvm/versions/$FLUTTER_VERSION" && git apply --check "$PATCH_PATH" ) 2>/dev/null; then
    log "applying Flutter dropdown_menu patch to $FLUTTER_VERSION"
    ( cd "$HOME/fvm/versions/$FLUTTER_VERSION" \
      && git apply "$PATCH_PATH" \
      && touch "$PATCHED_MARKER" )
  elif ( cd "$HOME/fvm/versions/$FLUTTER_VERSION" && git apply --reverse --check "$PATCH_PATH" ) 2>/dev/null; then
    log "Flutter patch already applied; recording marker"
    touch "$PATCHED_MARKER"
  else
    die "Flutter patch neither applies nor is already applied — Flutter SDK at $HOME/fvm/versions/$FLUTTER_VERSION may have unexpected state"
  fi
else
  log "Flutter patch already applied (marker present)"
fi

# ── 3. vcpkg manifest install + symlinks ─────────────────────────────────────
if [ "${SKIP_VCPKG:-0}" != "1" ]; then
  log "Step 3/8 — vcpkg arm64-ios manifest install"
  [ -d "$VCPKG_DIR" ] || die "vcpkg not found at $VCPKG_DIR (clone microsoft/vcpkg there)"
  [ -x "$VCPKG_DIR/vcpkg" ] || ( cd "$VCPKG_DIR" && ./bootstrap-vcpkg.sh -disableMetrics )

  if [ ! -d "vcpkg_installed/arm64-ios/lib" ]; then
    VCPKG_ROOT="$VCPKG_DIR" "$VCPKG_DIR/vcpkg" install --triplet=arm64-ios
  else
    log "vcpkg arm64-ios already built"
  fi

  # magnum-opus and hwcodec hardcode VCPKG_ROOT/installed/<triplet>/ and
  # ignore VCPKG_INSTALLED_ROOT. Symlink so they find the manifest install.
  for t in arm64-ios arm64-osx; do
    if [ -d "vcpkg_installed/$t" ] && [ ! -e "$VCPKG_DIR/installed/$t" ]; then
      ln -s "$REPO_ROOT/vcpkg_installed/$t" "$VCPKG_DIR/installed/$t"
    fi
  done
else
  log "Step 3/8 — vcpkg (skipped via SKIP_VCPKG=1)"
fi

# ── 4. flutter_rust_bridge codegen ───────────────────────────────────────────
if [ "${SKIP_CODEGEN:-0}" != "1" ]; then
  log "Step 4/8 — flutter_rust_bridge_codegen $FRB_VERSION"
  if ! command -v flutter_rust_bridge_codegen >/dev/null \
     || ! flutter_rust_bridge_codegen --version 2>&1 | grep -q "$FRB_VERSION"; then
    cargo install flutter_rust_bridge_codegen --version "$FRB_VERSION" --force
  fi

  if [ ! -f flutter/lib/generated_bridge.dart ] \
     || [ src/flutter_ffi.rs -nt flutter/lib/generated_bridge.dart ]; then
    PATH="$REPO_ROOT/.fvm/flutter_sdk/bin:$PATH" \
      flutter_rust_bridge_codegen \
        --rust-input src/flutter_ffi.rs \
        --dart-output flutter/lib/generated_bridge.dart \
        --c-output flutter/ios/Runner/bridge_generated.h
  else
    log "bindings up-to-date"
  fi
else
  log "Step 4/8 — codegen (skipped via SKIP_CODEGEN=1)"
fi

# ── 5. Flutter pub get ───────────────────────────────────────────────────────
log "Step 5/8 — flutter pub get"
( cd flutter && fvm flutter pub get )

# ── 6. Cargo build (Rust → librustdesk.a) ────────────────────────────────────
if [ "${SKIP_RUST:-0}" != "1" ]; then
  log "Step 6/8 — cargo build aarch64-apple-ios (IPHONEOS_DEPLOYMENT_TARGET=$IOS_DEPLOYMENT_TARGET)"
  IPHONEOS_DEPLOYMENT_TARGET="$IOS_DEPLOYMENT_TARGET" \
    VCPKG_ROOT="$VCPKG_DIR" \
    VCPKG_INSTALLED_ROOT="$REPO_ROOT/vcpkg_installed" \
    cargo build --features flutter,hwcodec --release --target aarch64-apple-ios --lib

  [ -f target/aarch64-apple-ios/release/liblibrustdesk.a ] \
    || die "liblibrustdesk.a was not produced"
else
  log "Step 6/8 — cargo (skipped via SKIP_RUST=1)"
fi

# ── 7. Flutter build ios ─────────────────────────────────────────────────────
if [ "${SKIP_FLUTTER:-0}" != "1" ]; then
  log "Step 7/8 — flutter build ios --release --no-codesign"
  ( cd flutter && fvm flutter build ios --release --no-codesign )
else
  log "Step 7/8 — flutter build ios (skipped via SKIP_FLUTTER=1)"
fi

# ── 8. Verify outputs ────────────────────────────────────────────────────────
log "Step 8/8 — verify"
APP_PATH="flutter/build/ios/iphoneos/Runner.app"
[ -d "$APP_PATH" ] || die "expected $APP_PATH to exist"
APP_SIZE=$(du -sh "$APP_PATH" | cut -f1)

cat <<EOF

\033[1;32m[build-ios] Success.\033[0m

  Unsigned app bundle:  $APP_PATH ($APP_SIZE)
  Static lib:           target/aarch64-apple-ios/release/liblibrustdesk.a

To sign and run on device:
  open flutter/ios/Runner.xcworkspace
  # In Xcode: Edit Scheme → Run → Build Configuration: Release
  # Destination: select the physical iPhone (USB icon, not the simulator)
  # ▶

To produce a signed IPA (Phase 5 work — currently still requires Xcode UI
or a fastlane setup):
  ( cd flutter && fvm flutter build ipa --release )

EOF
