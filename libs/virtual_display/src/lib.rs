#[cfg(windows)]
pub mod win10;

use hbb_common::{bail, lazy_static, ResultType};
use std::{ffi::CString, path::Path, sync::Mutex};

lazy_static::lazy_static! {
    static ref H_SW_DEVICE: Mutex<u64> = Mutex::new(0);
}

pub fn download_driver() -> ResultType<()> {
    #[cfg(windows)]
    let _download_url = win10::DRIVER_DOWNLOAD_URL;
    #[cfg(target_os = "linux")]
    let _download_url = "";

    // process download and report progress

    Ok(())
}

pub fn install_update_driver(_reboot_required: &mut bool) -> ResultType<()> {
    #[cfg(windows)]
    let install_path = win10::DRIVER_INSTALL_PATH;
    #[cfg(not(windows))]
    let install_path = "";

    let abs_path = Path::new(install_path).canonicalize()?;
    if !abs_path.exists() {
        bail!("{} not exists", install_path)
    }

    let _full_install_path = match abs_path.to_str() {
        Some(p) => CString::new(p)?.into_raw(),
        None => bail!("{} not exists", install_path),
    };

    #[cfg(windows)]
    unsafe {
        {
            let mut reboot_required_tmp = win10::idd::FALSE;
            if win10::idd::InstallUpdate(_full_install_path, &mut reboot_required_tmp)
                == win10::idd::FALSE
            {
                bail!("{}", win10::get_last_msg()?);
            }
            *_reboot_required = reboot_required_tmp == win10::idd::TRUE;
        }
    }

    Ok(())
}

pub fn uninstall_driver(_reboot_required: &mut bool) -> ResultType<()> {
    #[cfg(windows)]
    let install_path = win10::DRIVER_INSTALL_PATH;
    #[cfg(not(windows))]
    let install_path = "";

    let abs_path = Path::new(install_path).canonicalize()?;
    if !abs_path.exists() {
        bail!("{} not exists", install_path)
    }

    let _full_install_path = match abs_path.to_str() {
        Some(p) => CString::new(p)?.into_raw(),
        None => bail!("{} not exists", install_path),
    };

    #[cfg(windows)]
    unsafe {
        {
            let mut reboot_required_tmp = win10::idd::FALSE;
            if win10::idd::Uninstall(_full_install_path, &mut reboot_required_tmp)
                == win10::idd::FALSE
            {
                bail!("{}", win10::get_last_msg()?);
            }
            *_reboot_required = reboot_required_tmp == win10::idd::TRUE;
        }
    }

    Ok(())
}

pub fn is_device_created() -> bool {
    #[cfg(windows)]
    return *H_SW_DEVICE.lock().unwrap() != 0;
    #[cfg(not(windows))]
    return false;
}

#[cfg(windows)]
pub fn create_device() -> ResultType<()> {
    if is_device_created() {
        return Ok(());
    }
    unsafe {
        let mut h_sw_device = *H_SW_DEVICE.lock().unwrap() as win10::idd::HSWDEVICE;
        if win10::idd::DeviceCreate(&mut h_sw_device) == win10::idd::FALSE {
            bail!("{}", win10::get_last_msg()?);
        } else {
            *H_SW_DEVICE.lock().unwrap() = h_sw_device as u64;
            Ok(())
        }
    }
}

#[cfg(windows)]
pub fn close_device() {
    unsafe {
        win10::idd::DeviceClose(*H_SW_DEVICE.lock().unwrap() as win10::idd::HSWDEVICE);
        *H_SW_DEVICE.lock().unwrap() = 0;
    }
}

#[cfg(windows)]
pub fn plug_in_monitor() -> ResultType<()> {
    unsafe {
        if win10::idd::MonitorPlugIn(0, 0, 30) == win10::idd::FALSE {
            bail!("{}", win10::get_last_msg()?);
        }
        Ok(())
    }
}

#[cfg(windows)]
pub fn plug_out_monitor() -> ResultType<()> {
    unsafe {
        if win10::idd::MonitorPlugOut(0) == win10::idd::FALSE {
            bail!("{}", win10::get_last_msg()?);
        }
        Ok(())
    }
}
