use hbb_common::{anyhow::Context, bail, ResultType};
use std::path::{Path, PathBuf};

const ADAPTER_FILE: &str = "printer_driver_adapter.dll";

pub(crate) fn installed() -> ResultType<bool> {
    if !crate::is_custom_client() || !crate::platform::is_installed() {
        return Ok(false);
    }
    if !crate::platform::is_msi_installed()? {
        return Ok(false);
    }
    let app_name = crate::get_app_name();
    Ok(remote_printer::pdf::installed(&app_name)?.is_none()
        && remote_printer::is_rd_printer_installed(&app_name)?)
}

pub(crate) fn adapter_path(executable: &Path) -> ResultType<PathBuf> {
    let path = executable.with_file_name(ADAPTER_FILE);
    let metadata = std::fs::metadata(&path)
        .with_context(|| format!("The existing XPS printer requires {}", path.display()))?;
    if !metadata.is_file() {
        bail!("The XPS printer adapter is not a file: {}", path.display());
    }
    Ok(path)
}

pub(crate) fn ready() -> ResultType<bool> {
    if !installed()? {
        return Ok(false);
    }
    let (_, _, _, executable) = crate::platform::get_install_info();
    adapter_path(Path::new(&executable))?;
    Ok(true)
}

pub(crate) fn update_command(
    executable: &str,
    installed: bool,
    legacy_installed: impl FnOnce() -> ResultType<bool>,
) -> ResultType<String> {
    // EXE updates cannot replace the legacy MSI's embedded printer uninstall action.
    if installed && !legacy_installed()? {
        Ok(format!("\"{executable}\" --install-remote-printer"))
    } else {
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_update_preserves_the_legacy_msi_printer() {
        let executable = r"C:\Program Files\Custom Client\Custom Client.exe";
        assert_eq!(update_command(executable, true, || Ok(true)).unwrap(), "");
        assert_eq!(
            update_command(executable, true, || Ok(false)).unwrap(),
            format!("\"{executable}\" --install-remote-printer")
        );
        assert_eq!(update_command(executable, false, || Ok(false)).unwrap(), "");
    }

    #[test]
    fn update_without_a_printer_does_not_require_the_spooler() {
        let unavailable = || -> ResultType<bool> { bail!("Spooler is unavailable") };
        assert_eq!(
            update_command("client.exe", false, unavailable).unwrap(),
            ""
        );
        assert_eq!(
            update_command("client.exe", true, unavailable)
                .unwrap_err()
                .to_string(),
            "Spooler is unavailable"
        );
    }

    #[test]
    fn missing_legacy_adapter_is_an_error() {
        let executable = std::env::temp_dir()
            .join(format!("legacy-printer-{}", uuid::Uuid::new_v4()))
            .join("Custom Client.exe");
        let error = adapter_path(&executable).unwrap_err();
        assert!(error.to_string().contains(ADAPTER_FILE));
    }
}
