//! Audio handling module.
//!
//! This module abstracts over the underlying audio backend (CPAL) and provides
//! a simple API for initializing an output stream that works across platforms.
//! On Windows we explicitly prefer the ASIO host when it is available because
//! some devices (e.g., Astro MixAmp) expose their audio only through the ASIO
//! driver. Falling back to the default host ensures compatibility on all other
//! platforms.

use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Represents a running audio output stream.
pub struct AudioOutput {
    /// The underlying CPAL stream.
    stream: cpal::Stream,
}

impl AudioOutput {
    /// Starts playback of the supplied audio callback.
    ///
    /// The callback receives a mutable slice of interleaved f32 samples.
    /// It is called by CPAL on the appropriate thread.
    pub fn new<F>(mut callback: F) -> Result<Self, anyhow::Error>
    where
        F: FnMut(&mut [f32]) + Send + 'static,
    {
        // Choose the most appropriate host for the current platform.
        let host = select_preferred_host();

        // Pick the default output device for the chosen host.
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No default output device found"))?;

        // Build a stream with the device's preferred configuration.
        let config = device
            .default_output_config()
            .map_err(|e| anyhow::anyhow!("Failed to get default output config: {}", e))?
            .config();

        // CPAL expects a callback that works with the sample format.
        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let stream = match config.sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    callback(data);
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config,
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    // Convert i16 samples to f32, call the user callback, then back.
                    let mut temp = vec![0f32; data.len()];
                    callback(&mut temp);
                    for (dst, src) in data.iter_mut().zip(temp.iter()) {
                        *dst = (*src * i16::MAX as f32) as i16;
                    }
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::U16 => device.build_output_stream(
                &config,
                move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                    let mut temp = vec![0f32; data.len()];
                    callback(&mut temp);
                    for (dst, src) in data.iter_mut().zip(temp.iter()) {
                        *dst = ((*src + 1.0) * 0.5 * u16::MAX as f32) as u16;
                    }
                },
                err_fn,
                None,
            )?,
        };

        stream.play()?;

        Ok(Self { stream })
    }
}

/// Selects the most suitable CPAL host for the current platform.
///
/// On Windows we prefer the ASIO host if it is present because it provides
/// low‑latency, exclusive‑mode audio that many gaming headsets expose. If ASIO
/// is not available we fall back to the default host (usually WASAPI).
///
/// On non‑Windows platforms we simply return the default host.
fn select_preferred_host() -> cpal::Host {
    #[cfg(target_os = "windows")]
    {
        // Enumerate all available hosts and look for one whose name contains "ASIO".
        // CPAL's `HostId::Asio` is only defined on Windows.
        use cpal::HostId;
        let asio_host_id = HostId::Asio;
        if cpal::available_hosts().contains(&asio_host_id) {
            // SAFETY: ASIO is only available on Windows; the feature flag is enabled
            // in Cargo.toml, so this call is safe.
            return cpal::host_from_id(asio_host_id).expect("ASIO host should be available");
        }
        // If ASIO is not present, fall back to the default host (WASAPI on Windows).
        cpal::default_host()
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On other OSes the default host is the best choice.
        cpal::default_host()
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Starts audio playback using the provided sample generator.
///
/// The `generator` closure is called repeatedly with a mutable slice that should
/// be filled with interleaved f32 samples. The length of the slice corresponds
/// to the buffer size requested by the underlying audio backend.
///
/// Returns a handle that keeps the stream alive for as long as it is held.
pub fn start_playback<F>(generator: F) -> Result<Arc<AudioOutput>, anyhow::Error>
where
    F: FnMut(&mut [f32]) + Send + 'static,
{
    let output = AudioOutput::new(generator)?;
    Ok(Arc::new(output))
}
