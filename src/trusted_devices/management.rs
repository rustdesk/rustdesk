use super::{config, load_grants, policy, storage, DeviceInfo, TOKEN_BYTES};
use hbb_common::{bail, get_time, ResultType};
use serde_derive::Serialize;
use std::collections::HashSet;

#[derive(Serialize)]
struct ListedDevice<'a> {
    record_id: [u8; TOKEN_BYTES],
    #[serde(flatten)]
    info: &'a DeviceInfo,
}

pub fn devices() -> ResultType<String> {
    let storage = storage()?;
    if !storage.exists()? {
        return Ok(serde_json::json!({ "devices": [] }).to_string());
    }
    let current = config::snapshot()?;
    let grants = load_grants(&storage, &current.scope)?;
    let now = get_time();
    let mut seen = HashSet::new();
    let devices: Vec<_> = grants
        .records
        .iter()
        .filter(|record| record.current(now) && seen.insert(record.id()))
        .map(|record| ListedDevice {
            record_id: record.id(),
            info: &record.info,
        })
        .collect();
    Ok(serde_json::json!({ "devices": devices }).to_string())
}

pub fn remove(record_ids: &[[u8; TOKEN_BYTES]]) -> ResultType<()> {
    let storage = storage()?;
    let mut grants = storage.read()?;
    let previous = grants.records.len();
    grants
        .records
        .retain(|record| !record_ids.contains(&record.id()));
    if grants.records.len() != previous {
        storage.write(&grants)?;
    }
    Ok(())
}

pub fn renew(record_ids: &[[u8; TOKEN_BYTES]]) -> ResultType<()> {
    if record_ids.is_empty() {
        bail!("Select at least one trusted device");
    }
    let storage = storage()?;
    let settings = config::snapshot()?;
    if !settings.enabled || settings.totp.is_empty() {
        bail!("Trusted devices require two-factor authentication to be enabled");
    }
    if settings.validity_days == policy::NEVER_EXPIRE {
        bail!("Trusted devices are configured to never expire");
    }
    let mut grants = load_grants(&storage, &settings.scope)?;
    let now = get_time();
    for record_id in record_ids {
        if !grants
            .records
            .iter()
            .any(|record| record.id() == *record_id && record.current(now))
        {
            bail!("Trusted device expired or was revoked; refresh the list");
        }
    }
    let expires_at = policy::expires_at(now, settings.validity_days);
    for record in &mut grants.records {
        if record_ids.contains(&record.id()) {
            record.info.expires_at = Some(expires_at);
        }
    }
    storage.write(&grants)
}
