#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -z "${OHOS_NDK_HOME:-}" || -z "${OHOS_LIBYUV_ROOT:-}" ]]; then
  source "$repo_root/scripts/ohos-env.sh"
fi

if command -v cygpath >/dev/null 2>&1; then
  OHOS_NDK_HOME="$(cygpath -u "$OHOS_NDK_HOME")"
  OHOS_LIBYUV_ROOT="$(cygpath -u "$OHOS_LIBYUV_ROOT")"
fi

libyuv_commit="b56492e2dfc064f65ef27fed9c45d9bbfc2e2ad2"
cache_root="$repo_root/target/ohos-libyuv-$libyuv_commit"
source_root="$cache_root/source"
build_root="$cache_root/build"
codec_header="$OHOS_LIBYUV_ROOT/include/libyuv/convert.h"
static_library="$OHOS_LIBYUV_ROOT/lib/libyuv.a"
toolchain_file="$OHOS_NDK_HOME/native/build/cmake/ohos.toolchain.cmake"

if [[ -f "$codec_header" && -f "$static_library" ]]; then
  echo "Using cached OHOS libyuv: $OHOS_LIBYUV_ROOT"
  exit 0
fi

for command_name in cmake git ninja; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Missing required libyuv build command: $command_name" >&2
    exit 2
  fi
done

if [[ ! -f "$toolchain_file" ]]; then
  echo "HarmonyOS CMake toolchain was not found: $toolchain_file" >&2
  exit 2
fi

mkdir -p "$cache_root" "$source_root" "$build_root" "$OHOS_LIBYUV_ROOT"

if [[ ! -d "$source_root/.git" ]]; then
  git -C "$source_root" init -q
  git -C "$source_root" remote add origin https://chromium.googlesource.com/libyuv/libyuv
fi

current_commit="$(git -C "$source_root" rev-parse HEAD 2>/dev/null || true)"
if [[ "$current_commit" != "$libyuv_commit" ]]; then
  echo "Fetching upstream libyuv $libyuv_commit..."
  GIT_TERMINAL_PROMPT=0 git -C "$source_root" fetch --depth=1 origin "$libyuv_commit"
  git -C "$source_root" checkout --detach --force FETCH_HEAD
fi

if [[ ! -f "$build_root/build.ninja" ]]; then
  echo "Configuring upstream libyuv for OHOS arm64..."
  cmake -S "$source_root" -B "$build_root" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$toolchain_file" \
    -DOHOS_ARCH=arm64-v8a \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$OHOS_LIBYUV_ROOT" \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    -DBUILD_SHARED_LIBS=OFF \
    -DBUILD_TESTING=OFF
fi

echo "Building upstream libyuv for OHOS arm64..."
cmake --build "$build_root" --parallel "${CARGO_BUILD_JOBS:-4}"
cmake --install "$build_root"

if [[ ! -f "$codec_header" || ! -f "$static_library" ]]; then
  echo "libyuv build did not produce the expected OHOS headers and static library" >&2
  exit 1
fi

echo "Done: $OHOS_LIBYUV_ROOT"
