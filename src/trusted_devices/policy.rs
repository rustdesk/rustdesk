use super::config;
use base::config::keys;
use hbb_common::{config::Config, get_time, ResultType};
use std::collections::HashMap;

pub const DEFAULT_DAYS: u32 = 90;
pub const NEVER_EXPIRE: u32 = 0;
pub const DAY_MILLIS: i64 = 24 * 60 * 60 * 1000;
const AUTH_OPTIONS: [&str; 2] = ["2fa", keys::OPTION_ENABLE_TRUSTED_DEVICES];

pub fn days(options: &HashMap<String, String>) -> u32 {
    match options
        .get(keys::OPTION_TRUSTED_DEVICE_EXPIRATION)
        .map(String::as_str)
    {
        None | Some("") | Some("90") => DEFAULT_DAYS,
        Some("1") => 1,
        Some("7") => 7,
        Some("30") => 30,
        Some("0") => NEVER_EXPIRE,
        _ => {
            const LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
            hbb_common::throttled_log!(LOG_INTERVAL, warn,
                "Invalid trusted-device expiration period; using the default of {DEFAULT_DAYS} days");
            DEFAULT_DAYS
        }
    }
}

fn authentication_changed(old: &HashMap<String, String>, new: &HashMap<String, String>) -> bool {
    AUTH_OPTIONS
        .iter()
        .any(|key| old.get(*key) != new.get(*key))
}

pub fn changes_authentication(options: &HashMap<String, String>) -> bool {
    let old = Config::get_options();
    let new = config::effective_options(options.clone());
    authentication_changed(&old, &new) || days(&old) != days(&new)
}

pub async fn update_options(options: HashMap<String, String>) -> ResultType<()> {
    let old = Config::get_options();
    let new = config::effective_options(options.clone());
    let revoke = authentication_changed(&old, &new);
    config::update_auth(revoke, move || config::set_options(options)).await
}

pub(super) fn change_options<T>(
    options: HashMap<String, String>,
    apply: impl FnOnce() -> ResultType<T>,
) -> ResultType<T> {
    let old = Config::get_options();
    if authentication_changed(&old, &options) {
        return config::change(true, apply);
    }
    if days(&old) == days(&options) {
        return apply();
    }
    let storage = super::storage()?;
    if !storage.exists()? {
        return apply();
    }
    let now = get_time();
    let scope = config::snapshot()?.scope;
    let mut grants = storage.read()?;
    if !grants.matches_scope(&scope) {
        const LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
        hbb_common::throttled_log!(LOG_INTERVAL, warn,
            "Trusted-device grants remain out of sync; applying options without changing those grants");
        return apply();
    }
    update_expiration(&mut grants, days(&options), now);
    storage.write(&grants)?;
    apply()
}

pub(super) fn update_expiration(grants: &mut super::Grants, days: u32, now: i64) -> bool {
    let mut changed = grants.cleanup(now);
    let expiry = expires_at(now, days);
    for record in &mut grants.records {
        let Some(previous) = record.info.expires_at else {
            continue;
        };
        let next = if expiry == 0 || previous == 0 {
            expiry
        } else {
            previous.min(expiry)
        };
        if next != previous {
            record.info.expires_at = Some(next);
            changed = true;
        }
    }
    changed
}

pub fn expires_at(time: i64, days: u32) -> i64 {
    if days == NEVER_EXPIRE {
        return 0;
    }
    time.saturating_add(i64::from(days) * DAY_MILLIS)
}
