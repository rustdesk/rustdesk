#!/usr/bin/env bash

set -eo pipefail

MODE=${MODE:=release}

SCRIPTDIR="$(readlink -f "${0}")"
SCRIPTDIR="$(dirname "${SCRIPTDIR}")"

cd "${SCRIPTDIR}"

"${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip" \
	android/app/src/main/jniLibs/arm64-v8a/*

# Bail out on uncommitted changes to pubspec files
python3 "${SCRIPTDIR}/flutter_pub_get.py" check-local-pubspec-changes
# Revert possible pubspec alterations and package config freeze for idempotency
python3 "${SCRIPTDIR}/flutter_pub_get.py" revert-local-pubspec-changes
# Download flutter packages with default lockfile
python3 "${SCRIPTDIR}/flutter_pub_get.py" get-packages

# Revert possible pubspec and package config alterations
# on both successful operation and on error for idempotency
trap 'python3 "${SCRIPTDIR}/flutter_pub_get.py" revert-local-pubspec-changes' 0 1 2 3 6 15

flutter build apk --target-platform android-arm64,android-arm "--${MODE}" --obfuscate --split-debug-info ./split-debug-info
flutter build apk --split-per-abi --target-platform android-arm64,android-arm "--${MODE}" --obfuscate --split-debug-info ./split-debug-info
flutter build appbundle --target-platform android-arm64,android-arm "--${MODE}" --obfuscate --split-debug-info ./split-debug-info
