pub(crate) mod activity;
mod capture;
mod service;
mod storage;
mod writer;

use std::path::{Path, PathBuf};
use std::time::Duration;

pub use activity::{platform_idle_duration, ActivityTracker, IdleSource};
pub use capture::{CaptureDriver, CaptureWorker};
pub use service::{LocalRecorderService, RecorderState, StatusReason};
pub use storage::{SegmentPaths, StorageManager};
pub use writer::{SegmentEvent, SegmentPlanner, WebmSegmentWriter};

const DEFAULT_SEGMENT_DURATION: Duration = Duration::from_secs(60);
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_RETENTION_DAYS: u64 = 7;
const DEFAULT_MAX_STORAGE_BYTES: u64 = 5u64 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct LocalRecorderConfig {
    segment_duration: Duration,
    idle_timeout: Duration,
    retention_days: u64,
    max_storage_bytes: u64,
    output_dir: PathBuf,
}

impl LocalRecorderConfig {
    pub fn builder() -> LocalRecorderConfigBuilder {
        LocalRecorderConfigBuilder::default()
    }

    pub fn segment_duration(&self) -> Duration {
        self.segment_duration
    }

    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    pub fn retention_days(&self) -> u64 {
        self.retention_days
    }

    pub fn max_storage_bytes(&self) -> u64 {
        self.max_storage_bytes
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }
}

impl Default for LocalRecorderConfig {
    fn default() -> Self {
        Self {
            segment_duration: DEFAULT_SEGMENT_DURATION,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            retention_days: DEFAULT_RETENTION_DAYS,
            max_storage_bytes: DEFAULT_MAX_STORAGE_BYTES,
            output_dir: default_output_dir(),
        }
    }
}

#[derive(Debug, Default)]
pub struct LocalRecorderConfigBuilder {
    segment_duration: Option<Duration>,
    idle_timeout: Option<Duration>,
    retention_days: Option<u64>,
    max_storage_bytes: Option<u64>,
    output_dir: Option<PathBuf>,
}

impl LocalRecorderConfigBuilder {
    pub fn segment_duration(mut self, segment_duration: Duration) -> Self {
        self.segment_duration = Some(segment_duration);
        self
    }

    pub fn idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.idle_timeout = Some(idle_timeout);
        self
    }

    pub fn retention_days(mut self, retention_days: u64) -> Self {
        self.retention_days = Some(retention_days);
        self
    }

    pub fn max_storage_bytes(mut self, max_storage_bytes: u64) -> Self {
        self.max_storage_bytes = Some(max_storage_bytes);
        self
    }

    pub fn output_dir(mut self, output_dir: PathBuf) -> Self {
        self.output_dir = Some(output_dir);
        self
    }

    pub fn build(self) -> hbb_common::ResultType<LocalRecorderConfig> {
        let defaults = LocalRecorderConfig::default();
        let config = LocalRecorderConfig {
            segment_duration: self.segment_duration.unwrap_or(defaults.segment_duration),
            idle_timeout: self.idle_timeout.unwrap_or(defaults.idle_timeout),
            retention_days: self.retention_days.unwrap_or(defaults.retention_days),
            max_storage_bytes: self.max_storage_bytes.unwrap_or(defaults.max_storage_bytes),
            output_dir: self.output_dir.unwrap_or(defaults.output_dir),
        };

        if config.segment_duration.is_zero() {
            anyhow::bail!("segment duration must be greater than zero");
        }
        if config.idle_timeout.is_zero() {
            anyhow::bail!("idle timeout must be greater than zero");
        }
        if config.retention_days == 0 {
            anyhow::bail!("retention days must be greater than zero");
        }
        if config.max_storage_bytes == 0 {
            anyhow::bail!("max storage bytes must be greater than zero");
        }
        if config.output_dir.exists() && !config.output_dir.is_dir() {
            anyhow::bail!("output directory must be a directory");
        }

        Ok(config)
    }
}

fn default_output_dir() -> PathBuf {
    hbb_common::config::Config::path("local-recordings")
}
