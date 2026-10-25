use super::config;
use hbb_common::{
    anyhow::Context,
    config::{store_path, Config, Config2},
    toml, ResultType,
};
use std::{fs, io::ErrorKind, path::Path};

#[cfg(any(target_os = "linux", target_os = "macos", test))]
const PENDING_REVOCATION_EXTENSION: &str = "trusted-device-revocation";

#[cfg(target_os = "macos")]
pub(super) fn is_stopped() -> ResultType<bool> {
    if !super::authority::uses_service()? {
        return Ok(false);
    }
    // "Start" resumes incoming connections without reinstalling the daemon.
    Path::new(&format!(
        "/Library/LaunchDaemons/{}_service.plist",
        crate::get_full_name()
    ))
    .try_exists()
    .map(|registered| !registered)
    .context("Inspect stopped system service registration")
}

#[cfg(target_os = "linux")]
pub(super) fn is_stopped() -> ResultType<bool> {
    if Config::get_option("stop-service") != "Y" || !super::authority::uses_service()? {
        return Ok(false);
    }
    let output = std::process::Command::new("systemctl")
        .args(["show", "--property=ActiveState", "--value"])
        .arg(format!("{}.service", crate::get_app_name().to_lowercase()))
        .output()
        .context("Inspect stopped system service")?;
    if !output.status.success() {
        hbb_common::bail!("Cannot inspect system service state: {}", output.status);
    }
    Ok(std::str::from_utf8(&output.stdout)?.trim() == "inactive")
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn read_pending_revocation(path: &Path) -> ResultType<Option<Vec<u8>>> {
    match fs::read(path.with_extension(PENDING_REVOCATION_EXTENSION)) {
        // Presence requests revocation; the contents only distinguish overlapping edits.
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("Read pending trusted-device revocation"),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(super) fn pending_revocation() -> ResultType<Option<Vec<u8>>> {
    let _storage = super::storage()?;
    read_pending_revocation(&Config2::file())
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(super) fn update_stopped_auth<T>(apply: impl FnOnce() -> ResultType<T>) -> ResultType<T> {
    use std::io::Write;
    if super::storage()?.exists()? {
        config::snapshot()?;
    }
    let result = apply()?;
    config::snapshot()?;
    let _storage = super::storage()?;
    let pending = super::random_token()?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(Config2::file().with_extension(PENDING_REVOCATION_EXTENSION))
        .context("Record pending trusted-device revocation")?;
    file.write_all(&pending)?;
    file.sync_all()?;
    Ok(result)
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(super) fn acknowledge_revocation(expected: &[u8]) -> ResultType<()> {
    let _storage = super::storage()?;
    // A newer offline edit must survive acknowledgment of an earlier upload.
    if read_pending_revocation(&Config2::file())?.as_deref() == Some(expected) {
        fs::remove_file(Config2::file().with_extension(PENDING_REVOCATION_EXTENSION))
            .context("Acknowledge pending trusted-device revocation")?;
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) async fn revoke_pending_auth() -> ResultType<Option<Vec<u8>>> {
    let pending = config::update(pending_revocation).await?;
    if pending.is_some()
        && !matches!(
            request(super::authority::Request::Clear).await?,
            super::authority::Response::Done
        )
    {
        hbb_common::bail!("The system service did not confirm trusted-device revocation");
    }
    Ok(pending)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub async fn sync_pending_auth() -> ResultType<()> {
    if config::update(pending_revocation).await?.is_some() {
        config::sync_auth_to_service().await?;
        if config::update(pending_revocation).await?.is_some() {
            hbb_common::bail!(
                "Authentication settings changed during service synchronization; retry"
            );
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub async fn request(request: super::authority::Request) -> ResultType<super::authority::Response> {
    use super::authority::{Response, Unavailable};
    use crate::ipc::{connect_trusted_device_service, Data};
    use hbb_common::anyhow::anyhow;
    const TIMEOUT_MS: u64 = 1000;
    let mut connection = connect_trusted_device_service(TIMEOUT_MS).await?;
    connection
        .send(&Data::TrustedDeviceRequest(Box::new(request)))
        .await
        .map_err(Unavailable)?;
    match connection
        .next_timeout(TIMEOUT_MS)
        .await
        .map_err(Unavailable)?
    {
        Some(Data::TrustedDeviceResponse(Ok(Response::Unavailable(error)))) => {
            Err(Unavailable(anyhow!(error)).into())
        }
        Some(Data::TrustedDeviceResponse(result)) => result.map_err(|error| anyhow!(error)),
        _ => Err(Unavailable(anyhow!(
            "The system service did not confirm the trusted-device operation"
        ))
        .into()),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub async fn handle_request(
    request: super::authority::Request,
) -> Result<super::authority::Response, String> {
    use super::authority::{self, Response, Unavailable};
    match config::update(move || authority::handle(request)).await {
        Err(error) if error.is::<Unavailable>() => Ok(Response::Unavailable(error.to_string())),
        result => result.map_err(|error| error.to_string()),
    }
}

pub fn sync_config(value: Config, options: Config2) -> ResultType<()> {
    // A retained user session may still contain grants revoked by another user.
    config::set_config(value)?;
    config::set_config2(options)?;
    config::snapshot()?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn sync_from_service(value: Config, options: Config2) -> ResultType<()> {
    let _storage = super::storage()?;
    Config::set(value);
    Config2::set(options.clone());
    config::snapshot()?;
    options.sync_trusted_devices()
}

pub fn copy_config(source: &Path, destination: &Path) -> ResultType<()> {
    let mut incoming: toml::value::Table =
        toml::from_str(&fs::read_to_string(source).context("Read source service configuration")?)?;
    match fs::read_to_string(destination) {
        Ok(value) => {
            let current: toml::value::Table = toml::from_str(&value)?;
            // An empty existing store is authoritative too. Only first install imports grants.
            for field in ["trusted_devices", "trusted_devices_v2"] {
                incoming.remove(field);
                if let Some(value) = current.get(field) {
                    incoming.insert(field.into(), value.clone());
                }
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("Read destination service configuration"),
    }
    #[cfg(any(target_os = "linux", target_os = "macos", test))]
    if read_pending_revocation(source)?.is_some() {
        incoming.remove("trusted_devices");
        incoming.remove("trusted_devices_v2");
    }
    let incoming: Config2 = toml::Value::Table(incoming).try_into()?;
    store_path(destination.to_path_buf(), incoming)
}
