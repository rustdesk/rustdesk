#!/usr/bin/env bash

#
# Fix OpenSSL build with Android NDK clang on 32-bit architectures
#

export CFLAGS="-DBROKEN_CLANG_ATOMICS"
export CXXFLAGS="-DBROKEN_CLANG_ATOMICS"

cargo ndk --platform 21 --target i686-linux-android build --release --features flutter
