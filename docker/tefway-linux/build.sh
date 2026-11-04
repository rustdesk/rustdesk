#!/usr/bin/env bash
# Build a portable Tefway Remote AppImage in a rootless Podman container.
#
# Host usage (from anywhere in the repository):
#   docker/tefway-linux/build.sh --profile client
#   docker/tefway-linux/build.sh --profile master
#
# The host side only builds/uses the image and runs one container at a time.
# The container side performs the Rust build, the Flutter build and the direct
# AppDir assembly (no .deb intermediate).
set -euo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
readonly DEFAULT_IMAGE="localhost/tefway-linux-builder:bookworm-rust1.99.1-flutter3.44.8"

PROFILE="${TEFWAY_PROFILE:-client}"
IMAGE="${TEFWAY_LINUX_IMAGE:-$DEFAULT_IMAGE}"
SKIP_IMAGE=0
FORCE_BRIDGE=0
EXTRA_FEATURES="${TEFWAY_EXTRA_FEATURES:-}"
INSIDE=0

usage() {
  cat <<'EOF'
Usage: docker/tefway-linux/build.sh [options]

Host options:
  --profile client|master   Product profile to build (default: client)
  --image NAME:TAG          Builder image name (default: localhost/tefway-linux-builder:...)
  --skip-image              Do not run podman build; require the image to exist
  --features LIST           Extra cargo features (comma separated)
  --force-bridge            Regenerate the flutter_rust_bridge files
  -h, --help                This help

The builder runs exactly one container. It refuses to start while another
build of this tree holds the lock. No systemd unit, no --privileged, no global
SELinux or Wayland change is performed.
EOF
}

log() { printf '[tefway-linux] %s\n' "$*"; }
die() { printf '[tefway-linux] ERROR: %s\n' "$*" >&2; exit 1; }

parse_args() {
  while (($#)); do
    case "$1" in
      --profile)
        [[ $# -ge 2 ]] || die "--profile needs a value"
        PROFILE="$2"
        shift 2
        ;;
      --image)
        [[ $# -ge 2 ]] || die "--image needs a value"
        IMAGE="$2"
        shift 2
        ;;
      --skip-image)
        SKIP_IMAGE=1
        shift
        ;;
      --features)
        [[ $# -ge 2 ]] || die "--features needs a value"
        EXTRA_FEATURES="$2"
        shift 2
        ;;
      --force-bridge)
        FORCE_BRIDGE=1
        shift
        ;;
      --inside)
        INSIDE=1
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "unknown argument: $1"
        ;;
    esac
  done
  [[ "$PROFILE" == "client" || "$PROFILE" == "master" ]] \
    || die "--profile must be client or master"
}

profile_values() {
  case "$PROFILE" in
    client)
      PKG="tefway-remote-client"
      PRODUCT="Tefway Remote Client"
      EXE="TefwayRemoteClient"
      ;;
    master)
      PKG="tefway-remote-master"
      PRODUCT="Tefway Remote Master"
      EXE="TefwayRemoteMaster"
      ;;
  esac
  ARTIFACT="${EXE}-Linux-x86_64.AppImage"
  LEGACY_ARTIFACT="${EXE}-x86_64.AppImage"
  OUT="${REPO_ROOT}/build/tefway/${PROFILE}"
}

