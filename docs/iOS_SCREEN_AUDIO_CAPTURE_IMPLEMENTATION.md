# iOS Screen and Audio Capture Implementation Guide

## Overview

This document describes the complete implementation of screen and audio capture for iOS in RustDesk. The implementation uses Apple's ReplayKit framework through FFI, allowing screen recording with minimal overhead while maintaining compatibility with RustDesk's existing architecture.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                           iOS System                                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌─────────────────┐     ┌─────────────────┐    ┌────────────────┐ │
│  │   ReplayKit     │     │  Main App       │    │ Broadcast Ext. │ │
│  │                 │     │                 │    │ (System-wide)  │ │
│  │  - RPScreen     │────▶│  Objective-C    │◀───│                │ │
│  │    Recorder     │     │  ScreenCapture  │    │ SampleHandler  │ │
│  │  - Video/Audio  │     │       ↓         │    │                │ │
│  └─────────────────┘     │   C Interface   │    └────────────────┘ │
│                          │       ↓         │                        │
│                          │   Rust FFI      │                        │
│                          │       ↓         │                        │
│                          │  Capture/Audio  │                        │
│                          │    Services     │                        │
│                          └─────────────────┘                        │
└─────────────────────────────────────────────────────────────────────┘
```

## Directory Structure

```
rustdesk/
├── libs/scrap/src/ios/
│   ├── mod.rs                    # Rust capture implementation
│   ├── ffi.rs                    # FFI bindings
│   ├── native/
│   │   ├── ScreenCapture.h       # C interface header
│   │   └── ScreenCapture.m       # Objective-C implementation
│   └── README.md                 # iOS-specific documentation
├── flutter/ios/
│   ├── Runner/
│   │   └── Info.plist           # Permissions
│   └── BroadcastExtension/      # System-wide capture
│       ├── SampleHandler.h/m    # Broadcast extension
│       └── Info.plist           # Extension config
└── src/server/
    └── audio_service.rs         # iOS audio integration
```

## Implementation Components

### 1. Native Layer (Objective-C)

#### ScreenCapture.h - C Interface
```objective-c
// Video capture
void ios_capture_init(void);
bool ios_capture_start(void);
void ios_capture_stop(void);
uint32_t ios_capture_get_frame(uint8_t* buffer, uint32_t buffer_size, 
                               uint32_t* out_width, uint32_t* out_height);

// Audio capture
void ios_capture_set_audio_enabled(bool enable_mic, bool enable_app_audio);
typedef void (*audio_callback_t)(const uint8_t* data, uint32_t size, bool is_mic);
void ios_capture_set_audio_callback(audio_callback_t callback);

// System-wide capture
void ios_capture_show_broadcast_picker(void);
bool ios_capture_is_broadcasting(void);
```

#### ScreenCapture.m - Implementation Details
- Uses `RPScreenRecorder` for in-app capture
- Handles both video and audio sample buffers
- Converts BGRA to RGBA pixel format
- Thread-safe frame buffer management
- CFMessagePort for IPC with broadcast extension

### 2. FFI Layer (Rust)

#### ffi.rs - Safe Rust Bindings
```rust
pub fn init() 
pub fn start_capture() -> bool
pub fn stop_capture()
pub fn get_frame() -> Option<(Vec<u8>, u32, u32)>
pub fn enable_audio(mic: bool, app_audio: bool)
pub fn set_audio_callback(callback: Option<extern "C" fn(*const u8, u32, bool)>)
pub fn show_broadcast_picker()
```

Key features:
- Lazy static buffers to reduce allocations
- Callback mechanism for asynchronous frame updates
- Thread-safe frame buffer access

### 3. Rust Capture Implementation

#### mod.rs - Capturer Implementation
```rust
pub struct Capturer {
    width: usize,
    height: usize,
    display: Display,
    frame_data: Vec<u8>,
    last_frame: Vec<u8>,
}

impl TraitCapturer for Capturer {
    fn frame<'a>(&'a mut self, timeout: Duration) -> io::Result<crate::Frame<'a>>
}
```

Features:
- Implements RustDesk's `TraitCapturer` interface
- Frame deduplication using `would_block_if_equal`
- Automatic cleanup on drop
- Compatible with existing video pipeline

### 4. Audio Service Integration

#### audio_service.rs - iOS Audio Module
```rust
#[cfg(target_os = "ios")]
mod ios_impl {
    const SAMPLE_RATE: u32 = 48000;
    const CHANNELS: u16 = 2;
    const FRAMES_PER_BUFFER: usize = 480; // 10ms
    
