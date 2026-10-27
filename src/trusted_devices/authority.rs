use super::{Ticket, TOKEN_BYTES};
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
    List,
    Clear,
    Remove(Vec<[u8; TOKEN_BYTES]>),
    Renew(Vec<[u8; TOKEN_BYTES]>),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Response {
    Done,
    Devices(String),
}

#[derive(Clone)]
pub enum Credential {
    Token([u8; TOKEN_BYTES]),
    Legacy([u8; TOKEN_BYTES]),
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
    super::config::update(move || handle(request)).await
}

pub fn handle(request: Request) -> ResultType<Response> {
    match request {
        Request::Clear => super::clear()?,
        Request::List => return super::devices().map(Response::Devices),
        Request::Remove(record_ids) => super::remove(&record_ids)?,
        Request::Renew(record_ids) => super::renew(&record_ids)?,
    }
    Ok(Response::Done)
}

pub(super) fn authorize<T>(
    scope: &[u8; TOKEN_BYTES],
    apply: impl FnOnce(&mut super::Grants, u32) -> ResultType<(T, bool)>,
) -> ResultType<T> {
    // Configuration errors take precedence even if the grant lock is unavailable.
    let storage = super::storage();
    let settings = super::config::snapshot()?;
    if !settings.enabled || settings.totp.is_empty() {
        bail!("Trusted devices require two-factor authentication to be enabled");
    }
    if !utils::memcmp(scope, &settings.scope) {
        bail!("Trusted-device authentication settings changed; reconnect and authenticate again");
    }
    let storage = storage?;
    let mut grants = super::load_grants(&storage, &settings.scope)?;
    let (result, changed) = apply(&mut grants, settings.validity_days)?;
    if changed {
        storage.write(&grants).map_err(Unavailable)?;
    }
    Ok(result)
}

pub(super) fn begin(
    scope: [u8; TOKEN_BYTES],
    verifier: Option<[u8; TOKEN_BYTES]>,
    legacy: Option<[u8; TOKEN_BYTES]>,
) -> ResultType<(Ticket, Option<Credential>)> {
    authorize(&scope, |grants, _| {
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
        Ok(((ticket, credential), changed))
    })
}
