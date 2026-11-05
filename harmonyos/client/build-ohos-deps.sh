#!/bin/bash
# Build libvpx / libyuv for OpenHarmony (aarch64), producing
# harmonyos/client/deps/{include,lib} consumed by libs/scrap/build.rs via
# OHOS_DEPS_DIR.
#
# libyuv (CMake) comes from vcpkg's official `arm64-ohos` community triplet.
# libvpx (autotools) is built directly: vcpkg's configure_make loses the
# --target/--sysroot flags for the ohos triplet, so we drive its configure
# ourselves with the NDK clang (reusing vcpkg's msys2 make).
#
# AV1/libaom is intentionally NOT built for M1: the aomedia.googlesource git
# clone is >1 GB and the controller negotiates VP8/VP9. libaom symbols are
# provided by a stub library compiled in libs/scrap/build.rs (see
# libs/scrap/src/ohos/aom_stub.c).
#
# Environment:
#   OHOS_SDK_ROOT   OpenHarmony SDK dir (the .../sdk/default/openharmony dir,
#                   i.e. the parent of `native/`); defaults to
#                   ~/.rustdesk-ohos/sdk (space-free junction, see build-rust.sh)
#   VCPKG_ROOT      vcpkg checkout; defaults to ~/.rustdesk-ohos/vcpkg
set -e -o pipefail

SCRIPTDIR="$(cd "$(dirname "$0")" && pwd)"
LIBVPX_VERSION=1.16.0

OHOS_SDK_ROOT="${OHOS_SDK_ROOT:-$HOME/.rustdesk-ohos/sdk}"
[ -d "$OHOS_SDK_ROOT/native" ] || {
  echo "ERROR: \$OHOS_SDK_ROOT/native not found under: $OHOS_SDK_ROOT" >&2
  exit 1
}
NDK="$OHOS_SDK_ROOT/native"
LLVM="$NDK/llvm/bin"
SYSROOT="$NDK/sysroot"

if [ -z "$VCPKG_ROOT" ]; then
  VCPKG_ROOT="$HOME/.rustdesk-ohos/vcpkg"
  if [ ! -d "$VCPKG_ROOT" ]; then
    mkdir -p "$HOME/.rustdesk-ohos"
    # github first, gitee mirror as fallback (large repo, flaky links)
    git clone --depth 1 https://github.com/microsoft/vcpkg.git "$VCPKG_ROOT" ||
      git clone --depth 1 https://gitee.com/mirrors/vcpkg.git "$VCPKG_ROOT"
  fi
fi
[ -x "$VCPKG_ROOT/vcpkg" ] || (cd "$VCPKG_ROOT" && ./bootstrap-vcpkg.bat -disableMetrics)

export OHOS_SDK_ROOT

WORK="$SCRIPTDIR/.deps-work"
mkdir -p "$WORK"
DEPS="$SCRIPTDIR/deps"
rm -rf "$DEPS"
mkdir -p "$DEPS/include" "$DEPS/lib"

# --- libyuv + opus + libsodium (vcpkg, cmake) --------------------------------
cd "$VCPKG_ROOT"
if [ ! -f "installed/arm64-ohos/lib/libyuv.a" ] || [ ! -f "installed/arm64-ohos/lib/libopus.a" ] || [ ! -f "installed/arm64-ohos/lib/libsodium.a" ]; then
  ./vcpkg install libyuv opus libsodium --triplet arm64-ohos
fi
# magnum-opus's build script looks for vcpkg's linux layout (arm64-linux).
if [ ! -e "installed/arm64-linux" ]; then
  cmd //c mklink //J \
    "$(cygpath -w "$VCPKG_ROOT/installed/arm64-linux")" \
    "$(cygpath -w "$VCPKG_ROOT/installed/arm64-ohos")" >/dev/null
fi
cp -r "$VCPKG_ROOT/installed/arm64-ohos/include"/* "$DEPS/include/"
cp "$VCPKG_ROOT/installed/arm64-ohos/lib/"*.a "$DEPS/lib/"
# libsodium-sys's build script picks the link name with a HOST cfg, so a
# windows-msvc host asks for "libsodium" instead of "sodium": ship both names.
cp "$VCPKG_ROOT/installed/arm64-ohos/lib/libsodium.a"    "$VCPKG_ROOT/installed/arm64-ohos/lib/liblibsodium.a" 2>/dev/null || true

# --- libvpx (direct autotools build) ----------------------------------------
VPX_SRC="$WORK/libvpx-$LIBVPX_VERSION"
if [ ! -d "$VPX_SRC" ]; then
  # reuse the source vcpkg already downloaded when available
  VPX_VCPKG_SRC="$(ls -d "$VCPKG_ROOT/buildtrees/libvpx/src/"*".clean" 2>/dev/null | head -1)"
  if [ -n "$VPX_VCPKG_SRC" ] && [ -f "$VPX_VCPKG_SRC/configure" ]; then
    cp -r "$VPX_VCPKG_SRC" "$VPX_SRC"
  else
    curl -L -o "$WORK/libvpx.tar.gz" \
      "https://github.com/webmproject/libvpx/archive/refs/tags/v$LIBVPX_VERSION.tar.gz"
    tar -xzf "$WORK/libvpx.tar.gz" -C "$WORK"
  fi
fi

# vcpkg's msys2 provides make.exe on Windows; configure itself runs under the
# invoking bash (the trimmed msys2 environment mangles heredoc writes).
MSYS_ROOT="$(ls -d "$VCPKG_ROOT/downloads/tools/msys2/"*/ | head -1)"
MSYS_MAKE="$(cygpath -m "${MSYS_ROOT}")usr/bin/make.exe"
"$MSYS_MAKE" --version >/dev/null

CC="$LLVM/clang.exe --target=aarch64-linux-ohos --sysroot=$SYSROOT -D__MUSL__"
CXX="$LLVM/clang++.exe --target=aarch64-linux-ohos --sysroot=$SYSROOT -D__MUSL__ -stdlib=libc++"
AR="$LLVM/llvm-ar.exe"
LD="$LLVM/clang.exe --target=aarch64-linux-ohos --sysroot=$SYSROOT"

BUILD="$WORK/libvpx-build-arm64"
rm -rf "$BUILD"
mkdir -p "$BUILD"

(
  set -e
  cd "$BUILD"
  export TMPDIR="$(cygpath -m "$WORK")"
  export TMP="$TMPDIR"
  export CC="$CC"
  export CXX="$CXX"
  export AR="$AR"
  export LD="$LD"
  export STRIP=llvm-strip
  "$VPX_SRC/configure" \
    --target=arm64-linux-gcc \
    --disable-examples --disable-tools --disable-docs --disable-unit-tests \
    --disable-webm-io --disable-libyuv --enable-pic --enable-static --disable-shared \
    --prefix=/usr/local
  "$MSYS_MAKE" -j8
  "$MSYS_MAKE" install DESTDIR="$BUILD/install"
)

cp -r "$BUILD/install/usr/local/include"/* "$DEPS/include/"
cp "$BUILD/install/usr/local/lib/"*.a "$DEPS/lib/"

echo "INFO: deps ready at $DEPS"
ls -la "$DEPS/lib"
