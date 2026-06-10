# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a **Cislink-branded fork of RustDesk** (v1.4.2), a remote desktop application written in Rust with Flutter UI. It connects to self-hosted servers at `hbbs.cislink.nl` / `hbbr.cislink.nl` and uses a custom auto-update endpoint (`download.cislink.nl/rustdesk/latest.json`).

Original RustDesk installation files are stored in `/rustdesk original/` folder.

## Build Commands

### Windows Build (primary platform)
```bash
# Build Rust binary
cargo build --release

# Build with Flutter UI
python3 build.py --flutter --release

# Create Windows installer (requires Inno Setup 6)
cd D:\Rustdesk
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "RustDesk-Installer.iss"
```

### Other Build Variants
- `cargo run` - Build and run (requires libsciter library for legacy UI)
- `python3 build.py --hwcodec` - Build with hardware codec support
- `python3 build.py --vram` - Build with VRAM feature (Windows only)
- `python3 build.py --portable` - Build portable Windows version

### Flutter
- `cd flutter && flutter build android` / `flutter build ios` - Mobile builds
- `cd flutter && flutter test` - Run Flutter tests

### Testing
- `cargo test` - All Rust tests
- `cargo test --package hbb_common` - Common utilities
- `cargo test --package scrap` - Screen capture library

### Docker Build
```sh
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

### GCP API Server
```bash
cd gcp-api-server
npm run dev    # Development with hot reload
npm run build  # Compile TypeScript
npm start      # Run compiled server
```

## Architecture

### Cargo Workspace
Root crate + 7 member libraries in `libs/`:
- `libs/hbb_common` - Config (`src/config.rs`), protobuf, network, file transfer, codec wrappers
- `libs/scrap` - Screen capture (DXGI/Quartz/X11/Wayland/Android)
- `libs/enigo` - Platform-specific keyboard/mouse control
- `libs/clipboard` - Cross-platform clipboard
- `libs/virtual_display` + `dylib` - Windows virtual display
- `libs/remote_printer` - Remote printing
- `libs/portable` - Portable runtime

Binary targets: `rustdesk` (main), `naming` (naming service), `service` (background service)

### Core Source Layout (`src/`)
- `main.rs` - Entry point
- `lib.rs` - Library exports (30+ modules)
- `client.rs` + `client/io_loop.rs` - Peer connections, main I/O loop
- `server/` - Remote services (audio, video, input, clipboard, display, connection, printer, terminal)
- `platform/` - Platform-specific code (Windows, macOS, Linux)
- `rendezvous_mediator.rs` - Communication with rustdesk-server (TCP hole punching, relay)
- `flutter.rs` / `flutter_ffi.rs` - Flutter FFI bridge
- `ipc.rs` - Inter-process communication
- `common.rs` - Contains Cislink customizations (auto-update URL)
- `lang/` - 40+ language translations

### Flutter UI (`flutter/`)
- `flutter/lib/desktop/` - Desktop UI (pages, screen, widgets)
- `flutter/lib/mobile/` - Mobile UI
- `flutter/lib/common/` + `models/` - Shared code

### GCP API Server (`gcp-api-server/`)
TypeScript/Express API server using Firebase Admin SDK for RustDesk client management. Controllers: auth, group, ab (address book). Uses Firestore and Firebase Auth.

## Cislink Customizations

Key files modified from upstream RustDesk:
- `src/common.rs` - Auto-update queries `download.cislink.nl/rustdesk/latest.json` instead of GitHub
- `RustDesk_Config_Template.toml` / `RustDesk.toml` - Pre-configured for `hbbs.cislink.nl` / `hbbr.cislink.nl`
- `docker-compose.yml` - Server deployment config (hbbs + hbbr containers)
- `data/` - Server keypair (`id_ed25519` / `id_ed25519.pub`), public key: `VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=`

## Prerequisites

- **Rust** 1.75+ (Edition 2021)
- **vcpkg** with `VCPKG_ROOT` set: `vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static`
- **Flutter** 3.24.5 (CI reference version)
- **Inno Setup 6** for Windows installer

## Feature Flags

| Flag | Purpose |
|------|---------|
| `flutter` | Flutter UI (via flutter_rust_bridge) |
| `hwcodec` | Hardware video encoding/decoding |
| `vram` | VRAM optimization (Windows only) |
| `cli` | CLI mode |
| `plugin_framework` | Plugin system |
| `mediacodec` | Android media codec |
| `screencapturekit` | macOS ScreenCaptureKit (10.14+) |
| `unix-file-copy-paste` | X11 file clipboard |

Default features: `use_dasp` (audio resampling)

## Configuration System

All configs in `libs/hbb_common/src/config.rs`, 4 layers:
1. **Built-in** - Hardcoded defaults
2. **Local** - Machine-specific
3. **Settings** - User-configurable
4. **Display** - Per-display settings

Client config files: `RustDesk.toml`, `RustDesk2.toml`

## Build Notes

- First Docker build is slow; subsequent builds use cached dependencies
- Run commands from repo root so resources are found
- GitHub Actions CI uses Flutter 3.24.5, Rust 1.75 (Sciter) / 1.81 (macOS)
- Ignore: `target/`, `flutter/build/`, `flutter/.dart_tool/`, `gcp-api-server/node_modules/`
