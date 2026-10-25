#!/bin/bash
# x86_64-ohos variant of build-ohos-deps.sh: the Windows x64 emulator runs an
# x86_64 image, so an emulator build needs x86_64 C deps for librustdesk.so.
# Produces harmonyos/client/deps-x86_64/{include,lib}; point OHOS_DEPS_DIR at
# it when cross-compiling (see build-rust-x86_64.sh).
#
# libvpx is built with all x86 SIMD extensions disabled: no nasm/yasm is
# available on this host, so AS is preset (see below) to pass configure's
# assembler probe. The C fallback paths are enough for emulator decoding.
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

VCPKG_ROOT="${VCPKG_ROOT:-$HOME/.rustdesk-ohos/vcpkg}"
[ -x "$VCPKG_ROOT/vcpkg" ] || { echo "ERROR: vcpkg not bootstrapped at $VCPKG_ROOT" >&2; exit 1; }

export OHOS_SDK_ROOT

WORK="$SCRIPTDIR/.deps-work-x86_64"
mkdir -p "$WORK"
DEPS="$SCRIPTDIR/deps-x86_64"
rm -rf "$DEPS"
mkdir -p "$DEPS/include" "$DEPS/lib"

# --- libyuv + opus + libsodium (vcpkg, cmake) --------------------------------
cd "$VCPKG_ROOT"
if [ ! -f "installed/x64-ohos/lib/libyuv.a" ] || [ ! -f "installed/x64-ohos/lib/libopus.a" ] || [ ! -f "installed/x64-ohos/lib/libsodium.a" ]; then
  ./vcpkg install libyuv opus libsodium --triplet x64-ohos
fi
# magnum-opus's build script looks for vcpkg's linux layout; vcpkg names the
# x86_64 triplet "x64", hence x64-linux (arm64 is arm64-linux).
if [ ! -e "installed/x64-linux" ]; then
  cmd //c mklink //J \
    "$(cygpath -w "$VCPKG_ROOT/installed/x64-linux")" \
    "$(cygpath -w "$VCPKG_ROOT/installed/x64-ohos")" >/dev/null
fi
cp -r "$VCPKG_ROOT/installed/x64-ohos/include"/* "$DEPS/include/"
cp "$VCPKG_ROOT/installed/x64-ohos/lib/"*.a "$DEPS/lib/"
# libsodium-sys's build script picks the link name with a HOST cfg, so a
# windows-msvc host asks for "libsodium" instead of "sodium": ship both names.
cp "$VCPKG_ROOT/installed/x64-ohos/lib/libsodium.a" "$VCPKG_ROOT/installed/x64-ohos/lib/liblibsodium.a" 2>/dev/null || true

# --- libvpx (direct autotools build) ----------------------------------------
VPX_SRC="$WORK/libvpx-$LIBVPX_VERSION"
if [ ! -d "$VPX_SRC" ]; then
  # reuse the source the arm64 script already downloaded when available
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

CC="$LLVM/clang.exe --target=x86_64-linux-ohos --sysroot=$SYSROOT -D__MUSL__"
CXX="$LLVM/clang++.exe --target=x86_64-linux-ohos --sysroot=$SYSROOT -D__MUSL__ -stdlib=libc++"
AR="$LLVM/llvm-ar.exe"
LD="$LLVM/clang.exe --target=x86_64-linux-ohos --sysroot=$SYSROOT"

BUILD="$WORK/libvpx-build-x86_64"
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
  # No nasm/yasm on this host. configure dies on the assembler probe unless AS
  # is preset; with every x86 SIMD extension disabled below no .asm is ever
  # assembled, so the name here is never invoked.
  export AS=clang
  "$VPX_SRC/configure" \
    --target=x86_64-linux-gcc \
    --disable-examples --disable-tools --disable-docs --disable-unit-tests \
    --disable-webm-io --disable-libyuv --enable-pic --enable-static --disable-shared \
    --disable-mmx --disable-sse --disable-sse2 --disable-sse3 --disable-ssse3 \
    --disable-sse4_1 --disable-avx --disable-avx2 --disable-avx512 \
    --prefix=/usr/local
  "$MSYS_MAKE" -j8
  "$MSYS_MAKE" install DESTDIR="$BUILD/install"
)

cp -r "$BUILD/install/usr/local/include"/* "$DEPS/include/"
cp "$BUILD/install/usr/local/lib/"*.a "$DEPS/lib/"

echo "INFO: deps ready at $DEPS"
ls -la "$DEPS/lib"
