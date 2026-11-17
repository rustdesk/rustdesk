#! /bin/sh

export PREFIX="$(pwd)/libsodium-apple"
export MACOS_ARM64_PREFIX="${PREFIX}/tmp/macos-arm64"
export MACOS_X86_64_PREFIX="${PREFIX}/tmp/macos-x86_64"
export IOS32_PREFIX="${PREFIX}/tmp/ios32"
export IOS32s_PREFIX="${PREFIX}/tmp/ios32s"
export IOS64_PREFIX="${PREFIX}/tmp/ios64"
export IOS_SIMULATOR_ARM64_PREFIX="${PREFIX}/tmp/ios-simulator-arm64"
export IOS_SIMULATOR_I386_PREFIX="${PREFIX}/tmp/ios-simulator-i386"
export IOS_SIMULATOR_X86_64_PREFIX="${PREFIX}/tmp/ios-simulator-x86_64"
export WATCHOS32_PREFIX="${PREFIX}/tmp/watchos32"
export WATCHOS64_32_PREFIX="${PREFIX}/tmp/watchos64_32"
export WATCHOS_SIMULATOR_ARM64_PREFIX="${PREFIX}/tmp/watchos-simulator-arm64"
export WATCHOS_SIMULATOR_I386_PREFIX="${PREFIX}/tmp/watchos-simulator-i386"
export WATCHOS_SIMULATOR_X86_64_PREFIX="${PREFIX}/tmp/watchos-simulator-x86_64"
export TVOS64_PREFIX="${PREFIX}/tmp/tvos64"
export TVOS_SIMULATOR_ARM64_PREFIX="${PREFIX}/tmp/tvos-simulator-arm64"
export TVOS_SIMULATOR_X86_64_PREFIX="${PREFIX}/tmp/tvos-simulator-x86_64"
export CATALYST_ARM64_PREFIX="${PREFIX}/tmp/catalyst-arm64"
export CATALYST_X86_64_PREFIX="${PREFIX}/tmp/catalyst-x86_64"
export LOG_FILE="${PREFIX}/tmp/build_log"
export XCODEDIR="$(xcode-select -p)"

export MACOS_VERSION_MIN=${MACOS_VERSION_MIN-"10.10"}
export IOS_SIMULATOR_VERSION_MIN=${IOS_SIMULATOR_VERSION_MIN-"9.0.0"}
export IOS_VERSION_MIN=${IOS_VERSION_MIN-"9.0.0"}
export WATCHOS_SIMULATOR_VERSION_MIN=${WATCHOS_SIMULATOR_VERSION_MIN-"4.0.0"}
export WATCHOS_VERSION_MIN=${WATCHOS_VERSION_MIN-"4.0.0"}
export TVOS_SIMULATOR_VERSION_MIN=${TVOS_SIMULATOR_VERSION_MIN-"9.0.0"}
export TVOS_VERSION_MIN=${TVOS_VERSION_MIN-"9.0.0"}

echo
echo "Warnings related to headers being present but not usable are due to functions"
echo "that didn't exist in the specified minimum iOS version level."
echo "They can be safely ignored."
echo

if [ -z "$LIBSODIUM_FULL_BUILD" ]; then
  export LIBSODIUM_ENABLE_MINIMAL_FLAG="--enable-minimal"
else
  export LIBSODIUM_ENABLE_MINIMAL_FLAG=""
fi

APPLE_SILICON_SUPPORTED=false
echo 'int main(void){return 0;}' >comptest.c && cc --target=arm64-macos comptest.c 2>/dev/null && APPLE_SILICON_SUPPORTED=true
rm -f comptest.c

NPROCESSORS=$(getconf NPROCESSORS_ONLN 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null)
PROCESSORS=${NPROCESSORS:-3}

swift_module_map() {
  echo 'module Clibsodium {'
  echo '    header "sodium.h"'
  echo '    export *'
  echo '}'
}

