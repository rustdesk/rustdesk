use super::{DeviceInfo, Ticket, TOKEN_BYTES};
use hbb_common::{bail, get_time, ResultType};
use serde_derive::{Deserialize, Serialize};

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
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Begun(Ticket, bool),
    Done,
    Devices(String),
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

pub async fn request(request: Request) -> ResultType<Response> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if crate::platform::is_installed() && !crate::platform::is_root() {
        use crate::ipc::{connect_trusted_device_service, Data};
        const TIMEOUT_MS: u64 = 1000;
        let mut connection = connect_trusted_device_service(TIMEOUT_MS).await?;
        connection
            .send(&Data::TrustedDeviceRequest(Box::new(request)))
            .await?;
        return match connection.next_timeout(TIMEOUT_MS).await? {
            Some(Data::TrustedDeviceResponse(result)) => {
                result.map_err(|error| hbb_common::anyhow::anyhow!(error))
            }
            _ => bail!("The system service did not confirm the trusted-device operation"),
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
            storage.write(&grants)?;
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
