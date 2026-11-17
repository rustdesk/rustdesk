use hbb_common::{
    anyhow::anyhow, config::Config2, password_security::symmetric_crypt, sodiumoxide::base64,
    ResultType,
};
use std::sync::{Mutex, MutexGuard};

const ENCRYPTION_VERSION: &str = "00";
static GRANTS_LOCK: Mutex<()> = Mutex::new(());

pub(super) struct GrantsStorage {
    _lock: MutexGuard<'static, ()>,
}

impl GrantsStorage {
    pub fn lock() -> ResultType<Self> {
        Ok(Self {
            _lock: GRANTS_LOCK
                .lock()
                .map_err(|_| anyhow!("Trusted-device storage lock is poisoned"))?,
        })
    }

    pub fn read(&self) -> ResultType<super::Grants> {
        let (value, legacy) = Config2::get_trusted_device_stores()?;
        let mut grants = match decode(&value) {
            Ok(grants) => grants,
            Err(error) => {
                const LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
                hbb_common::throttled_log!(
                    LOG_INTERVAL,
                    warn,
                    "Revoking unreadable trusted-device grants: {error}"
                );
                // An unreadable V2 scope cannot authorize a remaining legacy copy.
                Config2::set_trusted_devices_v2(String::new())?;
                return Ok(super::Grants::default());
            }
        };
        if !legacy.is_empty() {
            super::legacy::import(&mut grants, &legacy, value.is_empty())?;
            self.write(&grants)?;
        }
        Ok(grants)
    }

    pub fn exists(&self) -> ResultType<bool> {
        Ok(Config2::has_trusted_devices_v2()?
            || !Config2::get_trusted_device_stores()?.1.is_empty())
    }

    pub fn write(&self, grants: &super::Grants) -> ResultType<()> {
        let ciphertext = symmetric_crypt(&serde_json::to_vec(grants)?, true)
            .map_err(|_| anyhow!("Could not encrypt trusted-device grants"))?;
        Config2::set_trusted_devices_v2(format!(
            "{ENCRYPTION_VERSION}{}",
            base64::encode(&ciphertext, base64::Variant::Original)
        ))
    }
}

fn decode(value: &str) -> ResultType<super::Grants> {
    if value.is_empty() {
        return Ok(super::Grants::default());
    }
    let ciphertext = value
        .strip_prefix(ENCRYPTION_VERSION)
        .ok_or_else(|| anyhow!("Invalid encrypted trusted-device grants"))?;
    let ciphertext = base64::decode(ciphertext, base64::Variant::Original)
        .map_err(|_| anyhow!("Invalid encrypted trusted-device grants"))?;
    let plaintext = symmetric_crypt(&ciphertext, false)
        .map_err(|_| anyhow!("Could not decrypt trusted-device grants"))?;
    serde_json::from_slice(&plaintext).map_err(|_| anyhow!("Invalid trusted-device grants"))
}