build_macos() {
  export BASEDIR="${XCODEDIR}/Platforms/MacOSX.platform/Developer"
  export PATH="${BASEDIR}/usr/bin:$BASEDIR/usr/sbin:$PATH"

  ## macOS arm64
  if [ "$APPLE_SILICON_SUPPORTED" = "true" ]; then
    export CFLAGS="-O2 -arch arm64 -mmacosx-version-min=${MACOS_VERSION_MIN}"
    export LDFLAGS="-arch arm64 -mmacosx-version-min=${MACOS_VERSION_MIN}"

    make distclean >/dev/null 2>&1
    ./configure --host=arm-apple-darwin20 --prefix="$MACOS_ARM64_PREFIX" \
      ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
    make -j${PROCESSORS} install || exit 1
  fi

  ## macOS x86_64
  export CFLAGS="-O2 -arch x86_64 -mmacosx-version-min=${MACOS_VERSION_MIN}"
  export LDFLAGS="-arch x86_64 -mmacosx-version-min=${MACOS_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=x86_64-apple-darwin10 --prefix="$MACOS_X86_64_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1
}

build_ios() {
  export BASEDIR="${XCODEDIR}/Platforms/iPhoneOS.platform/Developer"
  export PATH="${BASEDIR}/usr/bin:$BASEDIR/usr/sbin:$PATH"
  export SDK="${BASEDIR}/SDKs/iPhoneOS.sdk"

  ## 32-bit iOS
  export CFLAGS="-fembed-bitcode -O2 -mthumb -arch armv7 -isysroot ${SDK} -mios-version-min=${IOS_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -mthumb -arch armv7 -isysroot ${SDK} -mios-version-min=${IOS_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=arm-apple-darwin10 --prefix="$IOS32_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1

  ## 32-bit armv7s iOS
  export CFLAGS="-fembed-bitcode -O2 -mthumb -arch armv7s -isysroot ${SDK} -mios-version-min=${IOS_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -mthumb -arch armv7s -isysroot ${SDK} -mios-version-min=${IOS_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=arm-apple-darwin10 --prefix="$IOS32s_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1

  ## 64-bit iOS
  export CFLAGS="-fembed-bitcode -O2 -arch arm64 -isysroot ${SDK} -mios-version-min=${IOS_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -arch arm64 -isysroot ${SDK} -mios-version-min=${IOS_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=arm-apple-darwin10 --prefix="$IOS64_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1
}

build_ios_simulator() {
  export BASEDIR="${XCODEDIR}/Platforms/iPhoneSimulator.platform/Developer"
  export PATH="${BASEDIR}/usr/bin:$BASEDIR/usr/sbin:$PATH"
  export SDK="${BASEDIR}/SDKs/iPhoneSimulator.sdk"

  ## arm64 simulator
  if [ "$APPLE_SILICON_SUPPORTED" = "true" ]; then
    export CFLAGS="-fembed-bitcode -O2 -arch arm64 -isysroot ${SDK} -mios-simulator-version-min=${IOS_SIMULATOR_VERSION_MIN}"
    export LDFLAGS="-fembed-bitcode -arch arm64 -isysroot ${SDK} -mios-simulator-version-min=${IOS_SIMULATOR_VERSION_MIN}"

    make distclean >/dev/null 2>&1
    ./configure --host=arm-apple-darwin20 --prefix="$IOS_SIMULATOR_ARM64_PREFIX" \
      ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
    make -j${PROCESSORS} install || exit 1
  fi

  ## i386 simulator
  export CFLAGS="-fembed-bitcode -O2 -arch i386 -isysroot ${SDK} -mios-simulator-version-min=${IOS_SIMULATOR_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -arch i386 -isysroot ${SDK} -mios-simulator-version-min=${IOS_SIMULATOR_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=i686-apple-darwin10 --prefix="$IOS_SIMULATOR_I386_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1

  ## x86_64 simulator
  export CFLAGS="-fembed-bitcode -O2 -arch x86_64 -isysroot ${SDK} -mios-simulator-version-min=${IOS_SIMULATOR_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -arch x86_64 -isysroot ${SDK} -mios-simulator-version-min=${IOS_SIMULATOR_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=x86_64-apple-darwin10 --prefix="$IOS_SIMULATOR_X86_64_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG}
  make -j${PROCESSORS} install || exit 1
}

