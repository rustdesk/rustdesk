use std::ffi::{c_void, c_ulonglong, c_ulong};

use serde_json::Map;

pub mod session;

pub type NR = super::native::NativeReturnValue;
pub type PluginNativeHandlerRegistrar = NativeHandlerRegistrar<dyn PluginNativeHandler>;

pub struct NativeHandlerRegistrar<H>{
    handlers: Vec<H>
}

pub(crate) trait PluginNativeHandler {
    /// Try to handle the method with the given data.
    /// 
    /// Returns: None for the message does not be handled by this handler.
    fn on_message(method: &String, data: &Map<String, serde_json::Value>) -> Option<NR>;

    /// Try to handle the method with the given data and extra void binary data.
    /// 
    /// Returns: None for the message does not be handled by this handler.
    fn on_message_raw(method: &String, data: &Map<String, serde_json::Value>, raw: *const c_void, raw_len: usize) -> Option<NR>; 
}