use crate::{call_if_method, define_method_prefix, return_if_not_method};

use super::PluginNativeHandler;

#[derive(Default)]
/// Session related handler for librustdesk core.
pub struct PluginNativeSessionHandler;

impl PluginNativeHandler for PluginNativeSessionHandler {
    define_method_prefix!("session_");

    fn on_message(
        &self,
        method: &str,
        data: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<super::NR> {
        None
    }

    fn on_message_raw(
        &self,
        method: &str,
        data: &serde_json::Map<String, serde_json::Value>,
        raw: *const std::ffi::c_void,
        raw_len: usize,
    ) -> Option<super::NR> {
        None
    }
}

impl PluginNativeSessionHandler {
    fn create_session() {}

    fn add_session_hook() {}
}
