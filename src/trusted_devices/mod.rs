pub mod authority;
pub mod client;
pub mod config;
pub mod host;
mod legacy;
mod management;
mod policy;
mod storage;

pub use management::{devices, remove, renew};

#[cfg(test)]
pub(crate) mod tests;

use hbb_common::{
    anyhow::anyhow,
    bail,
    config::Config2,
    sodiumoxide::{crypto::hash::sha256, randombytes, utils},
    ResultType,
};
use serde_derive::{Deserialize, Serialize};
use storage::GrantsStorage;

pub const TOKEN_BYTES: usize = 32;
pub const MAX_RECORDS: usize = 128;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub time: i64,
    // Missing on V1 records until import; zero means no expiration.
    #[serde(default)]
    pub expires_at: Option<i64>,
    pub id: String,
    pub name: String,
    pub platform: String,
}

#[derive(Serialize, Deserialize)]
struct Record {
    verifier: [u8; TOKEN_BYTES],
    info: DeviceInfo,
    #[serde(default)]
    legacy: bool,
    // Keep migrated credentials and their legacy entry in one revocation group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<[u8; TOKEN_BYTES]>,
}

impl Record {
    fn id(&self) -> [u8; TOKEN_BYTES] {
        // Management identifiers must not expose authentication verifiers.
        sha256::hash(&self.origin.unwrap_or(self.verifier)).0
    }

    fn current(&self, now: i64) -> bool {
        now >= self.info.time
            && self
                .info
                .expires_at
                .is_some_and(|expiry| expiry == 0 || now < expiry)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Grants {
    scope: Option<[u8; TOKEN_BYTES]>,
    generation: [u8; TOKEN_BYTES],
    records: Vec<Record>,
}

#[derive(Clone)]
pub struct Ticket {
    scope: [u8; TOKEN_BYTES],
    generation: [u8; TOKEN_BYTES],
}

impl Grants {
    pub fn matches_scope(&self, scope: &[u8; TOKEN_BYTES]) -> bool {
        self.scope
            .as_ref()
            .is_some_and(|stored| utils::memcmp(stored, scope))
    }

    pub fn revoke(&mut self, scope: Option<[u8; TOKEN_BYTES]>) -> ResultType<()> {
        let generation = random_token()?;
        self.records.clear();
        self.generation = generation;
        self.scope = scope;
        Ok(())
    }

    pub fn begin(&self, scope: &[u8; TOKEN_BYTES]) -> ResultType<Ticket> {
        if !self.matches_scope(scope) {
            bail!(
                "Trusted-device authentication settings changed; reconnect and authenticate again"
            );
        }
        Ok(Ticket {
            scope: *scope,
            generation: self.generation,
        })
    }

    fn check_ticket(&self, ticket: &Ticket) -> ResultType<()> {
        if !self.matches_scope(&ticket.scope)
            || !utils::memcmp(&self.generation, &ticket.generation)
        {
            bail!("Trusted-device authorization was revoked; reconnect and authenticate again");
        }
        Ok(())
    }

    pub fn verify_digest(
        &self,
        ticket: &Ticket,
        verifier: Option<&[u8; TOKEN_BYTES]>,
        now: i64,
    ) -> ResultType<bool> {
        self.check_ticket(ticket)?;
        let Some(verifier) = verifier else {
            return Ok(false);
        };
        Ok(self.records.iter().any(|record| {
            !record.legacy && record.current(now) && utils::memcmp(&record.verifier, verifier)
        }))
    }

    pub fn enroll_digest(
        &mut self,
        ticket: &Ticket,
        verifier: [u8; TOKEN_BYTES],
        info: DeviceInfo,
    ) -> ResultType<()> {
        self.check_ticket(ticket)?;
        if !info.id.is_empty() {
            self.records.retain(|record| record.info.id != info.id);
        }
        let now = info.time;
        self.records.push(Record {
            verifier,
            info,
            legacy: false,
            origin: None,
        });
        self.cleanup(now);
        Ok(())
    }

    pub fn cleanup(&mut self, now: i64) -> bool {
        let previous = self.records.len();
        self.records.retain(|record| record.current(now));
        self.records.sort_by_key(|record| record.info.time);
        // Upgrading must not evict existing legacy registrations to satisfy the V2 cap.
        let mut excess = self
            .records
            .iter()
            .filter(|record| !record.legacy)
            .count()
            .saturating_sub(MAX_RECORDS);
        self.records.retain(|record| {
            if !record.legacy && excess > 0 {
                excess -= 1;
                return false;
            }
            true
        });
        previous != self.records.len()
    }
}

pub fn token_verifier(token: &[u8]) -> Option<[u8; TOKEN_BYTES]> {
    (token.len() == TOKEN_BYTES).then(|| sha256::hash(token).0)
}

pub fn random_token() -> ResultType<[u8; TOKEN_BYTES]> {
    hbb_common::sodiumoxide::init()
        .map_err(|_| anyhow!("Could not initialize secure random generation"))?;
    let mut token = [0; TOKEN_BYTES];
    randombytes::randombytes_into(&mut token);
    Ok(token)
}

fn storage() -> ResultType<GrantsStorage> {
    Ok(GrantsStorage::lock().map_err(authority::Unavailable)?)
}

fn load_grants(storage: &GrantsStorage, scope: &[u8; TOKEN_BYTES]) -> ResultType<Grants> {
    let mut grants: Grants = storage.read().map_err(authority::Unavailable)?;
    if !storage.exists().map_err(authority::Unavailable)? {
        grants.revoke(Some(*scope))?;
        storage.write(&grants).map_err(authority::Unavailable)?;
    } else if !grants.matches_scope(scope) {
        return Err(authority::Unavailable(anyhow!(
            "Trusted-device grants are out of sync; restart the host to synchronize"
        ))
        .into());
    }
    if grants.cleanup(hbb_common::get_time()) {
        storage.write(&grants).map_err(authority::Unavailable)?;
    }
    Ok(grants)
}

pub fn clear() -> ResultType<()> {
    let storage = storage()?;
    if !storage.exists()? {
        return Ok(());
    }
    let scope = match config::snapshot() {
        Ok(settings) => Some(settings.scope),
        Err(error) => {
            const LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
            hbb_common::throttled_log!(LOG_INTERVAL, warn,
                "Cannot read trusted-device authentication settings; revoking grants without an authentication scope: {error:#}");
            None
        }
    };
    let mut grants = Grants::default();
    grants.revoke(scope)?;
    storage.write(&grants)
}

pub fn initialize() -> ResultType<()> {
    if !Config2::file().try_exists()? {
        return Ok(());
    }
    let storage = storage()?;
    if !storage.exists()? {
        return Ok(());
    }
    let mut grants: Grants = storage.read()?;
    let current = config::snapshot()?;
    let scope_changed = !grants.matches_scope(&current.scope);
    if scope_changed {
        grants.revoke(Some(current.scope))?;
    }
    if policy::update_expiration(&mut grants, current.validity_days, hbb_common::get_time())
        || scope_changed
    {
        storage.write(&grants)?;
    }
    Ok(())
}
