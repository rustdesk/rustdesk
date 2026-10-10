use super::*;

#[test]
fn copy_and_verify_update_artifact_hashes_open_file_handle() {
    let test_dir = std::env::temp_dir().join(format!(
        "rustdesk-updater-open-handle-sha256-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();
    let final_path = test_dir.join("rustdesk-update.exe");
    let (mut file, _) = create_download_temp_file(&final_path).unwrap();
    let display_path = test_dir.join("path-must-not-be-opened.download");
    let mut data: &[u8] = b"rustdesk";

    let result = copy_and_verify_update_artifact(
        &mut file,
        &display_path,
        &mut data,
        8,
        "304ca1638c5effa6832e0e15b958a8f74847efe4df9c3f3187216e921c168fed",
    );
    let position = result
        .as_ref()
        .ok()
        .map(|_| std::io::Seek::stream_position(&mut file).unwrap());
    drop(file);
    std::fs::remove_dir_all(&test_dir).unwrap();

    assert!(result.is_ok(), "{:?}", result.err());
    assert_eq!(position, Some(0));
}

#[test]
fn remove_cached_update_artifact_rejects_directory() {
    let test_dir = std::env::temp_dir().join(format!(
        "rustdesk-updater-cache-dir-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();
    let cache_path = test_dir.join("rustdesk-update.exe");
    std::fs::create_dir(&cache_path).unwrap();
    std::fs::write(cache_path.join("stale"), b"stale").unwrap();

    let result = remove_cached_update_artifact(&cache_path);

    assert!(result.is_err());
    assert!(cache_path.exists());
    std::fs::remove_dir_all(&test_dir).unwrap();
}

#[cfg(unix)]
#[test]
fn verified_download_replaces_symlink_without_touching_target() {
    let test_dir = std::env::temp_dir().join(format!(
        "rustdesk-updater-symlink-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();
    let final_path = test_dir.join("rustdesk-update.exe");
    let victim_path = test_dir.join("victim");
    std::fs::write(&victim_path, b"victim").unwrap();
    std::os::unix::fs::symlink(&victim_path, &final_path).unwrap();
    let mut data: &[u8] = b"update";

    write_verified_update_artifact(
        &final_path,
        &mut data,
        6,
        "2937013f2181810606b2a799b05bda2849f3e369a20982a4138f0e0a55984ce4",
    )
    .unwrap();

    assert_eq!(std::fs::read(&victim_path).unwrap(), b"victim");
    assert_eq!(std::fs::read(&final_path).unwrap(), b"update");
    assert!(!std::fs::symlink_metadata(&final_path)
        .unwrap()
        .file_type()
        .is_symlink());
    std::fs::remove_dir_all(&test_dir).unwrap();
}
