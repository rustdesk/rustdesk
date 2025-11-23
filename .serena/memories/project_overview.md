# RustDesk Project Overview

## Purpose
RustDesk is a remote desktop software written in Rust. It works out of the box with no configuration required. The project provides full control of data with security in mind. It supports desktop platforms (Windows, macOS, Linux) and mobile platforms (Android, iOS).

## Tech Stack
- **Backend**: Rust
- **Desktop UI**: Flutter (modern) / Sciter (deprecated)
- **Mobile UI**: Flutter
- **Build System**: Cargo for Rust, Gradle for Android
- **Dependencies**: vcpkg for C++ dependencies (libvpx, libyuv, opus, aom)

## Project Structure
- **src/** - Main Rust application code
  - **src/ui/** - Legacy Sciter UI (deprecated)
  - **src/server/** - Audio/clipboard/input/video services and network connections
  - **src/client.rs** - Peer connection handling
  - **src/platform/** - Platform-specific code
- **flutter/** - Flutter UI code for desktop and mobile
  - **flutter/lib/** - Dart/Flutter code
  - **flutter/android/** - Android-specific code
  - **flutter/ios/** - iOS-specific code
- **libs/** - Core libraries
  - **libs/hbb_common/** - Video codec, config, network wrapper, protobuf, file transfer utilities
  - **libs/scrap/** - Screen capture functionality
  - **libs/enigo/** - Platform-specific keyboard/mouse control
  - **libs/clipboard/** - Cross-platform clipboard implementation

## Android Project Structure
- **flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/** - Kotlin source files
  - MainActivity.kt - Main Flutter activity with method channels
  - MainService.kt - Background service for screen capture
  - InputService.kt - Accessibility service for input
  - Other utility classes
- **flutter/android/app/build.gradle** - Android app build configuration
- **flutter/android/build.gradle** - Android project build configuration

## Flutter Project Structure
- **flutter/lib/utils/** - Utility classes including platform channels
- **flutter/lib/common/** - Shared common code
- **flutter/lib/mobile/** - Mobile-specific UI
- **flutter/lib/desktop/** - Desktop-specific UI
- **flutter/lib/models/** - Data models
