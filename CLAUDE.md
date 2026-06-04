# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Build Commands
- `cargo run` - Build and run the desktop application (requires libsciter library)
- `python3 build.py --flutter` - Build Flutter version (desktop)
- `python3 build.py --flutter --release` - Build Flutter version in release mode
- `python3 build.py --hwcodec` - Build with hardware codec support
- `python3 build.py --vram` - Build with VRAM feature (Windows only)
- `python3 build.py --unix-file-copy-paste` - Build with Unix file clipboard support
- `python3 build.py --portable` - Build portable Windows version
- `cargo build --release` - Build Rust binary in release mode
- `cargo build --features hwcodec` - Build with hardware codec
- `cargo build --features flutter` - Build with Flutter UI
- `cargo build --bin rustdesk` - Build main application binary
- `cargo build --bin naming` - Build naming service binary
- `cargo build --bin service` - Build background service binary
### original rustdesk installation files are stored in "/rustdesk original/" folder
### Docker Build
```sh
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

### Flutter Mobile Commands
- `cd flutter && flutter build android` - Build Android APK
- `cd flutter && flutter build ios` - Build iOS app
- `cd flutter && flutter run` - Run Flutter app in development mode
- `cd flutter && flutter test` - Run Flutter tests

### Testing
- `cargo test` - Run all Rust tests
- `cargo test --package hbb_common` - Test common utilities library
- `cargo test --package scrap` - Test screen capture library
- `cd flutter && flutter test` - Run Flutter tests

### Platform-Specific Build Scripts
- `flutter/build_android.sh` - Android build script
- `flutter/build_ios.sh` - iOS build script
- `flutter/build_fdroid.sh` - F-Droid build script

## Project Architecture

### Cargo Workspace Structure
This is a Cargo workspace with 7 member crates:
- **Root crate** - Main RustDesk application
- **`libs/scrap`** - Screen capture
- **`libs/hbb_common`** - Common utilities
- **`libs/enigo`** - Input control
- **`libs/clipboard`** - Clipboard handling
- **`libs/virtual_display`** - Windows virtual display
- **`libs/virtual_display/dylib`** - Dynamic library wrapper
- **`libs/remote_printer`** - Remote printing support
- **`libs/portable`** - Portable runtime

### Binary Targets
- `rustdesk` - Main application (default)
- `naming` - Naming service
- `service` - Background service

### Directory Structure
- **`src/`** - Main Rust application code
  - `src/main.rs` - Application entry point
  - `src/lib.rs` - Library exports (30+ modules)
  - `src/ui/` - Legacy Sciter UI (deprecated, use Flutter instead)
  - `src/server/` - Remote services (audio, video, input, clipboard, display, connection, printer, terminal)
  - `src/client.rs` - Peer connection handling (`src/client/io_loop.rs:142` - main I/O loop)
  - `src/platform/` - Platform-specific code (Windows, macOS, Linux)
  - `src/flutter.rs` / `src/flutter_ffi.rs` - Flutter FFI bridge
  - `src/rendezvous_mediator.rs` - Communication with rustdesk-server
  - `src/lang/` - Multi-language support (40+ languages)
  - `src/plugin/` - Plugin framework (optional)
- **`flutter/`** - Flutter UI code for desktop and mobile
  - `flutter/lib/desktop/` - Desktop UI (pages, screen, widgets)
  - `flutter/lib/mobile/` - Mobile UI
  - `flutter/lib/common/` - Shared code
  - `flutter/lib/models/` - Data models
  - `flutter/android/`, `flutter/ios/`, `flutter/linux/`, `flutter/macos/`, `flutter/windows/` - Platform-specific native config
- **`libs/`** - Core libraries
  - `libs/hbb_common/` - Video codec wrapper, config (`src/config.rs`), network wrapper, protobuf, file transfer utilities
  - `libs/scrap/` - Screen capture (DXGI for Windows, Quartz for macOS, X11/Wayland for Linux, Android)
  - `libs/enigo/` - Platform-specific keyboard/mouse control
  - `libs/clipboard/` - Cross-platform clipboard implementation

### Key Components
- **Remote Desktop Protocol**: Custom protocol in `src/rendezvous_mediator.rs` for communicating with rustdesk-server (TCP hole punching, relay connections)
- **Screen Capture**: Platform-specific implementation in `libs/scrap/` (DXGI, Quartz, X11, Wayland, Android)
- **Input Handling**: Cross-platform input simulation in `libs/enigo/`
- **Audio/Video Services**: Real-time streaming in `src/server/audio_service.rs` and `src/server/video_service.rs`
- **File Transfer**: Secure implementation in `libs/hbb_common/` and `src/client/file_trait.rs`
- **FFI Bridge**: Flutter ↔ Rust communication via flutter_rust_bridge
- **IPC**: Inter-process communication in `src/ipc.rs`
- **Configuration**: Layered config system in `libs/hbb_common/src/config.rs` (4 types)

### UI Architecture
- **Legacy UI**: Sciter-based (deprecated) - files in `src/ui/`
- **Modern UI**: Flutter-based - files in `flutter/`
  - Desktop: `flutter/lib/desktop/`
  - Mobile: `flutter/lib/mobile/`
  - Shared: `flutter/lib/common/` and `flutter/lib/models/`

## Important Build Notes

### Prerequisites
1. **Rust**: Version 1.75+ (Edition 2021)
2. **vcpkg**: For C++ dependencies
   - Windows: `vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static`
   - Linux/macOS: `vcpkg install libvpx libyuv opus aom`
   - Set `VCPKG_ROOT` environment variable
3. **Sciter**: Download appropriate library for legacy UI support (deprecated)
   - [Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll)
   - [Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so)
   - [macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

### Platform-Specific Dependencies

**Ubuntu 18 / Debian 10**:
```sh
sudo apt install -y zip g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev \
        libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake make \
        libclang-dev ninja-build libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpam0g-dev
