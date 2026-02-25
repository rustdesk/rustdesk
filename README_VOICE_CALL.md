# RustDesk Two-Way Voice Calling Feature

## Overview

This document describes the complete implementation of secure, real-time, two-way voice calling in RustDesk. The feature extends RustDesk's existing P2P architecture with a dedicated audio channel for voice communication while maintaining end-to-end encryption and low latency.

### Status
- **Phase 1**: Core audio loopback & Opus codec ✅
- **Phase 2**: Encrypted frame transmission 🔄 (This Implementation)
- **Phase 3**: Call signaling & session management 🔄
- **Phase 4**: Flutter UI integration 🔄
- **Phase 5**: Optimization & testing 📋

## Architecture

### 1. Audio Pipeline

```
┌──────────────────────────────────────────────────────────────────┐
│                    TWO-WAY VOICE CALLING                         │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  LOCAL SIDE:                            REMOTE SIDE:            │
│  ┌─────────────┐                        ┌─────────────┐          │
│  │   Capture   │                        │   Playback  │          │
│  │   (cpal)    │                        │   (cpal)    │          │
│  └──────┬──────┘                        └──────▲──────┘          │
│         │                                      │                 │
│         ▼                                      │                 │
│  ┌─────────────┐                        ┌──────────────┐         │
│  │   Encode    │                        │   Decode    │         │
│  │   (Opus)    │                        │   (Opus)    │         │
│  └──────┬──────┘                        └──────▲──────┘         │
│         │                                      │                 │
│         ▼                                      │                 │
│  ┌──────────────────────┐         ┌────────────┴─────────┐      │
│  │  Encrypt + Frame    │         │ Jitter Buffer       │      │
│  │  (E2E Encryption)   │────────▶│ (Playback Smoothing) │      │
│  │  + Sequence Number  │         └──────────────────────┘      │
│  │  + Timestamp        │                                        │
│  └─────────────────────┘                                        │
│         •                                                       │
│         • Network Transport (UDP primary, TCP fallback)         │
│         •                                                       │
│  ┌──────────────────────┐         ┌──────────────────────┐      │
│  │  Decrypt + Validate │◀────────│  Receive            │      │
│  │  E2E Encryption     │         │  + Validate         │      │
│  │  + Seq/Timestamp    │         │  + Decrypt          │      │
│  └─────────────────────┘         └──────────────────────┘      │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 2. Module Structure

```
src/audio/
├── mod.rs              # Main audio module & manager
├── capture.rs          # Audio input capture (cpal)
├── playback.rs         # Audio playback (cpal)
├── codec.rs            # Opus encoding/decoding
├── network.rs          # Audio frame transmission & reception
├── jitter_buffer.rs    # Playback smoothing & buffer management
└── protocol.rs         # Protocol definitions for audio frames
```

### 3. Protocol Layer

#### Message Types

**VoiceCallRequest** (Already exists in RustDesk)
```protobuf
message VoiceCallRequest {
    bool is_connect = 1;
    int64 req_timestamp = 2;
}
```

**VoiceCallResponse** (Already exists)
```protobuf
message VoiceCallResponse {
    bool accepted = 1;
    int64 req_timestamp = 2;
    int64 ack_timestamp = 3;
}
```

**New: AudioFrame** (To be added)
```protobuf
message AudioFrame {
    bytes data = 1;                // Opus-encoded audio
    uint32 sample_rate = 2;        // 16000 or 48000 Hz
    uint32 channels = 3;            // 1 = mono, 2 = stereo
    uint64 timestamp = 4;          // milliseconds since call start
    uint32 sequence_number = 5;    // Detects lost frames
    bool is_last_frame = 6;        // Marks call end
}
```

**New: AudioMute**
```protobuf
message AudioMute {
    bool is_muted = 1;
    int64 timestamp = 2;
}
```

**New: AudioError**
```protobuf
message AudioError {
    enum ErrorCode {
        MIC_PERMISSION_DENIED = 0;
        NO_AUDIO_DEVICE = 1;
        CODEC_ERROR = 2;
        NETWORK_ERROR = 3;
        RELAY_UNAVAILABLE = 4;
    }
    ErrorCode error_code = 1;
    string message = 2;
}
```

### 4. Session Management

#### Voice Call State Machine

```
         ┌─────────────────────────────────────────┐
         │          IDLE / DISCONNECTED            │
         └──┬──────────────────────────────────────┘
            │
            │ User clicks "Start Voice Call"
            ▼
   ┌────────────────────────┐
   │   VOICE_CALL_PENDING   │  (Sending VoiceCallRequest)
   │                        │
   │  Max timeout: 30s      │
   └────────┬────────┬──────┘
            │        │
            │ Accept │ Reject/Timeout
            ▼        ▼
    ┌──────────────────────┐    ┌──────────────────┐
    │  VOICE_CALL_ACTIVE   │    │   IDLE           │
    │                      │    └──────────────────┘
    │ • Audio Capture ON   │
    │ • Audio Playback ON  │
    │ • Frame TX/RX        │
    └────────────┬─────────┘
                 │
                 │ End Call (AudioEnd)
                 ▼
         ┌──────────────────┐
         │   IDLE           │
         └──────────────────┘
