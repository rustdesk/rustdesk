#![allow(dead_code)]

use hbb_common::{bail, ResultType};
use std::sync::{Arc, Mutex};

const LIB_NAME_VIRTUAL_DISPLAY: &str = "virtual_display";

lazy_static::lazy_static! {
    static ref LIB_VIRTUAL_DISPLAY: Arc<Mutex<Result<libloading::Library, libloading::Error>>> = {
        #[cfg(target_os = "windows")]
        let libname = format!("{}.dll", LIB_NAME_VIRTUAL_DISPLAY);
        #[cfg(target_os = "linux")]
        let libname = format!("lib{}.so", LIB_NAME_VIRTUAL_DISPLAY);
        #[cfg(target_os = "macos")]
        let libname = format!("lib{}.dylib", LIB_NAME_VIRTUAL_DISPLAY);
        Arc::new(Mutex::new(unsafe { libloading::Library::new(libname) }))
    };
}

pub(super) fn is_device_created() -> bool {
    match &*LIB_VIRTUAL_DISPLAY.lock().unwrap() {
        Ok(lib) => unsafe {
            match lib.get::<libloading::Symbol<fn() -> bool>>(b"is_device_created") {
                Ok(func) => func(),
                Err(..) => false,
            }
        },
        Err(..) => false,
    }
}

macro_rules! def_func_result {
    ($func:ident, $name: tt) => {
        pub(super) fn $func() -> ResultType<()> {
            match &*LIB_VIRTUAL_DISPLAY.lock().unwrap() {
                Ok(lib) => unsafe {
                    match lib.get::<libloading::Symbol<fn() -> ResultType<()>>>($name.as_bytes()) {
                        Ok(func) => func(),
                        Err(..) => bail!("Failed to load func {}", $name),
                    }
                },
                Err(e) => bail!("Failed to load library {}, {}", LIB_NAME_VIRTUAL_DISPLAY, e),
            }
        }
    };
}

def_func_result!(create_device, "create_device");

pub(super) fn close_device() {
    match &*LIB_VIRTUAL_DISPLAY.lock().unwrap() {
        Ok(lib) => unsafe {
            match lib.get::<libloading::Symbol<fn()>>(b"close_device") {
                Ok(func) => func(),
                Err(..) => {},
            }
        },
        Err(..) => {},
    }
}

def_func_result!(plug_in_monitor, "plug_in_monitor");
def_func_result!(plug_out_monitor, "plug_out_monitor");
def_func_result!(update_monitor_modes, "update_monitor_modes");
