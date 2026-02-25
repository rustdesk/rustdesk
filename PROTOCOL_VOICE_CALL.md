# Protocol Definitions Update for Voice Calling

This document describes the protobuf message definitions needed for the voice calling feature.

## Overview

RustDesk already has basic voice call signaling (`VoiceCallRequest` and `VoiceCallResponse`). This update adds:
1. Audio frame transmission messages
2. Mute/unmute messages
3. Audio error messages
4. Quality indicators

## Protobuf Messages to Add

Location: `libs/hbb_common/protos/message.proto` (or equivalent)

```protobuf
syntax = "proto3";

package message;

// Audio frame data for voice calling
message AudioFrame {
  // Opus-encoded audio data
  bytes data = 1;
  
  // Sample rate: 16000 or 48000 Hz
  uint32 sample_rate = 2;
  
  // Number of channels: 1 (mono) or 2 (stereo)
  uint32 channels = 3;
  
  // Timestamp in milliseconds since call start
  uint64 timestamp = 4;
  
  // Sequence number for detection of lost frames
  uint32 sequence_number = 5;
  
  // True if this is the last frame (call termination)
  bool is_last_frame = 6;
}

// Audio mute/unmute message
message AudioMute {
  // True = muted, False = unmuted
  bool is_muted = 1;
  
  // Server timestamp
  int64 timestamp = 2;
}

// Audio-related errors
message AudioError {
  enum ErrorCode {
    MIC_PERMISSION_DENIED = 0;  // User denied mic access
    NO_AUDIO_DEVICE = 1;        // No input/output device
    CODEC_ERROR = 2;             // Opus encode/decode error
    NETWORK_ERROR = 3;          // Network send error
    RELAY_UNAVAILABLE = 4;      // Relay server not available
    DEVICE_IN_USE = 5;          // Audio device in use by another app
    SAMPLE_RATE_MISMATCH = 6;   // Unsupported sample rate
  }
  
  ErrorCode error_code = 1;
  string message = 2;
  int64 timestamp = 3;
}

// Audio quality indicators for adaptive bitrate
message AudioQualityIndicator {
  // Estimated bandwidth in kbps
  uint32 bandwidth_kbps = 1;
  
  // Estimated packet loss percentage (0-100)
  uint32 packet_loss_percent = 2;
  
  // Average latency in milliseconds
  uint32 latency_ms = 3;
  
  // Jitter buffer delay in milliseconds
  uint32 jitter_buffer_ms = 4;
  
  // Recommended bitrate adjustment (-10 to +100, percentage)
  int32 bitrate_adjust_pct = 5;
  
  int64 timestamp = 6;
}

// Merged into the main Message union
message Message {
  oneof union {
    // ... existing message types ...
    
    // New audio messages
    AudioFrame audio_frame = 51;
    AudioMute audio_mute = 52;
    AudioError audio_error = 53;
    AudioQualityIndicator audio_quality = 54;
  }
}
```

## Integration Points

### 1. Client Message Routing

In `src/client/io_loop.rs`, add handler for new audio messages:

```rust
async fn handle_message(&mut self, msg: Message) {
    match msg.union {
        // ... existing handlers ...
        
        Some(message::Union::AudioFrame(frame)) => {
            self.handle_audio_frame(frame).await;
        }
        Some(message::Union::AudioMute(mute)) => {
            self.handle_audio_mute(mute).await;
        }
        Some(message::Union::AudioError(error)) => {
            self.handle_audio_error(error).await;
        }
        Some(message::Union::AudioQuality(quality)) => {
            self.handle_audio_quality(quality).await;
        }
        _ => {}
    }
}
```

### 2. Server Message Routing

In `src/server/connection.rs`, add handlers for outbound audio messages:

```rust
// Send audio frame to client
fn send_audio_frame(&mut self, frame: AudioFrame) -> ResultType<()> {
    let mut msg = Message::new();
    msg.set_audio_frame(frame);
    self.send_message(msg)?;
    Ok(())
}

// Handle mute notification
fn handle_audio_mute(&mut self, mute: AudioMute) -> ResultType<()> {
    log::debug!("Audio mute: {}", mute.is_muted);
    // Forward to other peers in group calls
    Ok(())
}

// Handle audio errors gracefully
fn handle_audio_error(&mut self, error: AudioError) -> ResultType<()> {
    log::warn!("Audio error: {:?} - {}", error.error_code(), error.message);
    // Notify UI or attempt recovery
    Ok(())
}
```

## Message Flow Diagram

