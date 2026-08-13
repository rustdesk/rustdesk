#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -z "${OHOS_NDK_HOME:-}" || -z "${OHOS_LIBOPUS_ROOT:-}" ]]; then
  source "$repo_root/scripts/ohos-env.sh"
fi

if command -v cygpath >/dev/null 2>&1; then
  OHOS_NDK_HOME="$(cygpath -u "$OHOS_NDK_HOME")"
  OHOS_LIBOPUS_ROOT="$(cygpath -u "$OHOS_LIBOPUS_ROOT")"
fi

opus_version="1.5.2"
opus_commit="5ec2f3c915d0529b94a3a302969c673531654824"
cache_root="$repo_root/target/ohos-libopus-v$opus_version"
source_root="$cache_root/source"
build_root="$cache_root/build"
codec_header="$OHOS_LIBOPUS_ROOT/include/opus/opus.h"
static_library="$OHOS_LIBOPUS_ROOT/lib/libopus.a"
toolchain_file="$OHOS_NDK_HOME/native/build/cmake/ohos.toolchain.cmake"

if [[ -f "$codec_header" && -f "$static_library" ]]; then
  echo "Using cached OHOS libopus: $OHOS_LIBOPUS_ROOT"
  exit 0
fi

for command_name in cmake git ninja; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Missing required libopus build command: $command_name" >&2
    exit 2
  fi
done

if [[ ! -f "$toolchain_file" ]]; then
  echo "HarmonyOS CMake toolchain was not found: $toolchain_file" >&2
  exit 2
fi

mkdir -p "$cache_root" "$source_root" "$build_root" "$OHOS_LIBOPUS_ROOT"

if [[ ! -d "$source_root/.git" ]]; then
  git -C "$source_root" init -q
  git -C "$source_root" remote add origin https://github.com/xiph/opus.git
fi

current_commit="$(git -C "$source_root" rev-parse HEAD 2>/dev/null || true)"
if [[ "$current_commit" != "$opus_commit" ]]; then
  echo "Fetching upstream libopus v$opus_version ($opus_commit)..."
  GIT_TERMINAL_PROMPT=0 git -C "$source_root" fetch --depth=1 origin "$opus_commit"
  git -C "$source_root" checkout --detach --force FETCH_HEAD
fi

if [[ ! -f "$build_root/build.ninja" ]]; then
  echo "Configuring upstream libopus for OHOS arm64..."
  cmake -S "$source_root" -B "$build_root" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$toolchain_file" \
    -DOHOS_ARCH=arm64-v8a \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$OHOS_LIBOPUS_ROOT" \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    -DBUILD_SHARED_LIBS=OFF \
    -DOPUS_BUILD_PROGRAMS=OFF \
    -DOPUS_BUILD_TESTING=OFF
fi

echo "Building upstream libopus for OHOS arm64..."
cmake --build "$build_root" --parallel "${CARGO_BUILD_JOBS:-4}"
cmake --install "$build_root"

if [[ ! -f "$codec_header" || ! -f "$static_library" ]]; then
  echo "libopus build did not produce the expected OHOS headers and static library" >&2
  exit 1
fi

echo "Done: $OHOS_LIBOPUS_ROOT"