```

#### Features During Active Call

| Feature | Description | Implementation |
|---------|-------------|-----------------|
| **Mute** | Pause local audio capture | Send AudioMute message, skip capture loop |
| **Unmute** | Resume audio capture | Cancel mute, restart capture |
| **Audio Device Selection** | Switch between mics/speakers | Dynamic device switching without reconnect |
| **Adaptive Bitrate** | Adjust based on network | Monitor packet loss, adjust Opus bitrate |
| **Connection Quality** | Display network metrics | Track latency, jitter, packet loss |
| **Auto-Reconnect** | Retry failed connections | Single automatic retry with backoff |

### 5. Networking Layer

#### Audio Channel Transport

**Primary**: UDP (Direct P2P)
- Low latency preferred
- Connectionless
- Natural packet loss handling (Opus can recover)

**Fallback**: TCP (Via Relay)
- When UDP not available
- Less efficient but guaranteed delivery
- Used when behind restrictive firewalls

#### Frame Encapsulation

```
┌─────────────────────────────────────────────┐
│   RustDesk Session Encryption Layer         │
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │ AudioFrame Protobuf                  │  │
│  ├───────────────────────────────────────┤  │
│  │ • Opus-encoded audio data             │  │
│  │ • Sample rate (16k/48kHz)             │  │
│  │ • Sequence number (detect loss)       │  │
│  │ • Timestamp (sync across network)     │  │
│  └───────────────────────────────────────┘  │
│                                             │
│  [E2E Encryption: ChaCha20-Poly1305]        │
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │ Transport (UDP or TCP)                │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

#### Sequence Number & Timestamp

- **Sequence Number**: Increments per frame (detect out-of-order/loss)
- **Timestamp**: Milliseconds since call initialization
  - Used for frame ordering and jitter buffer playback timing
  - Helps detect network jitter and reordering

### 6. Jitter Buffer

The jitter buffer smooths playback by:
1. Buffering arriving frames
2. Detecting jitter (delay variations)
3. Adapting playback speed
4. Handling packet loss gracefully

**Configuration**:
- Target buffer size: 50-100ms
- Max buffer size: 500ms
- Underrun threshold: 10ms
- Overrun threshold: 200ms

**Adaptation Algorithm**:
```
if (buffer_size < underrun_threshold):
    reduce_playback_speed()
elif (buffer_size > overrun_threshold):
    skip_frames() or increase_playback_speed()
else:
    maintain_normal_speed()
```

## Implementation Details

### Core Components

#### 1. Audio Capture (capture.rs)

