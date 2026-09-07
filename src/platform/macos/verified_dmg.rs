use super::{update_temp::create_update_temp_dmg_file, ResultType};
use hbb_common::{anyhow::anyhow, bail, log};
use std::{
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::Command,
};

const UPDATE_DMG_MOUNT_TEMPLATE: &str = "/tmp/.rustdeskmount-XXXXXX";

#[derive(Debug)]
pub(super) struct VerifiedDmg {
    pub(super) file: std::fs::File,
    pub(super) path: PathBuf,
    pub(super) expected_sha256: String,
}

impl Drop for VerifiedDmg {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.path) {
            if err.kind() != io::ErrorKind::NotFound {
                log::warn!(
                    "Failed to remove verified DMG copy {}: {}",
                    self.path.display(),
                    err
                );
            }
        }
    }
}

// Retain the named copy until the privileged installer has verified its root-owned copy.
pub(super) fn copy_and_verify_dmg_file(
    file: &str,
    expected_sha256: &str,
    expected_size: Option<u64>,
) -> ResultType<VerifiedDmg> {
    let mut source_file = open_dmg_file(file, expected_size)?;
    let mut verified_dmg = create_verified_dmg_file(expected_sha256)?;
    let copied_size = io::copy(&mut source_file, &mut verified_dmg.file)?;
    if expected_size.is_some_and(|size| size != copied_size) {
        bail!("DMG size mismatch for {}", file);
    }
    verified_dmg.file.flush()?;
    verify_dmg_contents(&mut verified_dmg.file, expected_sha256, file)?;
    Ok(verified_dmg)
}

fn open_dmg_file(file: &str, expected_size: Option<u64>) -> ResultType<std::fs::File> {
    let path_metadata = std::fs::symlink_metadata(file)?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        bail!("Update DMG path is not a regular file: {}", file);
    }
    if expected_size.is_some_and(|size| size != path_metadata.len()) {
        bail!("DMG size mismatch for {}", file);
    }
    let dmg_file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(hbb_common::libc::O_NOFOLLOW | hbb_common::libc::O_NONBLOCK)
        .open(file)?;
    let opened_metadata = dmg_file.metadata()?;
    if !opened_metadata.is_file() || expected_size.is_some_and(|size| size != opened_metadata.len())
    {
        bail!("DMG changed while opening {}", file);
    }
    Ok(dmg_file)
}

fn create_verified_dmg_file(expected_sha256: &str) -> ResultType<VerifiedDmg> {
    let (file, path) = create_update_temp_dmg_file()?;
    Ok(VerifiedDmg {
        file,
        path,
        expected_sha256: expected_sha256.trim().to_ascii_lowercase(),
    })
}

fn verify_dmg_contents(
    dmg_file: &mut std::fs::File,
    expected_sha256: &str,
    file: &str,
) -> ResultType<()> {
    use crate::update_hash::{verify_sha256_reader, Sha256VerificationError};

    match verify_sha256_reader(dmg_file, expected_sha256) {
        Ok(()) => Ok(()),
        Err(Sha256VerificationError::InvalidExpected) => {
            bail!("Expected DMG SHA256 is malformed for {}", file)
        }
        Err(Sha256VerificationError::Mismatch { .. }) => {
            bail!("SHA256 mismatch for {}", file)
        }
        Err(Sha256VerificationError::Io(err)) => Err(err.into()),
    }
}

pub(super) struct DmgGuard(String);

impl DmgGuard {
    pub(super) fn mount_point(&self) -> &str {
        &self.0
    }
}

impl Drop for DmgGuard {
    fn drop(&mut self) {
        match Command::new("/usr/bin/hdiutil")
            .args(["detach", self.0.as_str(), "-force"])
            .status()
        {
            Ok(status) if !status.success() => {
                log::warn!("Failed to detach DMG mount {}: {}", self.0, status);
            }
            Err(err) => log::warn!("Failed to detach DMG mount {}: {}", self.0, err),
            _ => {}
        }
        remove_dmg_mount_point(&self.0);
    }
}

pub(super) fn attach_dmg(dmg_path: &str) -> ResultType<DmgGuard> {
    let mount_point = create_dmg_mount_point()?;
    // Update images are input only, so never mount them writable.
    let output = match Command::new("/usr/bin/hdiutil")
        .args([
            "attach",
            "-readonly",
            "-nobrowse",
            "-mountpoint",
            mount_point.as_str(),
            dmg_path,
        ])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            remove_dmg_mount_point(&mount_point);
            return Err(err.into());
        }
    };
    if !output.status.success() {
        remove_dmg_mount_point(&mount_point);
        bail!(
            "Failed to attach DMG image at {dmg_path} to {mount_point}: {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(DmgGuard(mount_point))
}

pub(super) fn create_dmg_mount_point() -> ResultType<String> {
    let output = Command::new("/usr/bin/mktemp")
        .args(["-d", UPDATE_DMG_MOUNT_TEMPLATE])
        .output()?;
    if !output.status.success() {
        bail!(
            "Failed to create a private DMG mount directory: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mount_point = String::from_utf8(output.stdout)
        .map_err(|e| anyhow!("Invalid DMG mount directory: {}", e))?
        .trim()
        .to_owned();
    if mount_point.is_empty() {
        bail!("Failed to create a private DMG mount directory");
    }
    Ok(mount_point)
}

fn remove_dmg_mount_point(mount_point: &str) {
    if let Err(err) = std::fs::remove_dir(mount_point) {
        if err.kind() != io::ErrorKind::NotFound {
            log::warn!(
                "Failed to remove DMG mount directory {}: {}",
                mount_point,
                err
            );
        }
    }
}

pub(super) fn verified_dmg_path(verified_dmg: &VerifiedDmg) -> ResultType<&str> {
    use std::os::unix::fs::MetadataExt as _;

    let path_metadata = std::fs::symlink_metadata(&verified_dmg.path)?;
    let file_metadata = verified_dmg.file.metadata()?;
    if !path_metadata.is_file()
        || path_metadata.dev() != file_metadata.dev()
        || path_metadata.ino() != file_metadata.ino()
    {
        bail!("Verified DMG path changed: {}", verified_dmg.path.display());
    }
    verified_dmg
        .path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid verified DMG path: {}", verified_dmg.path.display()))
}

pub(super) fn verify_stored_dmg(verified_dmg: &VerifiedDmg) -> ResultType<()> {
    let mut file = verified_dmg.file.try_clone()?;
    verify_dmg_contents(
        &mut file,
        &verified_dmg.expected_sha256,
        &verified_dmg.path.to_string_lossy(),
    )
}

pub(super) fn extract_dmg_inner(dmg_path: &str, target_dir: &str) -> ResultType<()> {
    let guard = attach_dmg(dmg_path)?;
    extract_attached_dmg(guard, target_dir)
}

fn extract_attached_dmg(guard: DmgGuard, target_dir: &str) -> ResultType<()> {
    let mount_point = guard.mount_point();
    let app_name = format!("{}.app", crate::get_app_name());
    let src_path = format!("{}/{}", mount_point, app_name);
    let dest_path = format!("{}/{}", target_dir, app_name);
    let copy_status = Command::new("/usr/bin/ditto")
        .args(&[&src_path, &dest_path])
        .status()?;

    if !copy_status.success() {
        bail!(
            "Failed to copy application from {} to {}: {:?}",
            src_path,
            dest_path,
            copy_status
        );
    }
    if !Path::new(&dest_path).exists() {
        bail!(
            "Copy operation failed - destination not found at {}",
            dest_path
        );
    }
    Ok(())
}
