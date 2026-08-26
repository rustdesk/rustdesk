#[cfg(target_os = "macos")]
use hbb_common::{anyhow::Context, log};
use hbb_common::{bail, ResultType};
#[cfg(target_os = "macos")]
use std::{io::Read, path::Path};

#[cfg(target_os = "macos")]
const UPDATE_SIDECAR_MAX_BYTES: u64 = 1024 * 1024;
#[cfg(any(target_os = "macos", test))]
const PUBLIC_MACOS_BUNDLE_ID: &str = "com.carriez.rustdesk";
const MACOS_UPDATE_USAGE: &str = "Usage: --update [update.dmg --metadata rustdesk-update.json --signature rustdesk-update.json.sig]";
#[cfg(target_os = "macos")]
const LEGACY_UPDATE_WARNING: &str =
    "legacy offline update: signed metadata not found; release hash was not verified";

#[derive(Debug, Eq, PartialEq)]
enum MacosUpdateArguments<'a> {
    CurrentApp,
    LegacyDmg(&'a str),
    VerifiedDmg {
        dmg_path: &'a str,
        metadata_path: &'a str,
        signature_path: &'a str,
    },
}

fn parse_macos_update_args(args: &[String]) -> ResultType<MacosUpdateArguments<'_>> {
    match args {
        [command] if command == "--update" => Ok(MacosUpdateArguments::CurrentApp),
        [command, dmg_path] if command == "--update" && dmg_path.ends_with(".dmg") => {
            Ok(MacosUpdateArguments::LegacyDmg(dmg_path))
        }
        [command, dmg_path, metadata_flag, metadata_path, signature_flag, signature_path]
            if command == "--update"
                && dmg_path.ends_with(".dmg")
                && metadata_flag == "--metadata"
                && signature_flag == "--signature" =>
        {
            Ok(MacosUpdateArguments::VerifiedDmg {
                dmg_path,
                metadata_path,
                signature_path,
            })
        }
        _ => bail!(MACOS_UPDATE_USAGE),
    }
}

#[cfg(any(target_os = "macos", test))]
fn expected_update_package_id(
    is_custom_client: bool,
    bundle_id: Option<String>,
) -> ResultType<String> {
    let bundle_id = bundle_id
        .filter(|value| !value.is_empty())
        .ok_or_else(|| hbb_common::anyhow::anyhow!("client bundle identifier is missing"))?;
    if !is_custom_client && bundle_id == PUBLIC_MACOS_BUNDLE_ID {
        return Ok(crate::update_metadata::PUBLIC_UPDATE_PACKAGE_ID.to_owned());
    }
    Ok(bundle_id)
}

#[cfg(target_os = "macos")]
fn installed_update_package_id() -> ResultType<String> {
    expected_update_package_id(
        crate::common::is_custom_client(),
        crate::platform::macos::get_bundle_id(),
    )
}

