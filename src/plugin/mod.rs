use hbb_common::{libc, ResultType};
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    ptr::null,
};

mod callback_ext;
mod callback_msg;
mod config;
pub mod desc;
mod errno;
pub mod ipc;
pub mod native;
pub mod native_handlers;
mod plog;
mod plugins;

pub use plugins::{
    handle_client_event, handle_listen_event, handle_server_event, handle_ui_event, load_plugin,
    load_plugins, reload_plugin, sync_ui, unload_plugin, unload_plugins,
};

const MSG_TO_UI_TYPE_PLUGIN_DESC: &str = "plugin_desc";
const MSG_TO_UI_TYPE_PLUGIN_EVENT: &str = "plugin_event";
const MSG_TO_UI_TYPE_PLUGIN_RELOAD: &str = "plugin_reload";
const MSG_TO_UI_TYPE_PLUGIN_OPTION: &str = "plugin_option";

pub const EVENT_ON_CONN_CLIENT: &str = "on_conn_client";
pub const EVENT_ON_CONN_SERVER: &str = "on_conn_server";
pub const EVENT_ON_CONN_CLOSE_CLIENT: &str = "on_conn_close_client";
pub const EVENT_ON_CONN_CLOSE_SERVER: &str = "on_conn_close_server";

pub use config::{ManagerConfig, PeerConfig, SharedConfig};

/// Common plugin return.
///
/// [Note]
/// The msg must be nullptr if code is errno::ERR_SUCCESS.
/// The msg must be freed by caller if code is not errno::ERR_SUCCESS.
#[repr(C)]
#[derive(Debug)]
pub struct PluginReturn {
    pub code: c_int,
    pub msg: *const c_char,
}

impl PluginReturn {
    pub fn success() -> Self {
        Self {
            code: errno::ERR_SUCCESS,
            msg: null(),
        }
    }

    #[inline]
    pub fn is_success(&self) -> bool {
        self.code == errno::ERR_SUCCESS
    }

    pub fn new(code: c_int, msg: &str) -> Self {
        Self {
            code,
            msg: str_to_cstr_ret(msg),
        }
    }

    pub fn get_code_msg(&mut self) -> (i32, String) {
        if self.is_success() {
            (self.code, "".to_owned())
        } else {
            assert!(!self.msg.is_null());
            let msg = cstr_to_string(self.msg).unwrap_or_default();
            free_c_ptr(self.msg as _);
            self.msg = null();
            (self.code as _, msg)
        }
    }
}

#[inline]
fn cstr_to_string(cstr: *const c_char) -> ResultType<String> {
    Ok(String::from_utf8(unsafe {
        CStr::from_ptr(cstr).to_bytes().to_vec()
    })?)
}

#[inline]
fn str_to_cstr_ret(s: &str) -> *const c_char {
    let mut s = s.as_bytes().to_vec();
    s.push(0);
    unsafe {
        let r = libc::malloc(s.len()) as *mut c_char;
        libc::memcpy(
            r as *mut libc::c_void,
            s.as_ptr() as *const libc::c_void,
            s.len(),
        );
        r
    }
}

#[inline]
fn free_c_ptr(ret: *mut c_void) {
    if !ret.is_null() {
        unsafe {
            libc::free(ret);
        }
    }
}
