use std::{collections::HashMap, ffi::c_void, os::raw::c_int};

use serde_json::json;

use crate::{define_method_prefix, flutter::APP_TYPE_MAIN};

use super::PluginNativeHandler;

#[derive(Default)]
pub struct PluginNativeUIHandler;

/// Callback for UI interface.
///
/// [Note]
/// We will transfer the native callback to u64 and post it to flutter.
/// The flutter thread will directly call this method.
///
/// an example of `data` is:
/// ```
/// {
///     "cb": 0x1234567890
/// }
/// ```
/// [Safety]
/// Please make sure the callback u provided is VALID, or memory or calling issues may occur to cause the program crash!
pub type OnUIReturnCallback =
    extern "C" fn(return_code: c_int, data: *const c_void, data_len: u64, user_data: *const c_void);

impl PluginNativeHandler for PluginNativeUIHandler {
    define_method_prefix!("ui_");

    fn on_message(
        &self,
        method: &str,
        data: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<super::NR> {
        match method {
            "select_peers_async" => {
                if let Some(cb) = data.get("cb") {
                    if let Some(cb) = cb.as_u64() {
                        let user_data = match data.get("user_data") {
                            Some(user_data) => user_data.as_u64().unwrap_or(0),
                            None => 0,
                        };
                        self.select_peers_async(cb, user_data);
                        return Some(super::NR {
                            return_type: 0,
                            data: std::ptr::null(),
                        });
                    }
                }
                return Some(super::NR {
                    return_type: -1,
                    data: "missing cb field message".as_ptr() as _,
                });
            }
            "register_ui_entry" => {
                let title;
                if let Some(v) = data.get("title") {
                    title = v.as_str().unwrap_or("");
                } else {
                    title = "";
                }
                if let Some(on_tap_cb) = data.get("on_tap_cb") {
                    if let Some(on_tap_cb) = on_tap_cb.as_u64() {
                        let user_data = match data.get("user_data") {
                            Some(user_data) => user_data.as_u64().unwrap_or(0),
                            None => 0,
                        };
                        self.register_ui_entry(title, on_tap_cb, user_data);
                        return Some(super::NR {
                            return_type: 0,
                            data: std::ptr::null(),
                        });
                    }
                }
                return Some(super::NR {
                    return_type: -1,
                    data: "missing cb field message".as_ptr() as _,
                });
            }
            _ => {}
        }
        None
    }

    fn on_message_raw(
        &self,
        method: &str,
        data: &serde_json::Map<String, serde_json::Value>,
        raw: *const std::ffi::c_void,
        _raw_len: usize,
    ) -> Option<super::NR> {
        None
    }
}

impl PluginNativeUIHandler {
    /// Call with method `select_peers_async` and the following json:
    /// ```json
    /// {
    ///     "cb": 0, // The function address
    ///     "user_data": 0 // An opaque pointer value passed to the callback.
    /// }
    /// ```
    ///
    /// [Arguments]
    /// @param cb: the function address with type [OnUIReturnCallback].
    /// @param user_data: the function will be called with this value.
    fn select_peers_async(&self, cb: u64, user_data: u64) {
        let mut param = HashMap::new();
        param.insert("name", json!("native_ui"));
        param.insert("action", json!("select_peers"));
        param.insert("cb", json!(cb));
        param.insert("user_data", json!(user_data));
        crate::flutter::push_global_event(
            APP_TYPE_MAIN,
            serde_json::to_string(&param).unwrap_or("".to_string()),
        );
    }

    /// Call with method `register_ui_entry` and the following json:
    /// ```
    /// {
    ///     
    ///     "on_tap_cb": 0, // The function address
    ///     "user_data": 0, // An opaque pointer value passed to the callback.
    ///     "title": "entry name"
    /// }
    /// ```
    fn register_ui_entry(&self, title: &str, on_tap_cb: u64, user_data: u64) {
        let mut param = HashMap::new();
        param.insert("name", json!("native_ui"));
        param.insert("action", json!("register_ui_entry"));
        param.insert("title", json!(title));
        param.insert("cb", json!(on_tap_cb));
        param.insert("user_data", json!(user_data));
        crate::flutter::push_global_event(
            APP_TYPE_MAIN,
            serde_json::to_string(&param).unwrap_or("".to_string()),
        );
    }
}
