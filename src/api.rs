use std::ffi::c_char;

use crate::plugins::PLUGIN_REGISTRAR;

// API provided by RustDesk.
pub type LoadPluginFunc = fn(*const c_char) -> i32;
pub type UnloadPluginFunc = fn(*const c_char) -> i32;

#[repr(C)]
pub struct RustDeskApiTable {
    pub register_plugin: LoadPluginFunc,
    pub unload_plugin: UnloadPluginFunc,
}

#[no_mangle]
fn load_plugin(path: *const c_char) -> i32 {
    PLUGIN_REGISTRAR.load_plugin(path)
}

#[no_mangle]
fn unload_plugin(path: *const c_char) -> i32 {
    PLUGIN_REGISTRAR.unload_plugin(path)
}

#[no_mangle]
fn get_api_table() -> RustDeskApiTable {
    RustDeskApiTable::default()
}

impl Default for RustDeskApiTable {
    fn default() -> Self {
        Self {
            register_plugin: load_plugin,
            unload_plugin: unload_plugin,
        }
    }
}
