# Voice Calling Implementation - Testing & Optimization Guide

## Testing Strategy

### Phase 1: Unit Tests (Local)

#### Codec Testing
```bash
cd src/audio
cargo test codec --lib
```

**Test Coverage:**
- ✅ Codec initialization with different sample rates
- ✅ Opus encode/decode roundtrip
- ✅ Bitrate adaptation
- ✅ Error handling on invalid input

#### Jitter Buffer Testing
```bash
cargo test jitter_buffer --lib
```

**Test Coverage:**
- ✅ Frame insertion and ordering
- ✅ Sequence number gap detection
- ✅ Out-of-order frame reordering
- ✅ Underrun/overrun detection
- ✅ Loss percentage calculation

#### Network Layer Testing
```bash
cargo test network --lib
```

**Test Coverage:**
- ✅ Frame serialization/deserialization
- ✅ Sequence counter increment
- ✅ Timestamp monotonicity
- ✅ Bandwidth calculation

### Phase 2: Integration Tests

#### Local Loopback Test
```rust
#[tokio::test]
async fn test_audio_loopback() {
    // Test full audio pipeline: capture → encode → decode → playback
    let config = VoiceCallConfig::default();
    let codec = AudioCodec::new(CodecConfig::voip()).unwrap();
    let buffer = JitterBuffer::new(JitterBufferConfig::default());
    
    // Capture 10 frames
    for i in 0..10 {
        // Get PCM from capture
        // Encode to Opus
        // Add to jitter buffer
        // Decode from buffer
        // Verify audio integrity
    }
}
```

#### Frame Loss Simulation
```rust
#[tokio::test]
async fn test_packet_loss_resilience() {
    // Simulate 5% packet loss
    // Verify Opus can handle missing frames
    // Verify jitter buffer recovery
}
```

#### Latency Measurement
```rust
#[tokio::test]
async fn test_end_to_end_latency() {
    // Measure time from capture to playback
    // Expected: < 150ms on modern hardware
    // Breakdown:
    //   - Capture delay: ~5ms
    //   - Encode: ~10ms
    //   - Network: ~50-100ms
    //   - Jitter buffer: ~50ms
    //   - Decode: ~10ms
    //   - Playback: ~5ms
}
```

### Phase 3: Platform Testing

#### Windows Testing
```bash
# Test WASAPI audio capture/playback
cargo test --target x86_64-pc-windows-msvc --features voice-call

# Key scenarios:
# - Multiple audio devices
# - ASIO compatibility
# - Virtual audio cables (VB-Audio)
# - Bluetooth headsets
```

**Environment Setup:**
```powershell
# Install VB-Audio Cable (for loopback testing)
# https://vb-audio.com/Cable/

# Run test suite
cargo test --release --features voice-call
```

#### macOS Testing
```bash
# Test CoreAudio
cargo test --target x86_64-apple-darwin --features voice-call

# Key scenarios:
# - Microphone permissions dialog
# - Soundflower loopback
# - AirPods pairing
```

**Environment Setup:**
```bash
# Install Soundflower (loopback device)
# https://github.com/mattingalls/Soundflower

# Run tests
cargo test --release --features voice-call
```

#### Linux Testing
```bash
# Test PulseAudio & ALSA
cargo test --target x86_64-unknown-linux-gnu --features voice-call

# Key scenarios:
# - PulseAudio loopback module
# - ALSA fallback
# - Jack compatibility
```

**Environment Setup:**
```bash
# Install PulseAudio loopback module
sudo apt-get install pulseaudio-module-loopback

# Load loopback module
pactl load-module module-loopback

# Run tests
cargo test --release --features voice-call
```

### Phase 4: Network Testing

#### Packet Loss Scenarios
| Scenario | Packet Loss | Expected Behavior |
|----------|-------------|-------------------|
| Good Network | 0-1% | Seamless audio |
| Fair Network | 1-5% | Minor artifacts |
| Poor Network | 5-10% | Noticeable but usable |
| Very Poor | > 10% | Degraded quality |

#### Latency Scenarios
| Latency | Use Case | Acceptable |
|---------|----------|-----------|
| < 50ms | LAN | Excellent |
| 50-150ms | Direct internet | Good |
| 150-300ms | Relay/satellite | Acceptable |
| > 300ms | Very distant | Poor |

#### Bandwidth Limiting Test
```bash
# Use tc (traffic control) on Linux
# Simulate 128kbps bandwidth
tc qdisc add dev lo root tbf rate 128kbit burst 32kbit latency 400ms

# Run voice call test
cargo test network_bandwidth_adaptation --release

# Cleanup
tc qdisc del dev lo root
```

