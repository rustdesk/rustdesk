use super::{
    handles::{Buffer, Printer},
    queues,
};
use hbb_common::{bail, ResultType};
use std::mem::size_of;
use windows::{
    core::{w, HSTRING, PCWSTR},
    Win32::{
        Foundation::{HANDLE, WIN32_ERROR},
        Graphics::Printing::{EnumPortsW, XcvDataW, PORT_INFO_2W, SERVER_WRITE},
    },
};

const PORT_INFO_LEVEL: u32 = 2;
const LOCAL_MONITOR: &str = "Local Monitor";

pub(crate) fn exists(name: &str) -> ResultType<bool> {
    let mut count = 0;
    let Some(buffer) = Buffer::query(|buffer, needed| unsafe {
        EnumPortsW(PCWSTR::null(), PORT_INFO_LEVEL, buffer, needed, &mut count).ok()
    })?
    else {
        return Ok(false);
    };
    for index in 0..count {
        let port = unsafe { buffer.item::<PORT_INFO_2W>(index as usize)? };
        if !queues::text(port.pPortName)?.eq_ignore_ascii_case(name) {
            continue;
        }
        if queues::text(port.pMonitorName)? != LOCAL_MONITOR {
            bail!("The PDF port belongs to a different port monitor");
        }
        return Ok(true);
    }
    Ok(false)
}

pub(crate) fn add(name: &str) -> ResultType<()> {
    if exists(name)? {
        bail!("The PDF printer port already exists");
    }
    execute(name, w!("AddPort"))
}

pub(crate) fn remove(name: &str) -> ResultType<()> {
    if queues::all()?
        .iter()
        .any(|queue| queue.port.eq_ignore_ascii_case(name))
    {
        bail!("The PDF printer port is still used by a queue");
    }
    if exists(name)? {
        execute(name, w!("DeletePort"))?;
    }
    Ok(())
}

fn execute(name: &str, command: PCWSTR) -> ResultType<()> {
    let monitor = Printer::open(",XcvMonitor Local Port", SERVER_WRITE)?;
    let name = HSTRING::from(name);
    let input = unsafe {
        std::slice::from_raw_parts(
            name.as_ptr().cast::<u8>(),
            (name.len() + 1) * size_of::<u16>(),
        )
    };
    let mut needed = 0;
    let mut status = 0;
    unsafe {
        XcvDataW(
            HANDLE(monitor.0.Value),
            command,
            Some(input),
            None,
            &mut needed,
            Some(&mut status),
        )
        .ok()?;
    }
    if status != 0 {
        return Err(windows::core::Error::from_hresult(WIN32_ERROR(status).to_hresult()).into());
    }
    Ok(())
}
