#!/usr/bin/env bash

MODE=${MODE:=release}

# 从环境变量读取 APP_NAME，如果未设置则使用默认值 RustDesk
APP_NAME_VALUE=${APP_NAME:-RustDesk}

# 替换 strings.xml 中的 app_name
echo "Setting app_name to: ${APP_NAME_VALUE}"
sed -i "s|<string name=\"app_name\">.*</string>|<string name=\"app_name\">${APP_NAME_VALUE}</string>|" android/app/src/main/res/values/strings.xml

$ANDROID_NDK_HOME/toolchains/aarch64-linux-android-4.9/prebuilt/linux-x86_64/bin/aarch64-linux-android-strip android/app/src/main/jniLibs/arm64-v8a/*
flutter build apk --target-platform android-arm64,android-arm --${MODE} --obfuscate --split-debug-info ./split-debug-info
flutter build apk --split-per-abi --target-platform android-arm64,android-arm --${MODE} --obfuscate --split-debug-info ./split-debug-info
flutter build appbundle --target-platform  android-arm64,android-arm --${MODE} --obfuscate --split-debug-info ./split-debug-info

# build in linux
# $ANDROID_NDK/toolchains/aarch64-linux-android-4.9/prebuilt/linux-x86_64/bin/aarch64-linux-android-strip android/app/src/main/jniLibs/arm64-v8a/*
