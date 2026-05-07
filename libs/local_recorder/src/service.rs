use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::activity::platform_idle_duration;
use crate::{ActivityTracker, CaptureDriver, CaptureWorker, LocalRecorderConfig, StorageManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderState {
    Stopped,
    Idle,
    Recording,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusReason {
    PermissionDenied,
    UnsupportedPlatform,
    StorageError,
    CaptureError,
    EncoderError,
    ConcurrentRemoteCapture,
}

#[derive(Debug)]
struct ServiceInner {
    state: RecorderState,
    status_reason: Option<StatusReason>,
    activity: ActivityTracker,
    completed_segments: usize,
}

// ---------------------------------------------------------------------------
// Shared state kept alive by both the service handle and the tick thread.
// ---------------------------------------------------------------------------
struct Shared {
    config: LocalRecorderConfig,
    driver: CaptureDriver,
    inner: Mutex<ServiceInner>,
    tick_running: AtomicBool,
}

impl Shared {
    fn record_activity(&self, now: Instant) {
        if let Ok(mut g) = self.inner.lock() {
            g.activity.record_activity(now);
        }
    }

    fn tick(&self, now: Instant) -> hbb_common::ResultType<()> {
        self.tick_with_worker(now, CaptureWorker::start)
    }

    fn tick_with_worker(
        &self,
        now: Instant,
        start_worker: impl FnOnce(&CaptureDriver, LocalRecorderConfig) -> hbb_common::ResultType<()>,
    ) -> hbb_common::ResultType<()> {
        let should_start = {
            let g = self
                .inner
                .lock()
                .map_err(|_| anyhow::anyhow!("local recorder state is unavailable"))?;
            g.state == RecorderState::Idle && g.activity.is_active(now)
        };

        if should_start {
            if let Err(err) = start_worker(&self.driver, self.config.clone()) {
                let mut g = self
                    .inner
                    .lock()
                    .map_err(|_| anyhow::anyhow!("local recorder state is unavailable"))?;
                if g.state == RecorderState::Idle {
                    g.state = RecorderState::Paused;
                    g.status_reason = Some(StatusReason::CaptureError);
                }
                return Err(err);
            }

            let should_stop = {
                let mut g = self
                    .inner
                    .lock()
                    .map_err(|_| anyhow::anyhow!("local recorder state is unavailable"))?;
                match g.state {
                    RecorderState::Idle => {
                        g.state = RecorderState::Recording;
                        g.status_reason = None;
                        false
                    }
                    RecorderState::Recording => false,
                    RecorderState::Stopped | RecorderState::Paused => true,
                }
            };
            if should_stop {
                CaptureWorker::stop(&self.driver);
            }
            return Ok(());
        }

        let should_stop = {
            let mut g = self
                .inner
                .lock()
                .map_err(|_| anyhow::anyhow!("local recorder state is unavailable"))?;
            if g.state == RecorderState::Recording && !g.activity.is_active(now) {
                g.completed_segments += 1;
                g.state = RecorderState::Idle;
                true
            } else {
                false
            }
        };

        if should_stop {
            CaptureWorker::stop(&self.driver);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Public service handle
// ---------------------------------------------------------------------------

/// Poll every 2 s – cheap enough, responsive enough.
const TICK_INTERVAL: Duration = Duration::from_secs(2);

pub struct LocalRecorderService {
    shared: Arc<Shared>,
}

impl LocalRecorderService {
    pub fn new(config: LocalRecorderConfig) -> Self {
        Self {
            shared: Arc::new(Shared {
                driver: CaptureDriver::new(),
                inner: Mutex::new(ServiceInner {
                    state: RecorderState::Stopped,
                    status_reason: None,
                    activity: ActivityTracker::new(config.idle_timeout()),
                    completed_segments: 0,
                }),
                tick_running: AtomicBool::new(false),
                config,
            }),
        }
    }

    pub fn start(&self) -> hbb_common::ResultType<()> {
        self.start_with_worker(CaptureWorker::start)
    }

    fn start_with_worker(
        &self,
        _start_worker: impl FnOnce(&CaptureDriver, LocalRecorderConfig) -> hbb_common::ResultType<()>,
    ) -> hbb_common::ResultType<()> {
        {
            let mut g = self
                .shared
                .inner
                .lock()
                .map_err(|_| anyhow::anyhow!("local recorder state is unavailable"))?;
            match g.state {
                RecorderState::Stopped | RecorderState::Paused => {
                    let storage = StorageManager::new(self.shared.config.output_dir())?;
                    storage.enforce_retention_cap(self.shared.config.retention_days())?;
                    storage.enforce_storage_cap(self.shared.config.max_storage_bytes())?;
                    g.state = RecorderState::Idle;
                    g.status_reason = None;
                }
                RecorderState::Idle | RecorderState::Recording => {}
            }
        }
        self.ensure_tick_thread();
        Ok(())
    }

    pub fn stop(&self) -> hbb_common::ResultType<()> {
        self.shared.tick_running.store(false, Ordering::SeqCst);
        {
            let mut g = self
                .shared
                .inner
                .lock()
                .map_err(|_| anyhow::anyhow!("local recorder state is unavailable"))?;
            if g.state == RecorderState::Recording {
                g.completed_segments += 1;
            }
            g.state = RecorderState::Stopped;
            g.status_reason = None;
        }
        CaptureWorker::stop(&self.shared.driver);
        Ok(())
    }

    pub fn state(&self) -> RecorderState {
        self.shared
            .inner
            .lock()
            .map(|g| g.state)
            .unwrap_or(RecorderState::Paused)
    }

    pub fn status_reason(&self) -> Option<StatusReason> {
        self.shared
            .inner
            .lock()
            .ok()
            .and_then(|g| g.status_reason)
    }

    pub fn completed_segments(&self) -> usize {
        self.shared
            .inner
            .lock()
            .map(|g| g.completed_segments)
            .unwrap_or_default()
    }

    pub fn driver_active(&self) -> bool {
        self.shared.driver.running()
    }

    pub fn record_activity(&self, now: Instant) {
        self.shared.record_activity(now);
    }

    pub fn tick(&self, now: Instant) -> hbb_common::ResultType<()> {
        self.shared.tick(now)
    }

    pub fn pause(&self, reason: StatusReason) {
        if let Ok(mut g) = self.shared.inner.lock() {
            g.state = RecorderState::Paused;
            g.status_reason = Some(reason);
        }
    }

    pub fn idle_timeout(&self) -> Duration {
        self.shared.config.idle_timeout()
    }

    // -----------------------------------------------------------------------
    // Background tick thread
    // -----------------------------------------------------------------------

    fn ensure_tick_thread(&self) {
        if self
            .shared
            .tick_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return; // already running
        }

        let shared = self.shared.clone(); // Arc clone – no raw pointers needed

        thread::Builder::new()
            .name("local-recorder-tick".into())
            .spawn(move || {
                hbb_common::log::debug!("local recorder tick thread started");
                while shared.tick_running.load(Ordering::SeqCst) {
                    let now = Instant::now();

                    // Feed platform idle info into the activity tracker.
                    if let Some(idle) = platform_idle_duration() {
                        if idle <= shared.config.idle_timeout() {
                            // Approximate when the user was last active.
                            let last_active = now.checked_sub(idle).unwrap_or(now);
                            shared.record_activity(last_active);
                        }
                    }

                    // Advance the state machine.
                    if let Err(err) = shared.tick(now) {
                        hbb_common::log::warn!("local recorder tick error: {err}");
                    }

                    thread::sleep(TICK_INTERVAL);
                }
                hbb_common::log::debug!("local recorder tick thread stopped");
            })
            .ok(); // spawn failure is non-fatal; recording just won't start automatically
    }
}

impl Drop for LocalRecorderService {
    fn drop(&mut self) {
        // Signal the tick thread to stop.  The thread holds an Arc<Shared> so
        // it will keep running until it observes the flag and exits naturally.
        // We do not join here to avoid blocking; the thread will exit within
        // TICK_INTERVAL after the flag is cleared.
        self.shared.tick_running.store(false, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------
// Tests – use tick_with_worker shim so tests don't need a real display
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_config(prefix: &str) -> LocalRecorderConfig {
        LocalRecorderConfig::builder()
            .output_dir(unique_temp_path(prefix))
            .build()
            .unwrap()
    }

    fn unique_temp_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rustdesk-local-recorder-{prefix}-{nanos}"))
    }

    // Helper: call tick_with_worker via Shared directly so tests can inject
    // a fake start_worker without going through the public API.
    fn tick_via_shared(
        service: &LocalRecorderService,
        now: Instant,
        start_worker: impl FnOnce(&CaptureDriver, LocalRecorderConfig) -> hbb_common::ResultType<()>,
    ) -> hbb_common::ResultType<()> {
        service.shared.tick_with_worker(now, start_worker)
    }

    #[test]
    fn recording_start_failure_pauses_with_capture_error() {
        let start = Instant::now();
        let service = LocalRecorderService::new(test_config("recording-start-failure"));
        service.start().unwrap();
        service.record_activity(start);
        tick_via_shared(&service, start + Duration::from_secs(1), |_, _| Ok(())).unwrap();
        service.stop().unwrap();
        service.start().unwrap();
        service.record_activity(start + Duration::from_secs(2));

        let result = tick_via_shared(
            &service,
            start + Duration::from_secs(3),
            |_, _| anyhow::bail!("capture initialization failed"),
        );

        assert!(result.is_err());
        assert_eq!(RecorderState::Paused, service.state());
        assert_eq!(Some(StatusReason::CaptureError), service.status_reason());
    }

    #[test]
    fn concurrent_activity_start_does_not_stop_active_driver() {
        let start = Instant::now();
        let service = LocalRecorderService::new(test_config("concurrent-activity-start"));
        service.start().unwrap();
        service.record_activity(start);

        tick_via_shared(
            &service,
            start + Duration::from_secs(1),
            |driver, config| {
                CaptureWorker::start(driver, config)?;
                let mut g = service.shared.inner.lock().unwrap();
                g.state = RecorderState::Recording;
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(RecorderState::Recording, service.state());
        assert!(service.driver_active());
        service.stop().unwrap();
    }

    #[test]
    fn capture_start_failure_pauses_with_capture_error() {
        let start = Instant::now();
        let service = LocalRecorderService::new(test_config("capture-start-failure"));
        service.start().unwrap();
        service.record_activity(start);

        let result = tick_via_shared(
            &service,
            start + Duration::from_secs(1),
            |_, _| anyhow::bail!("capture initialization failed"),
        );

        assert!(result.is_err());
        assert_eq!(RecorderState::Paused, service.state());
        assert_eq!(Some(StatusReason::CaptureError), service.status_reason());
    }
}
