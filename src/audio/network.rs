//! Audio network layer for transmitting audio frames.
//!
//! Handles sending and receiving audio frames over the encrypted RustDesk connection.

use hbb_common::{
    anyhow::{anyhow, Result as ResultType},
    log,
    message_proto::{AudioMute, Message},
    tokio::sync::mpsc,
};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Instant;

/// Audio frame for network transmission
#[derive(Debug, Clone)]
pub struct NetworkAudioFrame {
    /// Frame sequence number for ordering and loss detection
    pub sequence_number: u32,
    /// Timestamp relative to call start (milliseconds)
    pub timestamp: u64,
    /// Opus-encoded audio data
    pub data: Vec<u8>,
    /// Sample rate (16000 or 48000)
    pub sample_rate: u32,
    /// Number of channels (1 or 2)
    pub channels: u16,
}

/// Audio network error
#[derive(Debug, Clone)]
pub enum AudioNetworkError {
    /// Microphone permission denied
    MicPermissionDenied,
    /// No audio device found
    NoAudioDevice,
    /// Codec operation failed
    CodecError(String),
    /// Network send error
    SendError(String),
    /// Connection failed
    ConnectionFailed,
    /// Relay unavailable
    RelayUnavailable,
}

impl std::fmt::Display for AudioNetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MicPermissionDenied => write!(f, "Microphone permission denied"),
            Self::NoAudioDevice => write!(f, "No audio device found"),
            Self::CodecError(msg) => write!(f, "Codec error: {}", msg),
            Self::SendError(msg) => write!(f, "Send error: {}", msg),
            Self::ConnectionFailed => write!(f, "Connection failed"),
            Self::RelayUnavailable => write!(f, "Relay unavailable"),
        }
    }
}

/// Audio network layer
pub struct AudioNetwork {
    /// Current sequence number for outgoing frames
    sequence_counter: Arc<AtomicU32>,
    /// Call start time for timestamp calculation
    start_time: Instant,
    /// Sender for outgoing frames
    tx: Option<mpsc::UnboundedSender<NetworkAudioFrame>>,
    /// Receiver for incoming frames
    rx: Option<mpsc::UnboundedReceiver<NetworkAudioFrame>>,
}

impl AudioNetwork {
    /// Create a new audio network layer
    pub fn new() -> (Self, mpsc::UnboundedSender<NetworkAudioFrame>, mpsc::UnboundedReceiver<NetworkAudioFrame>) {
        let (tx, rx) = mpsc::unbounded_channel();

        let network = Self {
            sequence_counter: Arc::new(AtomicU32::new(0)),
            start_time: Instant::now(),
            tx: Some(tx.clone()),
            rx: Some(rx),
        };

        (network, tx, rx)
    }

    /// Create a frame for transmission
    pub fn create_frame(
        &self,
        opus_data: Vec<u8>,
        sample_rate: u32,
        channels: u16,
    ) -> NetworkAudioFrame {
        let sequence = self.sequence_counter.fetch_add(1, Ordering::Relaxed);
        let timestamp = self.start_time.elapsed().as_millis() as u64;

        NetworkAudioFrame {
            sequence_number: sequence,
            timestamp,
            data: opus_data,
            sample_rate,
            channels,
        }
    }

    /// Get current sequence number
    pub fn current_sequence(&self) -> u32 {
        self.sequence_counter.load(Ordering::Relaxed)
    }

    /// Reset sequence counter (for new call)
    pub fn reset(&self) {
        self.sequence_counter.store(0, Ordering::Relaxed);
    }

    /// Get elapsed time since call started
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    /// Create a protobuf message from a network frame
    pub fn frame_to_protobuf(frame: &NetworkAudioFrame) -> Message {
        let mut audio_frame = hbb_common::message_proto::AudioFrame::new();
        audio_frame.set_data(frame.data.clone().into());
        audio_frame.set_sample_rate(frame.sample_rate);
        audio_frame.set_channels(frame.channels as u32);
        audio_frame.set_timestamp(frame.timestamp);
        audio_frame.set_sequence_number(frame.sequence_number);

        let mut msg = Message::new();
        msg.set_audio_frame(audio_frame);
        msg
    }

