#[cfg(windows)]
pub mod win10;

use hbb_common::{bail, lazy_static, ResultType};
use std::path::Path;

#[cfg(windows)]
use std::sync::Mutex;

#[cfg(windows)]
lazy_static::lazy_static! {
    // If device is uninstalled though "Device Manager" Window.
    // Rustdesk is unable to handle device any more...
    static ref H_SW_DEVICE: Mutex<u64> = Mutex::new(0);
}

#[no_mangle]
#[cfg(windows)]
pub fn get_driver_install_path() -> &'static str {
    win10::DRIVER_INSTALL_PATH
}

#[no_mangle]
pub fn download_driver() -> ResultType<()> {
    #[cfg(windows)]
    let _download_url = win10::DRIVER_DOWNLOAD_URL;
    #[cfg(target_os = "linux")]
    let _download_url = "";

    // process download and report progress

    Ok(())
}

#[no_mangle]
pub fn install_update_driver(_reboot_required: &mut bool) -> ResultType<()> {
    #[cfg(windows)]
    let install_path = win10::DRIVER_INSTALL_PATH;
    #[cfg(not(windows))]
    let install_path = "";

    let abs_path = Path::new(install_path).canonicalize()?;
    if !abs_path.exists() {
        bail!("{} not exists", install_path)
    }

    #[cfg(windows)]
    unsafe {
        {
            // Device must be created before install driver.
            // https://github.com/fufesou/RustDeskIddDriver/issues/1
            if let Err(e) = create_device() {
                bail!("{}", e);
            }

            let full_install_path: Vec<u16> = abs_path
                .to_string_lossy()
                .as_ref()
                .encode_utf16()
                .chain(Some(0).into_iter())
                .collect();

            let mut reboot_required_tmp = win10::idd::FALSE;
            if win10::idd::InstallUpdate(full_install_path.as_ptr() as _, &mut reboot_required_tmp)
                == win10::idd::FALSE
            {
                bail!("{}", win10::get_last_msg()?);
            }
            *_reboot_required = reboot_required_tmp == win10::idd::TRUE;
        }
    }

    Ok(())
}

#[no_mangle]
pub fn uninstall_driver(_reboot_required: &mut bool) -> ResultType<()> {
    #[cfg(windows)]
    let install_path = win10::DRIVER_INSTALL_PATH;
    #[cfg(not(windows))]
    let install_path = "";

    let abs_path = Path::new(install_path).canonicalize()?;
    if !abs_path.exists() {
        bail!("{} not exists", install_path)
    }

    #[cfg(windows)]
    unsafe {
        {
            let full_install_path: Vec<u16> = abs_path
                .to_string_lossy()
                .as_ref()
                .encode_utf16()
                .chain(Some(0).into_iter())
                .collect();

            let mut reboot_required_tmp = win10::idd::FALSE;
            if win10::idd::Uninstall(full_install_path.as_ptr() as _, &mut reboot_required_tmp)
                == win10::idd::FALSE
            {
                bail!("{}", win10::get_last_msg()?);
            }
            *_reboot_required = reboot_required_tmp == win10::idd::TRUE;
        }
    }

    Ok(())
}

#[no_mangle]
pub fn is_device_created() -> bool {
    #[cfg(windows)]
    return *H_SW_DEVICE.lock().unwrap() != 0;
    #[cfg(not(windows))]
    return false;
}

#[no_mangle]
pub fn create_device() -> ResultType<()> {
    if is_device_created() {
        return Ok(());
    }
    #[cfg(windows)]
    unsafe {
        let mut lock_device = H_SW_DEVICE.lock().unwrap();
        let mut h_sw_device = *lock_device as win10::idd::HSWDEVICE;
        if win10::idd::DeviceCreate(&mut h_sw_device) == win10::idd::FALSE {
            bail!("{}", win10::get_last_msg()?);
        } else {
            *lock_device = h_sw_device as u64;
        }
    }
    Ok(())
}

#[no_mangle]
pub fn close_device() {
    #[cfg(windows)]
    unsafe {
        win10::idd::DeviceClose(*H_SW_DEVICE.lock().unwrap() as win10::idd::HSWDEVICE);
        *H_SW_DEVICE.lock().unwrap() = 0;
    }
}

#[no_mangle]
pub fn plug_in_monitor(_monitor_index: u32, _edid: u32, _retries: u32) -> ResultType<()> {
    #[cfg(windows)]
    unsafe {
        if win10::idd::MonitorPlugIn(_monitor_index as _, _edid as _, _retries as _)
            == win10::idd::FALSE
        {
            bail!("{}", win10::get_last_msg()?);
        }
    }
    Ok(())
}

#[no_mangle]
pub fn plug_out_monitor(_monitor_index: u32) -> ResultType<()> {
    #[cfg(windows)]
    unsafe {
        if win10::idd::MonitorPlugOut(_monitor_index) == win10::idd::FALSE {
            bail!("{}", win10::get_last_msg()?);
        }
    }
    Ok(())
}

#[cfg(windows)]
type PMonitorMode = win10::idd::PMonitorMode;
#[cfg(not(windows))]
type PMonitorMode = *mut std::ffi::c_void;

#[no_mangle]
pub fn update_monitor_modes(
    _monitor_index: u32,
    _mode_count: u32,
    _modes: PMonitorMode,
) -> ResultType<()> {
    #[cfg(windows)]
    unsafe {
        if win10::idd::FALSE
            == win10::idd::MonitorModesUpdate(_monitor_index as _, _mode_count as _, _modes)
        {
            bail!("{}", win10::get_last_msg()?);
        }
    }
    Ok(())
}
