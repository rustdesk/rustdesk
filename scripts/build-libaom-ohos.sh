#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -z "${OHOS_NDK_HOME:-}" || -z "${OHOS_LIBAOM_ROOT:-}" ]] \
  || ! declare -F ohos_build_jobs >/dev/null; then
  source "$repo_root/scripts/ohos-env.sh"
fi

if command -v cygpath >/dev/null 2>&1; then
  OHOS_NDK_HOME="$(cygpath -u "$OHOS_NDK_HOME")"
  OHOS_LIBAOM_ROOT="$(cygpath -u "$OHOS_LIBAOM_ROOT")"
fi

aom_version="3.12.1"
aom_commit="10aece4157eb79315da205f39e19bf6ab3ee30d0"
cache_root="$repo_root/target/ohos-libaom-v$aom_version"
source_root="$cache_root/source"
build_root="$cache_root/build"
codec_header="$OHOS_LIBAOM_ROOT/include/aom/aom_codec.h"
static_library="$OHOS_LIBAOM_ROOT/lib/libaom.a"
toolchain_file="$OHOS_NDK_HOME/native/build/cmake/ohos.toolchain.cmake"

if [[ -f "$codec_header" && -f "$static_library" ]]; then
  echo "Using cached OHOS libaom: $OHOS_LIBAOM_ROOT"
  exit 0
fi

for command_name in cmake git ninja; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Missing required libaom build command: $command_name" >&2
    exit 2
  fi
done

if [[ ! -f "$toolchain_file" ]]; then
  echo "HarmonyOS CMake toolchain was not found: $toolchain_file" >&2
  exit 2
fi

mkdir -p "$cache_root" "$source_root" "$build_root" "$OHOS_LIBAOM_ROOT"

if [[ ! -d "$source_root/.git" ]]; then
  git -C "$source_root" init -q
  git -C "$source_root" remote add origin https://aomedia.googlesource.com/aom
fi

current_commit="$(git -C "$source_root" rev-parse HEAD 2>/dev/null || true)"
if [[ "$current_commit" != "$aom_commit" ]]; then
  echo "Fetching upstream libaom $aom_version ($aom_commit)..."
  GIT_TERMINAL_PROMPT=0 git -C "$source_root" fetch --depth=1 origin "$aom_commit"
  git -C "$source_root" checkout --detach --force FETCH_HEAD
fi

if [[ ! -f "$build_root/build.ninja" ]]; then
  echo "Configuring upstream libaom for OHOS arm64..."
  cmake -S "$source_root" -B "$build_root" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$toolchain_file" \
    -DOHOS_ARCH=arm64-v8a \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$OHOS_LIBAOM_ROOT" \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    -DAOM_TARGET_CPU=arm64 \
    -DBUILD_SHARED_LIBS=OFF \
    -DENABLE_DOCS=OFF \
    -DENABLE_EXAMPLES=OFF \
    -DENABLE_TESTDATA=OFF \
    -DENABLE_TESTS=OFF \
    -DENABLE_TOOLS=OFF
fi

task_jobs="$(ohos_build_jobs)"

echo "Building upstream libaom for OHOS arm64..."
cmake --build "$build_root" --parallel "$task_jobs"
cmake --install "$build_root"

if [[ ! -f "$codec_header" || ! -f "$static_library" ]]; then
  echo "libaom build did not produce the expected OHOS headers and static library" >&2
  exit 1
fi

echo "Done: $OHOS_LIBAOM_ROOT"
