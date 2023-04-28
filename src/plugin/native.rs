use std::{
    ffi::{c_char, c_int, c_void},
    os::raw::c_uint,
};

use hbb_common::log::error;

use super::{
    cstr_to_string,
    errno::ERR_NOT_HANDLED,
    native_handlers::{Callable, NATIVE_HANDLERS_REGISTRAR},
};
/// The native returned value from librustdesk native.
///
/// [Note]
/// The data is owned by librustdesk.
#[repr(C)]
pub struct NativeReturnValue {
    pub return_type: c_int,
    pub data: *const c_void,
}

pub(super) extern "C" fn cb_native_data(
    method: *const c_char,
    json: *const c_char,
    raw: *const c_void,
    raw_len: usize,
) -> NativeReturnValue {
    let ret = match cstr_to_string(method) {
        Ok(method) => NATIVE_HANDLERS_REGISTRAR.call(&method, json, raw, raw_len),
        Err(err) => {
            error!("cb_native_data error: {}", err);
            None
        }
    };
    return ret.unwrap_or(NativeReturnValue {
        return_type: ERR_NOT_HANDLED,
        data: std::ptr::null(),
    });
}
