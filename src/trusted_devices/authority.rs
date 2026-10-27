use super::{DeviceInfo, Ticket, TOKEN_BYTES};
use hbb_common::{bail, get_time, sodiumoxide::utils, ResultType};
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
        legacy: Option<[u8; TOKEN_BYTES]>,
    },
    Check {
        ticket: Ticket,
        credential: Option<Credential>,
    },
    Enroll {
        ticket: Ticket,
        verifier: [u8; TOKEN_BYTES],
        info: DeviceInfo,
        legacy: Option<[u8; TOKEN_BYTES]>,
    },
    List,
    Clear,
    Remove(Vec<[u8; TOKEN_BYTES]>),
    Renew(Vec<[u8; TOKEN_BYTES]>),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Begun(Ticket, Option<Credential>),
    Done,
    Devices(String),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Credential {
    Token([u8; TOKEN_BYTES]),
    Legacy([u8; TOKEN_BYTES]),
}

impl std::fmt::Debug for Request {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TrustedDeviceRequest")
    }
}

impl Request {
    fn check_scope(&self, current: &[u8; TOKEN_BYTES]) -> ResultType<()> {
        let scope = match self {
            Self::Begin { scope, .. } => scope,
            Self::Check { ticket, .. } | Self::Enroll { ticket, .. } => &ticket.scope,
            _ => bail!("Invalid trusted-device authorization operation"),
        };
        if !utils::memcmp(scope, current) {
            bail!(
                "Trusted-device authentication settings changed; reconnect and authenticate again"
            );
        }
        Ok(())
    }

    fn authorize(self, grants: &mut super::Grants, days: u32) -> ResultType<(Response, bool)> {
        match self {
            Self::Begin {
                scope,
                verifier,
                legacy,
            } => {
                let ticket = grants.begin(&scope)?;
                let mut changed = false;
                let credential = if grants.verify_digest(&ticket, verifier.as_ref(), get_time())? {
                    changed = verifier
                        .as_ref()
                        .is_some_and(|value| grants.confirm_migration(value));
                    verifier.map(Credential::Token)
                } else if grants.verify_legacy(&ticket, legacy.as_ref())? {
                    legacy.map(Credential::Legacy)
                } else {
                    None
                };
                Ok((Response::Begun(ticket, credential), changed))
            }
            Self::Check { ticket, credential } => {
                let valid = match credential.as_ref() {
                    Some(Credential::Legacy(verifier)) => {
                        grants.migration_source(&ticket, verifier)?.is_some()
                    }
                    Some(Credential::Token(verifier)) => {
                        grants.verify_digest(&ticket, Some(verifier), get_time())?
                    }
                    None => grants.verify_digest(&ticket, None, get_time())?,
                };
                if credential.is_some() && !valid {
                    bail!("Trusted-device credential expired or was revoked; reconnect and authenticate again");
                }
                Ok((Response::Done, false))
            }
            enrollment @ Self::Enroll { .. } => {
                enrollment.enroll(grants, days)?;
                Ok((Response::Done, true))
            }
            _ => bail!("Invalid trusted-device authorization operation"),
        }
    }

    fn enroll(self, grants: &mut super::Grants, days: u32) -> ResultType<()> {
        let Self::Enroll {
            ticket,
            verifier,
            info,
            legacy,
        } = self
        else {
            bail!("Invalid trusted-device enrollment operation");
        };
        if let Some(legacy) = legacy {
            return grants.migrate(&ticket, (legacy, verifier));
        }
        let time = get_time();
        grants.enroll_digest(
            &ticket,
            verifier,
            DeviceInfo {
                time,
                expires_at: Some(super::policy::expires_at(time, days)),
                ..info
            },
        )
    }
}

impl std::fmt::Debug for Response {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TrustedDeviceResponse")
    }
}

pub async fn request(request: Request) -> ResultType<Response> {
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
        Request::Renew(record_ids) => {
            super::renew(&record_ids)?;
            return Ok(Response::Done);
        }
        _ => {}
    }
    let storage = super::storage()?;
    let settings = super::config::snapshot()?;
    if !settings.enabled || settings.totp.is_empty() {
        bail!("Trusted devices require two-factor authentication to be enabled");
    }
    // Validate authentication settings before allowing an unavailable-grants fallback.
    request.check_scope(&settings.scope)?;
    let mut grants = super::load_grants(&storage, &settings.scope)?;
    let (response, changed) = request.authorize(&mut grants, settings.validity_days)?;
    if changed {
        storage.write(&grants).map_err(Unavailable)?;
    }
    Ok(response)
}

pub async fn manage(clear: bool) -> ResultType<String> {
    match request(if clear { Request::Clear } else { Request::List }).await? {
        Response::Done if clear => Ok(String::new()),
        Response::Devices(devices) if !clear => Ok(devices),
        _ => bail!("Invalid trusted-device management response"),
    }
}