```
CLIENT 1                           CLIENT 2
   |                                  |
   |-- VoiceCallRequest (seq=N) ----->|
   |                                  |
   |                          [User gets notification]
   |                                  |
   |<-- VoiceCallResponse (accept) ----|
   |                                  |
   |[Audio streams initialized]---[Audio streams initialized]
   |                                  |
   |--- AudioFrame (seq=1) ----------->|
   |--- AudioFrame (seq=2) ----------->|
   |--- AudioFrame (seq=3) ----------->|
   |                                  |[Playback via jitter buffer]
   |<-- AudioFrame (seq=1) ------------|
   |<-- AudioFrame (seq=2) ------------|
   |<-- AudioFrame (seq=3) ------------|
   |[Playback via jitter buffer]       |
   |                                  |
   |--- AudioMute (muted=true) ------->|
   |[Capture stopped, send silence]    |
   |<-- AudioMute (muted=false) -------|
   |                                  |
   |--- AudioQuality (loss=2%) ------->|
   |                                  |[Adapt bitrate]
   |<-- AudioQuality (loss=3%) --------|
   |                                  |
   |[User clicks end call]             |
   |--- AudioFrame (is_last=true) ---->|
   |                                  |
   |<-- AudioFrame (is_last=true) -----|
   |                                  |
   |[Streams closed]                   |[Streams closed]
```

## Protobuf Compilation

After adding these messages to `message.proto`, regenerate the Rust code:

```bash
# If using protobuf compiler directly
protoc --rust_out=. \
    --crate_name=hbb_common \
    libs/hbb_common/protos/message.proto

# Or use the build script if configured
cargo build --manifest-path libs/hbb_common/Cargo.toml
```

## Migration Strategy

### Phase 1: Non-Breaking Addition (Current)
- Add new message types to `Message` union
- Existing code ignores unknown message fields
- Version bump: minor (1.x.y → 1.(x+1).0)

### Phase 2: Feature Detection
- Both sides announce voice call capability in peer info
- Fall back to signaling-only if other side doesn't support audio

### Phase 3: Full Integration
- Make voice calling a standard feature
- Include in all builds

## Backward Compatibility

The protocol is designed for backward compatibility:

1. **Unknown message fields**: Older versions safely ignore new message types
2. **Version negotiation**: Peers exchange feature flags during handshake
3. **Graceful degradation**: If audio not supported, show error but don't crash

### Example Version Check

```rust
// In LoginConfigHandler::handle_peer_info()
if let Some(features) = &pi.features {
    self.voice_call_supported = features.voice_call; // or check version
} else {
    self.voice_call_supported = false;
}
```

## Testing Protocol Changes

### Unit Tests

```rust
#[test]
fn test_audio_frame_serialization() {
    let mut frame = AudioFrame::new();
    frame.set_data(vec![1, 2, 3, 4].into());
    frame.set_sample_rate(16000);
    frame.set_sequence_number(1);
    
    let bytes = frame.write_to_bytes().unwrap();
    let decoded = AudioFrame::parse_from_bytes(&bytes).unwrap();
    
    assert_eq!(decoded.sample_rate, 16000);
    assert_eq!(decoded.sequence_number, 1);
}
```

### Integration Tests

1. Test message sending between client and server
2. Verify sequence numbers preserved
3. Test large audio frame handling
4. Verify error message delivery

## Future Extensions

### 1. Group Audio Calls
```protobuf
message GroupAudioMix {
  uint32 group_id = 1;
  repeated bytes participant_audio = 2;  // Mixed audio from each participant
  uint64 timestamp = 3;
}
```

### 2. Advanced Codecs
```protobuf
message AudioCodecInfo {
  enum CodecType {
    OPUS = 0;
    FLAC = 1;      // For high-quality LAN
    AAC = 2;       // For mobile
  }
  CodecType codec = 1;
  uint32 sample_rate = 2;
  // ...
}
```

### 3. Audio Recording
```protobuf
message AudioRecordingControl {
  enum Action {
    START = 0;
    STOP = 1;
    PAUSE = 2;
    RESUME = 3;
  }
  Action action = 1;
  string encryption_key_id = 2;  // Use existing session key
}
```

## Deprecation Plan

Old voice call messages (if needed):
- Keep `VoiceCallRequest` and `VoiceCallResponse`
- Mark newer streaming messages as primary
- Maintain dual-path support for 2-3 versions

## References

- Protobuf 3 Syntax: https://developers.google.com/protocol-buffers/docs/proto3
- RustDesk Protocol: https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common
- Opus in Protobuf: Arbitrary binary `bytes` field

---

**Document Version**: 1.0  
**Last Updated**: 2026-02-25
