use librustdesk::{api::RustDeskApiTable};
/// This file demonstrates how to write a custom plugin for RustDesk.
use std::ffi::{c_char, c_int, CString};

lazy_static::lazy_static! {
    pub static ref PLUGIN_NAME: CString = CString::new("A Template Rust Plugin").unwrap();
    pub static ref PLUGIN_ID: CString = CString::new("TemplatePlugin").unwrap();
    // Do your own logic based on the API provided by RustDesk.
    pub static ref API: RustDeskApiTable = RustDeskApiTable::default();
}

#[no_mangle]
fn plugin_name() -> *const c_char {
    return PLUGIN_NAME.as_ptr();
}

#[no_mangle]
fn plugin_id() -> *const c_char {
    return PLUGIN_ID.as_ptr();
}

#[no_mangle]
fn plugin_init() -> c_int {
    return 0 as _;
}

#[no_mangle]
fn plugin_dispose() -> c_int {
    return 0 as _;
}