host_main() {
  command -v podman >/dev/null 2>&1 || die "podman is not installed"
  [[ -f "${REPO_ROOT}/Cargo.toml" ]] || die "not a Tefway Remote checkout: ${REPO_ROOT}"

  mkdir -p "${REPO_ROOT}/build/tefway"
  local lock="${REPO_ROOT}/build/tefway/.build.lock"
  exec 9>"$lock"
  if ! flock -n 9; then
    die "another Tefway Linux build is running (${lock})"
  fi

  if ((SKIP_IMAGE == 0)); then
    log "building builder image ${IMAGE}"
    podman build --pull=missing \
      --ignorefile "${SCRIPT_DIR}/containerignore" \
      -t "$IMAGE" -f "${SCRIPT_DIR}/Dockerfile" "$REPO_ROOT"
  else
    podman image exists "$IMAGE" || die "builder image ${IMAGE} not found and --skip-image was given"
  fi

  # Snapshot the host library set. The container uses it to bundle exactly the
  # runtime libraries this Fedora host lacks, instead of guessing. The small
  # always-bundle list in the container covers the non-desktop libraries that
  # other Fedora desktops may not have.
  local host_libs="${REPO_ROOT}/build/tefway/host-libs.txt"
  if command -v ldconfig >/dev/null 2>&1; then
    ldconfig -p 2>/dev/null | awk '/=>/ {print $1}' | sort -u > "$host_libs"
  else
    : > "$host_libs"
  fi

  local -a features=()
  if [[ -n "$EXTRA_FEATURES" ]]; then
    features=(--features "$EXTRA_FEATURES")
  fi

  log "building profile ${PROFILE} in ${IMAGE}"
  podman run --rm \
    --name "tefway-linux-${PROFILE}-$$" \
    --security-opt label=disable \
    --volume "${REPO_ROOT}:/work" \
    --volume "tefway-cargo-registry:/usr/local/cargo/registry" \
    --volume "tefway-cargo-git:/usr/local/cargo/git" \
    --volume "tefway-pub-cache:/root/.pub-cache" \
    --env "TEFWAY_PROFILE=${PROFILE}" \
    --env "CARGO_BUILD_JOBS=2" \
    --env "CMAKE_BUILD_PARALLEL_LEVEL=2" \
    --env "MAKEFLAGS=-j2" \
    --env "APPIMAGE_EXTRACT_AND_RUN=1" \
    --env "TEFWAY_FORCE_BRIDGE=${FORCE_BRIDGE}" \
    --env "TEFWAY_EXTRA_FEATURES=${EXTRA_FEATURES}" \
    --env "TEFWAY_BUILDER_IMAGE=${IMAGE}" \
    --env "HOME=/root" \
    "$IMAGE" /work/docker/tefway-linux/build.sh --inside --profile "$PROFILE" "${features[@]}"

  local artifact="${OUT}/${ARTIFACT}"
  [[ -f "$artifact" ]] || die "build finished but ${artifact} is missing"
  log "artifact: ${artifact}"
  log "sha256:   $(sha256sum "$artifact" | awk '{print $1}')"
}