```

**Fedora 28 / CentOS 8**:
```sh
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel gstreamer1-devel gstreamer1-plugins-base-devel pam-devel
```

**Arch / Manjaro**:
```sh
sudo pacman -Syu --needed unzip git cmake gcc curl wget yasm nasm zip make pkg-config clang gtk3 xdotool libxcb libxfixes alsa-lib pipewire
```

### Ignore Patterns
When working with files, ignore these directories:
- `target/` - Rust build artifacts
- `flutter/build/` - Flutter build output
- `flutter/.dart_tool/` - Flutter tooling files

### Cross-Platform Considerations
- **Windows**: Requires DLLs, virtual display drivers, and Windows SDK
- **macOS**: Requires signing and notarization for distribution, Xcode command-line tools
- **Linux**: Supports deb, rpm, AppImage, Flatpak packages; X11 and Wayland
- **Mobile**: Requires Android SDK/NDK (Android) or Xcode (iOS)

### Feature Flags
- `default` - Uses `dasp` for audio resampling
- `cli` - CLI mode
- `flutter` - Enable Flutter UI
- `hwcodec` - Hardware video encoding/decoding
- `vram` - VRAM optimization (Windows only)
- `mediacodec` - Android media codec
- `unix-file-copy-paste` - Unix file clipboard support (X11)
- `screencapturekit` - macOS ScreenCaptureKit (macOS 10.14+)
- `plugin_framework` - Plugin system support
- `use_samplerate` / `use_rubato` / `use_dasp` - Audio resampling library options

### Configuration System
All configurations are in `libs/hbb_common/src/config.rs`, 4 types:
1. **Settings** - User-configurable settings
2. **Local** - Local machine configuration
3. **Display** - Display-specific configuration
4. **Built-in** - Hardcoded defaults

### Build Scripts
- **`build.rs`**: Cargo build script for platform-specific compilation (Windows C++, macOS Objective-C++, Android NDK)
- **`build.py`**: Python multi-platform builder with feature flag support

### Important Notes
- First Docker build may take longer; subsequent builds are faster due to caching
- Run commands from repository root to ensure resources are found
- GitHub Actions CI uses Flutter 3.24.5, Rust 1.75 (Sciter) / 1.81 (macOS)
- Minimum Rust version: 1.75
- working command:  cd D:\Rustdesk
  & "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "RustDesk-Installer.iss"