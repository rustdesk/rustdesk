use std::{ffi::OsString, io};
use winreg::{enums::HKEY_CLASSES_ROOT, RegKey};

const FIREWALL_RULES: &str = "FIREWALLRULES";

pub(super) fn install_commands(
    exe: &str,
    app_name: &str,
    args: impl Iterator<Item = OsString>,
) -> io::Result<String> {
    // Resolve the saved choice before the reinstall script deletes its registry key.
    let enabled = resolve(args, || read_saved(app_name))?;
    Ok(commands(exe, app_name, enabled))
}

fn resolve(
    args: impl Iterator<Item = OsString>,
    read_saved: impl FnOnce() -> io::Result<Option<String>>,
) -> io::Result<bool> {
    let override_value = args
        .filter_map(|arg| {
            let arg = arg.to_string_lossy();
            let (name, value) = arg.split_once('=')?;
            (name == FIREWALL_RULES).then(|| value.to_owned())
        })
        .last();
    let value = match override_value {
        Some(value) => Some(value),
        None => read_saved()?,
    };
    match value.as_deref() {
        None | Some("1") => Ok(true),
        Some("0") => Ok(false),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "FIREWALLRULES must be 0 or 1",
        )),
    }
}

fn read_saved(app_name: &str) -> io::Result<Option<String>> {
    let result = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(format!(".{}", app_name.to_lowercase()))
        .and_then(|key| key.get_value(FIREWALL_RULES));
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!("Failed to read saved {FIREWALL_RULES}: {error}"),
        )),
    }
}

fn commands(exe: &str, app_name: &str, enabled: bool) -> String {
    let ext = app_name.to_lowercase();
    let value = if enabled { "1" } else { "0" };
    let rules = if enabled {
        format!(
            "netsh advfirewall firewall add rule name=\"{app_name} Service\" dir=out action=allow program=\"{exe}\" enable=yes\n\
             netsh advfirewall firewall add rule name=\"{app_name} Service\" dir=in action=allow program=\"{exe}\" enable=yes"
        )
    } else {
        String::new()
    };
    format!(
        "reg add HKEY_CLASSES_ROOT\\.{ext} /f /v {FIREWALL_RULES} /t REG_SZ /d \"{value}\"\n\
         if not \"%errorlevel%\"==\"0\" exit /b %errorlevel%\n\
         {rules}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, io};

    #[test]
    fn firewall_commands_follow_installation_choice() {
        let exe = r"C:\Program Files\RustDesk\RustDesk.exe";
        let generate = |value: &str| {
            install_commands(exe, "RustDesk", [OsString::from(value)].into_iter()).unwrap()
        };
        let disabled = generate("FIREWALLRULES=0");
        assert!(disabled.contains("/v FIREWALLRULES /t REG_SZ /d \"0\""));
        assert!(!disabled.contains("netsh"));
        let enabled = generate("FIREWALLRULES=1");
        assert!(enabled.contains("/v FIREWALLRULES /t REG_SZ /d \"1\""));
        assert!(enabled.contains("dir=out action=allow"));
        assert!(enabled.contains("dir=in action=allow"));
        assert!(enabled.contains(&format!("program=\"{exe}\"")));
    }

    #[test]
    fn firewall_choice_preserves_saved_value_and_allows_overrides() {
        for (argument, saved, expected) in [
            (None, None, true),
            (None, Some("0"), false),
            (None, Some("1"), true),
            (Some("FIREWALLRULES=0"), None, false),
            (Some("FIREWALLRULES=0"), Some("1"), false),
            (Some("FIREWALLRULES=1"), Some("0"), true),
        ] {
            let args = argument.into_iter().map(OsString::from);
            assert_eq!(
                resolve(args, || Ok(saved.map(str::to_owned))).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn firewall_choice_reports_invalid_values_and_registry_errors() {
        let args = [OsString::from("FIREWALLRULES=invalid")];
        assert!(resolve(args.into_iter(), || Ok(None)).is_err());
        assert!(resolve(std::iter::empty(), || Ok(Some("invalid".into()))).is_err());
        let error = resolve(std::iter::empty(), || {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
