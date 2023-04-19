use hbb_common::ResultType;
use std::ffi::{c_char, CStr};

mod callback_msg;
mod config;
pub mod desc;
mod plugins;

pub use plugins::load_plugins;

#[inline]
fn cstr_to_string(cstr: *const c_char) -> ResultType<String> {
    Ok(String::from_utf8(unsafe {
        CStr::from_ptr(cstr).to_bytes().to_vec()
    })?)
}