```rust
pub struct AudioCapture {
    device: cpal::Device,
    stream: Option<cpal::Stream>,
    sender: mpsc::Sender<AudioData>,
}

impl AudioCapture {
    pub async fn new(device_name: Option<&str>) -> ResultType<Self> { }
    pub async fn start(&mut self) -> ResultType<()> { }
    pub async fn stop(&mut self) -> ResultType<()> { }
}
```

**Key Features**:
- Cross-platform device enumeration
- Configurable sample rate (16kHz for low-bandwidth, 48kHz for quality)
- Mono/Stereo selection
- Thread-safe frame queue

#### 2. Audio Playback (playback.rs)

```rust
pub struct AudioPlayback {
    device: cpal::Device,
    stream: Option<cpal::Stream>,
    receiver: mpsc::Receiver<AudioData>,
    jitter_buffer: Arc<JitterBuffer>,
}

impl AudioPlayback {
    pub async fn new(device_name: Option<&str>) -> ResultType<Self> { }
    pub async fn start(&mut self) -> ResultType<()> { }
    pub fn feed_frame(&self, frame: AudioFrame) { }
}
```

#### 3. Opus Codec (codec.rs)

```rust
pub struct AudioCodec {
    encoder: Encoder,
    decoder: AudioDecoder,
    config: CodecConfig,
}

pub struct CodecConfig {
    pub sample_rate: u32,        // 16000 or 48000
    pub channels: u16,            // 1 or 2
    pub bitrate: u32,             // bits per second (8000-128000)
    pub vbr: bool,               // Variable bitrate
}

impl AudioCodec {
    pub fn encode(&mut self, pcm: &[f32]) -> ResultType<Bytes> { }
    pub fn decode(&mut self, opus_frame: &[u8]) -> ResultType<Vec<f32>> { }
}
```

#### 4. Network Layer (network.rs)

```rust
pub struct AudioNetwork {
    session: Arc<Session>,
    sender: mpsc::Sender<AudioFrame>,
    receiver: mpsc::Receiver<AudioFrame>,
}

impl AudioNetwork {
    pub async fn send_frame(&self, frame: AudioFrame) -> ResultType<()> { }
    pub async fn receive_frame(&mut self) -> ResultType<AudioFrame> { }
    pub async fn handle_error(&self, error: AudioError) { }
}
```

#### 5. Jitter Buffer (jitter_buffer.rs)

```rust
pub struct JitterBuffer {
    frames: Arc<Mutex<BTreeMap<u32, AudioFrame>>>,
    target_delay: Duration,
    config: JitterBufferConfig,
}

pub struct JitterBufferConfig {
    pub min_delay: Duration,
    pub max_delay: Duration,
    pub initial_delay: Duration,
}

impl JitterBuffer {
    pub fn insert(&self, frame: AudioFrame) -> ResultType<()> { }
    pub fn get_next_frame(&self) -> ResultType<Option<AudioFrame>> { }
    pub fn get_stats(&self) -> JitterBufferStats { }
}
```

### Integration Points

#### 1. Session Handler

Modify `src/client/io_loop.rs`:

```rust
pub struct Remote<T: InvokeUiSession> {
    // ... existing fields ...
    audio_channel: Option<AudioChannel>,
    voice_call_state: VoiceCallState,
}

impl<T: InvokeUiSession> Remote<T> {
    async fn handle_voice_call_request(&mut self, req: VoiceCallRequest) {
        // Accept/reject call
        let voice_channel = AudioChannel::new(
            self.handler.clone(),
            self.sender.clone(),
        ).await;
        
        self.handler.on_voice_call_started();
    }
    
    async fn handle_audio_frame(&mut self, frame: AudioFrame) {
        if let Some(channel) = &self.audio_channel {
            channel.playback.feed_frame(frame).await;
        }
    }
}
```

#### 2. UI Event Handlers

Modify `src/ui_session_interface.rs`:

