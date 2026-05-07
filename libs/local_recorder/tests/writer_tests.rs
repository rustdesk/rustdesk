use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use local_recorder::{
    SegmentEvent, SegmentPaths, SegmentPlanner, StorageManager, WebmSegmentWriter,
};

#[test]
fn segment_rotates_after_configured_duration() {
    let start = Instant::now();
    let mut planner = SegmentPlanner::new(Duration::from_secs(60));

    assert_eq!(SegmentEvent::Started, planner.start(start, 1920, 1080));
    assert_eq!(
        SegmentEvent::Continue,
        planner.update(start + Duration::from_secs(59), 1920, 1080)
    );
    assert_eq!(
        SegmentEvent::Rotate,
        planner.update(start + Duration::from_secs(60), 1920, 1080)
    );
}

#[test]
fn resolution_change_forces_segment_rotation() {
    let start = Instant::now();
    let mut planner = SegmentPlanner::new(Duration::from_secs(60));

    planner.start(start, 1920, 1080);

    assert_eq!(
        SegmentEvent::Rotate,
        planner.update(start + Duration::from_secs(10), 1280, 720)
    );
}

#[test]
fn finalized_segment_is_renamed_from_temporary_path() {
    let temp = unique_temp_path("writer-finalize");
    let storage = StorageManager::new(&temp).unwrap();
    let paths = storage.next_segment_paths().unwrap();

    fs::write(&paths.temporary, b"webm").unwrap();
    storage.mark_complete(&paths).unwrap();

    assert!(!paths.temporary.exists());
    assert!(paths.complete.exists());
}

#[test]
fn webm_writer_creates_complete_segment_from_encoded_frames() {
    let temp = unique_temp_path("writer-webm");
    let storage = StorageManager::new(&temp).unwrap();
    let paths = storage.next_segment_paths().unwrap();

    let mut writer = WebmSegmentWriter::new(paths.clone(), 16, 16).unwrap();
    writer.write_encoded_frame(b"key", 0, true).unwrap();
    writer.write_encoded_frame(b"delta", 33, false).unwrap();
    let complete = writer.finalize().unwrap();

    assert_eq!(paths.complete, complete);
    assert!(!paths.temporary.exists());
    assert!(paths.complete.exists());
    assert!(fs::metadata(paths.complete).unwrap().len() > 0);
}

#[test]
fn webm_writer_finalizes_to_timestamp_path() {
    let temp = unique_temp_path("writer-finalize-to");
    let storage = StorageManager::new(&temp).unwrap();
    let paths = storage.next_segment_paths().unwrap();
    let start = UNIX_EPOCH + Duration::from_secs(1_704_067_200);
    let end = UNIX_EPOCH + Duration::from_secs(1_704_070_800);
    let timestamp_paths = storage.segment_paths_for_period(start, end).unwrap();

    let mut writer = WebmSegmentWriter::new(paths.clone(), 16, 16).unwrap();
    writer.write_encoded_frame(b"key", 0, true).unwrap();
    let complete = writer.finalize_to(timestamp_paths.complete.clone()).unwrap();

    assert_eq!(timestamp_paths.complete, complete);
    assert!(!paths.temporary.exists());
    assert!(!paths.complete.exists());
    assert!(timestamp_paths.complete.exists());
}

#[cfg(unix)]
#[test]
fn webm_writer_restricts_temporary_segment_permissions() {
    let temp = unique_temp_path("writer-temp-permissions");
    let storage = StorageManager::new(&temp).unwrap();
    let paths = storage.next_segment_paths().unwrap();

    let _writer = WebmSegmentWriter::new(paths.clone(), 16, 16).unwrap();

    let mode = fs::metadata(&paths.temporary).unwrap().permissions().mode() & 0o777;
    assert_eq!(0o600, mode);
}

#[test]
fn retention_removes_oldest_complete_segments_first() {
    let temp = unique_temp_path("writer-retention");
    let storage = StorageManager::new(&temp).unwrap();

    let first = SegmentPaths {
        temporary: temp.join("a.webm.tmp"),
        complete: temp.join("a.webm"),
    };
    fs::write(&first.temporary, vec![1; 10]).unwrap();
    storage.mark_complete(&first).unwrap();

    let second = SegmentPaths {
        temporary: temp.join("b.webm.tmp"),
        complete: temp.join("b.webm"),
    };
    fs::write(&second.temporary, vec![2; 10]).unwrap();
    storage.mark_complete(&second).unwrap();

    storage.enforce_storage_cap(10).unwrap();

    assert!(!first.complete.exists());
    assert!(second.complete.exists());
}

#[test]
fn cleanup_removes_orphaned_temporary_segments() {
    let temp = unique_temp_path("writer-orphans");
    let storage = StorageManager::new(&temp).unwrap();
    let paths = storage.next_segment_paths().unwrap();
    fs::write(&paths.temporary, b"partial").unwrap();

    storage.clean_orphaned_temp_files().unwrap();

    assert!(!paths.temporary.exists());
}

#[test]
fn retention_age_removes_expired_segments() {
    let temp = unique_temp_path("writer-retention-age");
    let storage = StorageManager::new(&temp).unwrap();
    let paths = storage.next_segment_paths().unwrap();
    fs::write(&paths.temporary, b"webm").unwrap();
    storage.mark_complete(&paths).unwrap();

    storage.enforce_retention_cap(0).unwrap();

    assert!(!paths.complete.exists());
}

fn unique_temp_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rustdesk-local-recorder-{prefix}-{nanos}"))
}