#[cfg(target_os = "macos")]
fn read_update_sidecar(path: &str) -> ResultType<Vec<u8>> {
    let path_metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect update sidecar {path}"))?;
    if !path_metadata.file_type().is_file() {
        bail!("update sidecar is not a regular file: {path}");
    }
    if path_metadata.len() > UPDATE_SIDECAR_MAX_BYTES {
        bail!("update sidecar exceeds maximum size: {path}");
    }
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open update sidecar {path}"))?;
    let mut bytes = Vec::new();
    file.take(UPDATE_SIDECAR_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > UPDATE_SIDECAR_MAX_BYTES {
        bail!("update sidecar exceeds maximum size: {path}");
    }
    Ok(bytes)
}

#[cfg(target_os = "macos")]
fn update_current_app() -> ResultType<()> {
    println!("Starting update process...");
    log::info!("Starting update process...");
    crate::platform::update_me()?;
    println!(
        "{}",
        crate::client::translate("Updated successfully!".to_owned())
    );
    log::info!("Updated successfully!");
    Ok(())
}

#[cfg(target_os = "macos")]
fn update_legacy_dmg(dmg_path: &str) -> ResultType<()> {
    eprintln!("Warning: {LEGACY_UPDATE_WARNING}");
    hbb_common::log::warn!("{LEGACY_UPDATE_WARNING}");
    println!("Updating from DMG: {dmg_path}");
    crate::platform::update_from_dmg(dmg_path)?;
    println!("Update process from DMG started successfully.");
    Ok(())
}

#[cfg(target_os = "macos")]
fn update_verified_dmg(
    dmg_path: &str,
    metadata_path: &str,
    signature_path: &str,
) -> ResultType<()> {
    let file_name = Path::new(dmg_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| hbb_common::anyhow::anyhow!("invalid update DMG file name"))?;
    let metadata = read_update_sidecar(metadata_path)?;
    let signature = read_update_sidecar(signature_path)?;
    let expected_package_id = installed_update_package_id()?;
    let artifact = crate::update_metadata::verify_offline_update_metadata(
        &metadata,
        &signature,
        crate::update_metadata::OfflineUpdateMetadataRequirements {
            local_file_name: file_name,
            expected_package_id: &expected_package_id,
            artifact: crate::updater::current_update_artifact_query(false),
        },
    )?;
    println!("Updating from DMG verified by signed metadata: {dmg_path}");
    crate::platform::macos::update_to_verified_dmg(
        dmg_path,
        &artifact.sha256,
        Some(artifact.size),
    )?;
    println!("Verified update process started");
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn execute_macos_update(args: &[String]) -> ResultType<()> {
    match parse_macos_update_args(args)? {
        MacosUpdateArguments::CurrentApp => update_current_app(),
        MacosUpdateArguments::LegacyDmg(dmg_path) => update_legacy_dmg(dmg_path),
        MacosUpdateArguments::VerifiedDmg {
            dmg_path,
            metadata_path,
            signature_path,
        } => update_verified_dmg(dmg_path, metadata_path, signature_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parses_current_legacy_and_verified_macos_updates() {
        assert_eq!(
            MacosUpdateArguments::CurrentApp,
            parse_macos_update_args(&args(&["--update"])).unwrap()
        );
        assert_eq!(
            MacosUpdateArguments::LegacyDmg("update.dmg"),
            parse_macos_update_args(&args(&["--update", "update.dmg"])).unwrap()
        );
        assert_eq!(
            MacosUpdateArguments::VerifiedDmg {
                dmg_path: "update.dmg",
                metadata_path: "rustdesk-update.json",
                signature_path: "rustdesk-update.json.sig",
            },
            parse_macos_update_args(&args(&[
                "--update",
                "update.dmg",
                "--metadata",
                "rustdesk-update.json",
                "--signature",
                "rustdesk-update.json.sig",
            ]))
            .unwrap()
        );
    }

    #[test]
    fn rejects_partial_reordered_or_extra_macos_update_arguments() {
        for invalid in [
            args(&["--update", "update.zip"]),
            args(&["--update", "update.dmg", "--metadata", "update.json"]),
            args(&[
                "--update",
                "update.dmg",
                "--signature",
                "update.sig",
                "--metadata",
                "update.json",
            ]),
            args(&["--update", "update.dmg", "unexpected"]),
        ] {
            assert!(parse_macos_update_args(&invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn resolves_public_and_custom_update_package_identity() {
        assert_eq!(
            "rustdesk",
            expected_update_package_id(false, Some("com.carriez.rustdesk".to_owned())).unwrap()
        );
        assert_eq!(
            "com.example.rustdesk-custom",
            expected_update_package_id(true, Some("com.example.rustdesk-custom".to_owned()))
                .unwrap()
        );
        assert_eq!(
            "com.example.same-name-custom",
            expected_update_package_id(false, Some("com.example.same-name-custom".to_owned()))
                .unwrap()
        );
        assert_eq!(
            "com.carriez.rustdesk",
            expected_update_package_id(true, Some("com.carriez.rustdesk".to_owned())).unwrap()
        );
        assert!(expected_update_package_id(false, None).is_err());
        assert!(expected_update_package_id(true, None).is_err());
        assert!(expected_update_package_id(true, Some(String::new())).is_err());
    }
}