```rust
impl Session {
    pub fn request_voice_call(&self) {
        let msg = new_voice_call_request(true);
        self.send(Data::Message(msg));
        self.on_voice_call_waiting();
    }
    
    pub fn accept_voice_call(&self) {
        let msg = new_voice_call_response(
            self.voice_call.request_timestamp,
            true,
        );
        self.send(Data::Message(msg));
        self.start_audio_channel();
    }
    
    pub fn reject_voice_call(&self) {
        let msg = new_voice_call_response(
            self.voice_call.request_timestamp,
            false,
        );
        self.send(Data::Message(msg));
    }
    
    pub fn mute_voice_call(&self, is_muted: bool) {
        let msg = create_audio_mute_message(is_muted);
        self.send(Data::Message(msg));
    }
    
    pub fn end_voice_call(&self) {
        // Send end frame
        // Shutdown audio channels
        // Reset state
    }
}
```

#### 3. Protocol Message Handling

Modify `src/client/io_loop.rs` message routing:

```rust
async fn handle_message(&mut self, msg: Message) {
    match msg.union {
        Some(message::Union::VoiceCallRequest(req)) => {
            self.handle_voice_call_request(req).await;
        }
        Some(message::Union::VoiceCallResponse(resp)) => {
            self.handle_voice_call_response(resp).await;
        }
        Some(message::Union::AudioFrame(frame)) => {
            self.handle_audio_frame(frame).await;
        }
        Some(message::Union::AudioMute(mute)) => {
            self.handle_audio_mute(mute).await;
        }
        Some(message::Union::AudioError(error)) => {
            self.handle_audio_error(error).await;
        }
        // ... other message types ...
    }
}
```

## Feature Flag

Add to `Cargo.toml`:

```toml
[features]
default = ["use_dasp"]
# ... other features ...
voice-call = []  # Enable two-way voice calling
```

Build with voice calling:
```bash
cargo build --features voice-call
```

## Error Handling

### Graceful Degradation

| Scenario | Handling |
|----------|----------|
| Mic permission denied | Show UI error, disable voice call button |
| No audio device found | Fall back to relay-only mode |
| UDP blocked by firewall | Automatic fallback to TCP relay |
| Packet loss > 20% | Notify UI, reduce quality |
| Network disconnection | Attempt auto-reconnect (1 retry) |
| Codec initialization failure | Log error, disable voice call |

### Logging & Diagnostics

All audio operations log with `log::info!()`, `log::warn!()`, `log::error!()`:

```rust
log::info!("Voice call started with {}ms jitter buffer", delay_ms);
log::warn!("Audio packet loss detected: {}%", loss_percent);
log::error!("Opus decoder initialization failed: {}", err);
```

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| **Latency** | < 150ms E2E | Including encode/decode/network/jitter buffer |
| **CPU Usage** | < 10% on mid-tier machine | Opus is efficient, cpal is native |
| **Bandwidth** | 16-64 kbps | Adaptive based on network conditions |
| **Memory** | < 50MB per session | Jitter buffer + codec buffers |
| **Startup Time** | < 500ms | Call accept to audio flow |

## Flutter UI Integration

### Dart/Flutter Side

