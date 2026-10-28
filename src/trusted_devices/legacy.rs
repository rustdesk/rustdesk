use super::{config, policy, DeviceInfo, Grants, Record, Ticket, TOKEN_BYTES};
use hbb_common::{
    anyhow::anyhow,
    bail, get_time,
    password_security::decrypt_str_or_original,
    sodiumoxide::{crypto::hash::sha256, utils},
    ResultType,
};
use serde_derive::Deserialize;

const LEGACY_DAYS: u32 = 90;

#[derive(Deserialize)]
struct LegacyDevice {
    hwid: Vec<u8>,
    #[serde(flatten)]
    info: DeviceInfo,
}

pub(super) fn verifier(hwid: &[u8], info: &DeviceInfo) -> ResultType<Option<[u8; TOKEN_BYTES]>> {
    if hwid.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        sha256::hash(&serde_json::to_vec(&(
            hwid,
            &info.id,
            &info.name,
            &info.platform,
        ))?)
        .0,
    ))
}

pub(super) fn import(grants: &mut Grants, value: &str, first_store: bool) -> ResultType<()> {
    let settings = config::snapshot()?;
    if first_store {
        grants.revoke(Some(settings.scope))?;
    }
    // A previous revocation must take precedence over a remaining legacy copy.
    if !grants.matches_scope(&settings.scope) {
        return Ok(());
    }
    let (plaintext, decrypted, _) = decrypt_str_or_original(value, "00");
    if !decrypted {
        bail!("Could not decrypt legacy trusted devices");
    }
    let devices: Vec<LegacyDevice> =
        serde_json::from_str(&plaintext).map_err(|_| anyhow!("Invalid legacy trusted devices"))?;
    let now = get_time();
    for device in devices {
        let Some(verifier) = verifier(&device.hwid, &device.info)? else {
            continue;
        };
        let mut info = device.info;
        info.expires_at = Some(
            info.expires_at
                .unwrap_or_else(|| policy::expires_at(info.time, LEGACY_DAYS)),
        );
        let record = Record {
            verifier,
            info,
            legacy: true,
            origin: None,
        };
        if record.current(now) && !grants.records.iter().any(|old| old.id() == record.id()) {
            grants.records.push(record);
        }
    }
    policy::update_expiration(grants, settings.validity_days, now);
    Ok(())
}

impl Grants {
    pub(super) fn verify_legacy(
        &self,
        ticket: &Ticket,
        verifier: Option<&[u8; TOKEN_BYTES]>,
    ) -> ResultType<bool> {
        self.check_ticket(ticket)?;
        let Some(verifier) = verifier else {
            return Ok(false);
        };
        Ok(self.records.iter().any(|record| {
            record.legacy && record.current(get_time()) && utils::memcmp(&record.verifier, verifier)
        }))
    }

    pub(super) fn migrate(
        &mut self,
        ticket: &Ticket,
        credentials: ([u8; TOKEN_BYTES], [u8; TOKEN_BYTES]),
    ) -> ResultType<()> {
        let (legacy, verifier) = credentials;
        let mut info = self
            .migration_source(ticket, &legacy)?
            .ok_or_else(|| anyhow!("Legacy trusted device expired or was revoked"))?
            .info
            .clone();
        let now = get_time();
        // Eviction follows V2 issuance time without extending the legacy expiry.
        info.time = now;
        self.records.push(Record {
            verifier,
            info,
            legacy: false,
            origin: Some(legacy),
        });
        self.cleanup(now);
        Ok(())
    }

    pub(super) fn migration_source(
        &self,
        ticket: &Ticket,
        verifier: &[u8; TOKEN_BYTES],
    ) -> ResultType<Option<&Record>> {
        self.check_ticket(ticket)?;
        // An already authorized attempt may finish after another connection confirms migration.
        // New legacy logins still require a legacy record in verify_legacy.
        Ok(self.records.iter().find(|record| {
            record.current(get_time())
                && ((record.legacy && utils::memcmp(&record.verifier, verifier))
                    || record
                        .origin
                        .as_ref()
                        .is_some_and(|origin| utils::memcmp(origin, verifier)))
        }))
    }

    pub(super) fn confirm_migration(&mut self, verifier: &[u8; TOKEN_BYTES]) -> bool {
        let origin = self
            .records
            .iter()
            .find(|record| !record.legacy && record.verifier == *verifier)
            .and_then(|record| record.origin);
        let Some(origin) = origin else {
            return false;
        };
        let previous = self.records.len();
        self.records
            .retain(|record| !record.legacy || record.verifier != origin);
        previous != self.records.len()
    }
}