    /// Parse a protobuf AudioFrame into a NetworkAudioFrame
    pub fn protobuf_to_frame(pb_frame: &hbb_common::message_proto::AudioFrame) -> ResultType<NetworkAudioFrame> {
        Ok(NetworkAudioFrame {
            sequence_number: pb_frame.sequence_number,
            timestamp: pb_frame.timestamp,
            data: pb_frame.data.to_vec(),
            sample_rate: pb_frame.sample_rate,
            channels: pb_frame.channels as u16,
        })
    }

    /// Send an audio frame
    pub async fn send_frame(&self, frame: NetworkAudioFrame) -> ResultType<()> {
        if let Some(tx) = &self.tx {
            tx.send(frame)
                .map_err(|e| anyhow!("Failed to queue audio frame: {}", e))?;
            Ok(())
        } else {
            Err(anyhow!("Audio network not initialized"))
        }
    }

    /// Send a mute notification
    pub fn create_mute_message(is_muted: bool) -> Message {
        let mut mute = AudioMute::new();
        mute.set_is_muted(is_muted);
        mute.set_timestamp(hbb_common::get_time());

        let mut msg = Message::new();
        msg.set_audio_mute(mute);
        msg
    }
}

impl Default for AudioNetwork {
    fn default() -> Self {
        let (network, _tx, _rx) = Self::new();
        network
    }
}

/// Statistics for audio network
#[derive(Debug, Clone, Default)]
pub struct AudioNetworkStats {
    /// Frames sent
    pub frames_sent: u64,
    /// Frames received
    pub frames_received: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Estimated bandwidth out (kbps)
    pub bandwidth_out_kbps: u32,
    /// Estimated bandwidth in (kbps)
    pub bandwidth_in_kbps: u32,
}

impl AudioNetworkStats {
    /// Calculate current network statistics
    pub fn calculate(&mut self, elapsed_seconds: u64) {
        if elapsed_seconds > 0 {
            self.bandwidth_out_kbps = (self.bytes_sent * 8 / elapsed_seconds as u64) as u32 / 1000;
            self.bandwidth_in_kbps = (self.bytes_received * 8 / elapsed_seconds as u64) as u32 / 1000;
        }
    }

    /// Get human-readable bandwidth summary
    pub fn bandwidth_summary(&self) -> String {
        format!(
            "↑ {}kbps / ↓ {}kbps",
            self.bandwidth_out_kbps, self.bandwidth_in_kbps
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_network_creation() {
        let (network, _tx, _rx) = AudioNetwork::new();
        assert_eq!(network.current_sequence(), 0);
    }

    #[test]
    fn test_sequence_increment() {
        let (network, _tx, _rx) = AudioNetwork::new();

        let frame1 = network.create_frame(vec![1, 2, 3], 16000, 1);
        assert_eq!(frame1.sequence_number, 0);

        let frame2 = network.create_frame(vec![4, 5, 6], 16000, 1);
        assert_eq!(frame2.sequence_number, 1);
    }

    #[test]
    fn test_timestamp_increases() {
        let (network, _tx, _rx) = AudioNetwork::new();

        let frame1 = network.create_frame(vec![1, 2, 3], 16000, 1);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let frame2 = network.create_frame(vec![4, 5, 6], 16000, 1);

        assert!(frame2.timestamp >= frame1.timestamp);
    }

    #[test]
    fn test_mute_message_creation() {
        let msg = AudioNetwork::create_mute_message(true);
        assert!(msg.has_audio_mute());
    }

    #[test]
    fn test_audio_network_error_display() {
        let err = AudioNetworkError::MicPermissionDenied;
        assert_eq!(err.to_string(), "Microphone permission denied");
    }

    #[test]
    fn test_bandwidth_calculation() {
        let mut stats = AudioNetworkStats {
            bytes_sent: 80000,  // 80KB
            bytes_received: 60000,  // 60KB
            ..Default::default()
        };

        stats.calculate(10);  // 10 seconds

        assert_eq!(stats.bandwidth_out_kbps, 64);  // 80000 * 8 / 10 / 1000
        assert_eq!(stats.bandwidth_in_kbps, 48);  // 60000 * 8 / 10 / 1000
    }

    #[test]
    fn test_bandwidth_summary() {
        let stats = AudioNetworkStats {
            bandwidth_out_kbps: 32,
            bandwidth_in_kbps: 24,
            ..Default::default()
        };

        let summary = stats.bandwidth_summary();
        assert!(summary.contains("32kbps"));
        assert!(summary.contains("24kbps"));
    }
}
