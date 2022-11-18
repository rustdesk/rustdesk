#[cfg(windows)]
pub mod win10;

use hbb_common::{bail, lazy_static, ResultType};
use std::{path::Path, sync::Mutex};

lazy_static::lazy_static! {
    // If device is uninstalled though "Device Manager" Window.
    // Rustdesk is unable to handle device any more...
    static ref H_SW_DEVICE: Mutex<u64> = Mutex::new(0);
    static ref MONITOR_PLUGIN: Mutex<Vec<u32>> = Mutex::new(Vec::new());
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
        MONITOR_PLUGIN.lock().unwrap().clear();
    }
}

#[no_mangle]
pub fn plug_in_monitor() -> ResultType<()> {
    #[cfg(windows)]
    unsafe {
        let monitor_index = 0 as u32;
        let mut plug_in_monitors = MONITOR_PLUGIN.lock().unwrap();
        for i in 0..plug_in_monitors.len() {
            if let Some(d) = plug_in_monitors.get(i) {
                if *d == monitor_index {
                    return Ok(());
                }
            };
        }
        if win10::idd::MonitorPlugIn(monitor_index, 0, 30) == win10::idd::FALSE {
            bail!("{}", win10::get_last_msg()?);
        }
        (*plug_in_monitors).push(monitor_index);
    }
    Ok(())
}

#[no_mangle]
pub fn plug_out_monitor() -> ResultType<()> {
    #[cfg(windows)]
    unsafe {
        let monitor_index = 0 as u32;
        if win10::idd::MonitorPlugOut(monitor_index) == win10::idd::FALSE {
            bail!("{}", win10::get_last_msg()?);
        }
        let mut plug_in_monitors = MONITOR_PLUGIN.lock().unwrap();
        for i in 0..plug_in_monitors.len() {
            if let Some(d) = plug_in_monitors.get(i) {
                if *d == monitor_index {
                    plug_in_monitors.remove(i);
                    break;
                }
            };
        }
    }
    Ok(())
}

#[no_mangle]
pub fn update_monitor_modes() -> ResultType<()> {
    #[cfg(windows)]
    unsafe {
        let monitor_index = 0 as u32;
        let mut modes = vec![win10::idd::MonitorMode {
            width: 1920,
            height: 1080,
            sync: 60,
        }];
        if win10::idd::FALSE
            == win10::idd::MonitorModesUpdate(
                monitor_index as win10::idd::UINT,
                modes.len() as win10::idd::UINT,
                modes.as_mut_ptr(),
            )
        {
            bail!("{}", win10::get_last_msg()?);
        }
    }
    Ok(())
}
