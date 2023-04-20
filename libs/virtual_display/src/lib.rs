use hbb_common::{anyhow, dlopen::symbor::Library, log, ResultType};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

const LIB_NAME_VIRTUAL_DISPLAY: &str = "dylib_virtual_display";

pub type DWORD = ::std::os::raw::c_ulong;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _MonitorMode {
    pub width: DWORD,
    pub height: DWORD,
    pub sync: DWORD,
}
pub type MonitorMode = _MonitorMode;
pub type PMonitorMode = *mut MonitorMode;

pub type GetDriverInstallPath = fn() -> &'static str;
pub type IsDeviceCreated = fn() -> bool;
pub type CloseDevice = fn();
pub type DownLoadDriver = fn() -> ResultType<()>;
pub type CreateDevice = fn() -> ResultType<()>;
pub type InstallUpdateDriver = fn(&mut bool) -> ResultType<()>;
pub type UninstallDriver = fn(&mut bool) -> ResultType<()>;
pub type PlugInMonitor = fn(u32) -> ResultType<()>;
pub type PlugOutMonitor = fn(u32) -> ResultType<()>;
pub type UpdateMonitorModes = fn(u32, u32, PMonitorMode) -> ResultType<()>;

macro_rules! make_lib_wrapper {
    ($($field:ident : $tp:ty),+) => {
        struct LibWrapper {
            _lib: Option<Library>,
            $($field: Option<$tp>),+
        }

        impl LibWrapper {
            fn new() -> Self {
                let lib = match Library::open(get_lib_name()) {
                    Ok(lib) => Some(lib),
                    Err(e) => {
                        log::warn!("Failed to load library {}, {}", LIB_NAME_VIRTUAL_DISPLAY, e);
                        None
                    }
                };

                $(let $field = if let Some(lib) = &lib {
                    match unsafe { lib.symbol::<$tp>(stringify!($field)) } {
                        Ok(m) => {
                            log::info!("method found {}", stringify!($field));
                            Some(*m)
                        },
                        Err(e) => {
                            log::warn!("Failed to load func {}, {}", stringify!($field), e);
                            None
                        }
                    }
                } else {
                    None
                };)+

                Self {
                    _lib: lib,
                    $( $field ),+
                }
            }
        }

        impl Default for LibWrapper {
            fn default() -> Self {
                Self::new()
            }
        }
    }
}

make_lib_wrapper!(
    get_driver_install_path: GetDriverInstallPath,
    is_device_created: IsDeviceCreated,
    close_device: CloseDevice,
    download_driver: DownLoadDriver,
    create_device: CreateDevice,
    install_update_driver: InstallUpdateDriver,
    uninstall_driver: UninstallDriver,
    plug_in_monitor: PlugInMonitor,
    plug_out_monitor: PlugOutMonitor,
    update_monitor_modes: UpdateMonitorModes
);

lazy_static::lazy_static! {
    static ref LIB_WRAPPER: Arc<Mutex<LibWrapper>> = Default::default();
    static ref MONITOR_INDICES: Mutex<HashSet<u32>> = Mutex::new(HashSet::new());
}

#[cfg(target_os = "windows")]
fn get_lib_name() -> String {
    format!("{}.dll", LIB_NAME_VIRTUAL_DISPLAY)
}

#[cfg(target_os = "linux")]
fn get_lib_name() -> String {
    format!("lib{}.so", LIB_NAME_VIRTUAL_DISPLAY)
}

#[cfg(target_os = "macos")]
fn get_lib_name() -> String {
    format!("lib{}.dylib", LIB_NAME_VIRTUAL_DISPLAY)
}

#[cfg(windows)]
pub fn get_driver_install_path() -> Option<&'static str> {
    Some(LIB_WRAPPER.lock().unwrap().get_driver_install_path?())
}

pub fn is_device_created() -> bool {
    LIB_WRAPPER
        .lock()
        .unwrap()
        .is_device_created
        .map(|f| f())
        .unwrap_or(false)
}

pub fn close_device() {
    let _r = LIB_WRAPPER.lock().unwrap().close_device.map(|f| f());
}

pub fn download_driver() -> ResultType<()> {
    LIB_WRAPPER
        .lock()
        .unwrap()
        .download_driver
        .ok_or(anyhow::Error::msg("download_driver method not found"))?()
}

pub fn create_device() -> ResultType<()> {
    LIB_WRAPPER
        .lock()
        .unwrap()
        .create_device
        .ok_or(anyhow::Error::msg("create_device method not found"))?()
}

pub fn install_update_driver(reboot_required: &mut bool) -> ResultType<()> {
    LIB_WRAPPER
        .lock()
        .unwrap()
        .install_update_driver
        .ok_or(anyhow::Error::msg("install_update_driver method not found"))?(reboot_required)
}

pub fn uninstall_driver(reboot_required: &mut bool) -> ResultType<()> {
    LIB_WRAPPER
        .lock()
        .unwrap()
        .uninstall_driver
        .ok_or(anyhow::Error::msg("uninstall_driver method not found"))?(reboot_required)
}

#[cfg(windows)]
pub fn plug_in_monitor(monitor_index: u32) -> ResultType<()> {
    let mut lock = MONITOR_INDICES.lock().unwrap();
    if lock.contains(&monitor_index) {
        return Ok(());
    }
    let f = LIB_WRAPPER
        .lock()
        .unwrap()
        .plug_in_monitor
        .ok_or(anyhow::Error::msg("plug_in_monitor method not found"))?;
    f(monitor_index)?;
    lock.insert(monitor_index);
    Ok(())
}

#[cfg(windows)]
pub fn plug_out_monitor(monitor_index: u32) -> ResultType<()> {
    let f = LIB_WRAPPER
        .lock()
        .unwrap()
        .plug_out_monitor
        .ok_or(anyhow::Error::msg("plug_out_monitor method not found"))?;
    f(monitor_index)?;
    MONITOR_INDICES.lock().unwrap().remove(&monitor_index);
    Ok(())
}

#[cfg(windows)]
pub fn update_monitor_modes(monitor_index: u32, modes: &[MonitorMode]) -> ResultType<()> {
    let f = LIB_WRAPPER
        .lock()
        .unwrap()
        .update_monitor_modes
        .ok_or(anyhow::Error::msg("update_monitor_modes method not found"))?;
    f(monitor_index, modes.len() as _, modes.as_ptr() as _)
}
