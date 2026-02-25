// Audio Module Entry Point
// File: src/audio/mod.rs (refactored)
// Unified platform-aware audio module

pub mod codec;
pub mod jitter_buffer;
pub mod network;
pub mod capture;
pub mod playback;

#[cfg(target_os = "windows")]
pub mod windows_impl;

#[cfg(target_os = "macos")]
pub mod macos_impl;

#[cfg(target_os = "linux")]
pub mod linux_impl;

use hbb_common::{
    anyhow::Result as ResultType,
    log,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use codec::{AudioCodec, CodecConfig};
pub use jitter_buffer::{JitterBuffer, JitterBufferConfig};
pub use network::{AudioNetwork, AudioNetworkStats};
pub use capture::AudioCapture;
pub use playback::AudioPlayback;

/// Voice call feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceCallConfig {
    pub enabled: bool,
    pub codec: CodecConfig,
    pub jitter_buffer: JitterBufferConfig,
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
            jitter_buffer: JitterBufferConfig::default(),
            max_bitrate_kbps: 128,
            min_bitrate_kbps: 24,
            enable_echo_cancellation: true,
            enable_noise_suppression: true,
        }
    }

    pub fn quality(bitrate: u32) -> Self {
        let mut config = Self::default();
        config.codec.bitrate = bitrate;
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
            jitter_buffer: JitterBufferConfig::default(),
            max_bitrate_kbps: 32,
            min_bitrate_kbps: 16,
            enable_echo_cancellation: false,
            enable_noise_suppression: false,
        }
    }

    pub fn mobile() -> Self {
        Self {
            enabled: true,
            codec: CodecConfig {
                bitrate: 64,
                sample_rate: 16000,
                channels: 1,
                vbr: true,
                frame_duration_ms: 20,
            },
            jitter_buffer: JitterBufferConfig::default(),
            max_bitrate_kbps: 96,
            min_bitrate_kbps: 32,
            enable_echo_cancellation: true,
            enable_noise_suppression: true,
        }
    }
}

/// Voice call session state machine
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

/// Voice call quality metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    pub channels: u8,
}

impl VoiceCallStats {
    pub fn new() -> Self {
        Self {
            state: VoiceCallState::Idle.to_string(),
            ..Default::default()
        }
    }

    pub fn update_state(&mut self, state: VoiceCallState) {
        self.state = state.to_string();
    }

    pub fn packet_loss_rate(&self) -> f32 {
        if self.packets_received == 0 {
            return 0.0;
        }
        self.packets_lost as f32 / (self.packets_sent as f32 + self.packets_lost as f32)
    }

    pub fn incoming_bitrate_kbps(&self) -> u32 {
        if self.elapsed_seconds == 0 {
            return 0;
        }
        (self.bytes_received * 8 / 1000 / self.elapsed_seconds) as u32
    }

    pub fn outgoing_bitrate_kbps(&self) -> u32 {
        if self.elapsed_seconds == 0 {
            return 0;
        }
        (self.bytes_sent * 8 / 1000 / self.elapsed_seconds) as u32
    }
}

/// Initialize voice call module
pub fn init() -> ResultType<()> {
    log::info!("Initializing voice call module");
    // Platform-specific initialization
    #[cfg(target_os = "windows")]
    {
        log::info!("Using Windows WASAPI audio backend");
    }
    #[cfg(target_os = "macos")]
    {
        log::info!("Using macOS CoreAudio backend");
    }
    #[cfg(target_os = "linux")]
    {
        log::info!("Using Linux PulseAudio/ALSA backend");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_call_config_default() {
        let config = VoiceCallConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_bitrate_kbps, 128);
    }

    #[test]
    fn test_voice_call_config_low_bandwidth() {
        let config = VoiceCallConfig::low_bandwidth();
        assert_eq!(config.codec.bitrate, 24);
        assert_eq!(config.codec.sample_rate, 8000);
    }

    #[test]
    fn test_voice_call_state_display() {
        assert_eq!(VoiceCallState::Active.to_string(), "Active");
        assert_eq!(VoiceCallState::Idle.to_string(), "Idle");
    }

    #[test]
    fn test_voice_call_stats() {
        let mut stats = VoiceCallStats::new();
        assert_eq!(stats.packet_loss_rate(), 0.0);
        stats.packets_sent = 100;
        stats.packets_received = 90;
        stats.packets_lost = 10;
        let loss_rate = stats.packet_loss_rate();
        assert!(loss_rate > 0.0 && loss_rate < 1.0);
    }
}
