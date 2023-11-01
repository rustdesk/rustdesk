#!/bin/bash

ANDROID_ABI=$1

# Build libyuv / opus / libvpx / oboe for Android
# Required:
#   1. set VCPKG_ROOT / ANDROID_NDK path environment variables
#   2. vcpkg initialized
#   3. ndk, version: 22 (if ndk < 22 you need to change LD as `export LD=$TOOLCHAIN/bin/$NDK_LLVM_TARGET-ld`)

if [ -z "$ANDROID_NDK" ]; then
  echo "Failed! Please set ANDROID_NDK"
  exit 1
fi

if [ -z "$VCPKG_ROOT" ]; then
  echo "Failed! Please set VCPKG_ROOT"
  exit 1
fi

API_LEVEL="21"

# NDK llvm toolchain
HOST_TAG="linux-x86_64" # current platform, set as `ls $ANDROID_NDK/toolchains/llvm/prebuilt/`
TOOLCHAIN=$ANDROID_NDK/toolchains/llvm/prebuilt/$HOST_TAG

function build {
  ANDROID_ABI=$1

  case "$ANDROID_ABI" in
  arm64-v8a)
     ABI=aarch64-linux-android$API_LEVEL
     VCPKG_TARGET=arm64-android
     ;;
  armeabi-v7a)
     ABI=armv7a-linux-androideabi$API_LEVEL
     VCPKG_TARGET=arm-neon-android
     ;;
  x86_64)
     ABI=x86_64-linux-android$API_LEVEL
     VCPKG_TARGET=x64-android
     ;;
  x86)
     ABI=i686-linux-android$API_LEVEL
     VCPKG_TARGET=x86-android
     ;;
  esac

  PREFIX=$VCPKG_ROOT/installed/$VCPKG_TARGET/

  # 1
  echo "*** [$ANDROID_ABI][Start] Build opus / libyuv from vcpkg"
  export ANDROID_NDK_HOME=$ANDROID_NDK
  pushd $VCPKG_ROOT
  $VCPKG_ROOT/vcpkg install opus --triplet $VCPKG_TARGET
  $VCPKG_ROOT/vcpkg install libyuv --triplet $VCPKG_TARGET
  $VCPKG_ROOT/vcpkg install aom --triplet $VCPKG_TARGET
  $VCPKG_ROOT/vcpkg install cpu-features --triplet $VCPKG_TARGET
  popd
  echo "*** [$ANDROID_ABI][Finished] Build opus / libyuv from vcpkg"

  # 2
  echo "*** [$ANDROID_ABI][Start] Build libvpx"
  pushd build/libvpx

  export AR=$TOOLCHAIN/bin/llvm-ar
  export CC=$TOOLCHAIN/bin/"$ABI"-clang
  export CXX=$TOOLCHAIN/bin/"$ABI"-clang++
  export LD=$TOOLCHAIN/bin/"$ABI"-clang
  export RANLIB=$TOOLCHAIN/bin/llvm-ranlib
  export STRIP=$TOOLCHAIN/bin/llvm-strip

  case "$ANDROID_ABI" in
  arm64-v8a)
    export AS=$TOOLCHAIN/bin/llvm-as
    LIBVPX_EXTRA_BUILD_FLAGS="--target=arm64-android-gcc --enable-thumb --disable-runtime-cpu-detect"
    ;;
  armeabi-v7a)
    export AS=$TOOLCHAIN/bin/llvm-as
    LIBVPX_EXTRA_BUILD_FLAGS="--target=armv7-android-gcc --enable-thumb --disable-neon --disable-runtime-cpu-detect"
    ;;
  x86_64)
    export AS=$TOOLCHAIN/bin/yasm
    LIBVPX_EXTRA_BUILD_FLAGS="--target=x86_64-android-gcc --disable-avx --disable-avx2 --disable-avx512"
    ;;
  x86)
    export AS=$TOOLCHAIN/bin/yasm
    LIBVPX_EXTRA_BUILD_FLAGS="--target=x86-android-gcc --disable-sse4_1 --disable-avx --disable-avx2 --disable-avx512"
    ;;
  esac

  make clean

  ./configure \
              --enable-pic \
              --disable-webm-io \
              --disable-unit-tests \
              --disable-examples \
              --disable-libyuv \
              --disable-postproc \
              --disable-tools \
              --disable-docs \
              --prefix=$PREFIX \
              $LIBVPX_EXTRA_BUILD_FLAGS
  make -j
  make install

  unset AR CC CXX LD RANLIB STRIP

  popd
  echo "*** [$ANDROID_ABI][Finished] Build libvpx"

  # 3
  echo "*** [$ANDROID_ABI][Start] Build oboe"
  pushd build/oboe
  make clean
  cmake -DBUILD_SHARED_LIBS=true \
          -DCMAKE_BUILD_TYPE=RelWithDebInfo \
          -DANDROID_TOOLCHAIN=clang \
          -DANDROID_STL=c++_shared \
          -DCMAKE_TOOLCHAIN_FILE=$ANDROID_NDK/build/cmake/android.toolchain.cmake \
          -DCMAKE_INSTALL_PREFIX=$PREFIX \
          -DANDROID_ABI=$ANDROID_ABI \
          -DANDROID_PLATFORM=android-$API_LEVEL
  make -j
  make install
  mv $PREFIX/lib/$ANDROID_ABI/liboboe.a $PREFIX/lib/
  popd
  echo "*** [$ANDROID_ABI][Finished] Build oboe"

  echo "*** [$ANDROID_ABI][All Finished]"
}

# Bump libvpx to 1.13.1 to fix CVE-2023-5217
git clone -b v1.13.1 --depth=1 https://github.com/webmproject/libvpx.git build/libvpx
git clone -b 1.6.1 --depth=1 https://github.com/google/oboe build/oboe
patch -N -d build/oboe -p1 < ../src/oboe.patch

if [ ! -z "$ANDROID_ABI" ]; then
  build "$ANDROID_ABI"
else
  build arm64-v8a
  build armeabi-v7a
  build x86_64
  build x86
fi

[ -d "$VCPKG_ROOT/installed/arm-neon-android" ] && mv "$VCPKG_ROOT/installed/arm-neon-android" "$VCPKG_ROOT/installed/arm-android"

rm -rf build
