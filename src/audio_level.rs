//! Input level meters: the peak level of an input device, for the level indicators of the Audio dialog.
//!
//! `level(device)` starts a monitor on the first call for `device` and returns the highest absolute sample value
//! (0..=1) captured since the previous call. A monitor is a thread holding a CPAL input stream on the device; it ends
//! `IDLE` after the last call. CPAL's process-global device enumerator is created on `crate::audio_device_thread`
//! before the first monitor starts, so a monitor thread's exit leaves it valid.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hbb_common::log;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

const IDLE: Duration = Duration::from_secs(3);

struct Meter {
    /// Peak since the last read, as `f32` bits (non-negative floats order like their bits).
    peak: Arc<AtomicU32>,
    last_read: Arc<Mutex<Instant>>,
}

lazy_static::lazy_static! {
    static ref METERS: Mutex<HashMap<String, Meter>> = Default::default();
}

static ENUMERATOR_READY: AtomicBool = AtomicBool::new(false);

/// Peak level (0..=1) of input device `device` since the previous call; "" is the default input device.
pub fn level(device: &str) -> f32 {
    if !ENUMERATOR_READY.swap(true, Ordering::SeqCst) {
        crate::audio_device_thread::run(|| cpal::default_host().default_input_device().is_some());
    }
    let mut meters = METERS.lock().unwrap();
    if let Some(m) = meters.get(device) {
        *m.last_read.lock().unwrap() = Instant::now();
        return f32::from_bits(m.peak.swap(0, Ordering::Relaxed));
    }
    let meter = Meter {
        peak: Arc::new(AtomicU32::new(0)),
        last_read: Arc::new(Mutex::new(Instant::now())),
    };
    let (peak, last_read, name) = (meter.peak.clone(), meter.last_read.clone(), device.to_owned());
    meters.insert(device.to_owned(), meter);
    std::thread::spawn(move || {
        if let Err(e) = monitor(&name, peak, last_read) {
            log::warn!("Level meter of input device \"{name}\": {e}");
        }
        METERS.lock().unwrap().remove(&name);
    });
    0.0
}

fn monitor(name: &str, peak: Arc<AtomicU32>, last_read: Arc<Mutex<Instant>>) -> hbb_common::ResultType<()> {
    use hbb_common::anyhow::anyhow;
    let host = cpal::default_host();
    let device = if name.is_empty() {
        host.default_input_device()
    } else {
        host.input_devices()?.find(|d| d.name().ok().as_deref() == Some(name))
    }
    .ok_or_else(|| anyhow!("not present"))?;
    let config = device.default_input_config()?;
    let err = |e: cpal::StreamError| log::debug!("Level meter stream error: {e}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let peak = peak.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| store(&peak, data.iter().map(|s| s.abs())),
                err,
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let peak = peak.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| store(&peak, data.iter().map(|s| (*s as f32 / i16::MAX as f32).abs())),
                err,
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let peak = peak.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[u16], _| {
                    store(&peak, data.iter().map(|s| ((*s as f32 - 32768.0) / 32768.0).abs()))
                },
                err,
                None,
            )?
        }
        f => return Err(anyhow!("unsupported sample format {f:?}")),
    };
    stream.play()?;
    while last_read.lock().unwrap().elapsed() < IDLE {
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

fn store(peak: &AtomicU32, samples: impl Iterator<Item = f32>) {
    let max = samples.fold(0f32, f32::max).min(1.0);
    peak.fetch_max(max.to_bits(), Ordering::Relaxed);
}
