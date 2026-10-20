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
        let mut grants = if value.is_empty() {
            super::Grants::default()
        } else if let Some(ciphertext) = value.strip_prefix(ENCRYPTION_VERSION) {
            let ciphertext = base64::decode(ciphertext, base64::Variant::Original)
                .map_err(|_| anyhow!("Invalid encrypted trusted-device grants"))?;
            let plaintext = symmetric_crypt(&ciphertext, false)
                .map_err(|_| anyhow!("Could not decrypt trusted-device grants"))?;
            serde_json::from_slice(&plaintext)
                .map_err(|_| anyhow!("Invalid trusted-device grants"))?
        } else {
            serde_json::from_str(&value).map_err(|_| anyhow!("Invalid trusted-device grants"))?
        };
        if !legacy.is_empty() {
            super::legacy::import(&mut grants, &legacy, value.is_empty())?;
        }
        if !legacy.is_empty() || (!value.is_empty() && !value.starts_with(ENCRYPTION_VERSION)) {
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