build_watchos() {
  export BASEDIR="${XCODEDIR}/Platforms/WatchOS.platform/Developer"
  export PATH="${BASEDIR}/usr/bin:$BASEDIR/usr/sbin:$PATH"
  export SDK="${BASEDIR}/SDKs/WatchOS.sdk"

  # 32-bit watchOS
  export CFLAGS="-fembed-bitcode -O2 -mthumb -arch armv7k -isysroot ${SDK} -mwatchos-version-min=${WATCHOS_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -mthumb -arch armv7k -isysroot ${SDK} -mwatchos-version-min=${WATCHOS_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=arm-apple-darwin10 --prefix="$WATCHOS32_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1

  ## 64-bit arm64_32 watchOS
  export CFLAGS="-fembed-bitcode -O2 -mthumb -arch arm64_32 -isysroot ${SDK} -mwatchos-version-min=${WATCHOS_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -mthumb -arch arm64_32 -isysroot ${SDK} -mwatchos-version-min=${WATCHOS_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=arm-apple-darwin10 --prefix="$WATCHOS64_32_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1
}

build_watchos_simulator() {
  export BASEDIR="${XCODEDIR}/Platforms/WatchSimulator.platform/Developer"
  export PATH="${BASEDIR}/usr/bin:$BASEDIR/usr/sbin:$PATH"
  export SDK="${BASEDIR}/SDKs/WatchSimulator.sdk"

  ## arm64 simulator
  if [ "$APPLE_SILICON_SUPPORTED" = "true" ]; then
    export CFLAGS="-fembed-bitcode -O2 -arch arm64 -isysroot ${SDK} -mwatchos-simulator-version-min=${WATCHOS_SIMULATOR_VERSION_MIN}"
    export LDFLAGS="-fembed-bitcode -arch arm64 -isysroot ${SDK} -mwatchos-simulator-version-min=${WATCHOS_SIMULATOR_VERSION_MIN}"

    make distclean >/dev/null 2>&1
    ./configure --host=arm-apple-darwin20 --prefix="$WATCHOS_SIMULATOR_ARM64_PREFIX" \
      ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
    make -j${PROCESSORS} install || exit 1
  fi

  ## i386 simulator
  export CFLAGS="-fembed-bitcode -O2 -arch i386 -isysroot ${SDK} -mwatchos-simulator-version-min=${WATCHOS_SIMULATOR_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -arch i386 -isysroot ${SDK} -mwatchos-simulator-version-min=${WATCHOS_SIMULATOR_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=i686-apple-darwin10 --prefix="$WATCHOS_SIMULATOR_I386_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1

  ## x86_64 simulator
  export CFLAGS="-fembed-bitcode -O2 -arch x86_64 -isysroot ${SDK} -mwatchos-simulator-version-min=${WATCHOS_SIMULATOR_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -arch x86_64 -isysroot ${SDK} -mwatchos-simulator-version-min=${WATCHOS_SIMULATOR_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=x86_64-apple-darwin10 --prefix="$WATCHOS_SIMULATOR_X86_64_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1
}

build_tvos() {
  export BASEDIR="${XCODEDIR}/Platforms/AppleTVOS.platform/Developer"
  export PATH="${BASEDIR}/usr/bin:$BASEDIR/usr/sbin:$PATH"
  export SDK="${BASEDIR}/SDKs/AppleTVOS.sdk"

  ## 64-bit tvOS
  export CFLAGS="-fembed-bitcode -O2 -arch arm64 -isysroot ${SDK} -mtvos-version-min=${TVOS_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -arch arm64 -isysroot ${SDK} -mtvos-version-min=${TVOS_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=arm-apple-darwin10 --prefix="$TVOS64_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1
}

