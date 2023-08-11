use std::{
    ffi::c_void,
    sync::{Arc, RwLock},
    vec,
};

use hbb_common::libc::c_char;
use lazy_static::lazy_static;
use serde_json::Map;

use crate::return_if_not_method;

use self::{session::PluginNativeSessionHandler, ui::PluginNativeUIHandler};

use super::cstr_to_string;

mod macros;
pub mod session;
pub mod ui;

pub type NR = super::native::NativeReturnValue;
pub type PluginNativeHandlerRegistrar = NativeHandlerRegistrar<Box<dyn Callable + Send + Sync>>;

lazy_static! {
    pub static ref NATIVE_HANDLERS_REGISTRAR: Arc<PluginNativeHandlerRegistrar> =
        Arc::new(PluginNativeHandlerRegistrar::default());
}

#[derive(Clone)]
pub struct NativeHandlerRegistrar<H> {
    handlers: Arc<RwLock<Vec<H>>>,
}

impl Default for PluginNativeHandlerRegistrar {
    fn default() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(vec![
                // Add prebuilt native handlers here.
                Box::new(PluginNativeSessionHandler::default()),
                Box::new(PluginNativeUIHandler::default()),
            ])),
        }
    }
}

pub(self) trait PluginNativeHandler {
    /// The method prefix handled by this handler.s
    fn method_prefix(&self) -> &'static str;

    /// Try to handle the method with the given data.
    ///
    /// Returns: None for the message does not be handled by this handler.
    fn on_message(&self, method: &str, data: &Map<String, serde_json::Value>) -> Option<NR>;

    /// Try to handle the method with the given data and extra void binary data.
    ///
    /// Returns: None for the message does not be handled by this handler.
    fn on_message_raw(
        &self,
        method: &str,
        data: &Map<String, serde_json::Value>,
        raw: *const c_void,
        raw_len: usize,
    ) -> Option<NR>;
}

pub trait Callable {
    fn call(
        &self,
        method: &String,
        json: *const c_char,
        raw: *const c_void,
        raw_len: usize,
    ) -> Option<NR> {
        None
    }
}

impl<T> Callable for T
where
    T: PluginNativeHandler + Send + Sync,
{
    fn call(
        &self,
        method: &String,
        json: *const c_char,
        raw: *const c_void,
        raw_len: usize,
    ) -> Option<NR> {
        let prefix = self.method_prefix();
        return_if_not_method!(method, prefix);
        match cstr_to_string(json) {
            Ok(s) => {
                if let Ok(json) = serde_json::from_str(s.as_str()) {
                    let method_suffix = &method[prefix.len()..];
                    if raw != std::ptr::null() && raw_len > 0 {
                        return self.on_message_raw(method_suffix, &json, raw, raw_len);
                    } else {
                        return self.on_message(method_suffix, &json);
                    }
                } else {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
}

impl Callable for PluginNativeHandlerRegistrar {
    fn call(
        &self,
        method: &String,
        json: *const c_char,
        raw: *const c_void,
        raw_len: usize,
    ) -> Option<NR> {
        for handler in self.handlers.read().unwrap().iter() {
            let ret = handler.call(method, json, raw, raw_len);
            if ret.is_some() {
                return ret;
            }
        }
        None
    }
}
