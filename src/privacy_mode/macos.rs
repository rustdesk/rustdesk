use super::{PrivacyMode, PrivacyModeState};
use hbb_common::{anyhow::anyhow, ResultType};

extern "C" {
    fn MacSetPrivacyMode(on: bool) -> bool;
}

pub const PRIVACY_MODE_IMPL: &str = "privacy_mode_impl_macos";

pub struct PrivacyModeImpl {
    impl_key: String,
    conn_id: i32,
}

impl PrivacyModeImpl {
    pub fn new(impl_key: &str) -> Self {
        Self {
            impl_key: impl_key.to_owned(),
            conn_id: 0,
        }
    }
}

impl PrivacyMode for PrivacyModeImpl {
    fn is_async_privacy_mode(&self) -> bool {
        false
    }

    fn init(&self) -> ResultType<()> {
        Ok(())
    }

    fn clear(&mut self) {
        unsafe {
            MacSetPrivacyMode(false);
        }
        self.conn_id = 0;
    }

    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool> {
        if self.check_on_conn_id(conn_id)? {
            return Ok(true);
        }
        let success = unsafe { MacSetPrivacyMode(true) };
        if !success {
            return Err(anyhow!("Failed to turn on privacy mode"));
        }
        self.conn_id = conn_id;
        Ok(true)
    }

    fn turn_off_privacy(&mut self, conn_id: i32, _state: Option<PrivacyModeState>) -> ResultType<()> {
        self.check_off_conn_id(conn_id)?;
        let success = unsafe { MacSetPrivacyMode(false) };
        if !success {
            return Err(anyhow!("Failed to turn off privacy mode"));
        }
        self.conn_id = 0;
        Ok(())
    }

    fn pre_conn_id(&self) -> i32 {
        self.conn_id
    }

    fn get_impl_key(&self) -> &str {
        &self.impl_key
    }
}

impl Drop for PrivacyModeImpl {
    fn drop(&mut self) {
        unsafe {
            MacSetPrivacyMode(false);
        }
    }
}
