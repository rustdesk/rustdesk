use std::{ffi::{c_char, c_void}, os::raw::c_uint};

/// The native returned value from librustdesk native.
/// 
/// [Note]
/// The data is owned by librustdesk.
#[repr(C)]
pub struct NativeReturnValue{
    return_type: c_uint,
    data: *const c_void
}

pub(super) extern "C" fn cb_native_data(method: *const c_char, json: *const c_char, raw: *const c_void, raw_len: usize) -> NativeReturnValue {
    // TODO: cb for native data.
    return NativeReturnValue { return_type: 0, data: std::ptr::null() };
}