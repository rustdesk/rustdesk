# RustDesk Codebase Structure

## Main Directories

### `/src/` - Main Rust Application
- `src/main.rs` - Application entry point
- `src/client.rs` - Peer connection handling
- `src/server/` - Audio/clipboard/input/video services and network connections
- `src/ui/` - Legacy Sciter UI (deprecated)
- `src/platform/` - Platform-specific code
- `src/rendezvous_mediator.rs` - Custom protocol for rustdesk-server communication
- `src/naming.rs` - Naming service binary
- `src/service.rs` - Service binary

### `/flutter/` - Flutter UI (Modern)
- `flutter/lib/desktop/` - Desktop-specific UI
- `flutter/lib/mobile/` - Mobile-specific UI
- `flutter/lib/common/` - Shared UI components
- `flutter/lib/models/` - Data models
- `flutter/android/` - Android platform code
- `flutter/ios/` - iOS platform code
- `flutter/linux/`, `flutter/windows/`, `flutter/macos/` - Desktop platform code

### `/libs/` - Core Libraries
- `libs/hbb_common/` - Video codec, config, network wrapper, protobuf, file transfer utilities
  - Configuration is in `libs/hbb_common/src/config.rs`
- `libs/scrap/` - Screen capture functionality
- `libs/enigo/` - Platform-specific keyboard/mouse control
- `libs/clipboard/` - Cross-platform clipboard implementation
- `libs/virtual_display/` - Virtual display support (Windows)
- `libs/remote_printer/` - Remote printer support
- `libs/portable/` - Portable utilities

### Other Directories
- `/docs/` - Documentation and translations
- `/res/` - Resources (icons, images)
- `/examples/` - Example code
- `/flatpak/`, `/appimage/` - Linux packaging configurations
- `/.github/` - CI/CD workflows

## Build Artifacts (Ignored)
- `target/` - Rust build artifacts
- `flutter/build/` - Flutter build output
- `flutter/.dart_tool/` - Flutter tooling files
