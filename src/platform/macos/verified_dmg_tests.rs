use super::{
    verified_dmg::{
        copy_and_verify_dmg_file, create_dmg_mount_point, extract_dmg_inner, verified_dmg_path,
        verify_stored_dmg, VerifiedDmg,
    },
    verified_dmg_update_source, ResultType,
};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

const TEST_DMG_MARKER: &[u8] = b"verified DMG";

fn create_test_dmg(test_dir: &Path) -> PathBuf {
    let source_dir = test_dir.join("source");
    let app_dir = source_dir
        .join(format!("{}.app", crate::get_app_name()))
        .join("Contents");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join("marker"), TEST_DMG_MARKER).unwrap();
    let dmg_path = test_dir.join("update.dmg");
    let output = Command::new("/usr/bin/hdiutil")
        .args(["create", "-quiet", "-format", "UDZO", "-srcfolder"])
        .arg(&source_dir)
        .arg(&dmg_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    dmg_path
}

fn extract_verified_dmg(verified_dmg: &VerifiedDmg, target_dir: &str) -> ResultType<()> {
    let target_path = Path::new(target_dir);
    if target_path.exists() {
        std::fs::remove_dir_all(target_path)?;
    }
    std::fs::create_dir_all(target_path)?;
    verify_stored_dmg(verified_dmg)?;
    extract_dmg_inner(verified_dmg_path(verified_dmg)?, target_dir)
}

#[test]
fn update_dmg_mount_points_are_unique() {
    let first = create_dmg_mount_point().unwrap();
    let second = create_dmg_mount_point().unwrap();
    let are_unique = first != second;
    for mount_point in [&first, &second] {
        if mount_point.starts_with("/tmp/.rustdeskmount-") {
            std::fs::remove_dir(mount_point).unwrap();
        }
    }

    assert!(are_unique);
}

#[test]
fn verified_dmg_extracts_from_named_copy() {
    let test_dir = std::env::temp_dir().join(format!(
        "rustdesk-verified-dmg-extract-test-{}-{}",
        std::process::id(),
        hbb_common::rand::random::<u64>()
    ));
    std::fs::create_dir_all(&test_dir).unwrap();
    let dmg_path = create_test_dmg(&test_dir);
    let mut dmg_file = std::fs::File::open(&dmg_path).unwrap();
    let sha256 = crate::update_hash::sha256_reader_hex(&mut dmg_file).unwrap();
    let size = dmg_file.metadata().unwrap().len();
    let verified_dmg =
        copy_and_verify_dmg_file(&dmg_path.to_string_lossy(), &sha256, Some(size)).unwrap();
    let target_dir = test_dir.join("target");

    let result = extract_verified_dmg(&verified_dmg, &target_dir.to_string_lossy());
    let marker = std::fs::read(
        target_dir
            .join(format!("{}.app", crate::get_app_name()))
            .join("Contents/marker"),
    );
    std::fs::remove_dir_all(test_dir).unwrap();

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(marker.unwrap(), TEST_DMG_MARKER);
}

#[test]
fn verified_dmg_rejects_replaced_path() {
    let test_dir = std::env::temp_dir().join(format!(
        "rustdesk-verified-dmg-replacement-test-{}-{}",
        std::process::id(),
        hbb_common::rand::random::<u64>()
    ));
    let dmg_path = create_test_dmg(&test_dir.join("trusted"));
    let mut dmg_file = std::fs::File::open(&dmg_path).unwrap();
    let sha256 = crate::update_hash::sha256_reader_hex(&mut dmg_file).unwrap();
    let size = dmg_file.metadata().unwrap().len();
    let verified_dmg =
        copy_and_verify_dmg_file(&dmg_path.to_string_lossy(), &sha256, Some(size)).unwrap();
    let replacement_dmg = create_test_dmg(&test_dir.join("replacement"));
    std::fs::remove_file(&verified_dmg.path).unwrap();
    std::fs::rename(replacement_dmg, &verified_dmg.path).unwrap();

    let result = verified_dmg_update_source(&verified_dmg);
    drop(verified_dmg);
    std::fs::remove_dir_all(test_dir).unwrap();

    assert!(result.is_err());
}

#[test]
fn verified_dmg_rejects_in_place_mutation() {
    let test_dir = std::env::temp_dir().join(format!(
        "rustdesk-verified-dmg-mutation-test-{}-{}",
        std::process::id(),
        hbb_common::rand::random::<u64>()
    ));
    let dmg_path = create_test_dmg(&test_dir);
    let mut dmg_file = std::fs::File::open(&dmg_path).unwrap();
    let sha256 = crate::update_hash::sha256_reader_hex(&mut dmg_file).unwrap();
    let size = dmg_file.metadata().unwrap().len();
    let verified_dmg =
        copy_and_verify_dmg_file(&dmg_path.to_string_lossy(), &sha256, Some(size)).unwrap();
    std::fs::write(&verified_dmg.path, b"tampered").unwrap();

    let result = verified_dmg_update_source(&verified_dmg);
    drop(verified_dmg);
    std::fs::remove_dir_all(test_dir).unwrap();

    assert!(result
        .err()
        .unwrap()
        .to_string()
        .contains("SHA256 mismatch"));
}

#[test]
fn verified_dmg_rejects_sha256_mismatch() {
    let file_path =
        std::env::temp_dir().join(format!("rustdesk-verified-dmg-test-{}", std::process::id()));
    std::fs::write(&file_path, b"rustdesk").unwrap();
    let result = copy_and_verify_dmg_file(
        &file_path.to_string_lossy(),
        "0000000000000000000000000000000000000000000000000000000000000000",
        Some(8),
    );

    std::fs::remove_file(file_path).unwrap();
    assert!(result.is_err());
}

#[test]
fn verified_dmg_rejects_symlink() {
    let test_dir = std::env::temp_dir().join(format!(
        "rustdesk-verified-dmg-symlink-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();
    let target_path = test_dir.join("target.dmg");
    let link_path = test_dir.join("update.dmg");
    std::fs::write(&target_path, b"rustdesk").unwrap();
    std::os::unix::fs::symlink(&target_path, &link_path).unwrap();
    let result = copy_and_verify_dmg_file(
        &link_path.to_string_lossy(),
        "304ca1638c5effa6832e0e15b958a8f74847efe4df9c3f3187216e921c168fed",
        Some(8),
    );

    std::fs::remove_dir_all(test_dir).unwrap();
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("not a regular file"));
}

#[test]
fn verified_dmg_handle_survives_path_replacement() {
    use std::io::{Read as _, Seek as _};

    let test_dir = std::env::temp_dir().join(format!(
        "rustdesk-verified-dmg-handle-test-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir).unwrap();
    let file_path = test_dir.join("update.dmg");
    let original_path = test_dir.join("original.dmg");
    std::fs::write(&file_path, b"rustdesk").unwrap();

    let mut verified_dmg = copy_and_verify_dmg_file(
        &file_path.to_string_lossy(),
        "304ca1638c5effa6832e0e15b958a8f74847efe4df9c3f3187216e921c168fed",
        Some(8),
    )
    .unwrap();
    std::fs::rename(&file_path, &original_path).unwrap();
    std::fs::write(&file_path, b"tampered").unwrap();

    verified_dmg.file.seek(std::io::SeekFrom::Start(0)).unwrap();
    let mut contents = Vec::new();
    verified_dmg.file.read_to_end(&mut contents).unwrap();

    std::fs::remove_dir_all(test_dir).unwrap();
    assert_eq!(contents, b"rustdesk");
}
