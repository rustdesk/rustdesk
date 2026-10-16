use hbb_common::{
    anyhow::{anyhow, Context},
    base64::{engine::general_purpose::STANDARD, Engine as _},
    ResultType,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) const VERIFY_FILE_ARGUMENT: &str = "--verify-install-file";
pub(super) const DECODE_SHORTCUT_ARGUMENT: &str = "--decode-install-shortcut";

pub(super) fn helper_path() -> ResultType<String> {
    // Resolve mapped drives before UAC switches logon sessions.
    let executable = resolve_mapped_drive(&std::env::current_exe()?)?;
    let path = executable
        .to_str()
        .ok_or_else(|| anyhow!("Installer helper path is not valid Unicode"))?;
    if path.contains(['\0', '"', '\r', '\n']) {
        return Err(anyhow!(
            "Installer helper path is unsafe for cmd.exe: {path}"
        ));
    }
    Ok(path.to_owned())
}

fn resolve_mapped_drive(path: &Path) -> ResultType<PathBuf> {
    use std::path::{Component, Prefix};
    use windows::{core::PCWSTR, Win32::Storage::FileSystem::GetDriveTypeW};

    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Ok(path.to_owned());
    };
    let (Prefix::Disk(drive) | Prefix::VerbatimDisk(drive)) = prefix.kind() else {
        return Ok(path.to_owned());
    };
    let root = super::wide_string(&format!("{}:\\", drive as char));
    if unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } != winapi::um::winbase::DRIVE_REMOTE {
        return Ok(path.to_owned());
    }
    // Query only the mapping; normalizing SMB components can require extra access.
    let remote = mapped_drive_root(drive)?;
    Ok(remote.join(components.as_path().strip_prefix(r"\")?))
}

fn mapped_drive_root(drive: u8) -> ResultType<PathBuf> {
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::{
            Foundation::{ERROR_MORE_DATA, MAX_PATH},
            NetworkManagement::WNet::WNetGetConnectionW,
        },
    };

    let device = super::wide_string(&format!("{}:", drive as char));
    let mut length = MAX_PATH;
    let mut buffer = vec![0; length as usize];
    let mut result = unsafe {
        WNetGetConnectionW(
            PCWSTR(device.as_ptr()),
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut length,
        )
    };
    if result == ERROR_MORE_DATA {
        buffer.resize(length as usize, 0);
        result = unsafe {
            WNetGetConnectionW(
                PCWSTR(device.as_ptr()),
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut length,
            )
        };
    }
    result.ok().with_context(|| {
        format!(
            "Failed to resolve installer helper drive {}:",
            drive as char
        )
    })?;
    let end = buffer
        .iter()
        .position(|&value| value == 0)
        .ok_or_else(|| anyhow!("Installer helper drive mapping is not terminated"))?;
    let remote = String::from_utf16(&buffer[..end])
        .context("Installer helper drive mapping is not valid Unicode")?;
    Ok(PathBuf::from(remote))
}

pub(crate) fn handle_command(args: &[String]) -> Option<ResultType<()>> {
    match args.first()?.as_str() {
        VERIFY_FILE_ARGUMENT => Some(match args {
            [_, source, expected_hash, receipt] => verify(source, expected_hash, receipt),
            _ => Err(anyhow!(
                "Expected a file, SHA-256 hash and verification receipt path"
            )),
        }),
        DECODE_SHORTCUT_ARGUMENT => Some(match args {
            [_, source, destination] => decode_shortcut(source, destination),
            _ => Err(anyhow!("Expected encoded shortcut and destination paths")),
        }),
        _ => None,
    }
}

fn verify(source: &str, expected_hash: &str, receipt: &str) -> ResultType<()> {
    let mut file = fs::File::open(source)
        .with_context(|| format!("Failed to open installer file {source}"))?;
    crate::update_hash::verify_sha256_reader(&mut file, expected_hash)
        .map_err(|error| anyhow!("Failed to verify installer file {source}: {error:?}"))?;
    // Startup can exit before dispatch; the caller requires proof that verification ran.
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(receipt)
        .with_context(|| format!("Failed to write verification receipt {receipt}"))?;
    Ok(())
}

fn decode_shortcut(source: &str, destination: &str) -> ResultType<()> {
    let encoded = fs::read_to_string(source)
        .with_context(|| format!("Failed to read encoded installer shortcut {source}"))?;
    let bytes = STANDARD
        .decode(encoded.trim())
        .context("Failed to decode installer shortcut")?;
    fs::write(destination, bytes)
        .with_context(|| format!("Failed to write installer shortcut {destination}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_paths_preserve_local_and_unc_paths() {
        let local = std::env::current_exe()
            .unwrap()
            .with_file_name(format!("missing-{}.exe", uuid::Uuid::new_v4()));
        for path in [
            local,
            PathBuf::from(r"\\localhost\rustdesk-test-share\restricted\rustdesk.exe"),
            PathBuf::from(r"\\?\UNC\localhost\rustdesk-test-share\restricted\rustdesk.exe"),
        ] {
            assert_eq!(resolve_mapped_drive(&path).unwrap(), path);
        }
    }
}
