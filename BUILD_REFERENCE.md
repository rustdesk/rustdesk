# RustDesk Android Build Reference

This document outlines the steps to build the RustDesk Android application from source.

## Prerequisites

Ensure you have the following installed:
- **Linux OS** (Ubuntu 20.04+ recommended)
- **Rust** (latest stable)
- **Flutter** (latest stable)
- **Android SDK & NDK** (NDK version 26.1.10909125 recommended)
- **vcpkg** (Microsoft C++ Library Manager)
- **System Tools**: `git`, `curl`, `wget`, `unzip`, `nasm`, `yasm`, `cmake`, `pkg-config`, `libclang-dev`, `build-essential`

## Environment Setup

1.  **Install System Dependencies**:
    ```bash
    sudo apt-get update
    sudo apt-get install -y nasm yasm cmake pkg-config libclang-dev build-essential
    ```

2.  **Install Rust & Add Targets**:
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source $HOME/.cargo/env
    rustup target add aarch64-linux-android armv7-linux-androideabi
    ```

3.  **Install vcpkg**:
    ```bash
    git clone https://github.com/microsoft/vcpkg $HOME/vcpkg
    $HOME/vcpkg/bootstrap-vcpkg.sh
    export VCPKG_ROOT=$HOME/vcpkg
    ```

4.  **Configure Android Environment**:
    Set `ANDROID_HOME` and `ANDROID_NDK_HOME`.
    ```bash
    export ANDROID_HOME=$HOME/android-sdk
    export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/26.1.10909125
    ```

## Build Process

### 1. Build Native Dependencies

RustDesk uses `vcpkg` to manage C++ dependencies like `ffmpeg`, `libvpx`, etc. You need to build these for the Android architectures you want to support (usually `arm64-v8a` and `armeabi-v7a`).

```bash
cd flutter
# Build for ARM64
./build_android_deps.sh arm64-v8a
# Build for ARM32 (Optional but recommended)
./build_android_deps.sh armeabi-v7a
```

### 2. Build the APK

Once dependencies are built, use Flutter to build the application.

```bash
cd flutter
flutter build apk --release
```

The resulting APK will be located in `flutter/build/app/outputs/flutter-apk/`.

## Troubleshooting

-   **vcpkg errors**: Ensure `nasm` and `yasm` are installed. Check `vcpkg` logs for details.
-   **NDK errors**: Verify `ANDROID_NDK_HOME` points to a valid NDK installation (version 25+ or 26+ is usually required).
