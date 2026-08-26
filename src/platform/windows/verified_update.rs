mod file;
mod signature;

use super::{
    get_custom_client_staging_dir, handle_custom_client_staging_dir_before_update,
    remove_custom_client_staging_dir, run_uac, update_me_msi, ResultType,
};
use hbb_common::{allow_err, anyhow::anyhow, bail, log};
use std::path::Path;

pub use file::{copy_and_verify_update_file_sha256, VerifiedUpdateFile};

const VERIFIED_UPDATE_FILE_PREFIX: &str = "rustdesk-verified-";

pub fn update_to_verified(file: &str, expected_sha256: &str, expected_size: u64) -> ResultType<()> {
    let extension = file::update_file_extension(file).unwrap_or_default();
    if extension != "exe" && extension != "msi" {
        bail!("Unsupported update file format: {}", file);
    }

    let update_file = copy_and_verify_update_file_sha256(file, expected_sha256)?;
    if let Err(err) = update_file.verify_size(expected_size) {
        update_file.cleanup();
        return Err(err);
    }
    let update_file = update_file.verify_authenticode()?;
    let update_path = match update_file.path_str() {
        Ok(path) => path.to_owned(),
        Err(err) => {
            update_file.cleanup();
            return Err(err);
        }
    };
    let custom_client_staging_dir = get_custom_client_staging_dir();
    if crate::is_custom_client() {
        if let Err(err) = handle_custom_client_staging_dir_before_update(&custom_client_staging_dir)
        {
            update_file.cleanup();
            return Err(err);
        }
    } else {
        // Clean up any residual staging directory from previous custom client.
        allow_err!(remove_custom_client_staging_dir(&custom_client_staging_dir));
    }

    let result = launch_verified_update(&extension, &update_path);
    clear_custom_client_staging_after_launch_failure(&custom_client_staging_dir, &result);
    finish_verified_update_launch(update_file, &extension, result)
}

impl VerifiedUpdateFile {
    pub(crate) fn verify_authenticode(self) -> ResultType<Self> {
        match self
            .path_str()
            .and_then(|path| signature::verify_authenticode_signature(Path::new(path)))
        {
            Ok(()) => Ok(self),
            Err(err) => {
                self.cleanup();
                Err(err)
            }
        }
    }
}

fn launch_verified_update(extension: &str, update_path: &str) -> ResultType<()> {
    match extension {
        "exe" => {
            if !run_uac(update_path, "--update")? {
                bail!(
                    "Failed to run the update exe with UAC, error: {:?}",
                    std::io::Error::last_os_error()
                );
            }
        }
        "msi" => {
            if let Err(err) = update_me_msi(update_path, false) {
                bail!("Failed to run the update msi: {}", err);
            }
        }
        _ => bail!("Unsupported update file format: {}", update_path),
    }
    Ok(())
}

fn clear_custom_client_staging_after_launch_failure(
    staging_dir: &Path,
    launch_result: &ResultType<()>,
) {
    if launch_result.is_ok() {
        return;
    }
    if let Err(err) = remove_custom_client_staging_dir(staging_dir) {
        log::warn!(
            "Failed to clear custom client staging directory {:?}: {}",
            staging_dir,
            err
        );
    }
}

// MSI execution waits for installation to finish, so its verified copy can be
// removed. A successfully launched EXE still needs its copy while it starts.
pub fn finish_verified_update_launch(
    update_file: VerifiedUpdateFile,
    extension: &str,
    result: ResultType<()>,
) -> ResultType<()> {
    if result.is_err() || extension == "msi" {
        update_file.cleanup();
    }
    result
}

pub fn schedule_current_verified_update_file_cleanup() -> ResultType<()> {
    let current_exe = std::env::current_exe()?;
    if should_schedule_verified_update_cleanup(&current_exe) {
        schedule_file_removal_on_reboot(&current_exe)?;
    }
    Ok(())
}

fn should_schedule_verified_update_cleanup(path: &Path) -> bool {
    // UAC may run under another account whose TEMP directory differs from the caller's.
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(VERIFIED_UPDATE_FILE_PREFIX) && name.ends_with(".exe"))
}

fn schedule_file_removal_on_reboot(path: &Path) -> ResultType<()> {
    use windows::{
        core::{HSTRING, PCWSTR},
        Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT},
    };

    unsafe {
        MoveFileExW(
            &HSTRING::from(path),
            PCWSTR::null(),
            MOVEFILE_DELAY_UNTIL_REBOOT,
        )
    }
    .map_err(|err| {
        anyhow!(
            "Failed to schedule verified update file cleanup for {}: {}",
            path.display(),
            err
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_cleanup_for_verified_exe_only() {
        assert!(should_schedule_verified_update_cleanup(Path::new(
            r"C:\Temp\rustdesk-verified-123-456.exe"
        )));
        for path in [
            r"C:\Temp\rustdesk-verified-123-456.msi",
            r"C:\Temp\rustdesk.exe",
            r"C:\Temp\rustdesk-verified-123-456.exe.bak",
        ] {
            assert!(!should_schedule_verified_update_cleanup(Path::new(path)));
        }
    }

    #[test]
    fn finishing_successful_msi_update_removes_verified_copy() {
        let source_path = std::env::temp_dir().join(format!(
            "rustdesk-update-msi-cleanup-test-{}-{}.msi",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        std::fs::write(&source_path, b"rustdesk").unwrap();
        let verified_file = copy_and_verify_update_file_sha256(
            source_path.to_str().unwrap(),
            "304ca1638c5effa6832e0e15b958a8f74847efe4df9c3f3187216e921c168fed",
        )
        .unwrap();
        let verified_path = Path::new(verified_file.path_str().unwrap()).to_owned();
        assert!(verified_path.exists());

        assert!(finish_verified_update_launch(verified_file, "msi", Ok(())).is_ok());

        assert!(!verified_path.exists());
        std::fs::remove_file(source_path).unwrap();
    }

    #[test]
    fn failed_verified_update_launch_clears_staged_custom_client_file() {
        let staging_dir = std::env::temp_dir().join(format!(
            "rustdesk-update-staging-cleanup-test-{}-{}",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        std::fs::create_dir_all(&staging_dir).unwrap();
        let staged_file = staging_dir.join("custom.txt");
        std::fs::write(&staged_file, b"custom client").unwrap();
        let successful_launch: ResultType<()> = Ok(());
        clear_custom_client_staging_after_launch_failure(&staging_dir, &successful_launch);
        assert!(staged_file.exists());
        let launch_result: ResultType<()> = Err(hbb_common::anyhow::anyhow!("launch failed"));

        clear_custom_client_staging_after_launch_failure(&staging_dir, &launch_result);

        assert!(!staged_file.exists());
        std::fs::remove_dir_all(staging_dir).unwrap();
    }
}
