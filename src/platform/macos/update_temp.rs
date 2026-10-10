use super::ResultType;
use hbb_common::{bail, log};
use std::{
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::OnceLock,
};

static UPDATE_TEMP_DIR: OnceLock<PathBuf> = OnceLock::new();
const UPDATE_TEMP_DMG_CREATE_ATTEMPTS: usize = 16;
const STALE_UPDATE_TEMP_DIR_SECS: u64 = 24 * 60 * 60;

#[inline]
fn get_update_temp_dir() -> PathBuf {
    UPDATE_TEMP_DIR.get_or_init(new_update_temp_dir).clone()
}

fn new_update_temp_dir() -> PathBuf {
    let euid = unsafe { hbb_common::libc::geteuid() };
    Path::new("/tmp").join(format!(
        ".rustdeskupdate-{}-{}-{}",
        euid,
        std::process::id(),
        hbb_common::rand::random::<u64>()
    ))
}

fn legacy_update_temp_dir() -> PathBuf {
    let euid = unsafe { hbb_common::libc::geteuid() };
    Path::new("/tmp").join(format!(".rustdeskupdate-{}", euid))
}

fn stale_update_temp_dir_prefix() -> String {
    let euid = unsafe { hbb_common::libc::geteuid() };
    format!(".rustdeskupdate-{}-", euid)
}

fn is_stale_update_temp_dir_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix(&stale_update_temp_dir_prefix()) else {
        return false;
    };
    let mut parts = suffix.split('-');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(pid), Some(random), None)
            if !pid.is_empty()
                && !random.is_empty()
                && pid.bytes().all(|byte| byte.is_ascii_digit())
                && random.bytes().all(|byte| byte.is_ascii_digit())
    )
}

#[inline]
pub(super) fn get_update_temp_dir_string() -> String {
    get_update_temp_dir().to_string_lossy().into_owned()
}

fn ensure_real_update_temp_dir(path: &Path) -> ResultType<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "Update temp path is not a real directory: {}",
            path.display()
        );
    }
    Ok(())
}

pub(super) fn create_update_temp_dmg_file() -> ResultType<(std::fs::File, PathBuf)> {
    let update_temp_dir = get_update_temp_dir();
    std::fs::create_dir_all(&update_temp_dir)?;
    ensure_real_update_temp_dir(&update_temp_dir)?;
    std::fs::set_permissions(&update_temp_dir, std::fs::Permissions::from_mode(0o700))?;

    let dmg_dir = update_temp_dir.join("dmgdir");
    std::fs::create_dir_all(&dmg_dir)?;
    ensure_real_update_temp_dir(&dmg_dir)?;
    std::fs::set_permissions(&dmg_dir, std::fs::Permissions::from_mode(0o700))?;

    for _ in 0..UPDATE_TEMP_DMG_CREATE_ATTEMPTS {
        let file_path = dmg_dir.join(format!(
            "{}-{}-{}.dmg",
            crate::get_app_name(),
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&file_path);
        match file {
            Ok(file) => return Ok((file, file_path)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err.into()),
        }
    }

    bail!("Failed to create update DMG file")
}

#[inline]
pub fn try_remove_temp_update_dir(dir: Option<&str>) {
    if let Some(dir) = dir {
        remove_temp_update_dir(Path::new(dir));
    } else {
        remove_temp_update_dir(&legacy_update_temp_dir());
        remove_stale_update_temp_dirs();
    }
}

fn remove_stale_update_temp_dirs() {
    use std::os::unix::fs::MetadataExt as _;

    let current_update_temp_dir = get_update_temp_dir();
    let legacy_update_temp_dir = legacy_update_temp_dir();
    let euid = unsafe { hbb_common::libc::geteuid() };
    let Ok(entries) = std::fs::read_dir("/tmp") else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == current_update_temp_dir || path == legacy_update_temp_dir {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !is_stale_update_temp_dir_name(name) {
            continue;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.is_dir() || metadata.uid() != euid || !is_old_update_temp_dir(&metadata) {
            continue;
        }
        remove_temp_update_dir(&path);
    }
}

fn is_old_update_temp_dir(metadata: &std::fs::Metadata) -> bool {
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    modified
        .elapsed()
        .map(|age| age.as_secs() >= STALE_UPDATE_TEMP_DIR_SECS)
        .unwrap_or(false)
}

fn remove_temp_update_dir(path: &Path) {
    match std::fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(err)
            if err.kind() == io::ErrorKind::NotFound
                || err.raw_os_error() == Some(hbb_common::libc::ENOTDIR) => {}
        Err(err) => {
            log::warn!(
                "Failed to remove update temp dir {}: {}",
                path.display(),
                err
            );
        }
    }
}

pub(super) fn copy_dmg_to_update_temp_file(dmg_path: &str) -> ResultType<PathBuf> {
    let metadata = std::fs::symlink_metadata(dmg_path)?;
    if !metadata.file_type().is_file() {
        bail!("Update DMG path is not a regular file: {}", dmg_path);
    }
    let mut source_file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(hbb_common::libc::O_NOFOLLOW | hbb_common::libc::O_NONBLOCK)
        .open(dmg_path)?;
    if !source_file.metadata()?.is_file() {
        bail!("Update DMG path is not a regular file: {}", dmg_path);
    }
    let (mut dmg_file, file_path) = create_update_temp_dmg_file()?;
    io::copy(&mut source_file, &mut dmg_file)?;
    dmg_file.flush()?;
    Ok(file_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_dir_cleanup_does_not_follow_symlink() {
        let test_dir = std::env::temp_dir().join(format!(
            "rustdesk-macos-cleanup-symlink-test-{}-{}",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        let target_dir = test_dir.join("target");
        let link_path = test_dir.join("link");
        let target_file = target_dir.join("file");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(&target_file, b"target").unwrap();
        std::os::unix::fs::symlink(&target_dir, &link_path).unwrap();

        remove_temp_update_dir(&link_path);

        assert!(std::fs::symlink_metadata(&link_path).is_err());
        assert!(target_file.exists());
        std::fs::remove_dir_all(test_dir).unwrap();
    }

    #[test]
    fn update_temp_dir_rejects_symlink() {
        let test_dir = std::env::temp_dir().join(format!(
            "rustdesk-macos-temp-dir-symlink-test-{}-{}",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        let target_dir = test_dir.join("target");
        let link_path = test_dir.join("link");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::os::unix::fs::symlink(&target_dir, &link_path).unwrap();

        assert!(ensure_real_update_temp_dir(&link_path).is_err());
        assert!(ensure_real_update_temp_dir(&target_dir).is_ok());

        std::fs::remove_dir_all(test_dir).unwrap();
    }

    #[test]
    fn stale_update_temp_dir_name_requires_owned_randomized_dir() {
        let euid = unsafe { hbb_common::libc::geteuid() };

        assert!(is_stale_update_temp_dir_name(&format!(
            ".rustdeskupdate-{euid}-123-456"
        )));
        for name in [
            format!(".rustdeskupdate-{euid}"),
            format!(".rustdeskupdate-{euid}-abc-456"),
            format!(".rustdeskupdate-{euid}-123"),
            ".rustdeskupdate-999999-123-456".to_owned(),
            "rustdeskupdate-123-456".to_owned(),
        ] {
            assert!(!is_stale_update_temp_dir_name(&name), "{name}");
        }
    }
}
