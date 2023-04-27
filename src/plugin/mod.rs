use hbb_common::{libc, ResultType};
use std::ffi::{c_char, c_void, CStr};

mod callback_msg;
mod config;
pub mod desc;
mod errno;
pub mod ipc;
mod plog;
mod plugins;
pub mod native;
mod native_handlers;

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
fn get_code_msg_from_ret(ret: *const c_void) -> (i32, String) {
    assert!(ret.is_null() == false);
    let code_bytes = unsafe { std::slice::from_raw_parts(ret as *const u8, 4) };
    let code = i32::from_le_bytes([code_bytes[0], code_bytes[1], code_bytes[2], code_bytes[3]]);
    let msg = unsafe { CStr::from_ptr((ret as *const u8).add(4) as _) }
        .to_str()
        .unwrap_or("")
        .to_string();
    (code, msg)
}

#[inline]
fn free_c_ptr(ret: *mut c_void) {
    if !ret.is_null() {
        unsafe {
            libc::free(ret);
        }
    }
}
