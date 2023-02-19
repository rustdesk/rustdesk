#!/usr/bin/env bash
$ANDROID_NDK_HOME/toolchains/aarch64-linux-android-4.9/prebuilt/linux-x86_64/bin/aarch64-linux-android-strip android/app/src/main/jniLibs/arm64-v8a/*
#flutter build apk --target-platform android-arm64,android-arm --release --obfuscate --split-debug-info ./split-debug-info
flutter build apk --split-per-abi --target-platform android-arm64,android-arm --release --obfuscate --split-debug-info ./split-debug-info
#flutter build appbundle --target-platform  android-arm64,android-arm --release --obfuscate --split-debug-info ./split-debug-info

# build in linux
# $ANDROID_NDK/toolchains/aarch64-linux-android-4.9/prebuilt/linux-x86_64/bin/aarch64-linux-android-strip android/app/src/main/jniLibs/arm64-v8a/*
