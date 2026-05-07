use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;
#[cfg(feature = "capture")]
use std::time::Instant;
#[cfg(feature = "capture")]
use std::time::SystemTime;

#[cfg(feature = "capture")]
use scrap::codec::{EncoderApi, EncoderCfg};
#[cfg(feature = "capture")]
use scrap::vpxcodec::{VpxEncoder, VpxEncoderConfig, VpxVideoCodecId};
#[cfg(feature = "capture")]
use scrap::{Capturer, Display, TraitCapturer, STRIDE_ALIGN};

use crate::LocalRecorderConfig;
#[cfg(feature = "capture")]
use crate::{SegmentPlanner, StorageManager, WebmSegmentWriter};

type CaptureReady = mpsc::SyncSender<Result<(), String>>;

type CaptureLoop = fn(&AtomicBool, LocalRecorderConfig, CaptureReady) -> hbb_common::ResultType<()>;

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
        Self::start_with_loop(driver, config, run_capture_loop)
    }

    fn start_with_loop(
        driver: &CaptureDriver,
        config: LocalRecorderConfig,
        capture_loop: CaptureLoop,
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
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        *slot = Some(thread::spawn(move || {
            let result = capture_loop(&running, config, ready_tx);
            if let Err(err) = result {
                hbb_common::log::warn!("local activity recording stopped: {err}");
            }
            running.store(false, Ordering::SeqCst);
        }));

        let ready_result = ready_rx.recv_timeout(Duration::from_secs(5));
        let init_error = match ready_result {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(err)) => err,
            Err(mpsc::RecvTimeoutError::Timeout) => "capture initialization timed out".to_owned(),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                "capture initialization ended without status".to_owned()
            }
        };
        driver.running.store(false, Ordering::SeqCst);
        let handle = slot.take();
        drop(slot);
        if let Some(handle) = handle {
            let _ = handle.join();
        }
        anyhow::bail!(init_error)
    }

    pub fn stop(driver: &CaptureDriver) {
        driver.running.store(false, Ordering::SeqCst);
        driver.join_worker();
    }
}

fn run_capture_loop(
    running: &AtomicBool,
    config: LocalRecorderConfig,
    ready: CaptureReady,
) -> hbb_common::ResultType<()> {
    run_capture_loop_inner(running, config, ready)
}

#[cfg(feature = "capture")]
fn run_capture_loop_inner(
    running: &AtomicBool,
    config: LocalRecorderConfig,
    ready: CaptureReady,
) -> hbb_common::ResultType<()> {
    let init_result: hbb_common::ResultType<_> = (|| {
        hbb_common::log::debug!("local recorder: opening display");
        let display = Display::primary().or_else(|e| {
            hbb_common::log::warn!("local recorder: Display::primary failed ({e}), trying all");
            Display::all()?
                .into_iter()
                .next()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        })?;
        let width = display.width() as u32;
        let height = display.height() as u32;
        hbb_common::log::debug!("local recorder: creating capturer {width}x{height}");
        let capturer = Capturer::new(display).map_err(|e| {
            hbb_common::log::warn!("local recorder: Capturer::new failed: {e}");
            e
        })?;
        hbb_common::log::debug!("local recorder: creating VP8 encoder");
        let encoder = VpxEncoder::new(
            EncoderCfg::VPX(VpxEncoderConfig {
                width,
                height,
                quality: 1.0,
                codec: VpxVideoCodecId::VP8,
                keyframe_interval: Some(240),
            }),
            false,
        )
        .map_err(|e| {
            hbb_common::log::warn!("local recorder: VpxEncoder::new failed: {e}");
            e
        })?;
        hbb_common::log::debug!("local recorder: init OK");
        Ok((capturer, encoder, width, height))
    })();

    let (mut capturer, mut encoder, width, height) = match init_result {
        Ok(v) => {
            let _ = ready.send(Ok(()));
            v
        }
        Err(err) => {
            hbb_common::log::warn!(
                "local activity recorder: capture init failed: {err}"
            );
            let _ = ready.send(Err(err.to_string()));
            return Err(err);
        }
    };

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
fn run_capture_loop_inner(
    running: &AtomicBool,
    _config: LocalRecorderConfig,
    ready: CaptureReady,
) -> hbb_common::ResultType<()> {
    let _ = ready.send(Ok(()));
    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failing_capture_loop(
        _running: &AtomicBool,
        _config: LocalRecorderConfig,
        ready: CaptureReady,
    ) -> hbb_common::ResultType<()> {
        let _ = ready.send(Err("capture initialization failed".to_owned()));
        anyhow::bail!("capture initialization failed")
    }

    fn silent_capture_loop(
        _running: &AtomicBool,
        _config: LocalRecorderConfig,
        _ready: CaptureReady,
    ) -> hbb_common::ResultType<()> {
        Ok(())
    }

    fn ready_capture_loop(
        running: &AtomicBool,
        _config: LocalRecorderConfig,
        ready: CaptureReady,
    ) -> hbb_common::ResultType<()> {
        let _ = ready.send(Ok(()));
        while running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    #[test]
    fn start_succeeds_after_capture_initialization() {
        let driver = CaptureDriver::new();

        let result = CaptureWorker::start_with_loop(
            &driver,
            LocalRecorderConfig::default(),
            ready_capture_loop,
        );

        assert!(result.is_ok());
        assert!(driver.running());
        CaptureWorker::stop(&driver);
        assert!(!driver.running());
    }

    #[test]
    fn start_reports_missing_initialization_signal() {
        let driver = CaptureDriver::new();

        let result = CaptureWorker::start_with_loop(
            &driver,
            LocalRecorderConfig::default(),
            silent_capture_loop,
        );

        assert!(result.is_err());
        assert!(!driver.running());
    }

    #[test]
    fn start_reports_capture_initialization_failure() {
        let driver = CaptureDriver::new();

        let result = CaptureWorker::start_with_loop(
            &driver,
            LocalRecorderConfig::default(),
            failing_capture_loop,
        );

        assert!(result.is_err());
        assert!(!driver.running());
    }
}
