use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use hbb_common::log;

const MAX_LATENCY: i64 = 500;
const MIN_LATENCY: i64 = 100;

/// Latency controller for syncing audio with the video stream.
/// Only sync the audio to video, not the other way around.
#[derive(Debug)]
pub struct LatencyController {
    last_video_remote_ts: i64, // generated on remote deivce
    update_time: Instant,
    allow_audio: bool,
}

impl Default for LatencyController {
    fn default() -> Self {
        Self {
            last_video_remote_ts: Default::default(),
            update_time: Instant::now(),
            allow_audio: Default::default(),
        }
    }
}

impl LatencyController {
    /// Create a new latency controller.
    pub fn new() -> Arc<Mutex<LatencyController>> {
        Arc::new(Mutex::new(LatencyController::default()))
    }

    /// Update the latency controller with the latest video timestamp.
    pub fn update_video(&mut self, timestamp: i64) {
        self.last_video_remote_ts = timestamp;
        self.update_time = Instant::now();
    }

    /// Check if the audio should be played based on the current latency.
    pub fn check_audio(&mut self, timestamp: i64) -> bool {
        // Compute audio latency.
        let expected = self.update_time.elapsed().as_millis() as i64 + self.last_video_remote_ts;
        let latency = expected - timestamp;
        // Set MAX and MIN, avoid fixing too frequently.
        if self.allow_audio {
            if latency.abs() > MAX_LATENCY {
                log::debug!("LATENCY > {}ms cut off, latency:{}", MAX_LATENCY, latency);
                self.allow_audio = false;
            }
        } else {
            if latency.abs() < MIN_LATENCY {
                log::debug!("LATENCY < {}ms resume, latency:{}", MIN_LATENCY, latency);
                self.allow_audio = true;
            }
        }
        self.allow_audio
    }
}
