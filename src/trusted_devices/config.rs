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

const AUTH_OPTIONS: [&str; 2] = ["2fa", keys::OPTION_ENABLE_TRUSTED_DEVICES];
const SYNC_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

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
    if affected {
        super::authority::manage(true).await?;
    }
    let result = update(apply).await?;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if affected && super::authority::uses_service()? {
        sync_auth_to_service().await?;
    }
    Ok(result)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn sync_auth_to_service() -> ResultType<()> {
    use crate::ipc::{connect_trusted_device_service, Data};
    const TIMEOUT_MS: u64 = 1000;
    let mut connection = connect_trusted_device_service(TIMEOUT_MS).await?;
    connection
        .send(&Data::SyncConfig(Some(
            (Config::get(), Config2::get()).into(),
        )))
        .await?;
    match connection.next_timeout(TIMEOUT_MS).await? {
        Some(Data::SyncConfig(None)) => Ok(()),
        Some(Data::SecurityConfigError(error)) => bail!(error),
        _ => bail!("The system service did not confirm the authentication configuration update"),
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

fn effective_options(options: HashMap<String, String>) -> HashMap<String, String> {
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
    let scope = sha256::hash(&serde_json::to_vec(&(&h1, &salt, &totp, enabled))?).0;
    Ok(Snapshot {
        scope,
        h1,
        salt,
        totp,
        enabled,
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
    if !utils::memcmp(&current.scope, &stored.scope) {
        bail!("Trusted-device authentication settings are out of sync; reconnect after configuration sync");
    }
    Ok(current)
}

fn change<T>(affected: bool, apply: impl FnOnce() -> ResultType<T>) -> ResultType<T> {
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
    let affected = changes_authentication(&options);
    change(affected, || {
        Config::set_options(options);
        Ok(())
    })
}

pub fn changes_authentication(options: &HashMap<String, String>) -> bool {
    let old = Config::get_options();
    let new = effective_options(options.clone());
    AUTH_OPTIONS
        .iter()
        .any(|key| old.get(*key) != new.get(*key))
}

pub fn import(apply: impl FnOnce() -> ResultType<()>) -> ResultType<()> {
    let storage = super::storage()?;
    let mut grants = super::Grants::default();
    grants.revoke(None)?;
    storage.write(&grants)?;
    // Imported files become the runtime settings when the host next starts.
    apply()
}

pub fn set_password(value: &str) -> ResultType<bool> {
    change(true, || Ok(Config::set_permanent_password(value)))
}

pub fn sync_password(storage: &str, salt: &str) -> ResultType<bool> {
    let affected =
        Config::get_local_permanent_password_storage_and_salt() != (storage.into(), salt.into());
    change(affected, || {
        Config::set_permanent_password_storage_for_sync(storage, salt)
    })
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
    let affected = changes_authentication(&value.options);
    change(affected, || Ok(Config2::set(value)))
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