build_tvos_simulator() {
  export BASEDIR="${XCODEDIR}/Platforms/AppleTVSimulator.platform/Developer"
  export PATH="${BASEDIR}/usr/bin:$BASEDIR/usr/sbin:$PATH"
  export SDK="${BASEDIR}/SDKs/AppleTVSimulator.sdk"

  ## arm64 simulator
  if [ "$APPLE_SILICON_SUPPORTED" = "true" ]; then
    export CFLAGS="-fembed-bitcode -O2 -arch arm64 -isysroot ${SDK} -mtvos-simulator-version-min=${TVOS_SIMULATOR_VERSION_MIN}"
    export LDFLAGS="-fembed-bitcode -arch arm64 -isysroot ${SDK} -mtvos-simulator-version-min=${TVOS_SIMULATOR_VERSION_MIN}"

    make distclean >/dev/null 2>&1
    ./configure --host=arm-apple-darwin20 --prefix="$TVOS_SIMULATOR_ARM64_PREFIX" \
      ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
    make -j${PROCESSORS} install || exit 1
  fi

  ## x86_64 simulator
  export CFLAGS="-fembed-bitcode -O2 -arch x86_64 -isysroot ${SDK} -mtvos-simulator-version-min=${TVOS_SIMULATOR_VERSION_MIN}"
  export LDFLAGS="-fembed-bitcode -arch x86_64 -isysroot ${SDK} -mtvos-simulator-version-min=${TVOS_SIMULATOR_VERSION_MIN}"

  make distclean >/dev/null 2>&1
  ./configure --host=x86_64-apple-darwin10 --prefix="$TVOS_SIMULATOR_X86_64_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG}
  make -j${PROCESSORS} install || exit 1
}

build_catalyst() {
  export BASEDIR="${XCODEDIR}/Platforms/MacOSX.platform/Developer"
  export PATH="${BASEDIR}/usr/bin:$BASEDIR/usr/sbin:$PATH"
  export SDK="${BASEDIR}/SDKs/MacOSX.sdk"

  ## arm64 catalyst
  if [ "$APPLE_SILICON_SUPPORTED" = "true" ]; then
    export CFLAGS="-O2 -arch arm64 -target arm64-apple-ios13.0-macabi -isysroot ${SDK}"
    export LDFLAGS="-arch arm64 -target arm64-apple-ios13.0-macabi -isysroot ${SDK}"

    make distclean >/dev/null 2>&1
    ./configure --host=arm-apple-ios --prefix="$CATALYST_ARM64_PREFIX" \
      ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
    make -j${PROCESSORS} install || exit 1
  fi

  ## x86_64 catalyst
  export CFLAGS="-O2 -arch x86_64 -target x86_64-apple-ios13.0-macabi -isysroot ${SDK}"
  export LDFLAGS="-arch x86_64 -target x86_64-apple-ios13.0-macabi -isysroot ${SDK}"

  make distclean >/dev/null 2>&1
  ./configure --host=x86_64-apple-ios --prefix="$CATALYST_X86_64_PREFIX" \
    ${LIBSODIUM_ENABLE_MINIMAL_FLAG} || exit 1
  make -j${PROCESSORS} install || exit 1
}

mkdir -p "${PREFIX}/tmp"
echo "Building for macOS..."
build_macos >"$LOG_FILE" 2>&1 || exit 1
echo "Building for iOS..."
build_ios >"$LOG_FILE" 2>&1 || exit 1
echo "Building for the iOS simulator..."
build_ios_simulator >"$LOG_FILE" 2>&1 || exit 1
echo "Building for watchOS..."
build_watchos >"$LOG_FILE" 2>&1 || exit 1
echo "Building for the watchOS simulator..."
build_watchos_simulator >"$LOG_FILE" 2>&1 || exit 1
echo "Building for tvOS..."
build_tvos >"$LOG_FILE" 2>&1 || exit 1
echo "Building for the tvOS simulator..."
build_tvos_simulator >"$LOG_FILE" 2>&1 || exit 1
echo "Building for Catalyst..."
build_catalyst >"$LOG_FILE" 2>&1 || exit 1

