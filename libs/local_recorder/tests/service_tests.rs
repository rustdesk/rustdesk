use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use local_recorder::{LocalRecorderConfig, LocalRecorderService, RecorderState, StatusReason};

fn config() -> LocalRecorderConfig {
    LocalRecorderConfig::builder().build().unwrap()
}

#[test]
fn start_enters_recording_without_creating_segments() {
    let service = LocalRecorderService::new(config());

    assert_eq!(RecorderState::Stopped, service.state());

    service.start().unwrap();

    assert_eq!(RecorderState::Recording, service.state());
    assert_eq!(0, service.completed_segments());
}

#[test]
fn start_enters_recording_and_starts_driver() {
    let service = LocalRecorderService::new(config());

    service.start().unwrap();

    assert_eq!(RecorderState::Recording, service.state());
    assert!(service.driver_active());
    service.stop().unwrap();
}

#[test]
fn activity_transitions_idle_to_recording() {
    let start = Instant::now();
    let service = LocalRecorderService::new(config());

    service.start().unwrap();
    service.record_activity(start);
    service.tick(start + Duration::from_secs(1)).unwrap();

    assert_eq!(RecorderState::Recording, service.state());
}

#[test]
fn idle_timeout_finalizes_segment_and_returns_to_idle() {
    let start = Instant::now();
    let service = LocalRecorderService::new(config());

    service.start().unwrap();
    service.record_activity(start);
    service.tick(start + Duration::from_secs(1)).unwrap();
    service.tick(start + Duration::from_secs(36)).unwrap();

    assert_eq!(RecorderState::Idle, service.state());
    assert_eq!(1, service.completed_segments());
}

#[test]
fn repeated_start_is_idempotent() {
    let service = LocalRecorderService::new(config());

    service.start().unwrap();
    service.start().unwrap();

    assert_eq!(RecorderState::Recording, service.state());
}

#[test]
fn stop_before_start_is_safe() {
    let service = LocalRecorderService::new(config());

    service.stop().unwrap();

    assert_eq!(RecorderState::Stopped, service.state());
}

#[test]
fn pause_reports_specific_status_reason() {
    let service = LocalRecorderService::new(config());

    service.start().unwrap();
    service.pause(StatusReason::UnsupportedPlatform);

    assert_eq!(RecorderState::Paused, service.state());
    assert_eq!(
        Some(StatusReason::UnsupportedPlatform),
        service.status_reason()
    );
}

#[test]
fn start_enforces_storage_cap_before_recording() {
    let temp = unique_temp_path("service-storage-cap");
    fs::create_dir_all(&temp).unwrap();
    fs::write(temp.join("a.webm"), vec![1; 10]).unwrap();
    fs::write(temp.join("b.webm"), vec![2; 10]).unwrap();

    let service = LocalRecorderService::new(
        LocalRecorderConfig::builder()
            .output_dir(temp.clone())
            .max_storage_bytes(10)
            .build()
            .unwrap(),
    );

    service.start().unwrap();

    assert!(!temp.join("a.webm").exists());
    assert!(temp.join("b.webm").exists());
}

#[test]
#[cfg(feature = "capture")]
fn start_failure_returns_to_stopped() {
    let temp = unique_temp_path("service-start-failure");
    fs::write(&temp, b"not a directory").unwrap();

    let service = LocalRecorderService::new(
        LocalRecorderConfig::builder()
            .output_dir(temp.join("recordings"))
            .build()
            .unwrap(),
    );

    assert!(service.start().is_err());
    assert_eq!(RecorderState::Stopped, service.state());
    assert!(!service.driver_active());
}

fn unique_temp_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rustdesk-local-recorder-{prefix}-{nanos}"))
}
