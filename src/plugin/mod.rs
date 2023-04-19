use hbb_common::ResultType;
use std::ffi::{c_char, CStr};

mod callback_msg;
mod config;
pub mod desc;
mod plugins;
mod errno;

pub use plugins::{
    handle_client_event, handle_ui_event, load_plugin, load_plugins, reload_plugin, unload_plugin,
};

#[inline]
fn cstr_to_string(cstr: *const c_char) -> ResultType<String> {
    Ok(String::from_utf8(unsafe {
        CStr::from_ptr(cstr).to_bytes().to_vec()
    })?)
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
    Ok((code, msg))
}
