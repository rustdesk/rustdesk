use super::policy;
pub use super::policy::{changes_authentication, update_options};
use base::config::keys;
use hbb_common::{
    anyhow::{anyhow, Context},
    bail,
    config::{self, Config, Config2},
    password_security::decrypt_str_or_original,
    sodiumoxide::{crypto::hash::sha256, utils},
    ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::{collections::HashMap, io::ErrorKind, path::Path};

#[cfg(any(target_os = "linux", target_os = "macos"))]
const SYNC_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn log_sync_error(error: impl std::fmt::Display) {
    hbb_common::throttled_log!(
        SYNC_LOG_INTERVAL,
        error,
        "Configuration sync rejected: {error}"
    );
}

pub async fn update<T: Send + 'static>(
    apply: impl FnOnce() -> ResultType<T> + Send + 'static,
) -> ResultType<T> {
    hbb_common::tokio::task::spawn_blocking(apply).await?
}

pub async fn update_auth<T: Send + 'static>(
    affected: bool,
    apply: impl FnOnce() -> ResultType<T> + Send + 'static,
) -> ResultType<T> {
    update(move || {
        if affected && super::storage()?.exists()? {
            snapshot()?;
        }
        apply()
    })
    .await
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos", test))]
pub fn sync_config(value: Config, options: Config2) -> ResultType<()> {
    let _storage = super::storage()?;
    Config::set(value);
    Config2::set(options.clone());
    snapshot()?;
    // Explicit configuration sync carries the source's grants and removals.
    options.sync_trusted_devices()
}

#[cfg(target_os = "windows")]
pub async fn refresh_from_server() -> ResultType<()> {
    use crate::ipc::{connect, Data};

    const TIMEOUT_MS: u64 = 1000;
    let response = async {
        let mut connection = connect(TIMEOUT_MS, "").await?;
        connection.send(&Data::SyncConfig(None)).await?;
        connection.next_timeout(TIMEOUT_MS).await
    }
    .await;
    match response {
        Ok(Some(Data::SyncConfig(Some(configs)))) => {
            let (config, config2) = *configs;
            update(move || sync_config(config, config2)).await
        }
        Ok(Some(Data::SecurityConfigError(error))) => bail!(error),
        Ok(_) => bail!("The host did not provide its current configuration"),
        Err(error) => {
            hbb_common::log::warn!(
                "Host unavailable during service removal; revoking local trusted devices (re-enrollment required): {error:#}"
            );
            // The next service import must not restore an inactive, stale grant copy.
            update(super::clear).await
        }
    }
}

#[derive(Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Password {
    password: String,
    salt: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    options: HashMap<String, String>,
}

pub struct Snapshot {
    pub scope: [u8; 32],
    pub h1: Option<[u8; 32]>,
    pub salt: String,
    pub totp: String,
    pub enabled: bool,
    pub validity_days: u32,
}

