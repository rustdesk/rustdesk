shebang := if os() == 'windows' {
  'powershell.exe'
} else {
  '/usr/bin/env pwsh'
}

set shell := ["bash", "-c"]

set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]
set dotenv-load

home_dir := env_var('HOME')

export RS_PUB_KEY:=""
export RENDEZVOUS_SERVER:=""
export RUST_LOG:="debug"

export NDK_VERSION := "r25"
export BASE_ROOT := home_dir + "/rustdesk"
export ANDROID_SDK_ROOT := home_dir + "/android/sdk"

export TRDLIBS := BASE_ROOT + "/rustdesk_thirdparty_lib"
export VCPKG_ROOT := TRDLIBS + "/vcpkg"
export ANDROID_NDK_ROOT := ANDROID_SDK_ROOT + "/ndk/"
export ANDROID_NDK_HOME := ANDROID_NDK_ROOT + NDK_VERSION

default: gen

run: build-android-21 copy-desp
    cd flutter && flutter run

copy-desp:
    sed -i "s/signingConfigs.release/signingConfigs.debug/g" ./flutter/android/app/build.gradle
    mkdir -p ./flutter/android/app/src/main/jniLibs/arm64-v8a
    cp {{TRDLIBS}}/android/app/src/main/jniLibs/arm64-v8a/*.so ./flutter/android/app/src/main/jniLibs/arm64-v8a/
    cp ./target/aarch64-linux-android/release/liblibrustdesk.so ./flutter/android/app/src/main/jniLibs/arm64-v8a/librustdesk.so

    mkdir -p ./flutter/android/app/src/main/jniLibs/armeabi-v7a
    cp {{TRDLIBS}}/android/app/src/main/jniLibs/armeabi-v7a/*.so ./flutter/android/app/src/main/jniLibs/armeabi-v7a/
    cp ./target/armv7-linux-androideabi/release/liblibrustdesk.so ./flutter/android/app/src/main/jniLibs/armeabi-v7a/librustdesk.so

build-android-21:
    cargo ndk --platform 21 --target armeabi-v7a --target arm64-v8a build --release --features flutter,mediacodec --lib

desp-android: update
    rustup target add aarch64-linux-android armv7-linux-androideabi
    cd ./flutter; flutter precache --android; flutter pub get

gen:
    cd flutter && flutter pub get
    flutter_rust_bridge_codegen \
        --rust-input ./src/flutter_ffi.rs \
        --dart-output ./flutter/lib/generated_bridge.dart \
        --c-output ./flutter/ios/Runner/bridge_generated.h \
        --extra-c-output-path ./flutter/macos/Runner

update:
    rustup update
    cargo update

clean:
    cargo clean
    cd flutter && flutter clean
