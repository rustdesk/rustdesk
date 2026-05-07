use std::sync::Mutex;
use std::time::{Duration, Instant};

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

pub struct LocalRecorderService {
    config: LocalRecorderConfig,
    driver: CaptureDriver,
    inner: Mutex<ServiceInner>,
}

impl LocalRecorderService {
    pub fn new(config: LocalRecorderConfig) -> Self {
        Self {
            driver: CaptureDriver::new(),
            inner: Mutex::new(ServiceInner {
                state: RecorderState::Stopped,
                status_reason: None,
                activity: ActivityTracker::new(config.idle_timeout()),
                completed_segments: 0,
            }),
            config,
        }
    }

    pub fn start(&self) -> hbb_common::ResultType<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("local recorder state is unavailable"))?;
        match inner.state {
            RecorderState::Stopped | RecorderState::Paused => {
                let storage = StorageManager::new(self.config.output_dir())?;
                storage.enforce_retention_cap(self.config.retention_days())?;
                storage.enforce_storage_cap(self.config.max_storage_bytes())?;
                CaptureWorker::start(&self.driver, self.config.clone())?;
                inner.state = RecorderState::Recording;
                inner.status_reason = None;
            }
            RecorderState::Idle | RecorderState::Recording => {
                CaptureWorker::start(&self.driver, self.config.clone())?;
            }
        }
        Ok(())
    }

    pub fn stop(&self) -> hbb_common::ResultType<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("local recorder state is unavailable"))?;
        if inner.state == RecorderState::Recording {
            inner.completed_segments += 1;
        }
        inner.state = RecorderState::Stopped;
        inner.status_reason = None;
        CaptureWorker::stop(&self.driver);
        Ok(())
    }

    pub fn state(&self) -> RecorderState {
        self.inner
            .lock()
            .map(|inner| inner.state)
            .unwrap_or(RecorderState::Paused)
    }

    pub fn status_reason(&self) -> Option<StatusReason> {
        self.inner.lock().ok().and_then(|inner| inner.status_reason)
    }

    pub fn completed_segments(&self) -> usize {
        self.inner
            .lock()
            .map(|inner| inner.completed_segments)
            .unwrap_or_default()
    }

    pub fn driver_active(&self) -> bool {
        self.driver.running()
    }

    pub fn record_activity(&self, now: Instant) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.activity.record_activity(now);
        }
    }

    pub fn tick(&self, now: Instant) -> hbb_common::ResultType<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("local recorder state is unavailable"))?;
        match inner.state {
            RecorderState::Idle if inner.activity.is_active(now) => {
                inner.state = RecorderState::Recording;
            }
            RecorderState::Recording if !inner.activity.is_active(now) => {
                inner.completed_segments += 1;
                inner.state = RecorderState::Idle;
            }
            RecorderState::Stopped
            | RecorderState::Idle
            | RecorderState::Recording
            | RecorderState::Paused => {}
        }
        Ok(())
    }

    pub fn pause(&self, reason: StatusReason) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.state = RecorderState::Paused;
            inner.status_reason = Some(reason);
        }
    }

    pub fn idle_timeout(&self) -> Duration {
        self.config.idle_timeout()
    }
}