echo "Adding the Clibsodium module map for Swift..."

find "$PREFIX" -name "include" -type d -print | while read -r f; do
  swift_module_map >"${f}/module.modulemap"
done

echo "Bundling macOS targets..."

mkdir -p "${PREFIX}/macos/lib"
cp -a "${MACOS_X86_64_PREFIX}/include" "${PREFIX}/macos/"
for ext in a dylib; do
  if [ "$APPLE_SILICON_SUPPORTED" = "true" ]; then
    lipo -create \
      "${MACOS_ARM64_PREFIX}/lib/libsodium.${ext}" \
      "${MACOS_X86_64_PREFIX}/lib/libsodium.${ext}" \
      -output "${PREFIX}/macos/lib/libsodium.${ext}"
  else
    lipo -create \
      "${MACOS_X86_64_PREFIX}/lib/libsodium.${ext}" \
      -output "${PREFIX}/macos/lib/libsodium.${ext}"
  fi
done

echo "Bundling iOS targets..."

mkdir -p "${PREFIX}/ios/lib"
cp -a "${IOS64_PREFIX}/include" "${PREFIX}/ios/"
for ext in a dylib; do
  lipo -create \
    "$IOS32_PREFIX/lib/libsodium.${ext}" \
    "$IOS32s_PREFIX/lib/libsodium.${ext}" \
    "$IOS64_PREFIX/lib/libsodium.${ext}" \
    -output "$PREFIX/ios/lib/libsodium.${ext}"
done

echo "Bundling iOS simulators..."

mkdir -p "${PREFIX}/ios-simulators/lib"
cp -a "${IOS_SIMULATOR_X86_64_PREFIX}/include" "${PREFIX}/ios-simulators/"
for ext in a dylib; do
  if [ "$APPLE_SILICON_SUPPORTED" = "true" ]; then
    lipo -create \
      "${IOS_SIMULATOR_ARM64_PREFIX}/lib/libsodium.${ext}" \
      "${IOS_SIMULATOR_I386_PREFIX}/lib/libsodium.${ext}" \
      "${IOS_SIMULATOR_X86_64_PREFIX}/lib/libsodium.${ext}" \
      -output "${PREFIX}/ios-simulators/lib/libsodium.${ext}" || exit 1
  else
    lipo -create \
      "${IOS_SIMULATOR_I386_PREFIX}/lib/libsodium.${ext}" \
      "${IOS_SIMULATOR_X86_64_PREFIX}/lib/libsodium.${ext}" \
      -output "${PREFIX}/ios-simulators/lib/libsodium.${ext}" || exit 1
  fi
done

echo "Bundling watchOS targets..."

mkdir -p "${PREFIX}/watchos/lib"
cp -a "${WATCHOS64_32_PREFIX}/include" "${PREFIX}/watchos/"
for ext in a dylib; do
  lipo -create \
    "${WATCHOS32_PREFIX}/lib/libsodium.${ext}" \
    "${WATCHOS64_32_PREFIX}/lib/libsodium.${ext}" \
    -output "${PREFIX}/watchos/lib/libsodium.${ext}"
done

echo "Bundling watchOS simulators..."

