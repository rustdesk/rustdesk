use super::{
    normalize_msi_product_code, ResultType, MSI_WINDOWS_INSTALLER_VALUE, REG_NAME_WINDOWS_INSTALLER,
};
use hbb_common::{anyhow::anyhow, bail, log};
use std::collections::BTreeSet;
use winreg::{enums::*, RegKey};

const REG_NAME_DISPLAY_NAME: &str = "DisplayName";
const UNINSTALL_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

pub(super) fn find_product_code(app_name: &str) -> ResultType<Option<String>> {
    let product_codes = find_product_codes_in_view(app_name, false)?
        .into_iter()
        .chain(find_product_codes_in_view(app_name, true)?)
        .collect::<BTreeSet<_>>();
    let mut product_codes = product_codes.into_iter();
    let product_code = product_codes.next();
    if product_codes.next().is_some() {
        bail!("Multiple native MSI uninstall entries were found for {app_name}");
    }
    Ok(product_code)
}

fn find_product_codes_in_view(app_name: &str, wow: bool) -> ResultType<Vec<String>> {
    let flags = KEY_READ
        | if wow {
            KEY_WOW64_32KEY
        } else {
            KEY_WOW64_64KEY
        };
    let view_name = if wow { "32-bit" } else { "64-bit" };
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let uninstall_key = match hklm.open_subkey_with_flags(UNINSTALL_SUBKEY, flags) {
        Ok(uninstall_key) => uninstall_key,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => bail!("Failed to open {view_name} MSI uninstall registry: {err}"),
    };
    let mut matches = Vec::new();

    for key_name in uninstall_key.enum_keys() {
        let key_name = match key_name {
            Ok(key_name) => key_name,
            Err(err) => {
                log::warn!("Skipping unreadable {view_name} MSI uninstall key name: {err}");
                continue;
            }
        };
        let Some(product_code) = normalize_msi_product_code(&key_name) else {
            continue;
        };
        let is_match = uninstall_key
            .open_subkey_with_flags(&key_name, flags)
            .map_err(|err| {
                anyhow!("Failed to open {view_name} MSI uninstall entry {key_name}: {err}")
            })
            .and_then(|entry| is_matching_entry(&entry, app_name, &key_name));
        if scanned_entry_matches(is_match) {
            matches.push(product_code);
        }
    }

    Ok(matches)
}

pub(super) fn scanned_entry_matches(result: ResultType<bool>) -> bool {
    match result {
        Ok(is_match) => is_match,
        Err(err) => {
            log::warn!("Skipping invalid MSI uninstall entry: {err}");
            false
        }
    }
}

pub(super) fn is_matching_entry(
    entry: &RegKey,
    app_name: &str,
    key_name: &str,
) -> ResultType<bool> {
    match entry.get_value::<u32, _>(REG_NAME_WINDOWS_INSTALLER) {
        Ok(value) if value == MSI_WINDOWS_INSTALLER_VALUE => {}
        Ok(_) => return Ok(false),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => bail!(
            "Failed to read {REG_NAME_WINDOWS_INSTALLER} from MSI uninstall entry {key_name}: {err}"
        ),
    }

    match entry.get_value::<String, _>(REG_NAME_DISPLAY_NAME) {
        Ok(display_name) => Ok(display_name.eq_ignore_ascii_case(app_name)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => bail!(
            "Failed to read {REG_NAME_DISPLAY_NAME} from MSI uninstall entry {key_name}: {err}"
        ),
    }
}
