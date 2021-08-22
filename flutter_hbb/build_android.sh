#!/usr/bin/env bash
flutter build apk --target-platform  android-arm64  --release --obfuscate --split-debug-info ./split-debug-info
flutter build appbundle --target-platform  android-arm64  --release --obfuscate --split-debug-info ./split-debug-info
