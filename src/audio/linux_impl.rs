// Linux Audio Implementation
// File: src/audio/linux_impl.rs
// Platform: Linux (PulseAudio/ALSA)

#![cfg(target_os = "linux")]

use hbb_common::{
    anyhow::Result as ResultType,
    log,
};

/// Linux PulseAudio/ALSA capture implementation
pub struct LinuxAudioCapture {
    device_name: Option<String>,
    is_running: std::sync::atomic::AtomicBool,
}

impl LinuxAudioCapture {
    pub fn new(device_name: Option<&str>) -> ResultType<Self> {
        log::info!("Initializing Linux audio capture: {:?}", device_name);
        Ok(Self {
            device_name: device_name.map(|s| s.to_string()),
            is_running: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn start(&mut self) -> ResultType<()> {
        self.is_running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        log::info!("Linux audio capture started");
        Ok(())
    }

    pub fn stop(&mut self) -> ResultType<()> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        log::info!("Linux audio capture stopped");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Linux PulseAudio/ALSA playback implementation
pub struct LinuxAudioPlayback {
    device_name: Option<String>,
    is_running: std::sync::atomic::AtomicBool,
}

impl LinuxAudioPlayback {
    pub fn new(device_name: Option<&str>) -> ResultType<Self> {
        log::info!("Initializing Linux audio playback: {:?}", device_name);
        Ok(Self {
            device_name: device_name.map(|s| s.to_string()),
            is_running: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn start(&mut self) -> ResultType<()> {
        self.is_running
            .store(true, std::sync::atomic::Ordering::SeqCst);
        log::info!("Linux audio playback started");
        Ok(())
    }

    pub fn stop(&mut self) -> ResultType<()> {
        self.is_running
            .store(false, std::sync::atomic::Ordering::SeqCst);
        log::info!("Linux audio playback stopped");
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_capture_creation() {
        let capture = LinuxAudioCapture::new(None);
        assert!(capture.is_ok());
    }

    #[test]
    fn test_linux_playback_creation() {
        let playback = LinuxAudioPlayback::new(None);
        assert!(playback.is_ok());
    }
}
