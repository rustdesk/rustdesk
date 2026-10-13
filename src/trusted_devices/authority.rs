use super::{DeviceInfo, Ticket, TOKEN_BYTES};
use hbb_common::{bail, get_time, ResultType};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Unavailable(pub hbb_common::anyhow::Error);

impl std::fmt::Display for Unavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl std::error::Error for Unavailable {}

#[derive(Clone, Serialize, Deserialize)]
pub enum Request {
    Begin {
        scope: [u8; TOKEN_BYTES],
        verifier: Option<[u8; TOKEN_BYTES]>,
    },
    Check {
        ticket: Ticket,
        verifier: Option<[u8; TOKEN_BYTES]>,
    },
    Enroll {
        ticket: Ticket,
        verifier: [u8; TOKEN_BYTES],
        info: DeviceInfo,
    },
    List,
    Clear,
    Remove(Vec<[u8; TOKEN_BYTES]>),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Begun(Ticket, bool),
    Done,
    Devices(String),
    Unavailable(String),
}

impl std::fmt::Debug for Request {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TrustedDeviceRequest")
    }
}

impl std::fmt::Debug for Response {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TrustedDeviceResponse")
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn uses_service() -> ResultType<bool> {
    if crate::platform::is_root() || !crate::platform::is_installed() {
        return Ok(false);
    }
    #[cfg(target_os = "macos")]
    {
        use hbb_common::anyhow::Context;
        // The app bundle may be installed without its optional background service.
        return std::path::Path::new(&format!(
            "/Library/LaunchDaemons/{}_service.plist",
            crate::get_full_name()
        ))
        .try_exists()
        .context("Inspect trusted-device system service registration");
    }
    #[cfg(target_os = "linux")]
    Ok(true)
}

pub async fn request(request: Request) -> ResultType<Response> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if uses_service()? {
        use crate::ipc::{connect_trusted_device_service, Data};
        const TIMEOUT_MS: u64 = 1000;
        // Revoke the inactive user copy before the service acknowledges management.
        if matches!(&request, Request::Clear | Request::Remove(_)) {
            let local = request.clone();
            super::config::update(move || handle(local)).await?;
        }
        let mut connection = connect_trusted_device_service(TIMEOUT_MS).await?;
        connection
            .send(&Data::TrustedDeviceRequest(Box::new(request)))
            .await
            .map_err(Unavailable)?;
        return match connection
            .next_timeout(TIMEOUT_MS)
            .await
            .map_err(Unavailable)?
        {
            Some(Data::TrustedDeviceResponse(Ok(Response::Unavailable(error)))) => {
                Err(Unavailable(hbb_common::anyhow::anyhow!(error)).into())
            }
            Some(Data::TrustedDeviceResponse(result)) => {
                result.map_err(|error| hbb_common::anyhow::anyhow!(error))
            }
            _ => Err(Unavailable(hbb_common::anyhow::anyhow!(
                "The system service did not confirm the trusted-device operation"
            ))
            .into()),
        };
    }
    super::config::update(move || handle(request)).await
}

pub fn handle(request: Request) -> ResultType<Response> {
    match request {
        Request::Clear => {
            super::clear()?;
            return Ok(Response::Done);
        }
        Request::List => return super::devices().map(Response::Devices),
        Request::Remove(record_ids) => {
            super::remove(&record_ids)?;
            return Ok(Response::Done);
        }
        _ => {}
    }
    let storage = super::storage()?;
    let settings = super::config::snapshot()?;
    if !settings.enabled || settings.totp.is_empty() {
        bail!("Trusted devices require two-factor authentication to be enabled");
    }
    let mut grants = super::load_grants(&storage, &settings.scope)?;
    match request {
        Request::Begin { scope, verifier } => {
            let ticket = grants.begin(&scope)?;
            let valid = grants.verify_digest(&ticket, verifier.as_ref(), get_time())?;
            Ok(Response::Begun(ticket, valid))
        }
        Request::Check { ticket, verifier } => {
            let valid = grants.verify_digest(&ticket, verifier.as_ref(), get_time())?;
            if verifier.is_some() && !valid {
                bail!("Trusted-device credential expired or was revoked; reconnect and authenticate again");
            }
            Ok(Response::Done)
        }
        Request::Enroll {
            ticket,
            verifier,
            info,
        } => {
            grants.enroll_digest(
                &ticket,
                verifier,
                DeviceInfo {
                    time: get_time(),
                    ..info
                },
            )?;
            storage.write(&grants).map_err(Unavailable)?;
            Ok(Response::Done)
        }
        _ => bail!("Invalid trusted-device authorization operation"),
    }
}

pub async fn manage(clear: bool) -> ResultType<String> {
    match request(if clear { Request::Clear } else { Request::List }).await? {
        Response::Done if clear => Ok(String::new()),
        Response::Devices(devices) if !clear => Ok(devices),
        _ => bail!("Invalid trusted-device management response"),
    }
}

pub async fn remove(record_ids: Vec<[u8; TOKEN_BYTES]>) -> ResultType<()> {
    match request(Request::Remove(record_ids)).await? {
        Response::Done => Ok(()),
        _ => bail!("Invalid trusted-device removal response"),
    }
}
