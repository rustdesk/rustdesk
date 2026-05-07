use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use local_recorder::{LocalRecorderConfig, StorageManager};

#[test]
fn default_config_uses_safe_nonzero_limits() {
    let config = LocalRecorderConfig::default();

    assert_eq!(Duration::from_secs(60), config.segment_duration());
    assert_eq!(Duration::from_secs(30), config.idle_timeout());
    assert!(config.retention_days() > 0);
    assert!(config.max_storage_bytes() > 0);
}

#[test]
fn zero_segment_duration_is_rejected() {
    let err = LocalRecorderConfig::builder()
        .segment_duration(Duration::ZERO)
        .build()
        .unwrap_err();

    assert!(err.to_string().contains("segment duration"));
}

#[test]
fn zero_idle_timeout_is_rejected() {
    let err = LocalRecorderConfig::builder()
        .idle_timeout(Duration::ZERO)
        .build()
        .unwrap_err();

    assert!(err.to_string().contains("idle timeout"));
}

#[test]
fn invalid_output_directory_is_rejected() {
    let dir = unique_temp_path("config-file-parent");
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("not-a-directory");
    fs::write(&file, b"file").unwrap();

    let err = LocalRecorderConfig::builder()
        .output_dir(file)
        .build()
        .unwrap_err();

    assert!(err.to_string().contains("output directory"));
}

#[test]
fn storage_manager_allocates_timestamp_segment_paths() {
    let temp = unique_temp_path("config-timestamp-paths");
    let storage = StorageManager::new(&temp).unwrap();
    let start = UNIX_EPOCH + Duration::from_secs(1_704_067_200);
    let end = UNIX_EPOCH + Duration::from_secs(1_704_070_800);

    let paths = storage.segment_paths_for_period(start, end).unwrap();

    assert_eq!(temp.join("20240101-000000-010000.webm.tmp"), paths.temporary);
    assert_eq!(temp.join("20240101-000000-010000.webm"), paths.complete);
}

#[test]
fn storage_manager_creates_private_directory_and_opaque_segment_paths() {
    let temp = unique_temp_path("config-storage");
    let config = LocalRecorderConfig::builder()
        .output_dir(temp.join("recordings"))
        .build()
        .unwrap();

    let storage = StorageManager::new(config.output_dir()).unwrap();
    let first = storage.next_segment_path().unwrap();
    let second = storage.next_segment_path().unwrap();

    assert_eq!(Some("webm"), first.extension().and_then(|ext| ext.to_str()));
    assert_eq!(
        Some("webm"),
        second.extension().and_then(|ext| ext.to_str())
    );
    assert_ne!(first, second);
    assert!(!first
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains("2026"));
    assert!(config.output_dir().is_dir());
}

#[test]
fn default_output_directory_is_not_system_temp() {
    let config = LocalRecorderConfig::default();
    let temp = std::env::temp_dir();

    assert!(!config.output_dir().starts_with(&temp));
}

#[cfg(unix)]
#[test]
fn storage_manager_restricts_directory_permissions() {
    let temp = unique_temp_path("config-permissions");
    let storage = StorageManager::new(&temp).unwrap();

    let mode = fs::metadata(&temp).unwrap().permissions().mode() & 0o777;
    assert_eq!(0o700, mode);

    let paths = storage.next_segment_paths().unwrap();
    fs::write(&paths.temporary, b"webm").unwrap();
    storage.mark_complete(&paths).unwrap();

    let file_mode = fs::metadata(&paths.complete).unwrap().permissions().mode() & 0o777;
    assert_eq!(0o600, file_mode);
}

fn unique_temp_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rustdesk-local-recorder-{prefix}-{nanos}"))
}
