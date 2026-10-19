use base::config::keys;
use hbb_common::{sodiumoxide::crypto::hash::sha256, ResultType};
use std::collections::HashMap;

pub const DEFAULT_DAYS: u32 = 30;
pub const NEVER_EXPIRE: u32 = 0;
pub const DAY_MILLIS: i64 = 24 * 60 * 60 * 1000;

pub fn days(options: &HashMap<String, String>) -> u32 {
    match options
        .get(keys::OPTION_TRUSTED_DEVICE_EXPIRATION)
        .map(String::as_str)
    {
        None | Some("") | Some("30") => DEFAULT_DAYS,
        Some("1") => 1,
        Some("7") => 7,
        Some("0") => NEVER_EXPIRE,
        _ => {
            const LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
            hbb_common::throttled_log!(LOG_INTERVAL, warn,
                "Invalid trusted-device expiration period; using the default of {DEFAULT_DAYS} days");
            DEFAULT_DAYS
        }
    }
}

pub fn scope(
    authentication: &impl serde::Serialize,
    days: u32,
) -> ResultType<[u8; super::TOKEN_BYTES]> {
    let mut material = serde_json::to_vec(authentication)?;
    // Preserve existing V2 grants when the default 30-day policy is unchanged.
    if days != DEFAULT_DAYS {
        material.extend_from_slice(&days.to_le_bytes());
    }
    Ok(sha256::hash(&material).0)
}

pub fn expires_at(time: i64, days: u32) -> i64 {
    if days == NEVER_EXPIRE {
        return 0;
    }
    time.saturating_add(i64::from(days) * DAY_MILLIS)
}
