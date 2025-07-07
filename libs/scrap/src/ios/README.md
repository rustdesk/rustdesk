# iOS Screen Capture Implementation

This implementation provides screen capture functionality for iOS using ReplayKit framework through Rust FFI.

## Architecture

### Components

1. **Native Layer** (`native/ScreenCapture.m`)
   - Implements ReplayKit screen recording for in-app capture
   - Handles message port communication for system-wide capture
   - Converts pixel formats (BGRA to RGBA)
   - Provides C interface for Rust FFI

2. **FFI Layer** (`ffi.rs`)
   - Rust bindings to native C functions
   - Frame buffer management
   - Callback mechanism for frame updates

3. **Rust Interface** (`mod.rs`)
   - Implements `TraitCapturer` for compatibility with RustDesk
   - Frame management and duplicate detection
   - Display information handling

4. **Broadcast Extension** (`flutter/ios/BroadcastExtension/`)
   - Separate app extension for system-wide screen capture
   - Uses message ports to send frames to main app
   - Required for capturing content outside the app

## Features

### In-App Capture
- Uses `RPScreenRecorder` API
- Captures only RustDesk app content
- No additional permissions required beyond initial prompt

### System-Wide Capture
- Uses Broadcast Upload Extension
- Can capture entire screen including other apps
- Requires user to explicitly start from Control Center
- Communicates via CFMessagePort

## Usage

```rust
// Initialize and start capture
let display = Display::primary()?;
let mut capturer = Capturer::new(display)?;

// Get frames
match capturer.frame(Duration::from_millis(33)) {
    Ok(frame) => {
        // Process frame
    }
    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
        // No new frame available
    }
    Err(e) => {
        // Handle error
    }
}

// For system-wide capture
ffi::show_broadcast_picker();
```

## Setup Requirements

1. **Xcode Configuration**
   - Add Broadcast Upload Extension target
   - Configure app groups (if using shared container)
   - Set up proper code signing

2. **Info.plist**
   - Add microphone usage description (for audio capture)
   - Configure broadcast extension settings

3. **Build Settings**
   - Link ReplayKit framework
   - Enable Objective-C ARC
   - Set minimum iOS version to 11.0 (12.0 for broadcast picker)

## Limitations

- Screen recording requires iOS 11.0+
- System-wide capture requires iOS 12.0+
- User must grant permission for screen recording
- Performance depends on device capabilities
- Broadcast extension has memory limits (~50MB)

## Security Considerations

- Screen recording is a sensitive permission
- iOS shows recording indicator when active
- Broadcast extension runs in separate process
- Message port communication is local only