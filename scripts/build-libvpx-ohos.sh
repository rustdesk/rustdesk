#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -z "${OHOS_NDK_HOME:-}" || -z "${OHOS_LIBVPX_ROOT:-}" ]] \
  || ! declare -F ohos_build_jobs >/dev/null; then
  source "$repo_root/scripts/ohos-env.sh"
fi

if command -v cygpath >/dev/null 2>&1; then
  OHOS_NDK_HOME="$(cygpath -u "$OHOS_NDK_HOME")"
  OHOS_LIBVPX_ROOT="$(cygpath -u "$OHOS_LIBVPX_ROOT")"
fi

libvpx_version="1.16.0"
libvpx_sha256="7a479a3c66b9f5d5542a4c6a1b7d3768a983b1e5c14c60a9396edc9b649e015c"
cache_root="$repo_root/target/ohos-libvpx-v$libvpx_version"
archive="$cache_root/libvpx-v$libvpx_version.tar.gz"
source_root="$cache_root/source"
build_root="$cache_root/build"
codec_header="$OHOS_LIBVPX_ROOT/include/vpx/vpx_codec.h"
static_library="$OHOS_LIBVPX_ROOT/lib/libvpx.a"

if [[ -f "$codec_header" && -f "$static_library" ]]; then
  echo "Using cached OHOS libvpx: $OHOS_LIBVPX_ROOT"
  exit 0
fi

task_make="make"
if ! command -v "$task_make" >/dev/null 2>&1 && command -v mingw32-make >/dev/null 2>&1; then
  task_make="mingw32-make"
fi

for command_name in curl "$task_make" perl tar; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Missing required libvpx build command: $command_name" >&2
    exit 2
  fi
done

mkdir -p "$cache_root" "$source_root" "$build_root" "$OHOS_LIBVPX_ROOT"

if [[ ! -f "$archive" ]]; then
  echo "Downloading libvpx v$libvpx_version..."
  curl --fail --location --retry 3 \
    "https://github.com/webmproject/libvpx/archive/v$libvpx_version.tar.gz" \
    --output "$archive"
fi

if command -v shasum >/dev/null 2>&1; then
  actual_sha256="$(shasum -a 256 "$archive" | awk '{print $1}')"
elif command -v sha256sum >/dev/null 2>&1; then
  actual_sha256="$(sha256sum "$archive" | awk '{print $1}')"
else
  echo "Missing required libvpx checksum command: shasum or sha256sum" >&2
  exit 2
fi

if [[ "$actual_sha256" != "$libvpx_sha256" ]]; then
  echo "libvpx archive checksum mismatch: expected $libvpx_sha256, got $actual_sha256" >&2
  exit 1
fi

if [[ ! -x "$source_root/configure" ]]; then
  tar -xzf "$archive" --strip-components=1 -C "$source_root"
fi

task_sysroot="$OHOS_NDK_HOME/native/sysroot"
task_toolchain="$OHOS_NDK_HOME/native/llvm/bin"
task_target="aarch64-linux-ohos"
task_flags="--target=$task_target --sysroot=$task_sysroot -D__MUSL__"

export CC="$task_toolchain/clang $task_flags"
export CXX="$task_toolchain/clang++ $task_flags"
export AR="$task_toolchain/llvm-ar"
export RANLIB="$task_toolchain/llvm-ranlib"
export STRIP="$task_toolchain/llvm-strip"
export AS="$CC -c"

if [[ ! -f "$build_root/Makefile" ]]; then
  echo "Configuring upstream libvpx for OHOS arm64..."
  (
    cd "$build_root"
    "$source_root/configure" \
      --target=arm64-linux-gcc \
      --disable-examples \
      --disable-tools \
      --disable-docs \
      --disable-unit-tests \
      --enable-pic \
      --enable-static \
      --disable-shared \
      --prefix="$OHOS_LIBVPX_ROOT"
  )
fi

task_jobs="$(ohos_build_jobs)"

echo "Building upstream libvpx for OHOS arm64..."
"$task_make" -C "$build_root" -j"$task_jobs"
"$task_make" -C "$build_root" install

if [[ ! -f "$codec_header" || ! -f "$static_library" ]]; then
  echo "libvpx build did not produce the expected OHOS headers and static library" >&2
  exit 1
fi

echo "Done: $OHOS_LIBVPX_ROOT"