inside_main() {
  local profile="$PROFILE"
  local pkg product exe artifact legacy out
  case "$profile" in
    client)
      pkg=tefway-remote-client
      product="Tefway Remote Client"
      exe=TefwayRemoteClient
      ;;
    master)
      pkg=tefway-remote-master
      product="Tefway Remote Master"
      exe=TefwayRemoteMaster
      ;;
    *)
      die "unsupported profile: $profile"
      ;;
  esac
  artifact="${exe}-Linux-x86_64.AppImage"
  legacy="${exe}-x86_64.AppImage"
  out="/work/build/tefway/${profile}"

  cd /work
  [[ -f Cargo.toml ]] || die "Cargo.toml not found in /work"
  if [[ ! -f libs/hbb_common/Cargo.toml ]]; then
    log "initialising libs/hbb_common submodule"
    git submodule update --init --recursive
  fi
  git config --global --add safe.directory /work
  git config --global --add safe.directory /opt/flutter

  export TEFWAY_PROFILE="$profile"
  export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
  export CMAKE_BUILD_PARALLEL_LEVEL="${CMAKE_BUILD_PARALLEL_LEVEL:-2}"
  export MAKEFLAGS="${MAKEFLAGS:--j2}"

  local features="flutter,linux-pkg-config"
  if [[ -n "${TEFWAY_EXTRA_FEATURES:-}" ]]; then
    features="${features},${TEFWAY_EXTRA_FEATURES}"
  fi

  if [[ "${TEFWAY_FORCE_BRIDGE:-0}" == "1" ]] \
     || [[ ! -f src/bridge_generated.rs ]] \
     || [[ ! -f src/bridge_generated.io.rs ]] \
     || [[ ! -f flutter/lib/generated_bridge.dart ]] \
     || [[ ! -f flutter/lib/generated_bridge.freezed.dart ]]; then
    log "generating flutter_rust_bridge 1.80.1 files"
    (cd flutter && flutter pub get)
    flutter_rust_bridge_codegen \
      --rust-input ./src/flutter_ffi.rs \
      --dart-output ./flutter/lib/generated_bridge.dart \
      --c-output ./flutter/macos/Runner/bridge_generated.h
    sed -i 's/ffi.NativeFunction<ffi.Bool Function(DartPort/ffi.NativeFunction<ffi.Uint8 Function(DartPort/g' \
      flutter/lib/generated_bridge.dart
    if [[ ! -f flutter/lib/generated_bridge.freezed.dart ]]; then
      (cd flutter && dart run build_runner build --delete-conflicting-outputs)
    fi
  else
    log "reusing existing flutter_rust_bridge files"
  fi

  # TEFWAY_PROFILE is a compile-time constant. Force build.rs to run and the
  # root crate to rebuild even when only the profile changed.
  rm -f target/release/tefway-build-manifest.json
  touch build.rs

  log "cargo build: features=${features} jobs=${CARGO_BUILD_JOBS}"
  cargo build --locked --features "$features" --lib --release

  local manifest="target/release/tefway-build-manifest.json"
  [[ -f "$manifest" ]] || die "build.rs did not write ${manifest}"
  jq -e --arg p "$profile" '.profile == $p' "$manifest" >/dev/null \
    || die "binary embeds a profile other than ${profile} (${manifest})"

  log "flutter build linux --release"
  (cd flutter && flutter build linux --release)

  local bundle="flutter/build/linux/x64/release/bundle"
  [[ -x "$bundle/rustdesk" ]] || die "Flutter bundle executable missing: $bundle/rustdesk"
  # This is the layout the Dart loader and the CMake install both expect:
  # <bundle>/lib/librustdesk.so, not <bundle>/librustdesk.so.
  [[ -f "$bundle/lib/librustdesk.so" ]] \
    || die "expected Rust core at $bundle/lib/librustdesk.so"
  [[ -f "$bundle/lib/libflutter_linux_gtk.so" ]] \
    || die "expected Flutter engine at $bundle/lib/libflutter_linux_gtk.so"
  [[ -d "$bundle/data/flutter_assets" ]] \
    || die "expected Flutter assets at $bundle/data/flutter_assets"

  assemble_appdir "$bundle" "$pkg" "$product" "$exe" "$out"
  build_appimage "$pkg" "$exe" "$out" "$artifact"
  verify_appimage "$pkg" "$exe" "$out" "$artifact"
  write_manifest "$profile" "$pkg" "$product" "$exe" "$out" "$artifact" "$features"

  ln -sf "$artifact" "$out/$legacy"
  log "appimage: $out/$artifact"
  log "alias:    $out/$legacy"
}

