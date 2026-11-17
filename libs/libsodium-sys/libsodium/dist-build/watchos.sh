#! /bin/sh
#
#  Step 1.
#  Configure for base system so simulator is covered
#
#  Step 2.
#  Make for watchOS and watchOS simulator
#
#  Step 3.
#  Merge libs into final version for xcode import

export PREFIX="$(pwd)/libsodium-watchos"
export WATCHOS32_PREFIX="$PREFIX/tmp/watchos32"
export WATCHOS64_32_PREFIX="$PREFIX/tmp/watchos64_32"
export SIMULATOR32_PREFIX="$PREFIX/tmp/simulator32"
export SIMULATOR64_PREFIX="$PREFIX/tmp/simulator64"
export XCODEDIR=$(xcode-select -p)

export WATCHOS_SIMULATOR_VERSION_MIN=${WATCHOS_SIMULATOR_VERSION_MIN-"4.0.0"}
export WATCHOS_VERSION_MIN=${WATCHOS_VERSION_MIN-"4.0.0"}

mkdir -p $SIMULATOR32_PREFIX $SIMULATOR64_PREFIX $WATCHOS32_PREFIX $WATCHOS64_32_PREFIX || exit 1

# Build for the simulator
export BASEDIR="${XCODEDIR}/Platforms/WatchSimulator.platform/Developer"
export PATH="${BASEDIR}/usr/bin:$BASEDIR/usr/sbin:$PATH"
export SDK="${BASEDIR}/SDKs/WatchSimulator.sdk"

## i386 simulator
export CFLAGS="-O2 -arch i386 -isysroot ${SDK} -mwatchos-simulator-version-min=${WATCHOS_SIMULATOR_VERSION_MIN}"
export LDFLAGS="-arch i386 -isysroot ${SDK} -mwatchos-simulator-version-min=${WATCHOS_SIMULATOR_VERSION_MIN}"

make distclean > /dev/null

if [ -z "$LIBSODIUM_FULL_BUILD" ]; then
  export LIBSODIUM_ENABLE_MINIMAL_FLAG="--enable-minimal"
else
  export LIBSODIUM_ENABLE_MINIMAL_FLAG=""
fi

./configure --host=i686-apple-darwin10 \
            --disable-shared \
            ${LIBSODIUM_ENABLE_MINIMAL_FLAG} \
            --prefix="$SIMULATOR32_PREFIX" || exit 1


NPROCESSORS=$(getconf NPROCESSORS_ONLN 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null)
PROCESSORS=${NPROCESSORS:-3}

make -j${PROCESSORS} install || exit 1

## x86_64 simulator
export CFLAGS="-O2 -arch x86_64 -isysroot ${SDK} -mwatchos-simulator-version-min=${WATCHOS_SIMULATOR_VERSION_MIN}"
export LDFLAGS="-arch x86_64 -isysroot ${SDK} -mwatchos-simulator-version-min=${WATCHOS_SIMULATOR_VERSION_MIN}"

make distclean > /dev/null

./configure --host=x86_64-apple-darwin10 \
            --disable-shared \
            ${LIBSODIUM_ENABLE_MINIMAL_FLAG} \
            --prefix="$SIMULATOR64_PREFIX"

make -j${PROCESSORS} install || exit 1

# Build for watchOS
export BASEDIR="${XCODEDIR}/Platforms/WatchOS.platform/Developer"
export PATH="${BASEDIR}/usr/bin:$BASEDIR/usr/sbin:$PATH"
export SDK="${BASEDIR}/SDKs/WatchOS.sdk"

## 32-bit watchOS
export CFLAGS="-fembed-bitcode -O2 -mthumb -arch armv7k -isysroot ${SDK} -mwatchos-version-min=${WATCHOS_VERSION_MIN}"
export LDFLAGS="-fembed-bitcode -mthumb -arch armv7k -isysroot ${SDK} -mwatchos-version-min=${WATCHOS_VERSION_MIN}"

make distclean > /dev/null

./configure --host=arm-apple-darwin10 \
            --disable-shared \
            ${LIBSODIUM_ENABLE_MINIMAL_FLAG} \
            --prefix="$WATCHOS32_PREFIX" || exit 1

make -j${PROCESSORS} install || exit 1

## 64-bit arm64_32 watchOS
export CFLAGS="-fembed-bitcode -O2 -mthumb -arch arm64_32 -isysroot ${SDK} -mwatchos-version-min=${WATCHOS_VERSION_MIN}"
export LDFLAGS="-fembed-bitcode -mthumb -arch arm64_32 -isysroot ${SDK} -mwatchos-version-min=${WATCHOS_VERSION_MIN}"

make distclean > /dev/null

./configure --host=arm-apple-darwin10 \
            --disable-shared \
            ${LIBSODIUM_ENABLE_MINIMAL_FLAG} \
            --prefix="$WATCHOS64_32_PREFIX" || exit 1

make -j${PROCESSORS} install || exit 1

# Create universal binary and include folder
rm -fr -- "$PREFIX/include" "$PREFIX/libsodium.a" 2> /dev/null
mkdir -p -- "$PREFIX/lib"
lipo -create \
  "$SIMULATOR32_PREFIX/lib/libsodium.a" \
  "$SIMULATOR64_PREFIX/lib/libsodium.a" \
  "$WATCHOS32_PREFIX/lib/libsodium.a" \
  "$WATCHOS64_32_PREFIX/lib/libsodium.a" \
  -output "$PREFIX/lib/libsodium.a"
mv -f -- "$WATCHOS32_PREFIX/include" "$PREFIX/"

echo
echo "libsodium has been installed into $PREFIX"
echo
file -- "$PREFIX/lib/libsodium.a"

# Cleanup
rm -rf -- "$PREFIX/tmp"
make distclean > /dev/null
