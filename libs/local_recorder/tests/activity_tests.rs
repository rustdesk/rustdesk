use std::time::{Duration, Instant};

use local_recorder::{ActivityTracker, IdleSource};

#[derive(Debug, Clone)]
struct FakeIdleSource {
    idle_duration: Duration,
}

impl IdleSource for FakeIdleSource {
    fn idle_duration(&self) -> hbb_common::ResultType<Duration> {
        Ok(self.idle_duration)
    }
}

#[test]
fn tracker_is_idle_until_activity_is_observed() {
    let start = Instant::now();
    let tracker = ActivityTracker::new(Duration::from_secs(30));

    assert!(!tracker.is_active(start + Duration::from_secs(1)));
}

#[test]
fn activity_keeps_tracker_active_until_idle_timeout() {
    let start = Instant::now();
    let mut tracker = ActivityTracker::new(Duration::from_secs(30));

    tracker.record_activity(start + Duration::from_secs(5));

    assert!(tracker.is_active(start + Duration::from_secs(34)));
    assert!(!tracker.is_active(start + Duration::from_secs(36)));
}

#[test]
fn repeated_activity_updates_timestamp_monotonically() {
    let start = Instant::now();
    let mut tracker = ActivityTracker::new(Duration::from_secs(30));

    tracker.record_activity(start + Duration::from_secs(5));
    tracker.record_activity(start + Duration::from_secs(10));

    assert!(tracker.is_active(start + Duration::from_secs(39)));
}

#[test]
fn idle_source_recent_input_counts_as_activity() {
    let source = FakeIdleSource {
        idle_duration: Duration::from_secs(3),
    };

    assert!(ActivityTracker::source_is_active(&source, Duration::from_secs(30)).unwrap());
}

#[test]
fn idle_source_old_input_counts_as_idle() {
    let source = FakeIdleSource {
        idle_duration: Duration::from_secs(31),
    };

    assert!(!ActivityTracker::source_is_active(&source, Duration::from_secs(30)).unwrap());
}
