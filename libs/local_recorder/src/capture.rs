use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::time::SystemTime;
#[cfg(feature = "capture")]
use std::time::Instant;

#[cfg(feature = "capture")]
use scrap::codec::{EncoderApi, EncoderCfg};
#[cfg(feature = "capture")]
use scrap::vpxcodec::{VpxEncoder, VpxEncoderConfig, VpxVideoCodecId};
#[cfg(feature = "capture")]
use scrap::{Capturer, Display, TraitCapturer, STRIDE_ALIGN};

use crate::LocalRecorderConfig;
#[cfg(feature = "capture")]
use crate::{SegmentPlanner, StorageManager, WebmSegmentWriter};

pub struct CaptureDriver {
    running: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl CaptureDriver {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: Mutex::new(None),
        }
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn join_worker(&self) {
        if let Ok(mut handle) = self.handle.lock() {
            if let Some(handle) = handle.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Default for CaptureDriver {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CaptureWorker;

impl CaptureWorker {
    pub fn start(
        driver: &CaptureDriver,
        config: LocalRecorderConfig,
    ) -> hbb_common::ResultType<()> {
        let mut slot = driver
            .handle
            .lock()
            .map_err(|_| anyhow::anyhow!("local recorder worker state is unavailable"))?;
        if driver.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        if let Some(handle) = slot.take() {
            let _ = handle.join();
        }
        driver.running.store(true, Ordering::SeqCst);

        let running = driver.running.clone();
        *slot = Some(thread::spawn(move || {
            if let Err(err) = run_capture_loop(&running, config) {
                hbb_common::log::warn!("local activity recording stopped: {err}");
            }
            running.store(false, Ordering::SeqCst);
        }));
        Ok(())
    }

    pub fn stop(driver: &CaptureDriver) {
        driver.running.store(false, Ordering::SeqCst);
        driver.join_worker();
    }
}

#[cfg(feature = "capture")]
fn run_capture_loop(
    running: &AtomicBool,
    config: LocalRecorderConfig,
) -> hbb_common::ResultType<()> {
    let display = Display::primary().or_else(|_| {
        Display::all()?
            .into_iter()
            .next()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
    })?;
    let width = display.width() as u32;
    let height = display.height() as u32;
    let mut capturer = Capturer::new(display)?;
    let mut encoder = VpxEncoder::new(
        EncoderCfg::VPX(VpxEncoderConfig {
            width,
            height,
            quality: 1.0,
            codec: VpxVideoCodecId::VP8,
            keyframe_interval: Some(240),
        }),
        false,
    )?;
    let yuvfmt = encoder.yuvfmt();
    let storage = StorageManager::new(config.output_dir())?;
    let mut planner = SegmentPlanner::new(config.segment_duration());
    let mut writer: Option<WebmSegmentWriter> = None;
    let mut segment_start: Option<SystemTime> = None;
    let start = Instant::now();
    let spf = Duration::from_millis(200);
    let mut yuv = Vec::new();
    let mut mid_data = Vec::new();

    while running.load(Ordering::SeqCst) {
        let now = Instant::now();
        if let Ok(frame) = capturer.frame(Duration::from_millis(0)) {
            match planner.update(now, width, height) {
                crate::SegmentEvent::Started => {
                    segment_start = Some(SystemTime::now());
                    writer = Some(WebmSegmentWriter::new(
                        storage.next_segment_paths()?,
                        width,
                        height,
                    )?);
                }
                crate::SegmentEvent::Rotate => {
                    finalize_segment(&storage, segment_start, writer.take());
                    segment_start = Some(SystemTime::now());
                    writer = Some(WebmSegmentWriter::new(
                        storage.next_segment_paths()?,
                        width,
                        height,
                    )?);
                }
                crate::SegmentEvent::Continue => {}
            }

            frame.to(yuvfmt.clone(), &mut yuv, &mut mid_data)?;
            let timestamp_ms = now.duration_since(start).as_millis() as i64;
            for encoded in encoder.encode(timestamp_ms, &yuv, STRIDE_ALIGN)? {
                if let Some(segment) = writer.as_mut() {
                    segment.write_encoded_frame(encoded.data, encoded.pts as u64, encoded.key)?;
                }
            }
        }

        let elapsed = now.elapsed();
        if elapsed < spf {
            thread::sleep(spf - elapsed);
        }
    }

    finalize_segment(&storage, segment_start, writer.take());
    Ok(())
}

#[cfg(feature = "capture")]
fn finalize_segment(
    storage: &StorageManager,
    start: Option<SystemTime>,
    segment: Option<WebmSegmentWriter>,
) {
    if let (Some(start), Some(segment)) = (start, segment) {
        let end = SystemTime::now();
        let result = storage
            .segment_paths_for_period(start, end)
            .and_then(|paths| segment.finalize_to(paths.complete));
        if let Err(err) = result {
            hbb_common::log::warn!("failed to finalize local recording segment: {err}");
        }
    }
}

#[cfg(not(feature = "capture"))]
fn run_capture_loop(
    running: &AtomicBool,
    _config: LocalRecorderConfig,
) -> hbb_common::ResultType<()> {
    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}