mkdir -p "${PREFIX}/watchos-simulators/lib"
cp -a "${WATCHOS_SIMULATOR_X86_64_PREFIX}/include" "${PREFIX}/watchos-simulators/"
for ext in a dylib; do
  if [ "$APPLE_SILICON_SUPPORTED" = "true" ]; then
    lipo -create \
      "${WATCHOS_SIMULATOR_ARM64_PREFIX}/lib/libsodium.${ext}" \
      "${WATCHOS_SIMULATOR_I386_PREFIX}/lib/libsodium.${ext}" \
      "${WATCHOS_SIMULATOR_X86_64_PREFIX}/lib/libsodium.${ext}" \
      -output "${PREFIX}/watchos-simulators/lib/libsodium.${ext}"
  else
    lipo -create \
      "${WATCHOS_SIMULATOR_I386_PREFIX}/lib/libsodium.${ext}" \
      "${WATCHOS_SIMULATOR_X86_64_PREFIX}/lib/libsodium.${ext}" \
      -output "${PREFIX}/watchos-simulators/lib/libsodium.${ext}"
  fi
done

echo "Bundling tvOS targets..."

mkdir -p "${PREFIX}/tvos/lib"
cp -a "${TVOS64_PREFIX}/include" "${PREFIX}/tvos/"
for ext in a dylib; do
  lipo -create \
    "$TVOS64_PREFIX/lib/libsodium.${ext}" \
    -output "$PREFIX/tvos/lib/libsodium.${ext}"
done

echo "Bundling tvOS simulators..."

mkdir -p "${PREFIX}/tvos-simulators/lib"
cp -a "${TVOS_SIMULATOR_X86_64_PREFIX}/include" "${PREFIX}/tvos-simulators/"
for ext in a dylib; do
  if [ "$APPLE_SILICON_SUPPORTED" = "true" ]; then
    lipo -create \
      "${TVOS_SIMULATOR_ARM64_PREFIX}/lib/libsodium.${ext}" \
      "${TVOS_SIMULATOR_X86_64_PREFIX}/lib/libsodium.${ext}" \
      -output "${PREFIX}/tvos-simulators/lib/libsodium.${ext}" || exit 1
  else
    lipo -create \
      "${TVOS_SIMULATOR_X86_64_PREFIX}/lib/libsodium.${ext}" \
      -output "${PREFIX}/tvos-simulators/lib/libsodium.${ext}" || exit 1
  fi
done

echo "Bundling Catalyst targets..."

mkdir -p "${PREFIX}/catalyst/lib"
cp -a "${CATALYST_X86_64_PREFIX}/include" "${PREFIX}/catalyst/"
for ext in a dylib; do
  if [ ! -f "${CATALYST_X86_64_PREFIX}/lib/libsodium.${ext}" ]; then
    continue
  fi
  if [ "$APPLE_SILICON_SUPPORTED" = "true" ]; then
    lipo -create \
      "${CATALYST_ARM64_PREFIX}/lib/libsodium.${ext}" \
      "${CATALYST_X86_64_PREFIX}/lib/libsodium.${ext}" \
      -output "${PREFIX}/catalyst/lib/libsodium.${ext}"
  else
    lipo -create \
      "${CATALYST_X86_64_PREFIX}/lib/libsodium.${ext}" \
      -output "${PREFIX}/catalyst/lib/libsodium.${ext}"
  fi
done

echo "Creating Clibsodium.xcframework..."

rm -rf "${PREFIX}/Clibsodium.xcframework"

XCFRAMEWORK_ARGS=""
for f in macos ios ios-simulators watchos watchos-simulators tvos tvos-simulators catalyst; do
  XCFRAMEWORK_ARGS="${XCFRAMEWORK_ARGS} -library ${PREFIX}/${f}/lib/libsodium.a"
  XCFRAMEWORK_ARGS="${XCFRAMEWORK_ARGS} -headers ${PREFIX}/${f}/include"
done
xcodebuild -create-xcframework \
  ${XCFRAMEWORK_ARGS} \
  -output "${PREFIX}/Clibsodium.xcframework" >/dev/null

ls -ld -- "$PREFIX"
ls -l -- "$PREFIX"
ls -l -- "$PREFIX/Clibsodium.xcframework"

echo "Done!"

# Cleanup
rm -rf -- "$PREFIX/tmp"
make distclean >/dev/null