assemble_appdir() {
  local bundle="$1" pkg="$2" product="$3" exe="$4" out="$5"
  local appdir="$out/appimage/AppDir"
  local app_root="$appdir/usr/lib/${pkg}"

  rm -rf "$appdir"
  mkdir -p "$app_root" "$appdir/usr/share/applications" "$appdir/usr/share/icons/hicolor"

  cp -a "$bundle/." "$app_root/"
  mv "$app_root/rustdesk" "$app_root/$exe"

  # Correct the runtime layout explicitly. Anything that regresses this must
  # fail the build, not produce an AppImage that cannot load its core.
  [[ -f "$app_root/lib/librustdesk.so" ]] \
    || die "AppDir is missing usr/lib/${pkg}/lib/librustdesk.so"
  [[ -f "$app_root/lib/libflutter_linux_gtk.so" ]] \
    || die "AppDir is missing the Flutter engine library"
  [[ -d "$app_root/data/flutter_assets" ]] \
    || die "AppDir is missing data/flutter_assets"

  local size
  for size in 16 32 48 64 128 256 512; do
    local icon_dir="$appdir/usr/share/icons/hicolor/${size}x${size}/apps"
    mkdir -p "$icon_dir"
    cp "res/tefway/icon-${size}.png" "$icon_dir/${pkg}.png"
  done
  cp "res/tefway/icon-512.png" "$appdir/${pkg}.png"
  ln -sf "${pkg}.png" "$appdir/.DirIcon"

  cat > "$appdir/${pkg}.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=${product}
GenericName=Remote Desktop
Comment=Tefway Remote portable client
Exec=${exe} %u
Icon=${pkg}
Terminal=false
StartupNotify=true
Categories=Network;RemoteAccess;
Keywords=tefway;remote;desktop;support;
StartupWMClass=${pkg}
EOF

  cat > "$appdir/AppRun" <<EOF
#!/bin/sh
set -eu
APPDIR="\${APPDIR:-\$(CDPATH= cd -- "\$(dirname -- "\$0")" && pwd)}"
APP_ROOT="\$APPDIR/usr/lib/${pkg}"
export PATH="\$APP_ROOT:\$PATH"
export LD_LIBRARY_PATH="\$APP_ROOT/lib:\$APP_ROOT\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
export XDG_DATA_DIRS="\$APPDIR/usr/share:\${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
exec "\$APP_ROOT/${exe}" "\$@"
EOF
  chmod 0755 "$appdir/AppRun"

  bundle_runtime_libs "$appdir" "$app_root" "$exe"

  # Optional size reduction. The RPATH is set by flutter/linux/CMakeLists.txt.
  if command -v strip >/dev/null 2>&1; then
    strip --strip-unneeded "$app_root/lib/librustdesk.so" 2>/dev/null || true
    strip --strip-unneeded "$app_root/$exe" 2>/dev/null || true
    local lib
    for lib in "$app_root"/lib/*.so*; do
      [[ -f "$lib" ]] && strip --strip-unneeded "$lib" 2>/dev/null || true
    done
  fi
}

bundle_runtime_libs() {
  local appdir="$1" app_root="$2" exe="$3"
  local host_libs="/work/build/tefway/host-libs.txt"
  local dest="$app_root/lib"
  local -a roots=("$app_root/$exe" "$app_root/lib/librustdesk.so" "$app_root/lib/libflutter_linux_gtk.so")
  local -a all=()

  if ! command -v lddtree >/dev/null 2>&1; then
    log "lddtree not available; skipping runtime library collection"
    return 0
  fi

  mapfile -t all < <(lddtree -l "${roots[@]}" 2>/dev/null | grep '^/' | sort -u)
  [[ ${#all[@]} -gt 0 ]] || die "lddtree returned no dependencies"

  local lib base real bundled=0
  for lib in "${all[@]}"; do
    base="$(basename "$lib")"
    case "$base" in
      ld-linux*|libc.so*|libm.so*|libdl.so*|libpthread.so*|librt.so*|libresolv.so*|libnsl.so*|libutil.so*|libanl.so*|libmvec.so*|libcrypt.so*|libgcc_s.so*|libstdc++.so*)
        continue
        ;;
    esac
    [[ -e "$dest/$base" ]] && continue
    # Always ship the non-desktop codec/indicator libraries; additionally ship
    # anything this host does not already provide.
    if [[ "$base" =~ ^(libopus|libvpx|libaom|libyuv|libayatana-appindicator3|libdbusmenu|libdrmtap) ]] \
       || ! grep -qxF "$base" "$host_libs" 2>/dev/null; then
      real="$(readlink -f "$lib")"
      [[ -f "$real" ]] || continue
      cp -a "$real" "$dest/$base"
      bundled=$((bundled + 1))
    fi
  done
  log "bundled ${bundled} runtime library file(s) into lib/"
}

build_appimage() {
  local pkg="$1" exe="$2" out="$3" artifact="$4"
  local appdir="$out/appimage/AppDir"
  local target="$out/$artifact"

  rm -f "$target"
  # Deterministic archive timestamps when the checkout exposes one.
  local epoch
  epoch="$(git -C /work log -1 --format=%ct 2>/dev/null || echo 0)"
  [[ "$epoch" =~ ^[0-9]+$ ]] || epoch=0
  export SOURCE_DATE_EPOCH="$epoch"
  export ARCH=x86_64

  log "appimagetool: $target"
  /opt/appimage/appimagetool --no-appstream "$appdir" "$target"
  [[ -f "$target" ]] || die "appimagetool did not produce ${target}"
  chmod 0755 "$target"
}

verify_appimage() {
  local pkg="$1" exe="$2" out="$3" artifact="$4"
  local appimage="$out/$artifact"
  local tmp
  tmp="$(mktemp -d)"

  (
    cd "$tmp"
    "$appimage" --appimage-extract >/dev/null
  )

  local root="$tmp/squashfs-root"
  local app_root="$root/usr/lib/${pkg}"
  [[ -x "$root/AppRun" ]] || die "AppImage has no AppRun"
  [[ -x "$app_root/$exe" ]] || die "AppImage is missing the executable"
  [[ -f "$app_root/lib/librustdesk.so" ]] \
    || die "AppImage is missing lib/librustdesk.so"
  [[ -f "$app_root/lib/libflutter_linux_gtk.so" ]] \
    || die "AppImage is missing lib/libflutter_linux_gtk.so"
  [[ -d "$app_root/data/flutter_assets" ]] \
    || die "AppImage is missing data/flutter_assets"
  find "$app_root/data/flutter_assets" -maxdepth 1 -name 'AssetManifest.*' -print -quit | grep -q . \
    || die "AppImage Flutter assets have no AssetManifest"
  [[ -f "$root/${pkg}.desktop" ]] || die "AppImage has no root desktop file"

  if ! LD_LIBRARY_PATH="$app_root/lib:$app_root" ldd "$app_root/lib/librustdesk.so" 2>&1 | grep -q 'not found'; then
    :
  else
    LD_LIBRARY_PATH="$app_root/lib:$app_root" ldd "$app_root/lib/librustdesk.so" >&2 || true
    die "librustdesk.so has unresolved runtime libraries"
  fi

  if command -v readelf >/dev/null 2>&1; then
    readelf -d "$app_root/$exe" | grep -q 'RUNPATH.*\$ORIGIN/lib' \
      || die "executable RUNPATH is not \$ORIGIN/lib"
  fi

  log "verified embedded layout:"
  log "  $app_root/$exe"
  log "  $app_root/lib/librustdesk.so"
  log "  $app_root/lib/libflutter_linux_gtk.so"
  log "  $app_root/data/flutter_assets"
  rm -rf "$tmp"
}

write_manifest() {
  local profile="$1" pkg="$2" product="$3" exe="$4" out="$5" artifact="$6" features="$7"
  local appimage="$out/$artifact"
  local sha size commit rustc_version flutter_version
  sha="$(sha256sum "$appimage" | awk '{print $1}')"
  size="$(stat -c '%s' "$appimage")"
  commit="$(git -C /work rev-parse HEAD 2>/dev/null || echo unknown)"
  rustc_version="$(rustc --version 2>/dev/null || echo unknown)"
  flutter_version="$(flutter --version 2>/dev/null | head -1 || echo unknown)"

  jq -n \
    --arg profile "$profile" \
    --arg product "$product" \
    --arg pkg "$pkg" \
    --arg exe "$exe" \
    --arg artifact "$artifact" \
    --arg sha "$sha" \
    --argjson size "$size" \
    --arg commit "$commit" \
    --arg rustc "$rustc_version" \
    --arg flutter "$flutter_version" \
    --arg features "$features" \
    --arg image "${TEFWAY_BUILDER_IMAGE:-unknown}" \
    --arg built "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{
      profile: $profile,
      product: $product,
      package: $pkg,
      executable: $exe,
      artifact: $artifact,
      sha256: $sha,
      size_bytes: $size,
      commit: $commit,
      built_at_utc: $built,
      builder_image: $image,
      rustc: $rustc,
      flutter: $flutter,
      cargo_features: $features,
      embedded: {
        library_layout: "lib/librustdesk.so",
        rust_core: "usr/lib/\($pkg)/lib/librustdesk.so",
        flutter_engine: "usr/lib/\($pkg)/lib/libflutter_linux_gtk.so",
        flutter_assets: "usr/lib/\($pkg)/data/flutter_assets"
      }
    }' > "$out/${artifact}.manifest.json"
  log "manifest: $out/${artifact}.manifest.json"
}

parse_args "$@"
profile_values
if ((INSIDE)); then
  inside_main
else
  host_main
fi