```dart
class VoiceCallWidget extends StatefulWidget {
  @override
  _VoiceCallWidgetState createState() => _VoiceCallWidgetState();
}

class _VoiceCallWidgetState extends State<VoiceCallWidget> {
  bool _isMuted = false;
  bool _inCall = false;
  
  void _startVoiceCall() {
    bind.sessionRequestVoiceCall();  // Call Rust backend
    setState(() => _inCall = true);
  }
  
  void _endVoiceCall() {
    bind.sessionEndVoiceCall();
    setState(() => _inCall = false);
  }
  
  void _toggleMute() {
    setState(() => _isMuted = !_isMuted);
    bind.sessionMuteVoiceCall(_isMuted);
  }
  
  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        if (_inCall) ...[
          Text("Voice Call Active"),
          Row(
            children: [
              ElevatedButton(
                onPressed: _toggleMute,
                child: Text(_isMuted ? "Unmute" : "Mute"),
              ),
              ElevatedButton(
                onPressed: _endVoiceCall,
                child: Text("End Call"),
              ),
            ],
          ),
        ] else ...[
          ElevatedButton(
            onPressed: _startVoiceCall,
            child: Text("Start Voice Call"),
          ),
        ],
      ],
    );
  }
}
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_codec_encode_decode() {
        let config = CodecConfig::default();
        let mut codec = AudioCodec::new(config).unwrap();
        
        let pcm_input = vec![0.1; 480];  // 10ms @ 48kHz
        let encoded = codec.encode(&pcm_input).unwrap();
        let decoded = codec.decode(&encoded).unwrap();
        
        assert!(!decoded.is_empty());
    }
    
    #[test]
    fn test_sequence_detection() {
        let mut buffer = JitterBuffer::new(Default::default());
        
        // Insert out-of-order frames
        buffer.insert(frame_seq(2)).unwrap();
        buffer.insert(frame_seq(1)).unwrap();
        
        // Should reorder correctly
        let next = buffer.get_next_frame().unwrap().unwrap();
        assert_eq!(next.sequence_number, 1);
    }
}
```

### Integration Tests

- Test E2E audio transmission between two instances
- Verify packet loss recovery
- Test network failover (UDP → TCP)
- Validate encryption/decryption

### Platform-Specific Tests

- **Windows**: Test WASAPI audio capture
- **macOS**: Test CoreAudio & permissions
- **Linux**: Test PulseAudio/ALSA fallback

## Deployment

### Build Variants

```bash
# Build with voice calling enabled
python3 build.py --flutter --features voice-call

# Build with hardware codec + voice calling
python3 build.py --flutter --hwcodec --features voice-call

# Release build
python3 build.py --flutter --release --features voice-call
```

### Package Configuration

Update CI/CD pipelines to:
1. Add `voice-call` to default features (once stable)
2. Include audio library dependencies
3. Test on all target platforms

## Known Limitations & Future Enhancements

### Current Phase
- ✅ Mono audio (Stereo optional)
- ✅ 16kHz & 48kHz sample rates
- ✅ Opus adaptive bitrate
- ✅ Jitter buffer with smooth playback
- ✅ E2E encryption via existing RustDesk keys

### Future Enhancements
- 🔄 Group voice calls (conference)
- 🔄 Audio recording with encryption
- 🔄 Noise suppression (WebRTC VAD)
- 🔄 Echo cancellation
- 🔄 Advanced codec selection (FLAC for LAN)
- 🔄 Mobile-optimized UI (iOS optimizations)
- 🔄 Network bandwidth adaptation UI

## Security Considerations

### Encryption
- All audio frames use RustDesk's existing E2E encryption (ChaCha20-Poly1305)
- Session keys derived from handshake are reused for audio
- No plaintext audio over network

### Dos Prevention
- Call request throttling (max 1 call per 5 seconds)
- Frame size limits (max 16KB per frame)
- Sequence number validation prevents replay attempts

### Permissions
- Gracefully handle microphone permission denial
- Warn user if speaker permissions denied (iOS)
- Respect system privacy controls

## References & Resources

- **Opus Codec**: https://github.com/xiph/opus
- **cpal Audio Library**: https://github.com/RustAudio/cpal
- **RustDesk Architecture**: https://github.com/rustdesk/rustdesk/wiki
- **Protocol Buffers**: https://developers.google.com/protocol-buffers

## Contributing

When implementing voice calling features:
1. Follow RustDesk code style guidelines
2. Add comprehensive error handling
3. Include logging for diagnostics
4. Write platform-specific tests
5. Test on Windows, macOS, and Linux
6. Document any new module APIs

---

**Document Version**: 1.0
**Last Updated**: 2026-02-25
**Maintainers**: RustDesk Team
