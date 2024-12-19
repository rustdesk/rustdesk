#!/usr/bin/env bash
# https://docs.flutter.dev/deployment/ios
# flutter build ipa --release --obfuscate --split-debug-info=./split-debug-info
# no obfuscate, because no easy to check errors
cd $(dirname $(dirname $(which flutter)))
git apply ~/rustdesk/.github/patches/flutter_3.24.4_dropdown_menu_enableFilter.diff
flutter build ipa --release