    pub struct State {
        encoder: Option<Encoder>,
        receiver: Option<Receiver<Vec<f32>>>,
        // ...
    }
}
```

Features:
- Opus encoder with 48kHz stereo
- PCM i16 to f32 conversion
- Zero detection for silence gating
- Non-blocking audio processing

### 5. Broadcast Upload Extension

For system-wide capture (captures other apps):

#### SampleHandler.m
- Runs in separate process
- Captures entire screen
- Sends frames via CFMessagePort to main app
- Memory-efficient frame transfer

## Capture Modes

### 1. In-App Capture (Default)
```rust
// Captures only RustDesk app
let display = Display::primary()?;
let mut capturer = Capturer::new(display)?;
```

### 2. System-Wide Capture
```rust
// Shows iOS broadcast picker
ffi::show_broadcast_picker();
// User must manually start from Control Center
```

## Build Configuration

### Cargo.toml
```toml
[build-dependencies]
cc = "1.0"  # For compiling Objective-C
```

### build.rs
```rust
if target_os == "ios" {
    cc::Build::new()
        .file("src/ios/native/ScreenCapture.m")
        .flag("-fobjc-arc")
        .flag("-fmodules")
        .compile("ScreenCapture");
}
```

### Info.plist Permissions
```xml
<key>NSMicrophoneUsageDescription</key>
<string>This app needs microphone access for screen recording with audio</string>
```

## Data Flow

### Video Capture Flow
1. ReplayKit captures screen → CMSampleBuffer
2. Native code converts BGRA → RGBA
3. Frame callback or polling from Rust
4. Rust checks for duplicate frames
5. Creates `Frame::PixelBuffer` for video pipeline
6. Existing video encoder/transmission

### Audio Capture Flow
1. ReplayKit captures app audio → CMSampleBuffer
2. Native extracts Linear PCM data
3. FFI callback to Rust audio service
4. Convert i16 PCM → f32 normalized
5. Opus encoding at 48kHz
6. Send as `AudioFrame` protobuf

## Memory Management

### Optimizations
- Reuse static buffers for frame data (33MB max)
- Lazy allocation based on actual frame size
- Frame deduplication to avoid redundant processing
- Proper synchronization with `@synchronized` blocks
- Weak references in completion handlers

### Cleanup
- `dealloc` method for CFMessagePort cleanup
- Drop implementation stops capture
- Automatic buffer cleanup

## Performance Considerations

### Frame Rate
- 30-60 FPS depending on device
- Frame skipping in broadcast extension (every 2nd frame)
- Non-blocking frame retrieval

### Latency
- In-app: ~2-5ms capture latency
- System-wide: ~10-20ms (IPC overhead)
- Audio: ~10ms chunks for low latency

### CPU Usage
- Hardware-accelerated capture
- Efficient pixel format conversion
- Minimal memory copies

## Security & Privacy

### Permissions Required
- Screen Recording (always required)
- Microphone (optional, for mic audio)

### User Control
- Recording indicator shown by iOS
- User must grant permission
- Can stop anytime from Control Center

### App Groups (for Broadcast Extension)
```
group.com.carriez.rustdesk.screenshare
```

## Integration with RustDesk

### Video Service
- Works with existing `scrap` infrastructure
- Compatible with all video encoders (VP8/9, H264/5)
- Standard frame processing pipeline

### Audio Service
- Integrated as platform-specific implementation
- Same Opus encoding as other platforms
- Compatible with existing audio routing

## Limitations

1. **No cursor capture** - iOS doesn't expose cursor
2. **Permission required** - User must explicitly allow
3. **Broadcast extension memory** - Limited to ~50MB
4. **Background execution** - Limited by iOS policies

## Testing

### Build for iOS
```bash
cd flutter
flutter build ios
```

### Required Setup in Xcode
1. Add Broadcast Upload Extension target
2. Configure app groups
3. Set up code signing
4. Link ReplayKit framework

### Test Scenarios
1. In-app screen capture
2. System-wide broadcast
3. Audio capture (app/mic)
4. Permission handling
5. Background/foreground transitions

## Troubleshooting

### Common Issues

1. **No frames received**
   - Check screen recording permission
   - Verify capture is started
   - Check frame timeout settings

2. **Audio not working**
   - Verify microphone permission
   - Check audio callback registration
   - Confirm audio format compatibility

3. **Broadcast extension not appearing**
   - Verify bundle identifiers
   - Check code signing
   - Ensure extension is included in build

4. **Memory warnings**
   - Reduce frame rate in broadcast extension
   - Check buffer allocations
   - Monitor memory usage

## Future Improvements

1. **Hardware encoding** - Use VideoToolbox for H.264
2. **Adaptive quality** - Adjust based on network/CPU
3. **Picture-in-Picture** - Support PiP mode
4. **Screen orientation** - Better rotation handling
5. **Audio enhancements** - Noise suppression, echo cancellation

## Conclusion

This implementation provides full screen and audio capture capabilities for iOS while maintaining compatibility with RustDesk's cross-platform architecture. The use of FFI minimizes overhead while allowing native iOS features to be accessed from Rust code.