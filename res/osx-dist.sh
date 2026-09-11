#!/usr/bin/env bash

echo $MACOS_CODESIGN_IDENTITY
cargo install flutter_rust_bridge_codegen --version 1.80.1 --features uuid --locked
cd flutter; flutter pub get; cd -
~/.cargo/bin/flutter_rust_bridge_codegen --rust-input ./src/flutter_ffi.rs --dart-output ./flutter/lib/generated_bridge.dart --c-output ./flutter/macos/Runner/bridge_generated.h
./build.py --flutter
rm rustdesk-$VERSION.dmg
# security find-identity -v
APP_PATH="$(ls -d ./flutter/build/macos/Build/Products/Release/*.app | head -n 1)"
APP_NAME="$(basename "$APP_PATH")"
codesign --force --options runtime -s $MACOS_CODESIGN_IDENTITY --deep --strict "$APP_PATH" -vvv
create-dmg --icon "$APP_NAME" 200 190 --hide-extension "$APP_NAME" --window-size 800 400 --app-drop-link 600 185 rustdesk-$VERSION.dmg "$APP_PATH"
codesign --force --options runtime -s $MACOS_CODESIGN_IDENTITY --deep --strict rustdesk-$VERSION.dmg -vvv
# notarize the rustdesk-${{ env.VERSION }}.dmg
rcodesign notary-submit --api-key-path ~/.p12/api-key.json  --staple rustdesk-$VERSION.dmg
