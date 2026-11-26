use super::{connect_root_service, Connection, Data};
use hbb_common::{
    anyhow::{anyhow, ensure},
    config::{unix_unlock_pin::RootPin, Config, Config2},
    libc,
    rand::{rngs::OsRng, RngCore},
    sha2::{Digest, Sha256},
    tokio, ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

type Token = [u8; 32];
const TIMEOUT_MS: u64 = 1000;
const IDLE: Duration = Duration::from_secs(30 * 60);
const MAX_GRANTS: usize = 16;
const RETRY: Duration = Duration::from_secs(1);
const REQUIRED: &str = "Authentication Required";
static GRANTS: OnceLock<Mutex<Grants>> = OnceLock::new();
static CLIENT: Mutex<Option<Token>> = Mutex::new(None);

#[derive(Clone, Serialize, Deserialize)]
pub enum Request {
    Configured,
    // Network settings only verify the PIN. Security settings also request a grant.
    Verify(String, Option<Token>),
    Approve(u32, Token),
    Authorized(Token),
    Set(Option<Token>, String),
}

impl Request {
    fn check_active_user(
        &self,
        peer: u32,
        active_uid: impl FnOnce() -> Option<u32>,
    ) -> ResultType<()> {
        if peer != 0 && matches!(self, Self::Set(..)) {
            ensure!(
                super::ipc_auth::is_allowed_service_peer_uid(peer, active_uid()),
                "{REQUIRED}"
            );
        }
        Ok(())
    }
}

fn current_console_uid() -> Option<u32> {
    #[cfg(target_os = "linux")]
    return crate::platform::linux::get_active_userid_fresh()
        .trim()
        .parse()
        .ok();
    #[cfg(target_os = "macos")]
    super::active_uid()
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UnlockPin")
    }
}

fn digest(token: &Token) -> Token {
    Sha256::digest(token).into()
}

#[derive(Default)]
struct Grants {
    approved: HashMap<(u32, Token), Instant>,
    last_attempt: Option<Instant>,
}

impl Grants {
    fn approve(&mut self, uid: u32, key: Token) -> bool {
        let now = Instant::now();
        self.approved.retain(|_, until| *until > now);
        if self.approved.len() >= MAX_GRANTS && !self.approved.contains_key(&(uid, key)) {
            return false;
        }
        self.approved.insert((uid, key), now + IDLE);
        true
    }

    fn admin_approve(&mut self, peer: u32, target: u32, key: Token) -> bool {
        peer == 0 && self.approve(target, key)
    }

    fn authorized(&mut self, uid: u32, token: &Token) -> bool {
        let Some(deadline) = self.approved.get_mut(&(uid, digest(token))) else {
            return false;
        };
        let now = Instant::now();
        if *deadline <= now {
            return false;
        }
        *deadline = now + IDLE;
        true
    }

    fn verify<F>(&mut self, uid: u32, key: Option<Token>, matches_pin: F) -> ResultType<bool>
    where
        F: FnOnce() -> ResultType<bool>,
    {
        let now = Instant::now();
        let too_frequent = self.last_attempt.is_some_and(|last| now - last < RETRY);
        ensure!(!too_frequent, "Too frequent");
        self.last_attempt = Some(now);
        let valid = matches_pin()?;
        let grant_denied = valid && key.is_some_and(|key| !self.approve(uid, key));
        ensure!(!grant_denied, "Please try later");
        Ok(valid)
    }
}

pub(super) fn preserve_legacy_root_pin() -> ResultType<()> {
    // Serialize first-use migration with an administrator setting a new PIN.
    let _grants = GRANTS.get_or_init(Default::default).lock().unwrap();
    RootPin::load()?;
    Ok(())
}

fn perform(peer: u32, request: Request) -> ResultType<bool> {
    ensure!(
        unsafe { libc::geteuid() } == 0 && crate::platform::is_installed(),
        "{REQUIRED}"
    );
    request.check_active_user(peer, current_console_uid)?;
    let mut grants = GRANTS.get_or_init(Default::default).lock().unwrap();
    match request {
        Request::Configured => {
            Ok(!Config::is_disable_unlock_pin() && RootPin::load()?.configured())
        }
        Request::Verify(candidate, key) => {
            ensure!(!Config::is_disable_unlock_pin(), "Unlock PIN is disabled");
            grants.verify(peer, key, || RootPin::load()?.matches(&candidate))
        }
        Request::Approve(uid, key) => Ok(grants.admin_approve(peer, uid, key)),
        Request::Authorized(token) => Ok(grants.authorized(peer, &token)),
        Request::Set(token, pin) => {
            ensure!(
                peer == 0 || token.is_some_and(|t| grants.authorized(peer, &t)),
                "{REQUIRED}"
            );
            ensure!(!Config::is_disable_unlock_pin(), "Unlock PIN is disabled");
            super::validate_unlock_pin(&pin, false)?;
            if peer != 0 {
                RootPin::load()?;
            }
            RootPin::set(&pin)?;
            Ok(true)
        }
    }
}

