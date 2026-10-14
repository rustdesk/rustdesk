use hbb_common::{anyhow::anyhow, config::Config2, ResultType};
use std::sync::{Mutex, MutexGuard};

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
        let value = Config2::get_trusted_devices_v2()?;
        if value.is_empty() {
            return Ok(super::Grants::default());
        }
        serde_json::from_str(&value)
            .map_err(|_| hbb_common::anyhow::anyhow!("Invalid trusted-device grants"))
    }

    pub fn exists(&self) -> ResultType<bool> {
        Ok(!Config2::get_trusted_devices_v2()?.is_empty())
    }

    pub fn write(&self, grants: &super::Grants) -> ResultType<()> {
        Config2::set_trusted_devices_v2(serde_json::to_string(grants)?)
    }
}
