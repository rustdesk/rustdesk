use std::time::{Duration, Instant};

pub trait IdleSource {
    fn idle_duration(&self) -> hbb_common::ResultType<Duration>;
}

#[derive(Debug, Clone)]
pub struct ActivityTracker {
    idle_timeout: Duration,
    last_activity: Option<Instant>,
}

impl ActivityTracker {
    pub fn new(idle_timeout: Duration) -> Self {
        Self {
            idle_timeout,
            last_activity: None,
        }
    }

    pub fn is_active(&self, now: Instant) -> bool {
        self.last_activity
            .and_then(|last| now.checked_duration_since(last))
            .is_some_and(|elapsed| elapsed <= self.idle_timeout)
    }

    pub fn record_activity(&mut self, now: Instant) {
        self.last_activity = Some(now);
    }

    pub fn source_is_active(
        source: &impl IdleSource,
        idle_timeout: Duration,
    ) -> hbb_common::ResultType<bool> {
        Ok(source.idle_duration()? <= idle_timeout)
    }
}
