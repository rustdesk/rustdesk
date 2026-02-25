//! Audio module for RustDesk voice calling feature.
//!
//! This module provides:
//! - Audio capture and playback using cpal
//! - Opus compression/decompression
//! - Jitter buffer for smooth playback
//! - Network transmission of audio frames
//! - Call state management
//!
//! # Feature Flags
//!
//! This module is enabled with the `voice-call` feature flag.

use hbb_common::{
    anyhow::{anyhow, Result as ResultType},
    log,
    message_proto::{AudioFrame, AudioMute, Message},
    tokio::sync::{mpsc, Mutex as TokioMutex},
};
use std::sync::Arc;

pub mod capture;
pub mod codec;
pub mod jitter_buffer;
pub mod network;
pub mod playback;

pub use capture::AudioCapture;
pub use codec::{AudioCodec, CodecConfig};
pub use jitter_buffer::JitterBuffer;
pub use network::AudioNetwork;
pub use playback::AudioPlayback;

/// Configuration for audio voice calling
#[derive(Debug, Clone)]
pub struct VoiceCallConfig {
    /// Sample rate in Hz: 16000 or 48000
    pub sample_rate: u32,
    /// Number of channels: 1 (mono) or 2 (stereo)
    pub channels: u16,
    /// Initial bitrate in bits per second
    pub initial_bitrate: u32,
    /// Maximum jitter buffer delay in milliseconds
    pub max_jitter_delay_ms: u32,
    /// Input device name (None = default)
    pub input_device: Option<String>,
    /// Output device name (None = default)
    pub output_device: Option<String>,
}

impl Default for VoiceCallConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,  // Lower bandwidth by default
            channels: 1,          // Mono for lower bandwidth
            initial_bitrate: 32000, // 32 kbps
            max_jitter_delay_ms: 200,
            input_device: None,
            output_device: None,
        }
    }
}

impl VoiceCallConfig {
    /// Create config optimized for quality
    pub fn quality() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            initial_bitrate: 128000,
            ..Default::default()
        }
    }

    /// Create config optimized for low bandwidth
    pub fn low_bandwidth() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            initial_bitrate: 16000,
            max_jitter_delay_ms: 300,
            ..Default::default()
        }
    }

    /// Create config optimized for mobile
    pub fn mobile() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            initial_bitrate: 24000,
            max_jitter_delay_ms: 250,
            ..Default::default()
        }
    }
}

/// Voice call state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceCallState {
    /// Not in a call
    Idle,
    /// Waiting for remote to answer
    Pending,
    /// Call is active
    Active,
    /// Call is being terminated
    Ending,
}

impl std::fmt::Display for VoiceCallState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Pending => write!(f, "Pending"),
            Self::Active => write!(f, "Active"),
            Self::Ending => write!(f, "Ending"),
        }
    }
}

/// Statistics for voice call quality monitoring
#[derive(Debug, Clone, Default)]
pub struct VoiceCallStats {
    /// Number of frames transmitted
    pub frames_sent: u64,
    /// Number of frames received
    pub frames_received: u64,
    /// Number of frames lost (detected by sequence number gaps)
    pub frames_lost: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f32,
    /// Current jitter buffer size in milliseconds
    pub jitter_buffer_delay_ms: u32,
    /// Estimated bandwidth used in kbps
    pub bandwidth_kbps: u32,
    /// Package loss percentage
    pub packet_loss_pct: f32,
}

impl VoiceCallStats {
    /// Calculate packet loss percentage
    pub fn calculate_loss_pct(&mut self) {
        let total = self.frames_sent + self.frames_lost;
        if total > 0 {
            self.packet_loss_pct = (self.frames_lost as f32 / total as f32) * 100.0;
        }
    }

    /// Update bandwidth estimate (bytes per second)
    pub fn update_bandwidth(&mut self, bytes_per_sec: u32) {
        self.bandwidth_kbps = (bytes_per_sec * 8) / 1000;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = VoiceCallConfig::default();
        assert_eq!(config.sample_rate, 16000);
        assert_eq!(config.channels, 1);
    }

    #[test]
    fn test_quality_config() {
        let config = VoiceCallConfig::quality();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
        assert!(config.initial_bitrate > 100000);
    }

    #[test]
    fn test_voice_call_state_display() {
        assert_eq!(VoiceCallState::Idle.to_string(), "Idle");
        assert_eq!(VoiceCallState::Active.to_string(), "Active");
    }

    #[test]
    fn test_packet_loss_calculation() {
        let mut stats = VoiceCallStats {
            frames_sent: 100,
            frames_lost: 10,
            ..Default::default()
        };
        stats.calculate_loss_pct();
        assert_eq!(stats.packet_loss_pct, 10.0);
    }
}
