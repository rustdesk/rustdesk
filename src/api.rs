use std::{ffi::CStr, os::raw::c_char};

use crate::plugins::PLUGIN_REGISTRAR;

pub type LoadPluginFunc = fn(*const i8) -> i32;
pub type UnloadPluginFunc = fn(*const i8) -> i32;

pub struct RustDeskApiTable {
    pub register_plugin: LoadPluginFunc,
    pub unload_plugin: UnloadPluginFunc,
}

#[no_mangle]
fn load_plugin(path: *const i8) -> i32 {
    PLUGIN_REGISTRAR.load_plugin(path)
}

#[no_mangle]
fn unload_plugin(path: *const i8) -> i32 {
    PLUGIN_REGISTRAR.unload_plugin(path)
}

impl Default for RustDeskApiTable {
    fn default() -> Self {
        let f = load_plugin;
        Self {
            register_plugin: load_plugin,
            unload_plugin: unload_plugin,
        }
    }
}
