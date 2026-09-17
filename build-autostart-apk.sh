#!/bin/bash
# Local build script for RustDesk Android APK with Autostart
# This script mimics the GitHub Actions workflow locally

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}RustDesk Autostart APK Local Build${NC}"
echo "======================================"

# Configuration
ARCH="${1:-aarch64}"
VERSION="1.4.7"
RELEASE_TYPE="release"
JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64"

# Map arch to targets
case $ARCH in
  aarch64)
    TARGET="aarch64-linux-android"
    ANDROID_TARGET="arm64-v8a"
    ANDROID_PLATFORM="android-arm64"
    ;;
  armv7)
    TARGET="armv7-linux-androideabi"
    ANDROID_TARGET="armeabi-v7a"
    ANDROID_PLATFORM="android-arm"
    ;;
  x86_64)
    TARGET="x86_64-linux-android"
    ANDROID_TARGET="x86_64"
    ANDROID_PLATFORM="android-x64"
    ;;
  *)
    echo -e "${RED}Error: Invalid architecture ${ARCH}${NC}"
    echo "Usage: $0 [aarch64|armv7|x86_64]"
    exit 1
    ;;
esac

echo -e "${YELLOW}Configuration:${NC}"
echo "  Architecture: $ARCH"
echo "  Target: $TARGET"
echo "  Android Target: $ANDROID_TARGET"
echo "  Version: $VERSION"
echo ""

# Check prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

required_tools=("rustc" "cargo" "flutter" "gradle" "java")
for tool in "${required_tools[@]}"; do
  if ! command -v $tool &> /dev/null; then
    echo -e "${YELLOW}⚠ $tool not found - build may fail${NC}"
  else
    echo -e "${GREEN}✓ $tool found${NC}"
  fi
done

# Verify code modifications
echo -e "${YELLOW}Verifying code modifications...${NC}"
if grep -q "putExtra(EXT_INIT_FROM_BOOT, true)" flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/BootReceiver.kt; then
  echo -e "${GREEN}✓ BootReceiver.kt modified${NC}"
else
  echo -e "${RED}✗ BootReceiver.kt not properly modified${NC}"
fi

if grep -q "return START_STICKY" flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainService.kt; then
  echo -e "${GREEN}✓ MainService.kt modified (START_STICKY)${NC}"
else
  echo -e "${RED}✗ MainService.kt not properly modified${NC}"
fi

if grep -q "override fun onTaskRemoved" flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/MainService.kt; then
  echo -e "${GREEN}✓ MainService.kt modified (onTaskRemoved)${NC}"
else
  echo -e "${RED}✗ MainService.kt not properly modified${NC}"
fi

# Build steps
echo ""
echo -e "${YELLOW}Build Steps:${NC}"
echo "1. Build Rust library for $ARCH"
echo "2. Copy compiled libraries"
echo "3. Build Flutter APK"
echo "4. Create output APK"
echo ""

# Step 1: Build Rust library
if command -v cargo &> /dev/null; then
  echo -e "${YELLOW}[1/4] Building Rust library...${NC}"
  if [ -z "$ANDROID_NDK_HOME" ]; then
    echo -e "${RED}Error: ANDROID_NDK_HOME not set${NC}"
    echo "Please set: export ANDROID_NDK_HOME=/path/to/ndk"
    exit 1
  fi
  
  cargo build --release --target $TARGET || {
    echo -e "${RED}Rust build failed${NC}"
    exit 1
  }
  echo -e "${GREEN}✓ Rust library built${NC}"
else
  echo -e "${YELLOW}⚠ Skipping Rust build (cargo not found)${NC}"
fi

# Step 2: Copy libraries
echo -e "${YELLOW}[2/4] Copying compiled libraries...${NC}"
if [ -f "target/$TARGET/release/liblibrustdesk.so" ]; then
  mkdir -p "flutter/android/app/src/main/jniLibs/$ANDROID_TARGET"
  cp "target/$TARGET/release/liblibrustdesk.so" "flutter/android/app/src/main/jniLibs/$ANDROID_TARGET/"
  echo -e "${GREEN}✓ Libraries copied${NC}"
else
  echo -e "${YELLOW}⚠ liblibrustdesk.so not found (expected if Rust build skipped)${NC}"
fi

# Step 3: Build Flutter APK
if command -v flutter &> /dev/null; then
  echo -e "${YELLOW}[3/4] Building Flutter APK...${NC}"
  
  # Modify gradle configuration
  sed -i "s/org.gradle.jvmargs=-Xmx1024M/org.gradle.jvmargs=-Xmx2g/g" flutter/android/gradle.properties
  sed -i "s/signingConfigs.release/signingConfigs.debug/g" flutter/android/app/build.gradle
  
  export PATH="$JAVA_HOME/bin:$PATH"
  
  cd flutter
  flutter build apk --release --target-platform $ANDROID_PLATFORM --split-per-abi || {
    echo -e "${RED}Flutter build failed${NC}"
    cd ..
    exit 1
  }
  cd ..
  
  echo -e "${GREEN}✓ Flutter APK built${NC}"
else
  echo -e "${RED}Error: flutter not found${NC}"
  exit 1
fi

# Step 4: Create output
echo -e "${YELLOW}[4/4] Creating output APK...${NC}"
OUTPUT_APK="flutter/build/app/outputs/flutter-apk/app-${ANDROID_TARGET}-${RELEASE_TYPE}.apk"
FINAL_APK="rustdesk-autostart-${VERSION}-${ARCH}.apk"

if [ -f "$OUTPUT_APK" ]; then
  cp "$OUTPUT_APK" "$FINAL_APK"
  echo -e "${GREEN}✓ APK created: $FINAL_APK${NC}"
  ls -lh "$FINAL_APK"
else
  echo -e "${RED}Error: Output APK not found at $OUTPUT_APK${NC}"
  exit 1
fi

echo ""
echo -e "${GREEN}Build complete!${NC}"
echo -e "${GREEN}Output: $FINAL_APK${NC}"
echo ""
echo "Installation instructions:"
echo "  adb install -r $FINAL_APK"
echo ""
