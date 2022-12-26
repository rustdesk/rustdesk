#!/bin/bash

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
  VCPKG_TARGET=$2
  NDK_LLVM_TARGET=$3
  LIBVPX_TARGET=$4

  PREFIX=$VCPKG_ROOT/installed/$VCPKG_TARGET/

  # 1
  echo "*** [$ANDROID_ABI][Start] Build opus / libyuv from vcpkg"
  export ANDROID_NDK_HOME=$ANDROID_NDK
  pushd $VCPKG_ROOT
  $VCPKG_ROOT/vcpkg install opus --triplet $VCPKG_TARGET
  $VCPKG_ROOT/vcpkg install libyuv --triplet $VCPKG_TARGET
  popd
  echo "*** [$ANDROID_ABI][Finished] Build opus / libyuv from vcpkg"

  # 2
  echo "*** [$ANDROID_ABI][Start] Build libvpx"
  pushd build/libvpx
  export AR=$TOOLCHAIN/bin/${NDK_LLVM_TARGET}-ar
  export AS=$TOOLCHAIN/bin/${NDK_LLVM_TARGET}-as
  export LD=$TOOLCHAIN/bin/${NDK_LLVM_TARGET}-ld.gold  # if ndk < 22, use aarch64-linux-android-ld
  export RANLIB=$TOOLCHAIN/bin/${NDK_LLVM_TARGET}-ranlib
  export STRIP=$TOOLCHAIN/bin/${NDK_LLVM_TARGET}-strip

  if [ $NDK_LLVM_TARGET == "arm-linux-androideabi" ]
  then
    export CC=$TOOLCHAIN/bin/armv7a-linux-androideabi${API_LEVEL}-clang
    export CXX=$TOOLCHAIN/bin/armv7a-linux-androideabi${API_LEVEL}-clang++
  else
    export CC=$TOOLCHAIN/bin/${NDK_LLVM_TARGET}${API_LEVEL}-clang
    export CXX=$TOOLCHAIN/bin/${NDK_LLVM_TARGET}${API_LEVEL}-clang++
  fi
  make clean
  ./configure --target=$LIBVPX_TARGET \
              --enable-pic --disable-vp8 \
              --disable-webm-io \
              --disable-unit-tests \
              --disable-examples \
              --disable-libyuv \
              --disable-postproc \
              --disable-vp8 \
              --disable-tools \
              --disable-docs \
              --prefix=$PREFIX
  make -j5
  make install

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
  make -j5
  make install
  mv $PREFIX/lib/$ANDROID_ABI/liboboe.a $PREFIX/lib/
  popd
  echo "*** [$ANDROID_ABI][Finished] Build oboe"

  echo "*** [$ANDROID_ABI][All Finished]"
}

git clone -b v1.11.0 --depth=1 https://github.com/webmproject/libvpx.git build/libvpx
git clone -b 1.6.1 --depth=1 https://github.com/google/oboe build/oboe
patch -N -d build/oboe -p1 < ../src/oboe.patch

# VCPKG_TARGET	        ANDROID_ABI
#   arm64-android	        arm64-v8a
#   arm-android	          armeabi-v7a
#   x64-android	          x86_64
#   x86-android	          x86

# NDK_LLVM_TARGET
#   aarch64-linux-android
#   arm-linux-androideabi
#   x86_64-linux-android
#   i686-linux-android

# LIBVPX_TARGET :
#   arm64-android-gcc
#   armv7-android-gcc
#   x86_64-android-gcc
#   x86-android-gcc

# args: ANDROID_ABI  VCPKG_TARGET  NDK_LLVM_TARGET  LIBVPX_TARGET
build arm64-v8a arm64-android aarch64-linux-android arm64-android-gcc
build armeabi-v7a arm-android arm-linux-androideabi armv7-android-gcc

# rm -rf build/libvpx
# rm -rf build/oboe