### Phase 5: Performance Testing

#### CPU Usage Benchmark
```bash
# Measure CPU usage during call
# Target: < 10% on mid-tier machine (Intel i5-8400)

cargo bench --features voice-call --bench audio_codec
# Sample output:
# test encode_16k_mono      ... bench:  1,234,567 ns/iter
# test decode_16k_mono      ... bench:    987,654 ns/iter
```

**Expected Breakdown:**
- Capture: 2-3%
- Encode: 3-4%
- Network: 1-2%
- Decode: 3-4%
- Playback: 1-2%

#### Memory Usage
```bash
# Measure memory footprint
valgrind --leak-check=yes target/release/rustdesk --test-voice-call

# Expected: < 50MB per call
# - Jitter buffer: ~10MB
# - Codec buffers: ~5MB
# - Network queues: ~10MB
# - PCM buffers: ~10MB
```

#### Battery Impact (Mobile)
```bash
# Monitor battery drain with continuous call
# Expected drain rate: < 5% per hour over cellular
# Expected drain rate: < 2% per hour over WiFi
```

## Optimization Techniques

### 1. Codec Optimization

#### Adaptive Bitrate
```rust
pub fn adapt_bitrate(stats: &VoiceCallStats) -> u32 {
    let loss_pct = stats.packet_loss_pct;
    let latency = stats.avg_latency_ms;
    
    match (loss_pct, latency) {
        // Good conditions
        (l, _) if l < 1.0 => 64000,
        // Fair conditions  
        (1.0..=5.0, _) => 48000,
        // Poor conditions
        (5.0..=10.0, _) => 32000,
        // Very poor
        _ => 24000,
    }
}
```

#### Frame Size Selection
```rust
pub fn select_frame_duration(latency_ms: u32) -> u32 {
    // Lower latency = smaller frames = more overhead
    // Higher latency = larger frames = more buffering
    match latency_ms {
        0..=50 => 10,      // 10ms frames
        51..=150 => 20,    // 20ms frames (default)
        151..=300 => 40,   // 40ms frames
        _ => 60,           // 60ms frames
    }
}
```

### 2. Jitter Buffer Optimization

#### Dynamic Delay Adjustment
```rust
impl JitterBuffer {
    pub fn adapt_delay(&self, detected_jitter: u32) {
        // Increase delay gradually as jitter increases
        let new_delay = (self.config.target_delay_ms as f32
            * (1.0 + detected_jitter as f32 / 100.0)) as u32;
        
        let new_delay = new_delay
            .max(self.config.min_delay_ms)
            .min(self.config.max_delay_ms);
        
        if new_delay != self.config.target_delay_ms {
            log::info!("Jitter buffer delay: {} → {}ms",
                self.config.target_delay_ms, new_delay);
            self.config.target_delay_ms = new_delay;
        }
    }
}
```

#### Frame Resampling
```rust
pub fn resample_if_needed(
    audio: &[f32],
    from_rate: u32,
    to_rate: u32,
) -> Vec<f32> {
    if from_rate == to_rate {
        return audio.to_vec();
    }
    
    // Use linear interpolation for quality
    // or SpeexDSP for high quality
    interpolate(audio, from_rate as f64 / to_rate as f64)
}
```

### 3. Network Optimization

#### Frame Aggregation
```rust
pub struct FrameAggregator {
    pending_frames: Vec<AudioFrame>,
    max_aggregate_size: usize,
    max_wait_time: Duration,
}

impl FrameAggregator {
    pub fn should_flush(&self) -> bool {
        self.pending_frames.len() >= self.max_aggregate_size ||
        self.elapsed_time() >= self.max_wait_time
    }
}
```

#### Priority-Based Transmission
```rust
pub enum FramePriority {
    Critical,   // Send immediately (first frames)
    Normal,     // Batch with others
    Low,        // Can be dropped on congestion
}

impl AudioNetwork {
    pub fn send_with_priority(
        &self,
        frame: AudioFrame,
        priority: FramePriority,
    ) -> ResultType<()> {
        match priority {
            FramePriority::Critical => self.send_frame(frame),
            FramePriority::Normal => self.queue_frame(frame),
            FramePriority::Low => {
                if self.queue_size() < self.max_queue_size {
                    self.queue_frame(frame)
                } else {
                    Ok(())  // Drop if queue full
                }
            }
        }
    }
}
```

### 4. CPU Optimization

#### SIMD Audio Processing
```rust
#[cfg(target_arch = "x86_64")]
pub fn resample_simd(input: &[f32], ratio: f32) -> Vec<f32> {
    // Use SSE2/AVX for faster resampling
    #[cfg(target_feature = "avx2")]
    {
        // AVX2 optimized version
    }
    
    #[cfg(not(target_feature = "avx2"))]
    {
        // Fallback to standard implementation
    }
}
```

