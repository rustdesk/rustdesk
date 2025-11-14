# iOS Audio Capture Implementation

## Overview

RustDesk iOS audio capture is implemented following the existing audio service pattern, capturing app audio by default and sending it to peers using the Opus codec.

## Architecture

### Components

1. **Native Layer** (`libs/scrap/src/ios/native/ScreenCapture.m`)
   - Captures audio using ReplayKit's audio sample buffers
   - Supports both app audio and microphone audio
   - Converts audio format information for Rust processing

2. **FFI Layer** (`libs/scrap/src/ios/ffi.rs`)
   - Provides safe Rust bindings for audio control
   - `enable_audio(mic: bool, app_audio: bool)` - Enable/disable audio sources
   - `set_audio_callback()` - Register callback for audio data

3. **Audio Service** (`src/server/audio_service.rs::ios_impl`)
   - Follows the same pattern as other platforms
   - Uses Opus encoder with 48kHz stereo configuration
   - Processes audio in 10ms chunks (480 samples)
   - Sends encoded audio as `AudioFrame` messages

## Audio Flow

1. **Capture**: ReplayKit provides audio as Linear PCM in CMSampleBuffer format
2. **Callback**: Native code passes raw PCM data to Rust via FFI callback
3. **Conversion**: Rust converts audio data from i16 to f32 normalized [-1.0, 1.0]
4. **Encoding**: Opus encoder compresses audio for network transmission
5. **Transmission**: Encoded audio sent to peers as protobuf messages

## Configuration

- **Sample Rate**: 48,000 Hz (standard for all platforms)
- **Channels**: 2 (Stereo)
- **Format**: Linear PCM, typically 16-bit
- **Encoder**: Opus with LowDelay application mode
- **Frame Size**: 480 samples (10ms at 48kHz)

## Usage

By default, app audio is captured automatically when screen recording starts:

```rust
// In audio_service.rs
enable_audio(false, true); // mic=false, app_audio=true
```

To enable microphone:
```rust
enable_audio(true, true); // mic=true, app_audio=true
```

## Permissions

- **App Audio**: No additional permission required (part of screen recording)
- **Microphone**: Requires `NSMicrophoneUsageDescription` in Info.plist

## Implementation Details

### Audio Format Handling

The native layer logs audio format on first capture:
```
Audio format - Sample rate: 48000, Channels: 2, Bits per channel: 16, Format: 1819304813
```

### Zero Detection

Like other platforms, implements audio zero gate to avoid sending silent frames:
- Tracks consecutive zero frames
- Stops sending after 800 frames of silence
- Resumes immediately when audio detected

### Thread Safety

- Audio callback runs on ReplayKit's audio queue
- Uses Rust channels for thread-safe communication
- Non-blocking receive in service loop

## Limitations

- Audio only available during active screen capture
- System audio requires Broadcast Upload Extension
- Audio/video synchronization handled separately