fn read<T: serde::de::DeserializeOwned + Default>(path: &Path) -> ResultType<T> {
    match std::fs::read_to_string(path) {
        Ok(data) => {
            hbb_common::toml::from_str(&data).context("Read trusted-device authentication settings")
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(T::default()),
        Err(error) => Err(error).context("Open trusted-device authentication settings"),
    }
}

fn password(config: &Config) -> ResultType<Password> {
    Ok(serde_json::from_value(serde_json::to_value(config)?)?)
}

pub(super) fn effective_options(options: HashMap<String, String>) -> HashMap<String, String> {
    let mut effective = config::DEFAULT_SETTINGS.read().unwrap().clone();
    effective.extend(options);
    effective.extend(config::OVERWRITE_SETTINGS.read().unwrap().clone());
    effective
}

fn normalize(mut password: Password, on_disk: bool) -> (Option<[u8; 32]>, String) {
    if on_disk && password.password.starts_with("00") {
        let (plain, decrypted, _) = decrypt_str_or_original(&password.password, "00");
        if decrypted {
            password.password = plain;
        }
    }
    if !password.password.is_empty() {
        if !config::local_permanent_password_storage_is_usable_for_auth(
            &password.password,
            &password.salt,
        ) {
            return (None, password.salt);
        }
        let h1 = config::decode_permanent_password_h1_from_storage(&password.password)
            .unwrap_or_else(|| {
                config::compute_permanent_password_h1(&password.password, &password.salt)
            });
        return (Some(h1), password.salt);
    }
    let (preset, salt) = Config::get_preset_password_storage_and_salt();
    if !config::preset_permanent_password_storage_is_usable_for_auth(&preset, &salt) {
        return (None, password.salt);
    }
    if salt.is_empty() {
        (
            Some(config::compute_permanent_password_h1(
                &preset,
                &password.salt,
            )),
            password.salt,
        )
    } else {
        (
            config::decode_preset_password_h1_from_storage(&preset),
            salt,
        )
    }
}

fn build(
    password: Password,
    options: HashMap<String, String>,
    on_disk: bool,
) -> ResultType<Snapshot> {
    let (h1, salt) = normalize(password, on_disk);
    let totp = options.get("2fa").cloned().unwrap_or_default();
    let enabled = config::option2bool(
        keys::OPTION_ENABLE_TRUSTED_DEVICES,
        options
            .get(keys::OPTION_ENABLE_TRUSTED_DEVICES)
            .map(String::as_str)
            .unwrap_or_default(),
    );
    let validity_days = policy::days(&options);
    let scope = sha256::hash(&serde_json::to_vec(&(&h1, &salt, &totp, enabled))?).0;
    Ok(Snapshot {
        scope,
        h1,
        salt,
        totp,
        enabled,
        validity_days,
    })
}

pub fn snapshot() -> ResultType<Snapshot> {
    let current = build(password(&Config::get())?, Config::get_options(), false)?;
    let stored_options: Options = read(&Config2::file())?;
    let stored = build(
        read(&Config::file())?,
        effective_options(stored_options.options),
        true,
    )?;
    if !utils::memcmp(&current.scope, &stored.scope)
        || current.validity_days != stored.validity_days
    {
        bail!("Trusted-device authentication settings are out of sync; reconnect after configuration sync");
    }
    Ok(current)
}

pub(super) fn change<T>(affected: bool, apply: impl FnOnce() -> ResultType<T>) -> ResultType<T> {
    if !affected {
        return apply();
    }
    let storage = super::storage()?;
    if !storage.exists()? {
        return apply();
    }
    let mut grants = super::Grants::default();
    grants.revoke(None)?;
    storage.write(&grants)?;
    // Keep the credential lock until the new authentication settings are visible.
    let result = apply()?;
    grants.revoke(Some(snapshot()?.scope))?;
    storage.write(&grants)?;
    Ok(result)
}

pub fn set_options(options: HashMap<String, String>) -> ResultType<()> {
    policy::change_options(effective_options(options.clone()), || {
        Config::set_options(options);
        Ok(())
    })
}

pub fn set_password(value: &str) -> ResultType<bool> {
    change(changes_password(value), || {
        Ok(Config::set_permanent_password(value))
    })
}

pub fn changes_password(value: &str) -> bool {
    if Config::is_disable_change_permanent_password() {
        return false;
    }
    let (password, salt) = Config::get_local_permanent_password_storage_and_salt();
    // Match the setter's no-op cases; existing local passwords are re-encrypted.
    if !password.is_empty() {
        return true;
    }
    if value.is_empty() {
        return false;
    }
    let (h1, salt) = normalize(Password { password, salt }, false);
    h1.map(|h1| !utils::memcmp(&h1, &config::compute_permanent_password_h1(value, &salt)))
        .unwrap_or(true)
}

pub fn set_salt(salt: &str) -> ResultType<()> {
    let affected = Config::get_local_permanent_password_storage_and_salt().1 != salt;
    change(affected, || {
        Config::set_salt(salt);
        Ok(())
    })
}

pub fn set_config(value: Config) -> ResultType<bool> {
    let affected =
        normalize(password(&Config::get())?, false) != normalize(password(&value)?, false);
    change(affected, || Ok(Config::set(value)))
}

pub fn set_config2(value: Config2) -> ResultType<bool> {
    policy::change_options(effective_options(value.options.clone()), || {
        Ok(Config2::set(value))
    })
}

impl Snapshot {
    pub fn verify_password(&self, proof: &super::host::LoginProof) -> ResultType<()> {
        let h1 = self
            .h1
            .ok_or_else(|| anyhow!("Permanent password is unavailable for trusted devices"))?;
        let mut material = h1.to_vec();
        material.extend_from_slice(proof.challenge.as_bytes());
        if self.salt != proof.salt || !utils::memcmp(&sha256::hash(&material).0, &proof.password) {
            bail!(
                "Trusted-device password configuration changed; reconnect and authenticate again"
            );
        }
        Ok(())
    }
}