#### Thread Affinity
```rust
pub fn start_audio_thread_with_affinity() {
    std::thread::Builder::new()
        .name("audio-capture".to_string())
        .spawn(|| {
            #[cfg(target_os = "windows")]
            {
                // Pin to specific CPU core
                // Windows: SetThreadAffinityMask
            }
            
            audio_capture_loop();
        }).unwrap();
}
```

### 5. Memory Optimization

#### Ring Buffer Usage
```rust
pub struct RingBuffer<T> {
    data: Vec<T>,
    head: usize,
    tail: usize,
    capacity: usize,
}

impl<T: Clone> RingBuffer<T> {
    // Pre-allocated, no heap churn
    // O(1) insertion/removal
}
```

#### Pool Allocation
```rust
pub struct BufferPool {
    available: Vec<Vec<f32>>,
    in_use: Vec<Vec<f32>>,
    capacity: usize,
}

impl BufferPool {
    pub fn get(&mut self, size: usize) -> Vec<f32> {
        self.available.pop()
            .unwrap_or_else(|| vec![0.0; size])
    }
    
    pub fn return_buffer(&mut self, buf: Vec<f32>) {
        if self.in_use.len() < 10 {
            self.in_use.push(buf);
        }
    }
}
```

## Performance Profiling

### Using Flamegraph
```bash
cargo install flamegraph
cargo flamegraph --features voice-call -- --bench audio_codec

# Analyze output
# Look for long call chains in audio processing
# Identify hot loops for optimization
```

### Using perf (Linux)
```bash
cargo build --release --features voice-call
perf record -F 99 ./target/release/rustdesk
perf report
```

### Using Instruments (macOS)
```bash
cargo build --release --features voice-call
open -a Instruments ./target/release/rustdesk
```

### Using Profiler (Windows)
```powershell
cargo build --release --features voice-call
# Use Windows Performance Analyzer
# wpa.exe target\release\rustdesk.exe
```

## Continuous Benchmarking

Create `benches/audio_codec.rs`:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn audio_codec_benchmark(c: &mut Criterion) {
    c.bench_function("encode_16k_mono_20ms", |b| {
        let codec = AudioCodec::new(CodecConfig::voip()).unwrap();
        let pcm = vec![0.0; 320];  // 16000 * 0.02
        
        b.iter(|| {
            codec.encode(black_box(&pcm))
        });
    });
}

criterion_group!(benches, audio_codec_benchmark);
criterion_main!(benches);
```

Run benchmarks:
```bash
cargo bench --features voice-call
```

## Regression Testing

Check performance doesn't degrade:
```bash
# Store baseline
cargo bench --features voice-call -- --save-baseline main

# After changes, compare
cargo bench --features voice-call -- --baseline main
```

## Deployment Testing Checklist

- [ ] Unit tests pass on all platforms
- [ ] Integration tests pass with 0% packet loss
- [ ] Integration tests pass with 5% packet loss
- [ ] CPU usage < 10% on target hardware
- [ ] Memory usage < 50MB per session
- [ ] Latency < 150ms E2E on LAN
- [ ] Audio quality acceptable on mobile (16kHz)
- [ ] Audio quality acceptable on desktop (48kHz)
- [ ] Graceful degradation on poor networks
- [ ] All error cases logged properly
- [ ] Flutter UI integration tested
- [ ] Cross-platform audio device enumeration works
- [ ] Microphone permission handling works
- [ ] Call state persistence tested
- [ ] Reconnection after network drop tested

## Common Issues & Solutions

### Issue: High CPU Usage (>15%)
**Solutions:**
1. Reduce codec frame size from 20ms to 40ms
2. Decrease sample rate to 16kHz
3. Lower bitrate by 10-20%
4. Check for UI rendering in hot path

### Issue: Audio Dropout
**Solutions:**
1. Increase jitter buffer delay (100ms → 150ms)
2. Reduce Opus application complexity
3. Check system load during test
4. Verify no other audio apps running

### Issue: Echo on Headphone
**Solutions:**
1. This is by design (capture from ear mic, output to speaker)
2. On Linux: use module-loopback with mute option
3. Recommend users use headphones instead

### Issue: Latency > 200ms
**Solutions:**
1. Check if using relay (adds 50-100ms)
2. Reduce frame size to 10ms (if CPU allows)
3. Check network path latency with ping
4. Use direct connection instead of relay

---

**Document Version**: 1.0  
**Last Updated**: 2026-02-25
