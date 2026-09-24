#!/usr/bin/env bash
# AppRun v2.0.0, which appimage-builder downloads, splits every argument into a 1 KiB stack
# buffer, so any argument of 1024 bytes or more aborts the AppImage before RustDesk starts.
# Build it with apprun-split-arguments.patch into the path appimage-builder checks before
# downloading. ubuntu:16.04 is what upstream released from, so the host glibc floor is unchanged.
set -euo pipefail

arch=$1
version=v2.0.0
commit=407700ccef58e1a64cc49856074aee4b1c75d599
here=$(cd "$(dirname "$0")" && pwd)

case $arch in
  x86_64) packages="cmake make g++" ;;
  aarch64) packages="cmake make g++ g++-aarch64-linux-gnu" ;;
  *) echo "Unsupported arch: $arch" >&2; exit 1 ;;
esac

src=$(mktemp -d)
curl -fsSL "https://github.com/AppImageCrafters/AppRun/archive/$commit.tar.gz" | tar -xz -C "$src" --strip-components=1
patch -d "$src" -p1 < "$here/apprun-split-arguments.patch"

docker run --rm -v "$src:/src" -w /src -e DEBIAN_FRONTEND=noninteractive ubuntu:16.04 bash -c "
  set -e
  apt-get update
  apt-get install -y --no-install-recommends $packages
  mkdir build && cd build
  cmake .. -DCMAKE_BUILD_TYPE=Release -DCMAKE_TOOLCHAIN_FILE=../cmake/$arch-toolchain.cmake
  cmake --build . --target AppRun"

mkdir -p "$here/appimage-build/AppRun/$version"
cp "$src/build/src/apprun/AppRun" "$here/appimage-build/AppRun/$version/AppRun-Release-$arch"
