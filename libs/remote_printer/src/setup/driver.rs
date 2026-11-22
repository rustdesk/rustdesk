use super::{common_enum, get_wstr_bytes, is_name_equal};
use hbb_common::{bail, log, ResultType};
use std::{io, ptr::null_mut, time::Duration};
use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, FALSE, LPBYTE, LPDWORD, MAX_PATH},
        ntdef::{DWORDLONG, LPCWSTR},
        winerror::{ERROR_UNKNOWN_PRINTER_DRIVER, S_OK},
    },
    um::{
        winspool::{
            DeletePrinterDriverExW, DeletePrinterDriverPackageW, EnumPrinterDriversW,
            InstallPrinterDriverFromPackageW, UploadPrinterDriverPackageW, DPD_DELETE_ALL_FILES,
            DRIVER_INFO_6W, DRIVER_INFO_8W, IPDFP_COPY_ALL_FILES, UPDP_SILENT_UPLOAD,
            UPDP_UPLOAD_ALWAYS,
        },
        winuser::GetForegroundWindow,
    },
};
use windows_strings::PCWSTR;

const HRESULT_ERR_ELEMENT_NOT_FOUND: u32 = 0x80070490;

fn enum_printer_driver(
    level: DWORD,
    p_driver_info: LPBYTE,
    cb_buf: DWORD,
    pcb_needed: LPDWORD,
    pc_returned: LPDWORD,
) -> BOOL {
    unsafe {
        // https://learn.microsoft.com/en-us/windows/win32/printdocs/enumprinterdrivers
        // This is a blocking or synchronous function and might not return immediately.
        // How quickly this function returns depends on run-time factors
        // such as network status, print server configuration, and printer driver implementation factors that are difficult to predict when writing an application.
        // Calling this function from a thread that manages interaction with the user interface could make the application appear to be unresponsive.
        EnumPrinterDriversW(
            null_mut(),
            null_mut(),
            level,
            p_driver_info,
            cb_buf,
            pcb_needed,
            pc_returned,
        )
    }
}

pub fn get_installed_driver_version(name: &PCWSTR) -> ResultType<Option<DWORDLONG>> {
    common_enum(
        "EnumPrinterDriversW",
        enum_printer_driver,
        6,
        |info: &DRIVER_INFO_6W| {
            if is_name_equal(name, info.pName) {
                Some(info.dwlDriverVersion)
            } else {
                None
            }
        },
        || None,
    )
}

fn find_inf(name: &PCWSTR) -> ResultType<Vec<u16>> {
    let r = common_enum(
        "EnumPrinterDriversW",
        enum_printer_driver,
        8,
        |info: &DRIVER_INFO_8W| {
            if is_name_equal(name, info.pName) {
                Some(get_wstr_bytes(info.pszInfPath))
            } else {
                None
            }
        },
        || None,
    )?;
    Ok(r.unwrap_or(vec![]))
}

fn delete_printer_driver(name: &PCWSTR) -> ResultType<()> {
    unsafe {
        // If the printer is used after the spooler service is started. E.g., printing a document through RustDesk Printer.
        // `DeletePrinterDriverExW()` may fail with `ERROR_PRINTER_DRIVER_IN_USE`(3001, 0xBB9).
        // We can only ignore this error for now.
        // Though restarting the spooler service is a solution, it's not a good idea to restart the service.
        //
        // Deleting the printer driver after deleting the printer is a common practice.
        // No idea why `DeletePrinterDriverExW()` fails with `ERROR_UNKNOWN_PRINTER_DRIVER` after using the printer once.
        // https://github.com/ChromiumWebApps/chromium/blob/c7361d39be8abd1574e6ce8957c8dbddd4c6ccf7/cloud_print/virtual_driver/win/install/setup.cc#L422
        // AnyDesk printer driver and the simplest printer driver also have the same issue.
        if FALSE
            == DeletePrinterDriverExW(
                null_mut(),
                null_mut(),
                name.as_ptr() as _,
                DPD_DELETE_ALL_FILES,
                0,
            )
        {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(ERROR_UNKNOWN_PRINTER_DRIVER as _) {
                return Ok(());
            } else {
                bail!("Failed to delete the printer driver, {}", err)
            }
        }
    }
    Ok(())
}

// https://github.com/dvalter/chromium-android-ext-dev/blob/dab74f7d5bc5a8adf303090ee25c611b4d54e2db/cloud_print/virtual_driver/win/install/setup.cc#L190
fn delete_printer_driver_package(inf: Vec<u16>) -> ResultType<()> {
    if inf.is_empty() {
        return Ok(());
    }
    let slen = if inf[inf.len() - 1] == 0 {
        inf.len() - 1
    } else {
        inf.len()
    };
    let inf_path = String::from_utf16_lossy(&inf[..slen]);
    if !std::path::Path::new(&inf_path).exists() {
        return Ok(());
    }

    let mut retries = 3;
    loop {
        unsafe {
            let res = DeletePrinterDriverPackageW(null_mut(), inf.as_ptr(), null_mut());
            if res == S_OK || res == HRESULT_ERR_ELEMENT_NOT_FOUND as i32 {
                return Ok(());
            }
            log::error!("Failed to delete the printer driver, result: {}", res);
        }
        retries -= 1;
        if retries <= 0 {
            bail!("Failed to delete the printer driver");
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

pub fn uninstall_driver(name: &PCWSTR) -> ResultType<()> {
    // Note: inf must be found before `delete_printer_driver()`.
    let inf = find_inf(name)?;
    delete_printer_driver(name)?;
    delete_printer_driver_package(inf)
}

pub fn install_driver(name: &PCWSTR, inf: LPCWSTR) -> ResultType<()> {
    let mut size = (MAX_PATH * 10) as u32;
    let mut package_path = [0u16; MAX_PATH * 10];
    unsafe {
        let mut res = UploadPrinterDriverPackageW(
            null_mut(),
            inf,
            null_mut(),
            UPDP_SILENT_UPLOAD | UPDP_UPLOAD_ALWAYS,
            null_mut(),
            package_path.as_mut_ptr(),
            &mut size as _,
        );
        if res != S_OK {
            log::error!(
                "Failed to upload the printer driver package to the driver cache silently, {}. Will try with user UI.",
                res
            );

            res = UploadPrinterDriverPackageW(
                null_mut(),
                inf,
                null_mut(),
                UPDP_UPLOAD_ALWAYS,
                GetForegroundWindow(),
                package_path.as_mut_ptr(),
                &mut size as _,
            );
            if res != S_OK {
                bail!(
                    "Failed to upload the printer driver package to the driver cache with UI, {}",
                    res
                );
            }
        }

        // https://learn.microsoft.com/en-us/windows/win32/printdocs/installprinterdriverfrompackage
        res = InstallPrinterDriverFromPackageW(
            null_mut(),
            package_path.as_ptr(),
            name.as_ptr(),
            null_mut(),
            IPDFP_COPY_ALL_FILES,
        );
        if res != S_OK {
            bail!("Failed to install the printer driver from package, {}", res);
        }
    }

    Ok(())
}
