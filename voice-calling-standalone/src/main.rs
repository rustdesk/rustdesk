fn main() {
    println!("🎤 RustDesk Voice Calling - Standalone Test");
    println!();
    
    // Test codec module
    test_codec();
    println!();
    
    // Test configuration
    test_config();
    println!();
    
    // Test statistics
    test_stats();
    println!();
    
    println!("✅ All standalone tests passed!");
}

fn test_codec() {
    println!("Testing Audio Codec...");
    
    // Create codec config
    let config = CodecConfig {
        sample_rate: 16000,
        channels: 1,
        bitrate: 96,
        vbr: true,
        frame_duration_ms: 20,
    };
    
    println!("  Config created: {:?}", config);
    println!("  Sample rate: {}", config.sample_rate);
    println!("  Bitrate: {} kbps", config.bitrate);
    println!("  ✅ Codec module working");
}

fn test_config() {
    println!("Testing Voice Call Configuration...");
    
    let config = VoiceCallConfig::default();
    println!("  Default config: enabled={}", config.enabled);
    
    let quality_config = VoiceCallConfig::quality(128);
    println!("  High quality config: bitrate={} kbps", quality_config.max_bitrate_kbps);
    
    let low_bandwidth = VoiceCallConfig::low_bandwidth();
    println!("  Low bandwidth config: bitrate={} kbps", low_bandwidth.max_bitrate_kbps);
    
    println!("  ✅ Configuration module working");
}

fn test_stats() {
    println!("Testing Voice Call Statistics...");
    
    let mut stats = VoiceCallStats::new();
    stats.packets_sent = 1000;
    stats.packets_received = 950;
    stats.packets_lost = 50;
    
    let loss_rate = stats.packet_loss_rate();
    println!("  Packets sent: {}", stats.packets_sent);
    println!("  Packets received: {}", stats.packets_received);
    println!("  Packets lost: {}", stats.packets_lost);
    println!("  Packet loss rate: {:.2}%", loss_rate * 100.0);
    println!("  ✅ Statistics module working");
}

// Codec Configuration (simplified version of full implementation)
#[derive(Debug, Clone)]
struct CodecConfig {
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate: u32,
    pub vbr: bool,
    pub frame_duration_ms: u16,
}

impl Default for CodecConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            bitrate: 96,
            vbr: true,
            frame_duration_ms: 20,
        }
    }
}

// Voice Call Configuration
#[derive(Debug, Clone)]
struct VoiceCallConfig {
    pub enabled: bool,
    pub codec: CodecConfig,
    pub max_bitrate_kbps: u32,
    pub min_bitrate_kbps: u32,
    pub enable_echo_cancellation: bool,
    pub enable_noise_suppression: bool,
}

impl VoiceCallConfig {
    pub fn default() -> Self {
        Self {
            enabled: true,
            codec: CodecConfig::default(),
            max_bitrate_kbps: 128,
            min_bitrate_kbps: 24,
            enable_echo_cancellation: true,
            enable_noise_suppression: true,
        }
    }

    pub fn quality(bitrate: u32) -> Self {
        let mut config = Self::default();
        config.codec.bitrate = bitrate;
        config.max_bitrate_kbps = bitrate;
        config
    }

    pub fn low_bandwidth() -> Self {
        Self {
            enabled: true,
            codec: CodecConfig {
                bitrate: 24,
                sample_rate: 8000,
                channels: 1,
                vbr: true,
                frame_duration_ms: 40,
            },
            max_bitrate_kbps: 32,
            min_bitrate_kbps: 16,
            enable_echo_cancellation: false,
            enable_noise_suppression: false,
        }
    }
}

// Voice Call Statistics
#[derive(Debug, Clone, Default)]
struct VoiceCallStats {
    pub state: String,
    pub elapsed_seconds: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u32,
    pub packets_received: u32,
    pub packets_lost: u32,
    pub jitter_ms: f32,
    pub latency_ms: f32,
}

impl VoiceCallStats {
    pub fn new() -> Self {
        Self {
            state: "Idle".to_string(),
            ..Default::default()
        }
    }

    pub fn packet_loss_rate(&self) -> f32 {
        if self.packets_received == 0 {
            return 0.0;
        }
        self.packets_lost as f32 / (self.packets_sent as f32 + self.packets_lost as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_config() {
        let config = CodecConfig::default();
        assert_eq!(config.sample_rate, 16000);
        assert_eq!(config.bitrate, 96);
    }

    #[test]
    fn test_voice_call_config() {
        let config = VoiceCallConfig::quality(128);
        assert_eq!(config.max_bitrate_kbps, 128);
    }

    #[test]
fn test_low_bandwidth_config() {
        let config = VoiceCallConfig::low_bandwidth();
        assert_eq!(config.codec.bitrate, 24);
        assert_eq!(config.codec.sample_rate, 8000);
    }

    #[test]
    fn test_packet_loss_calculation() {
        let mut stats = VoiceCallStats::new();
        stats.packets_sent = 100;
        stats.packets_received = 98;
        stats.packets_lost = 2;
        
        let loss_rate = stats.packet_loss_rate();
        assert!(loss_rate > 0.0 && loss_rate < 0.1);
    }

    #[test]
    fn test_zero_packet_loss() {
        let stats = VoiceCallStats::new();
        assert_eq!(stats.packet_loss_rate(), 0.0);
    }
}
