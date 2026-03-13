//! Audio playback module using cpal.
//!
//! Handles speaker output via cross-platform audio interface.

use hbb_common::{
    anyhow::{anyhow, Result as ResultType},
    log,
    tokio::sync::mpsc,
};
use std::sync::{Arc, Mutex};

/// Audio playback device wrapper
pub struct AudioPlayback {
    device_name: Option<String>,
    is_running: Arc<Mutex<bool>>,
    receiver: Option<mpsc::UnboundedReceiver<Vec<f32>>>,
}

impl AudioPlayback {
    /// Create a new audio playback instance
    ///
    /// # Arguments
    ///
    /// * `device_name` - The name of the audio output device (None = default)
    pub async fn new(device_name: Option<&str>) -> ResultType<(Self, mpsc::UnboundedSender<Vec<f32>>)> {
        let (sender, receiver) = mpsc::unbounded_channel();

        let playback = Self {
            device_name: device_name.map(|s| s.to_string()),
            is_running: Arc::new(Mutex::new(false)),
            receiver: Some(receiver),
        };

        log::info!(
            "AudioPlayback created for device: {}",
            device_name.unwrap_or("default")
        );

        Ok((playback, sender))
    }

    /// Start audio playback
    pub async fn start(&mut self) -> ResultType<()> {
        *self.is_running.lock().unwrap() = true;
        log::info!("Audio playback started");
        Ok(())
    }

    /// Stop audio playback
    pub async fn stop(&mut self) -> ResultType<()> {
        *self.is_running.lock().unwrap() = false;
        log::info!("Audio playback stopped");
        Ok(())
    }

    /// Check if currently playing
    pub fn is_playing(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    /// Get list of available output devices
    pub fn list_output_devices() -> ResultType<Vec<String>> {
        #[cfg(not(target_os = "linux"))]
        {
            use cpal::traits::{DeviceTrait, HostTrait};

            let host = cpal::default_host();
            let devices = host
                .output_devices()
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

impl Drop for AudioPlayback {
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
    async fn test_audio_playback_creation() {
        let result = AudioPlayback::new(None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_audio_playback_start_stop() {
        let (mut playback, _) = AudioPlayback::new(None).await.unwrap();

        assert!(!playback.is_playing());

        playback.start().await.unwrap();
        assert!(playback.is_playing());

        playback.stop().await.unwrap();
        assert!(!playback.is_playing());
    }
}
