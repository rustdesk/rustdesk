# RustDesk Project Overview

## Purpose
RustDesk is an open-source remote desktop software written in Rust. It provides full control of data with no security concerns, supporting self-hosted or public rendezvous/relay servers. It's a cross-platform solution for Windows, macOS, Linux, Android, and iOS.

## Tech Stack
- **Core Language**: Rust (edition 2021, minimum version 1.75)
- **UI Framework**: Flutter (modern, primary) and Sciter (legacy, deprecated)
- **Screen Capture**: Platform-specific implementations in `libs/scrap/`
- **Audio Processing**: Multiple backends (dasp, rubato, samplerate)
- **Video Codecs**: libvpx, libyuv, opus, aom (via vcpkg)
- **Networking**: Custom protocol with protobuf, tokio-ipc
- **Build System**: Cargo for Rust, Python build scripts for Flutter

## Key Dependencies
- vcpkg for C++ dependencies (libvpx, libyuv, opus, aom)
- Flutter SDK for UI
- Platform-specific libraries (GTK on Linux, Cocoa on macOS, WinAPI on Windows)

## Platform Support
- Desktop: Windows, macOS, Linux
- Mobile: Android, iOS
- Distribution formats: deb, rpm, AppImage, Flatpak, F-Droid
