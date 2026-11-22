use super::{
    driver::{get_installed_driver_version, install_driver, uninstall_driver},
    port::{check_add_local_port, check_delete_local_port},
    printer::{add_printer, delete_printer},
};
use hbb_common::{allow_err, bail, lazy_static, log, ResultType};
use std::{path::PathBuf, sync::Mutex};
use windows_strings::PCWSTR;

lazy_static::lazy_static!(
    static ref SETUP_MTX: Mutex<()> = Mutex::new(());
);

fn get_driver_inf_abs_path() -> ResultType<PathBuf> {
    use crate::RD_DRIVER_INF_PATH;

    let exe_file = std::env::current_exe()?;
    let abs_path = match exe_file.parent() {
        Some(parent) => parent.join(RD_DRIVER_INF_PATH),
        None => bail!(
            "Invalid exe parent for {}",
            exe_file.to_string_lossy().as_ref()
        ),
    };
    if !abs_path.exists() {
        bail!(
            "The driver inf file \"{}\" does not exists",
            RD_DRIVER_INF_PATH
        )
    }
    Ok(abs_path)
}

// Note: This function must be called in a separate thread.
// Because many functions in this module are blocking or synchronous.
// Calling this function from a thread that manages interaction with the user interface could make the application appear to be unresponsive.
// Steps:
// 1. Add the local port.
// 2. Check if the driver is installed.
//    Uninstall the existing driver if it is installed.
//    We should not check the driver version because the driver is deployed with the application.
//    It's better to uninstall the existing driver and install the driver from the application.
// 3. Add the printer.
pub fn install_update_printer(app_name: &str) -> ResultType<()> {
    let printer_name = crate::get_printer_name(app_name);
    let driver_name = crate::get_driver_name();
    let port = crate::get_port_name(app_name);
    let rd_printer_name = PCWSTR::from_raw(printer_name.as_ptr());
    let rd_printer_driver_name = PCWSTR::from_raw(driver_name.as_ptr());
    let rd_printer_port = PCWSTR::from_raw(port.as_ptr());

    let inf_file = get_driver_inf_abs_path()?;
    let inf_file: Vec<u16> = inf_file
        .to_string_lossy()
        .as_ref()
        .encode_utf16()
        .chain(Some(0).into_iter())
        .collect();
    let _lock = SETUP_MTX.lock().unwrap();

    check_add_local_port(&rd_printer_port)?;

    let should_install_driver = match get_installed_driver_version(&rd_printer_driver_name)? {
        Some(_version) => {
            delete_printer(&rd_printer_name)?;
            allow_err!(uninstall_driver(&rd_printer_driver_name));
            true
        }
        None => true,
    };

    if should_install_driver {
        allow_err!(install_driver(&rd_printer_driver_name, inf_file.as_ptr()));
    }

    add_printer(&rd_printer_name, &rd_printer_driver_name, &rd_printer_port)?;

    Ok(())
}

pub fn uninstall_printer(app_name: &str) {
    let printer_name = crate::get_printer_name(app_name);
    let driver_name = crate::get_driver_name();
    let port = crate::get_port_name(app_name);
    let rd_printer_name = PCWSTR::from_raw(printer_name.as_ptr());
    let rd_printer_driver_name = PCWSTR::from_raw(driver_name.as_ptr());
    let rd_printer_port = PCWSTR::from_raw(port.as_ptr());

    let _lock = SETUP_MTX.lock().unwrap();

    allow_err!(delete_printer(&rd_printer_name));
    allow_err!(uninstall_driver(&rd_printer_driver_name));
    allow_err!(check_delete_local_port(&rd_printer_port));
}
