use super::{common_enum, get_wstr_bytes, is_name_equal};
use hbb_common::{bail, ResultType};
use std::{io, ptr::null_mut};
use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, FALSE, LPBYTE, LPDWORD},
        ntdef::HANDLE,
        winerror::{ERROR_INSUFFICIENT_BUFFER, ERROR_INVALID_PRINTER_NAME},
    },
    um::winspool::{
        AddPrinterW, ClosePrinter, DeletePrinter, EnumPrintersW, OpenPrinterW, SetPrinterW,
        PRINTER_ALL_ACCESS, PRINTER_ATTRIBUTE_LOCAL, PRINTER_CONTROL_PURGE, PRINTER_DEFAULTSW,
        PRINTER_ENUM_LOCAL, PRINTER_INFO_1W, PRINTER_INFO_2W,
    },
};
use windows_strings::{w, PCWSTR};

fn enum_local_printer(
    level: DWORD,
    p_printer_info: LPBYTE,
    cb_buf: DWORD,
    pcb_needed: LPDWORD,
    pc_returned: LPDWORD,
) -> BOOL {
    unsafe {
        // https://learn.microsoft.com/en-us/windows/win32/printdocs/enumprinters
        // This is a blocking or synchronous function and might not return immediately.
        // How quickly this function returns depends on run-time factors
        // such as network status, print server configuration, and printer driver implementation factors that are difficult to predict when writing an application.
        // Calling this function from a thread that manages interaction with the user interface could make the application appear to be unresponsive.
        EnumPrintersW(
            PRINTER_ENUM_LOCAL,
            null_mut(),
            level,
            p_printer_info,
            cb_buf,
            pcb_needed,
            pc_returned,
        )
    }
}

#[inline]
pub fn is_printer_added(name: &PCWSTR) -> ResultType<bool> {
    const PRINTER_INFO_LEVEL: DWORD = 1;
    let mut needed = 0;
    let mut returned = 0;
    if enum_local_printer(
        PRINTER_INFO_LEVEL,
        null_mut(),
        0,
        &mut needed,
        &mut returned,
    ) == FALSE
    {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) || needed == 0 {
            bail!("Failed to query EnumPrintersW buffer size: {error}");
        }
    }
    if needed == 0 {
        return Ok(false);
    }

    let mut buffer = vec![0u8; needed as usize];
    if enum_local_printer(
        PRINTER_INFO_LEVEL,
        buffer.as_mut_ptr(),
        needed,
        &mut needed,
        &mut returned,
    ) == FALSE
    {
        bail!(
            "Failed to call EnumPrintersW, error: {}",
            io::Error::last_os_error()
        );
    }

    let printers = buffer.as_ptr() as *const PRINTER_INFO_1W;
    for index in 0..returned {
        let printer = unsafe { &*printers.add(index as usize) };
        if is_name_equal(name, printer.pName) {
            return Ok(true);
        }
    }
    Ok(false)
}

// Only return the first matched printer
pub fn get_printer_installed_on_port(port: &PCWSTR) -> ResultType<Option<Vec<u16>>> {
    common_enum(
        "EnumPrintersW",
        enum_local_printer,
        2,
        |info: &PRINTER_INFO_2W| {
            if is_name_equal(port, info.pPortName) {
                Some(get_wstr_bytes(info.pPrinterName))
            } else {
                None
            }
        },
        || None,
    )
}

pub fn add_printer(name: &PCWSTR, driver: &PCWSTR, port: &PCWSTR) -> ResultType<()> {
    let mut printer_info = PRINTER_INFO_2W {
        pServerName: null_mut(),
        pPrinterName: name.as_ptr() as _,
        pShareName: null_mut(),
        pPortName: port.as_ptr() as _,
        pDriverName: driver.as_ptr() as _,
        pComment: null_mut(),
        pLocation: null_mut(),
        pDevMode: null_mut(),
        pSepFile: null_mut(),
        pPrintProcessor: w!("WinPrint").as_ptr() as _,
        pDatatype: w!("RAW").as_ptr() as _,
        pParameters: null_mut(),
        pSecurityDescriptor: null_mut(),
        Attributes: PRINTER_ATTRIBUTE_LOCAL,
        Priority: 0,
        DefaultPriority: 0,
        StartTime: 0,
        UntilTime: 0,
        Status: 0,
        cJobs: 0,
        AveragePPM: 0,
    };
    unsafe {
        let h_printer = AddPrinterW(
            null_mut(),
            2,
            &mut printer_info as *mut PRINTER_INFO_2W as _,
        );
        if h_printer.is_null() {
            bail!(format!(
                "Failed to add printer. Error: {}",
                io::Error::last_os_error()
            ))
        }
    }
    Ok(())
}

pub fn delete_printer(name: &PCWSTR) -> ResultType<()> {
    let mut dft = PRINTER_DEFAULTSW {
        pDataType: null_mut(),
        pDevMode: null_mut(),
        DesiredAccess: PRINTER_ALL_ACCESS,
    };
    let mut h_printer: HANDLE = null_mut();
    unsafe {
        if FALSE
            == OpenPrinterW(
                name.as_ptr() as _,
                &mut h_printer,
                &mut dft as *mut PRINTER_DEFAULTSW as _,
            )
        {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(ERROR_INVALID_PRINTER_NAME as _) {
                return Ok(());
            } else {
                bail!(format!("Failed to open printer. Error: {}", err))
            }
        }

        if FALSE == SetPrinterW(h_printer, 0, null_mut(), PRINTER_CONTROL_PURGE) {
            ClosePrinter(h_printer);
            bail!(format!(
                "Failed to purge printer queue. Error: {}",
                io::Error::last_os_error()
            ))
        }

        if FALSE == DeletePrinter(h_printer) {
            ClosePrinter(h_printer);
            bail!(format!(
                "Failed to delete printer. Error: {}",
                io::Error::last_os_error()
            ))
        }

        ClosePrinter(h_printer);
    }

    Ok(())
}
