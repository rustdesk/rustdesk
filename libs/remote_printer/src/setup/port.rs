use super::{common_enum, is_name_equal, printer::get_printer_installed_on_port};
use hbb_common::{bail, ResultType};
use std::{io, ptr::null_mut};
use winapi::{
    shared::minwindef::{BOOL, DWORD, FALSE, LPBYTE, LPDWORD},
    um::{
        winnt::HANDLE,
        winspool::{
            ClosePrinter, EnumPortsW, OpenPrinterW, XcvDataW, PORT_INFO_2W, PRINTER_DEFAULTSW,
            SERVER_WRITE,
        },
    },
};
use windows_strings::{w, PCWSTR};

const XCV_MONITOR_LOCAL_PORT: PCWSTR = w!(",XcvMonitor Local Port");

fn enum_printer_port(
    level: DWORD,
    p_port_info: LPBYTE,
    cb_buf: DWORD,
    pcb_needed: LPDWORD,
    pc_returned: LPDWORD,
) -> BOOL {
    unsafe {
        // https://learn.microsoft.com/en-us/windows/win32/printdocs/enumports
        // This is a blocking or synchronous function and might not return immediately.
        // How quickly this function returns depends on run-time factors
        // such as network status, print server configuration, and printer driver implementation factors that are difficult to predict when writing an application.
        // Calling this function from a thread that manages interaction with the user interface could make the application appear to be unresponsive.
        EnumPortsW(
            null_mut(),
            level,
            p_port_info,
            cb_buf,
            pcb_needed,
            pc_returned,
        )
    }
}

fn is_port_exists(name: &PCWSTR) -> ResultType<bool> {
    let r = common_enum(
        "EnumPortsW",
        enum_printer_port,
        2,
        |info: &PORT_INFO_2W| {
            if is_name_equal(name, info.pPortName) {
                Some(true)
            } else {
                None
            }
        },
        || None,
    )?;
    Ok(r.unwrap_or(false))
}

unsafe fn execute_on_local_port(port: &PCWSTR, command: &PCWSTR) -> ResultType<()> {
    let mut dft = PRINTER_DEFAULTSW {
        pDataType: null_mut(),
        pDevMode: null_mut(),
        DesiredAccess: SERVER_WRITE,
    };
    let mut h_monitor: HANDLE = null_mut();
    if FALSE
        == OpenPrinterW(
            XCV_MONITOR_LOCAL_PORT.as_ptr() as _,
            &mut h_monitor,
            &mut dft as *mut PRINTER_DEFAULTSW as _,
        )
    {
        bail!(format!(
            "Failed to open Local Port monitor. Error: {}",
            io::Error::last_os_error()
        ))
    }

    let mut output_needed: u32 = 0;
    let mut status: u32 = 0;
    if FALSE
        == XcvDataW(
            h_monitor,
            command.as_ptr(),
            port.as_ptr() as *mut u8,
            (port.len() + 1) as u32 * 2,
            null_mut(),
            0,
            &mut output_needed,
            &mut status,
        )
    {
        ClosePrinter(h_monitor);
        bail!(format!(
            "Failed to execute the command on the printer port, Error: {}",
            io::Error::last_os_error()
        ))
    }

    ClosePrinter(h_monitor);

    Ok(())
}

fn add_local_port(port: &PCWSTR) -> ResultType<()> {
    unsafe { execute_on_local_port(port, &w!("AddPort")) }
}

fn delete_local_port(port: &PCWSTR) -> ResultType<()> {
    unsafe { execute_on_local_port(port, &w!("DeletePort")) }
}

pub fn check_add_local_port(port: &PCWSTR) -> ResultType<()> {
    if !is_port_exists(port)? {
        return add_local_port(port);
    }
    Ok(())
}

pub fn check_delete_local_port(port: &PCWSTR) -> ResultType<()> {
    if is_port_exists(port)? {
        if get_printer_installed_on_port(port)?.is_some() {
            bail!("The printer is installed on the port. Please remove the printer first.");
        }
        return delete_local_port(port);
    }
    Ok(())
}
