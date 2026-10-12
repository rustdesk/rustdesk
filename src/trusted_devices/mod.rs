pub mod authority;
pub mod client;
pub mod config;
pub mod host;
mod storage;

#[cfg(test)]
pub(crate) mod tests;

use hbb_common::{
    anyhow::anyhow,
    bail,
    config::Config,
    sodiumoxide::{crypto::hash::sha256, randombytes, utils},
    ResultType,
};
use serde_derive::{Deserialize, Serialize};
use storage::Storage;

pub const TOKEN_BYTES: usize = 32;
pub const LIFETIME_MILLIS: i64 = 30 * 24 * 60 * 60 * 1000;
pub const MAX_RECORDS: usize = 32;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub time: i64,
    pub id: String,
    pub name: String,
    pub platform: String,
}

#[derive(Serialize, Deserialize)]
struct Record {
    verifier: [u8; TOKEN_BYTES],
    info: DeviceInfo,
}

#[derive(Serialize)]
struct ListedDevice<'a> {
    record_id: [u8; TOKEN_BYTES],
    #[serde(flatten)]
    info: &'a DeviceInfo,
}

impl Record {
    fn id(&self) -> [u8; TOKEN_BYTES] {
        // Management identifiers must not expose authentication verifiers.
        sha256::hash(&self.verifier).0
    }

    fn current(&self, now: i64) -> bool {
        now >= self.info.time && now < self.info.time.saturating_add(LIFETIME_MILLIS)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Grants {
    scope: Option<[u8; TOKEN_BYTES]>,
    generation: [u8; TOKEN_BYTES],
    records: Vec<Record>,
}

#[derive(Clone, Serialize, Deserialize)]
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
        Ok(self
            .records
            .iter()
            .any(|record| record.current(now) && utils::memcmp(&record.verifier, verifier)))
    }

    pub fn enroll_digest(
        &mut self,
        ticket: &Ticket,
        verifier: [u8; TOKEN_BYTES],
        info: DeviceInfo,
    ) -> ResultType<()> {
        self.check_ticket(ticket)?;
        let now = info.time;
        self.records.push(Record { verifier, info });
        self.cleanup(now);
        Ok(())
    }

    pub fn cleanup(&mut self, now: i64) -> bool {
        let previous = self.records.len();
        self.records.retain(|record| record.current(now));
        self.records.sort_by_key(|record| record.info.time);
        let excess = self.records.len().saturating_sub(MAX_RECORDS);
        self.records.drain(..excess);
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

fn storage() -> ResultType<Storage> {
    Ok(Storage::lock(&Config::path("trusted-devices-v2.json")).map_err(authority::Unavailable)?)
}

fn load_grants(storage: &Storage, scope: &[u8; TOKEN_BYTES]) -> ResultType<Grants> {
    let mut grants: Grants = storage.read().map_err(authority::Unavailable)?;
    if !storage.exists().map_err(authority::Unavailable)? {
        grants.revoke(Some(*scope))?;
        storage.write(&grants).map_err(authority::Unavailable)?;
    } else if !grants.matches_scope(scope) {
        bail!("Trusted-device authentication settings changed; restart the host to synchronize");
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
    let mut grants: Grants = storage.read()?;
    grants.revoke(grants.scope)?;
    storage.write(&grants)
}

pub fn remove(record_ids: &[[u8; TOKEN_BYTES]]) -> ResultType<()> {
    let storage = storage()?;
    let mut grants: Grants = storage.read()?;
    let previous = grants.records.len();
    grants
        .records
        .retain(|record| !record_ids.contains(&record.id()));
    if grants.records.len() != previous {
        storage.write(&grants)?;
    }
    Ok(())
}

pub fn initialize() -> ResultType<()> {
    let path = Config::path("trusted-devices-v2.json");
    if !path.try_exists()? {
        return Ok(());
    }
    let storage = storage()?;
    let mut grants: Grants = storage.read()?;
    let current = config::snapshot()?;
    if !grants.matches_scope(&current.scope) {
        grants.revoke(Some(current.scope))?;
        storage.write(&grants)?;
    }
    Ok(())
}

pub fn devices() -> ResultType<String> {
    let storage = storage()?;
    if !storage.exists()? {
        return Ok("[]".into());
    }
    let current = config::snapshot()?;
    let grants = load_grants(&storage, &current.scope)?;
    let now = hbb_common::get_time();
    let devices: Vec<_> = grants
        .records
        .iter()
        .filter(|record| record.current(now))
        .map(|record| ListedDevice {
            record_id: record.id(),
            info: &record.info,
        })
        .collect();
    Ok(serde_json::to_string(&devices)?)
}