pub(super) async fn reply(request: Request, stream: &mut Connection) -> ResultType<()> {
    let peer = stream
        .peer_uid()
        .ok_or_else(|| anyhow!("Missing unlock PIN peer UID"))?;
    let result = tokio::task::spawn_blocking(move || perform(peer, request)).await?;
    stream
        .send(&Data::UnlockPinResult(
            result.map_err(|err| err.to_string()),
        ))
        .await
}

async fn exchange(stream: &mut Connection, request: Request) -> ResultType<bool> {
    stream.send(&Data::UnlockPin(request)).await?;
    match stream.next_timeout(TIMEOUT_MS).await? {
        Some(Data::UnlockPinResult(result)) => result.map_err(|err| anyhow!(err)),
        _ => Err(anyhow!(
            "The root service did not confirm the PIN operation"
        )),
    }
}

async fn unlock(stream: &mut Connection, candidate: Option<String>) -> ResultType<bool> {
    let mut token = Token::default();
    OsRng
        .try_fill_bytes(&mut token)
        .map_err(|err| anyhow!("Failed to generate PIN grant: {err}"))?;
    let key = digest(&token);
    let ok = match candidate {
        Some(pin) => exchange(stream, Request::Verify(pin, Some(key))).await?,
        None => {
            let uid = unsafe { libc::geteuid() };
            tokio::task::spawn_blocking(move || {
                crate::platform::unix_approval::request("--approve-unlock-pin", uid, &key)
            })
            .await??;
            exchange(stream, Request::Authorized(token)).await?
        }
    };
    if ok {
        *CLIENT.lock().unwrap() = Some(token);
    }
    Ok(ok)
}

#[tokio::main(flavor = "current_thread")]
pub(crate) async fn admin_approve(uid: &str, encoded: &str) -> ResultType<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0 && crate::platform::is_installed(),
        "{REQUIRED}"
    );
    let mut key = Token::default();
    hex::decode_to_slice(encoded, &mut key)?;
    let mut stream = connect_root_service(TIMEOUT_MS).await?;
    ensure!(
        exchange(&mut stream, Request::Approve(uid.parse()?, key)).await?,
        "{REQUIRED}"
    );
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub(super) async fn set(pin: String) -> ResultType<()> {
    let mut stream = connect_root_service(TIMEOUT_MS).await?;
    let token = *CLIENT.lock().unwrap();
    ensure!(
        exchange(&mut stream, Request::Set(token, pin)).await?,
        "{REQUIRED}"
    );
    Ok(())
}

#[cfg(feature = "flutter")]
#[tokio::main(flavor = "current_thread")]
pub(crate) async fn gui_request(action: &str) -> ResultType<String> {
    use std::io::ErrorKind::{ConnectionRefused, NotFound};
    let mut stream = match connect_root_service(TIMEOUT_MS).await {
        Ok(stream) => stream,
        Err(err) => {
            let io = err
                .downcast_ref::<crate::trusted_devices::authority::Unavailable>()
                .and_then(|error| error.0.downcast_ref::<std::io::Error>());
            if action == "unlock-method"
                && io.is_some_and(|e| matches!(e.kind(), NotFound | ConnectionRefused))
            {
                return Ok("offline".into());
            }
            return Err(err);
        }
    };
    let root = unsafe { libc::geteuid() } == 0;
    let ok = match action {
        "unlock-method" if root => return Ok("root".into()),
        "unlock-method" => {
            let configured = exchange(&mut stream, Request::Configured).await?;
            return Ok(if configured { "pin" } else { "admin" }.into());
        }
        "pin-set" => exchange(&mut stream, Request::Configured).await?,
        "admin" => root || unlock(&mut stream, None).await?,
        _ => match action.split_once(':') {
            Some(("pin", pin)) => unlock(&mut stream, Some(pin.to_owned())).await?,
            Some(("check-pin", pin)) => {
                exchange(&mut stream, Request::Verify(pin.to_owned(), None)).await?
            }
            _ => return Err(anyhow!("Unknown unlock PIN request")),
        },
    };
    Ok(ok.to_string())
}

pub(super) mod sync_config {
    use super::{Config, Config2};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    type Snapshot = Option<Box<(Config, Config2)>>;

    pub fn serialize<S: Serializer>(value: &Snapshot, serializer: S) -> Result<S::Ok, S::Error> {
        if crate::platform::is_installed() {
            return value
                .as_ref()
                .map(|pair| (&pair.0, pair.1.clone().without_unlock_pin()))
                .serialize(serializer);
        }
        value.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Snapshot, D::Error> {
        let mut value = Snapshot::deserialize(deserializer)?;
        if crate::platform::is_installed() {
            if let Some(pair) = value.as_mut() {
                pair.1 = std::mem::take(&mut pair.1).without_unlock_pin();
            }
        }
        Ok(value)
    }
}

#[cfg(test)]
#[path = "unix_unlock_pin/tests.rs"]
mod tests;
