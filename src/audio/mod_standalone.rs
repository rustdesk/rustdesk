// Standalone Voice Calling Module - Can be compiled independently
// This is a complete, testable implementation of the voice-call feature

pub mod codec;
pub mod jitter_buffer;
pub mod network;
pub mod capture;
pub mod playback;

pub use codec::AudioCodec;
pub use jitter_buffer::JitterBuffer;
pub use network::AudioNetwork;

use std::sync::Arc;

/// Main voice calling module configuration
#[derive(Debug, Clone)]
pub struct VoiceCallConfig {
    pub enabled: bool,
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
}

impl Default for VoiceCallConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bitrate_kbps: 96,
            sample_rate: 16000,
        }
    }
}

/// Voice call state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceCallState {
    Idle,
    Pending,
    Active,
    Muted,
    Ending,
    Error,
}

impl std::fmt::Display for VoiceCallState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Pending => write!(f, "Pending"),
            Self::Active => write!(f, "Active"),
            Self::Muted => write!(f, "Muted"),
            Self::Ending => write!(f, "Ending"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// Statistics for voice calls
#[derive(Debug, Clone, Default)]
pub struct VoiceCallStats {
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
            state: VoiceCallState::Idle.to_string(),
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
    fn test_voice_call_config() {
        let config = VoiceCallConfig::default();
        assert!(config.enabled);
        assert_eq!(config.bitrate_kbps, 96);
    }

    #[test]
    fn test_voice_call_state() {
        assert_eq!(VoiceCallState::Active.to_string(), "Active");
        assert_eq!(VoiceCallState::Idle.to_string(), "Idle");
    }

    #[test]
    fn test_voice_call_stats() {
        let stats = VoiceCallStats::new();
        assert_eq!(stats.packet_loss_rate(), 0.0);
    }
}
