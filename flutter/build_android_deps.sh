#!/bin/bash

# Build libyuv / opus / libvpx / oboe for Android 
# Required: 
#   1. set VCPKG_ROOT / ANDROID_NDK path environment variables
#   2. vcpkg initialized
#   3. ndk >= 22 (if ndk< 22 you need to change LD as `export LD=$TOOLCHAIN/bin/aarch64-linux-android-ld`)

if [ -z "$ANDROID_NDK" ]; then
  echo "Failed! Please set ANDROID_NDK"
  exit 1
fi

if [ -z "$VCPKG_ROOT" ]; then
  echo "Failed! Please set VCPKG_ROOT"
  exit 1
fi

PREFIX=$VCPKG_ROOT/installed/arm64-android/

echo "*** [Start] Build opus / libyuv from vcpkg"
export ANDROID_NDK_HOME=$ANDROID_NDK
pushd $VCPKG_ROOT
$VCPKG_ROOT/vcpkg install opus --triplet arm64-android
$VCPKG_ROOT/vcpkg install libyuv --triplet arm64-android
popd
echo "*** [Finished] Build opus / libyuv from vcpkg"

echo "*** [Start] Build libvpx"
git clone -b v1.11.0 --depth=1 https://github.com/webmproject/libvpx.git build/libvpx
pushd build/libvpx
export NDK=$ANDROID_NDK
export HOST_TAG=linux-x86_64
export TOOLCHAIN=$NDK/toolchains/llvm/prebuilt/$HOST_TAG
export AR=$TOOLCHAIN/bin/aarch64-linux-android-ar
export AS=$TOOLCHAIN/bin/aarch64-linux-android-as
export CC=$TOOLCHAIN/bin/aarch64-linux-android21-clang
export CXX=$TOOLCHAIN/bin/aarch64-linux-android21-clang++
export LD=$TOOLCHAIN/bin/aarch64-linux-android-ld.gold  # if ndk < 22, use aarch64-linux-android-ld
export RANLIB=$TOOLCHAIN/bin/aarch64-linux-android-ranlib
export STRIP=$TOOLCHAIN/bin/aarch64-linux-android-strip

./configure --target=arm64-android-gcc \
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
echo "*** [Finished] Build libvpx"


echo "*** [Start] Build oboe"
git clone -b 1.6.1 --depth=1 https://github.com/google/oboe build/oboe
patch -d build/oboe -p1 < ../src/oboe.patch
pushd build/oboe
cmake -DBUILD_SHARED_LIBS=true \
        -DCMAKE_BUILD_TYPE=RelWithDebInfo \
        -DANDROID_TOOLCHAIN=clang \
        -DANDROID_STL=c++_shared \
        -DCMAKE_TOOLCHAIN_FILE=$ANDROID_NDK/build/cmake/android.toolchain.cmake \
        -DCMAKE_INSTALL_PREFIX=$PREFIX \
        -DANDROID_ABI=arm64-v8a \
        -DANDROID_PLATFORM=android-21
make -j5
make install
mv $PREFIX/lib/arm64-v8a/liboboe.a $PREFIX/lib/
popd
echo "*** [Finished] Build oboe"
echo "*** [All Finished]"

# rm -rf build/libvpx
# rm -rf build/oboe