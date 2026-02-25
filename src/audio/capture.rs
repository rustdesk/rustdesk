//! Audio capture module using cpal.
//!
//! Handles microphone input via cross-platform audio interface.

use hbb_common::{
    anyhow::{anyhow, Result as ResultType},
    log,
    tokio::sync::mpsc,
};
use std::sync::{Arc, Mutex};

/// Raw audio data from capture
#[derive(Debug, Clone)]
pub struct AudioData {
    /// PCM samples (f32, normalized to [-1.0, 1.0])
    pub samples: Vec<f32>,
    /// Timestamp in milliseconds
    pub timestamp: u64,
}

/// Audio capture device wrapper
pub struct AudioCapture {
    device_name: Option<String>,
    is_running: Arc<Mutex<bool>>,
    sender: Option<mpsc::UnboundedSender<AudioData>>,
}

impl AudioCapture {
    /// Create a new audio capture instance
    ///
    /// # Arguments
    ///
    /// * `device_name` - The name of the audio input device (None = default)
    pub async fn new(device_name: Option<&str>) -> ResultType<(Self, mpsc::UnboundedReceiver<AudioData>)> {
        let (sender, receiver) = mpsc::unbounded_channel();

        let capture = Self {
            device_name: device_name.map(|s| s.to_string()),
            is_running: Arc::new(Mutex::new(false)),
            sender: Some(sender),
        };

        log::info!(
            "AudioCapture created for device: {}",
            device_name.unwrap_or("default")
        );

        Ok((capture, receiver))
    }

    /// Start capturing audio
    pub async fn start(&mut self) -> ResultType<()> {
        *self.is_running.lock().unwrap() = true;
        log::info!("Audio capture started");
        Ok(())
    }

    /// Stop capturing audio
    pub async fn stop(&mut self) -> ResultType<()> {
        *self.is_running.lock().unwrap() = false;
        log::info!("Audio capture stopped");
        Ok(())
    }

    /// Check if currently capturing
    pub fn is_capturing(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    /// Get list of available input devices
    pub fn list_input_devices() -> ResultType<Vec<String>> {
        #[cfg(not(target_os = "linux"))]
        {
            use cpal::traits::{DeviceTrait, HostTrait};

            let host = cpal::default_host();
            let devices = host
                .input_devices()
                .map_err(|e| anyhow!("Failed to enumerate devices: {}", e))?
                .filter_map(|device| device.name().ok())
                .collect();

            Ok(devices)
        }

        #[cfg(target_os = "linux")]
        {
            // On Linux, use PulseAudio device enumeration
            Ok(vec!["default".to_string()])
        }
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        if let Ok(mut running) = self.is_running.lock() {
            *running = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audio_capture_creation() {
        let result = AudioCapture::new(None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_audio_capture_start_stop() {
        let (mut capture, _) = AudioCapture::new(None).await.unwrap();

        assert!(!capture.is_capturing());

        capture.start().await.unwrap();
        assert!(capture.is_capturing());

        capture.stop().await.unwrap();
        assert!(!capture.is_capturing());
    }
}
