#[cfg(windows)]
pub use dylib_virtual_display::win10;

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

pub fn is_device_created() -> bool {
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

pub fn close_device() {
    match &*LIB_VIRTUAL_DISPLAY.lock().unwrap() {
        Ok(lib) => unsafe {
            match lib.get::<libloading::Symbol<fn()>>(b"close_device") {
                Ok(func) => func(),
                Err(..) => {}
            }
        },
        Err(..) => {}
    }
}

macro_rules! def_func_result {
    ($func:ident, $name: tt) => {
        pub fn $func() -> ResultType<()> {
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

pub fn install_update_driver(reboot_required: &mut bool) -> ResultType<()> {
    match &*LIB_VIRTUAL_DISPLAY.lock().unwrap() {
        Ok(lib) => unsafe {
            match lib.get::<libloading::Symbol<fn(&mut bool) -> ResultType<()>>>(b"install_update_driver") {
                Ok(func) => func(reboot_required),
                Err(..) => bail!("Failed to load func install_update_driver"),
            }
        },
        Err(e) => bail!("Failed to load library {}, {}", LIB_NAME_VIRTUAL_DISPLAY, e),
    }
}

pub fn uninstall_driver(reboot_required: &mut bool) -> ResultType<()> {
    match &*LIB_VIRTUAL_DISPLAY.lock().unwrap() {
        Ok(lib) => unsafe {
            match lib.get::<libloading::Symbol<fn(&mut bool) -> ResultType<()>>>(b"uninstall_driver") {
                Ok(func) => func(reboot_required),
                Err(..) => bail!("Failed to load func uninstall_driver"),
            }
        },
        Err(e) => bail!("Failed to load library {}, {}", LIB_NAME_VIRTUAL_DISPLAY, e),
    }
}

def_func_result!(download_driver, "download_driver");
def_func_result!(create_device, "create_device");
def_func_result!(plug_in_monitor, "plug_in_monitor");
def_func_result!(plug_out_monitor, "plug_out_monitor");
def_func_result!(update_monitor_modes, "update_monitor_modes");
