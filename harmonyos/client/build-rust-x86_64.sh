#!/bin/bash
# x86_64-ohos variant of build-rust.sh for emulator debugging (the Windows x64
# emulator runs an x86_64 image). Same env handling as build-rust.sh, which
# see. Requires the x86_64 C deps from build-ohos-deps-x86_64.sh.
#
# Usage:  bash harmonyos/client/build-rust-x86_64.sh [--check] [--debug]
# Output: harmonyos/client/out/x86_64-ohos/librustdesk.so
set -e -o pipefail

SCRIPTDIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPTDIR/../.." && pwd)"

MODE_CHECK=0
MODE_DEBUG=0
for arg in "$@"; do
  case "$arg" in
    --check) MODE_CHECK=1 ;;
    --debug) MODE_DEBUG=1 ;;
    *) echo "unknown arg: $arg" >&2; exit 1 ;;
  esac
done

# --- toolchain detection ----------------------------------------------------
if [ -z "$OHOS_NDK_HOME" ]; then
  OHOS_NDK_HOME="$HOME/.rustdesk-ohos/ndk"
fi
if [ ! -d "$OHOS_NDK_HOME" ]; then
  if [ -z "$DEVECO_SDK_HOME" ]; then
    for d in "D:/Huawei/DevEco Studio" "C:/Huawei/DevEco Studio"; do
      [ -d "$d/sdk/default/openharmony/native" ] && DEVECO_SDK_HOME="$d" && break
    done
  fi
  if [ -n "$DEVECO_SDK_HOME" ]; then
    NDK="$DEVECO_SDK_HOME/sdk/default/openharmony/native"
    mkdir -p "$HOME/.rustdesk-ohos"
    cmd //c mklink //J "$(cygpath -w "$HOME/.rustdesk-ohos/ndk")" "$(cygpath -w "$NDK")" >/dev/null
    OHOS_NDK_HOME="$HOME/.rustdesk-ohos/ndk"
  fi
fi
[ -d "$OHOS_NDK_HOME" ] || { echo "ERROR: OHOS_NDK_HOME not found" >&2; exit 1; }
OHOS_NDK_HOME="$(cygpath -m "$OHOS_NDK_HOME")"
echo "INFO: OHOS_NDK_HOME=$OHOS_NDK_HOME"

LLVM="$OHOS_NDK_HOME/llvm/bin"
SYSROOT="$OHOS_NDK_HOME/sysroot"
CLANG="$LLVM/clang.exe"

export LIBCLANG_PATH="$LLVM"

if [ -z "$OHOS_DEPS_DIR" ]; then
  OHOS_DEPS_DIR="$SCRIPTDIR/deps-x86_64"
fi
[ -d "$OHOS_DEPS_DIR/include" ] || {
  echo "ERROR: $OHOS_DEPS_DIR/include not found - run build-ohos-deps-x86_64.sh first" >&2
  exit 1
}
export OHOS_DEPS_DIR
export VCPKG_ROOT="$HOME/.rustdesk-ohos/vcpkg"
export SODIUM_LIB_DIR_x86_64_unknown_linux_ohos="$(cygpath -m "$VCPKG_ROOT/installed/x64-ohos/lib")"

# --- cargo cross env --------------------------------------------------------
TARGET="x86_64-unknown-linux-ohos"
TARGET_ENV="x86_64_unknown_linux_ohos"
TFLAGS="--target=x86_64-linux-ohos --sysroot=$SYSROOT -D__MUSL__"

export CC_${TARGET_ENV}="$CLANG"
export CXX_${TARGET_ENV}="$LLVM/clang++.exe"
export AR_${TARGET_ENV}="$LLVM/llvm-ar.exe"
export CFLAGS_${TARGET_ENV}="$TFLAGS"
export CXXFLAGS_${TARGET_ENV}="$TFLAGS -stdlib=libc++"
export OPENSSL_CFLAGS="$TFLAGS"
# Vendored openssl drives its Configure through make; point at the msys2 make
# fetched by build-ohos-deps.sh (Git Bash ships none).
MSYS_MAKE="$(ls "$HOME/.rustdesk-ohos/vcpkg/downloads/tools/msys2/"*/usr/bin/make.exe 2>/dev/null | head -1)"
[ -n "$MSYS_MAKE" ] && export OPENSSL_MAKE="$(cygpath -m "$MSYS_MAKE")"
# openssl-src invokes "make" from PATH and its recipes need a POSIX sh plus
# binutils (ranlib/ar); the trimmed msys2 lacks the latter, so shim them from
# the NDK llvm tools.
MSYS_BIN="$(dirname "$MSYS_MAKE" 2>/dev/null)"
SHIM_BIN="$HOME/.rustdesk-ohos/bin"
mkdir -p "$SHIM_BIN"
for t in ranlib ar; do
  [ -e "$SHIM_BIN/$t.exe" ] || cp "$LLVM/llvm-$t.exe" "$SHIM_BIN/$t.exe" 2>/dev/null
done
export PATH="$(cygpath -u "$SHIM_BIN"):$PATH"
[ -n "$MSYS_BIN" ] && export PATH="$(cygpath -u "$MSYS_BIN"):$PATH"
# Vendored openssl's Configure needs core perl modules the Git Bash perl
# strips; they are vendored under ~/.rustdesk-ohos/perl5lib (fetched from
# CPAN, see docs). Do NOT put a Strawberry (MSWin32) perl on PATH: OpenSSL's
# unix Configure rejects its Windows-style paths.
if [ -d "$HOME/.rustdesk-ohos/perl5lib" ]; then
  export PERL5LIB="${PERL5LIB:+$PERL5LIB:}$(cygpath -u "$HOME/.rustdesk-ohos/perl5lib")"
fi
# bindgen (kcp-sys) needs the ohos sysroot for its clang probe.
export BINDGEN_EXTRA_CLANG_ARGS_x86_64_unknown_linux_ohos="--target=x86_64-linux-ohos --sysroot=$SYSROOT"
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_OHOS_LINKER="$CLANG"
export RUSTFLAGS="-C link-arg=--target=x86_64-linux-ohos -C link-arg=--sysroot=$SYSROOT -C link-arg=-D__MUSL__"

cd "$REPO_ROOT"

PROFILE=release
[ "$MODE_DEBUG" -eq 1 ] && PROFILE=debug
# cargo has no --debug alias; the dev profile writes to debug/ as well.
PROFILE_FLAG=--release
[ "$MODE_DEBUG" -eq 1 ] && PROFILE_FLAG=--profile dev

OUT_DIR="$SCRIPTDIR/out/x86_64-ohos"
mkdir -p "$OUT_DIR"

if [ "$MODE_CHECK" -eq 1 ]; then
  cargo check --target "$TARGET" --lib --no-default-features --features use_dasp
  exit 0
fi

cargo build --target "$TARGET" --lib --no-default-features --features use_dasp $PROFILE_FLAG

# the crate is named librustdesk, so the cdylib is liblibrustdesk.so; the
# ArkTS side dlopens "librustdesk.so".
cp "target/$TARGET/$PROFILE/liblibrustdesk.so" "$OUT_DIR/librustdesk.so"
echo "INFO: built $OUT_DIR/librustdesk.so"
