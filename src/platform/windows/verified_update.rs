mod file;

use super::{
    get_custom_client_staging_dir, handle_custom_client_staging_dir_before_update,
    installer_handoff::run_cmds, installer_shell::path_for_cmd_environment,
    remove_custom_client_staging_dir, ResultType,
};
use hbb_common::{allow_err, anyhow::anyhow, bail, log};
use std::path::Path;

pub use file::{copy_and_verify_update_file_sha256, VerifiedUpdateFile};

const VERIFIED_UPDATE_FILE_PREFIX: &str = "rustdesk-verified-";
const VERIFIED_EXE_FILENAME: &str = "update.exe";
const VERIFIED_MSI_FILENAME: &str = "update.msi";
const VERIFIED_UPDATE_COPY_FAILURE_EXIT_CODE: u32 = 0x5253_0009;
const VERIFIED_UPDATE_HASH_FAILURE_EXIT_CODE: u32 = 0x5253_000A;
const VERIFIED_UPDATE_HASH_MISMATCH_EXIT_CODE: u32 = 0x5253_000B;
const VERIFIED_EXE_LAUNCH_FAILURE_EXIT_CODE: u32 = 0x5253_000C;
const SHA256_HASH_LENGTH: usize = 32;

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

    let result = launch_verified_update(&extension, &update_path, expected_sha256);
    clear_custom_client_staging_after_launch_failure(&custom_client_staging_dir, &result);
    finish_verified_update_launch(update_file, result)
}

fn launch_verified_update(
    extension: &str,
    update_path: &str,
    expected_sha256: &str,
) -> ResultType<()> {
    match extension {
        "exe" => {
            if let Err(err) = launch_verified_exe(update_path, expected_sha256) {
                bail!("Failed to run the update exe: {}", err);
            }
        }
        "msi" => {
            if let Err(err) = install_verified_msi(update_path, expected_sha256, false) {
                bail!("Failed to run the update msi: {}", err);
            }
        }
        _ => bail!("Unsupported update file format: {}", update_path),
    }
    Ok(())
}

fn verified_update_hash_pattern(expected_sha256: &str) -> ResultType<String> {
    let hash = hex::decode(expected_sha256.trim())
        .map_err(|err| anyhow!("Invalid expected update SHA-256: {err}"))?;
    if hash.len() != SHA256_HASH_LENGTH {
        bail!("Invalid expected update SHA-256 length");
    }
    // FINDSTR /R /I /X /C matches the complete certutil hash line; ` *` allows
    // optional spaces between byte pairs.
    Ok(hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" *"))
}

fn launch_verified_exe(exe: &str, expected_sha256: &str) -> ResultType<()> {
    let source = path_for_cmd_environment(Path::new(exe))?;
    let hash_pattern = verified_update_hash_pattern(expected_sha256)?;
    // Keep the hash outside the EXE directory because update_me() copies that
    // entire directory into the installation.
    let commands = format!(
        "set \"RUSTDESK_VERIFIED_EXE_SOURCE={source}\"\r\n\
         set \"RUSTDESK_VERIFIED_EXE=%RUSTDESK_OUTPUT_DIR%\\{VERIFIED_EXE_FILENAME}\"\r\n\
         copy /B /Y \"%RUSTDESK_VERIFIED_EXE_SOURCE%\" \"%RUSTDESK_VERIFIED_EXE%\" > nul || exit /b {VERIFIED_UPDATE_COPY_FAILURE_EXIT_CODE}\r\n\
         certutil.exe -hashfile \"%RUSTDESK_VERIFIED_EXE%\" SHA256 > \"%~f0.exe.sha256\" || exit /b {VERIFIED_UPDATE_HASH_FAILURE_EXIT_CODE}\r\n\
         findstr.exe /R /I /X /C:\"{hash_pattern}\" \"%~f0.exe.sha256\" > nul || exit /b {VERIFIED_UPDATE_HASH_MISMATCH_EXIT_CODE}\r\n\
         start \"\" /wait \"%RUSTDESK_VERIFIED_EXE%\" --update || exit /b {VERIFIED_EXE_LAUNCH_FAILURE_EXIT_CODE}"
    );
    run_cmds(commands, false, "update-exe")
}

fn verified_msi_install_commands(
    msi: &Path,
    expected_sha256: &str,
    quiet: bool,
) -> ResultType<String> {
    let source = path_for_cmd_environment(msi)?;
    let hash_pattern = verified_update_hash_pattern(expected_sha256)?;
    let quiet_args = if quiet { " /qn LAUNCH_TRAY_APP=N" } else { "" };
    Ok(format!(
        "set \"RUSTDESK_VERIFIED_MSI_SOURCE={source}\"\r\n\
         set \"RUSTDESK_VERIFIED_MSI=%RUSTDESK_OUTPUT_DIR%\\{VERIFIED_MSI_FILENAME}\"\r\n\
         copy /B /Y \"%RUSTDESK_VERIFIED_MSI_SOURCE%\" \"%RUSTDESK_VERIFIED_MSI%\" > nul || exit /b {VERIFIED_UPDATE_COPY_FAILURE_EXIT_CODE}\r\n\
         certutil.exe -hashfile \"%RUSTDESK_VERIFIED_MSI%\" SHA256 > \"%RUSTDESK_VERIFIED_MSI%.sha256\" || exit /b {VERIFIED_UPDATE_HASH_FAILURE_EXIT_CODE}\r\n\
         findstr.exe /R /I /X /C:\"{hash_pattern}\" \"%RUSTDESK_VERIFIED_MSI%.sha256\" > nul || exit /b {VERIFIED_UPDATE_HASH_MISMATCH_EXIT_CODE}\r\n\
         msiexec.exe /i \"%RUSTDESK_VERIFIED_MSI%\"{quiet_args} REBOOT=ReallySuppress /norestart"
    ))
}

pub(super) fn install_verified_msi(
    msi: &str,
    expected_sha256: &str,
    quiet: bool,
) -> ResultType<()> {
    run_cmds(
        verified_msi_install_commands(Path::new(msi), expected_sha256, quiet)?,
        false,
        "update-msi",
    )
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

// Each caller waits until the installer no longer needs the verified source.
pub fn finish_verified_update_launch(
    update_file: VerifiedUpdateFile,
    result: ResultType<()>,
) -> ResultType<()> {
    update_file.cleanup();
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

        assert!(finish_verified_update_launch(verified_file, Ok(())).is_ok());

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
