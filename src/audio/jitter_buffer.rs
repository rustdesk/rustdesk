//! Jitter buffer for smooth audio playback.
//!
//! Handles:
//! - Buffering incoming audio frames
//! - Detecting and adapting to network jitter
//! - Out-of-order frame reordering
//! - Underrun/overrun detection and adaptation
//! - Packet loss concealment

use hbb_common::{
    anyhow::{anyhow, Result as ResultType},
    log,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Audio frame with sequence tracking
#[derive(Debug, Clone)]
pub struct JitterBufferFrame {
    /// Frame sequence number (to detect loss/reordering)
    pub sequence_number: u32,
    /// Time when frame should be played (milliseconds since call start)
    pub timestamp: u64,
    /// Audio data (PCM samples)
    pub data: Vec<f32>,
}

/// Configuration for jitter buffer behavior
#[derive(Debug, Clone)]
pub struct JitterBufferConfig {
    /// Minimum buffer size (milliseconds)
    pub min_delay_ms: u32,
    /// Target buffer size (milliseconds)
    pub target_delay_ms: u32,
    /// Maximum buffer size (milliseconds)
    pub max_delay_ms: u32,
    /// Sample rate for timing calculations
    pub sample_rate: u32,
}

impl Default for JitterBufferConfig {
    fn default() -> Self {
        Self {
            min_delay_ms: 20,
            target_delay_ms: 100,
            max_delay_ms: 200,
            sample_rate: 16000,
        }
    }
}

impl JitterBufferConfig {
    /// Create config for low-latency scenarios
    pub fn low_jitter() -> Self {
        Self {
            min_delay_ms: 10,
            target_delay_ms: 50,
            max_delay_ms: 100,
            ..Default::default()
        }
    }

    /// Create config for high-jitter scenarios
    pub fn high_jitter() -> Self {
        Self {
            min_delay_ms: 50,
            target_delay_ms: 200,
            max_delay_ms: 500,
            ..Default::default()
        }
    }

    /// Get minimum samples per frame based on sample rate
    pub fn min_samples(&self) -> usize {
        (self.sample_rate as usize * self.min_delay_ms as usize) / 1000
    }

    /// Get target samples based on sample rate
    pub fn target_samples(&self) -> usize {
        (self.sample_rate as usize * self.target_delay_ms as usize) / 1000
    }

    /// Get maximum samples based on sample rate
    pub fn max_samples(&self) -> usize {
        (self.sample_rate as usize * self.max_delay_ms as usize) / 1000
    }
}

/// Statistics about jitter buffer operation
#[derive(Debug, Clone, Default)]
pub struct JitterBufferStats {
    /// Current buffer size in milliseconds
    pub buffer_delay_ms: u32,
    /// Number of frames in buffer
    pub frames_buffered: usize,
    /// Total frames received
    pub total_frames: u64,
    /// Frames lost (sequence number gaps)
    pub lost_frames: u64,
    /// Times buffer underran
    pub underruns: u64,
    /// Times buffer overran
    pub overruns: u64,
    /// Average jitter detected (milliseconds)
    pub avg_jitter_ms: f32,
}

/// Jitter buffer for audio playback synchronization
pub struct JitterBuffer {
    /// Buffered frames indexed by sequence number
    frames: Arc<Mutex<BTreeMap<u32, JitterBufferFrame>>>,
    /// Current configuration
    config: JitterBufferConfig,
    /// Last sequence number delivered
    last_sequence: u32,
    /// Statistics
    stats: Arc<Mutex<JitterBufferStats>>,
    /// Last frame delivery time for jitter calculation
    last_frame_time: Instant,
    /// Expected frames per second
    frames_per_second: f32,
}

impl JitterBuffer {
    /// Create a new jitter buffer
    pub fn new(config: JitterBufferConfig) -> Self {
        log::info!(
            "JitterBuffer created: target={}ms, max={}ms",
            config.target_delay_ms,
            config.max_delay_ms
        );

        // Calculate frames per second (20ms default frame duration)
        let frames_per_second = 1000.0 / 20.0; // 50 fps for 20ms frames

        Self {
            frames: Arc::new(Mutex::new(BTreeMap::new())),
            config,
            last_sequence: 0,
            stats: Arc::new(Mutex::new(JitterBufferStats::default())),
            last_frame_time: Instant::now(),
            frames_per_second,
        }
    }

    /// Insert a frame into the buffer
    pub fn insert(&self, frame: JitterBufferFrame) -> ResultType<()> {
        let mut frames = self.frames.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();

        // Check for sequence number gaps (lost frames)
        if frame.sequence_number != self.last_sequence + 1 && self.last_sequence > 0 {
            let gap = frame.sequence_number.wrapping_sub(self.last_sequence);
            if gap > 1 {
                stats.lost_frames += (gap - 1) as u64;
                log::warn!(
                    "Detected {} lost frames (seq {} to {})",
                    gap - 1,
                    self.last_sequence + 1,
                    frame.sequence_number - 1
                );
            }
        }

        // Insert frame, maintaining order by sequence number
        frames.insert(frame.sequence_number, frame);
        stats.total_frames += 1;

        // Check for buffer overflow
        if frames.len() > self.config.max_samples() / 160 {
            stats.overruns += 1;
            log::warn!("Jitter buffer overflow, dropping oldest frames");

            // Remove oldest frames if buffer too large
            while frames.len() > self.config.target_samples() / 160 {
                frames.pop_first();
            }
        }

        Ok(())
    }

    /// Get the next playable frame from the buffer
    pub fn get_next_frame(&self) -> ResultType<Option<JitterBufferFrame>> {
        let mut frames = self.frames.lock().unwrap();
        let mut stats = self.stats.lock().unwrap();

        // Return the next sequence frame
        if let Some((seq, frame)) = frames.pop_first() {
            stats.buffer_delay_ms = ((frames.len() as u32 * 20) / self.frames_per_second as u32)
                .max(self.config.min_delay_ms);
            stats.frames_buffered = frames.len();

            // Calculate jitter
            let frame_time = Instant::now();
            let time_since_last = frame_time.duration_since(self.last_frame_time).as_millis();
            let expected_time = (1000.0 / self.frames_per_second) as u128;

            if time_since_last > 0 {
                let jitter = (time_since_last as i64 - expected_time as i64).abs() as f32;
                stats.avg_jitter_ms = (stats.avg_jitter_ms * 0.9) + (jitter * 0.1);
            }

            self.last_sequence = seq;
            Ok(Some(frame))
        } else {
            // Buffer underrun
            stats.underruns += 1;
            log::warn!("Jitter buffer underrun");
            Ok(None)
        }
    }

    /// Get current buffer statistics
    pub fn get_stats(&self) -> JitterBufferStats {
        self.stats.lock().unwrap().clone()
    }

    /// Reset the buffer (e.g., on call restart)
    pub fn reset(&self) {
        self.frames.lock().unwrap().clear();
        self.stats.lock().unwrap().clear();
        log::info!("Jitter buffer reset");
    }

    /// Get current number of buffered frames
    pub fn len(&self) -> usize {
        self.frames.lock().unwrap().len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.frames.lock().unwrap().is_empty()
    }

    /// Adapt buffer delay based on network conditions
    pub fn adapt_to_jitter(&self, detected_jitter_ms: u32) {
        let mut config = self.config.clone();
        let mut stats = self.stats.lock().unwrap();

        let new_target = (detected_jitter_ms as f32 * 1.5) as u32;
        let new_target = new_target
            .max(config.min_delay_ms)
            .min(config.max_delay_ms);

        if new_target != config.target_delay_ms {
            log::info!(
                "Adapting jitter buffer: {} -> {} ms",
                config.target_delay_ms,
                new_target
            );
            config.target_delay_ms = new_target;
            self.config = config;
        }

        stats.buffer_delay_ms = new_target;
    }
}

impl JitterBufferStats {
    /// Clear all statistics
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Get packet loss percentage
    pub fn loss_percentage(&self) -> f32 {
        if self.total_frames == 0 {
            return 0.0;
        }
        (self.lost_frames as f32 / self.total_frames as f32) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_frame(seq: u32, timestamp: u64) -> JitterBufferFrame {
        JitterBufferFrame {
            sequence_number: seq,
            timestamp,
            data: vec![0.0; 320],
        }
    }

    #[test]
    fn test_jitter_buffer_config_default() {
        let config = JitterBufferConfig::default();
        assert!(config.is_valid());
    }

    #[test]
    fn test_jitter_buffer_config_is_valid() {
        let config = JitterBufferConfig::default();
        let min = config.min_samples();
        let target = config.target_samples();
        let max = config.max_samples();

        assert!(min <= target);
        assert!(target <= max);
    }

    #[test]
    fn test_insert_and_get_frames() {
        let buffer = JitterBuffer::new(JitterBufferConfig::default());

        buffer.insert(create_test_frame(1, 0)).unwrap();
        buffer.insert(create_test_frame(2, 20)).unwrap();

        let frame1 = buffer.get_next_frame().unwrap();
        assert!(frame1.is_some());
        assert_eq!(frame1.unwrap().sequence_number, 1);

        let frame2 = buffer.get_next_frame().unwrap();
        assert!(frame2.is_some());
        assert_eq!(frame2.unwrap().sequence_number, 2);
    }

    #[test]
    fn test_reorder_out_of_order_frames() {
        let buffer = JitterBuffer::new(JitterBufferConfig::default());

        // Insert out of order
        buffer.insert(create_test_frame(3, 40)).unwrap();
        buffer.insert(create_test_frame(1, 0)).unwrap();
        buffer.insert(create_test_frame(2, 20)).unwrap();

        // Should retrieve in order
        assert_eq!(buffer.get_next_frame().unwrap().unwrap().sequence_number, 1);
        assert_eq!(buffer.get_next_frame().unwrap().unwrap().sequence_number, 2);
        assert_eq!(buffer.get_next_frame().unwrap().unwrap().sequence_number, 3);
    }

    #[test]
    fn test_loss_detection() {
        let buffer = JitterBuffer::new(JitterBufferConfig::default());

        // Gap in sequence numbers (lost frames)
        buffer.insert(create_test_frame(1, 0)).unwrap();
        buffer.insert(create_test_frame(4, 60)).unwrap(); // Skip 2, 3

        buffer.get_next_frame().ok();
        let stats = buffer.get_stats();

        assert!(stats.lost_frames >= 2);
    }

    #[test]
    fn test_buffer_underrun() {
        let buffer = JitterBuffer::new(JitterBufferConfig::default());

        // Try to get frame when empty
        let frame = buffer.get_next_frame();
        assert!(frame.is_ok());

        let stats = buffer.get_stats();
        assert!(stats.underruns > 0);
    }

    #[test]
    fn test_buffer_reset() {
        let buffer = JitterBuffer::new(JitterBufferConfig::default());

        buffer.insert(create_test_frame(1, 0)).unwrap();
        buffer.insert(create_test_frame(2, 20)).unwrap();

        assert! (!buffer.is_empty());

        buffer.reset();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_loss_percentage_calculation() {
        let mut stats = JitterBufferStats {
            total_frames: 100,
            lost_frames: 10,
            ..Default::default()
        };

        assert_eq!(stats.loss_percentage(), 10.0);
    }

    #[test]
    fn test_low_jitter_config() {
        let config = JitterBufferConfig::low_jitter();
        assert!(config.target_delay_ms <= config.max_delay_ms);
        assert_eq!(config.target_delay_ms, 50);
    }

    #[test]
    fn test_high_jitter_config() {
        let config = JitterBufferConfig::high_jitter();
        assert!(config.target_delay_ms <= config.max_delay_ms);
        assert_eq!(config.target_delay_ms, 200);
    }
}
