use super::{
    display_name, handles::Printer, owner_marker, port_name, ports, queues, Queue, DRIVER_NAME,
};
use hbb_common::{anyhow::Context, bail, ResultType};
use std::{
    thread,
    time::{Duration, Instant},
};
use windows::{
    core::{w, HSTRING, PCWSTR, PWSTR},
    Win32::{
        Foundation::ERROR_BUSY,
        Graphics::Printing::{
            AddPrinterW, DeletePrinter, SetPrinterW, PRINTER_ALL_ACCESS,
            PRINTER_ATTRIBUTE_KEEPPRINTEDJOBS, PRINTER_ATTRIBUTE_LOCAL, PRINTER_CONTROL_PURGE,
            PRINTER_HANDLE, PRINTER_INFO_2W,
        },
    },
};

const QUEUE_INFO_LEVEL: u32 = 2;
const DEFAULT_PRIORITY: u32 = 1;
const UPDATE_PRINTER: u32 = 0;
const CONTROL_LEVEL: u32 = 0;
const QUEUE_REMOVAL_TIMEOUT: Duration = Duration::from_secs(10);
const QUEUE_REMOVAL_POLL: Duration = Duration::from_millis(100);

pub fn install(app_name: &str, instance_id: &str) -> ResultType<Queue> {
    if let Some(queue) = queues::installed(app_name)? {
        return Ok(queue);
    }
    let queue = Queue {
        name: display_name(app_name)?,
        port: port_name(instance_id)?,
    };
    let existing = queues::get(&queue.name)?;
    if let Some(info) = &existing {
        info.check_migration(app_name)?;
    }
    ports::add(&queue.port).context("Failed to add the PDF Local Port")?;
    let result = if existing.is_some() {
        replace_legacy(&queue, app_name)
    } else {
        add_queue(&queue, &owner_marker(app_name)?)
    };
    if let Err(error) = result {
        if let Err(cleanup) = ports::remove(&queue.port) {
            bail!("PDF queue installation failed: {error:#}; port cleanup failed: {cleanup:#}");
        }
        return Err(
            error.context("Microsoft Print To PDF must be installed and allowed by system policy")
        );
    }
    Ok(queue)
}

pub fn uninstall(
    app_name: &str,
    remember_pdf_port: impl FnOnce(&str) -> ResultType<()>,
) -> ResultType<()> {
    let queue = if let Some(queue) = queues::installed(app_name)? {
        remember_pdf_port(&queue.port)?;
        queue
    } else {
        let Some(info) =
            queues::get(&display_name(app_name)?)?.filter(|info| info.is_legacy(app_name))
        else {
            return Ok(());
        };
        Queue {
            name: info.name,
            port: info.port,
        }
    };
    let printer = removable_printer(&queue, app_name)?;
    let started = Instant::now();
    unsafe { SetPrinterW(printer.0, CONTROL_LEVEL, None, PRINTER_CONTROL_PURGE) }
        .context("Failed to cancel print jobs before removing the printer")?;
    unsafe { DeletePrinter(printer.0)? };
    // Output committed during cancellation can leave a retained COMPLETE job.
    while unsafe { queues::read(&printer)?.item::<PRINTER_INFO_2W>(0)? }.cJobs != 0 {
        if started.elapsed() >= QUEUE_REMOVAL_TIMEOUT {
            bail!("Timed out waiting for the PDF printer queue to release its jobs");
        }
        unsafe { SetPrinterW(printer.0, CONTROL_LEVEL, None, PRINTER_CONTROL_PURGE) }
            .context("Failed to cancel remaining jobs while removing the printer")?;
        thread::sleep(QUEUE_REMOVAL_POLL);
    }
    drop(printer);
    finish_removal(&queue, started)
        .context("Failed to finish removing the PDF printer queue and port")
}

fn finish_removal(queue: &Queue, started: Instant) -> ResultType<()> {
    // An open printer handle can keep the port busy even after the queue disappears.
    loop {
        if !queues::all()?.iter().any(|current| {
            current.name.eq_ignore_ascii_case(&queue.name)
                && current.port.eq_ignore_ascii_case(&queue.port)
        }) {
            match ports::remove(&queue.port) {
                Ok(()) => return Ok(()),
                Err(error)
                    if error
                        .downcast_ref::<windows::core::Error>()
                        .is_some_and(|error| error.code() == ERROR_BUSY.to_hresult()) => {}
                Err(error) => return Err(error),
            }
        }
        if started.elapsed() >= QUEUE_REMOVAL_TIMEOUT {
            bail!("Timed out waiting for the PDF printer queue and port to be released");
        }
        thread::sleep(QUEUE_REMOVAL_POLL);
    }
}

fn removable_printer(queue: &Queue, app_name: &str) -> ResultType<Printer> {
    let printer = Printer::open(&queue.name, PRINTER_ALL_ACCESS)?;
    let info = queues::get(&queue.name)?.context("The PDF queue was removed during uninstall")?;
    if !info.is_legacy(app_name) {
        queues::validate(&info)?;
        if info.comment != owner_marker(app_name)? {
            bail!("The PDF queue changed ownership during uninstall");
        }
    }
    if info.port != queue.port {
        bail!("The PDF queue changed ownership during uninstall");
    }
    Ok(printer)
}

fn replace_legacy(queue: &Queue, app_name: &str) -> ResultType<()> {
    let printer = Printer::open(&queue.name, PRINTER_ALL_ACCESS)?;
    let buffer = queues::read(&printer)?;
    let mut info = unsafe { buffer.item::<PRINTER_INFO_2W>(0)? };
    queues::info_from(info)?.check_migration(app_name)?;
    info.pSecurityDescriptor = Default::default();
    let port = HSTRING::from(&queue.port);
    let driver = HSTRING::from(DRIVER_NAME);
    let comment = HSTRING::from(owner_marker(app_name)?);
    info.pPortName = PWSTR(port.as_ptr().cast_mut());
    info.pDriverName = PWSTR(driver.as_ptr().cast_mut());
    info.pComment = PWSTR(comment.as_ptr().cast_mut());
    // Driver-specific settings cannot be reused with the inbox PDF driver.
    info.pDevMode = std::ptr::null_mut();
    info.Attributes |= PRINTER_ATTRIBUTE_KEEPPRINTEDJOBS;
    unsafe {
        SetPrinterW(
            printer.0,
            QUEUE_INFO_LEVEL,
            Some((&info as *const PRINTER_INFO_2W).cast()),
            UPDATE_PRINTER,
        )?;
    }
    Ok(())
}

fn add_queue(queue: &Queue, comment: &str) -> ResultType<()> {
    let name = HSTRING::from(&queue.name);
    let port = HSTRING::from(&queue.port);
    let driver = HSTRING::from(DRIVER_NAME);
    let comment = HSTRING::from(comment);
    let info = PRINTER_INFO_2W {
        pPrinterName: PWSTR(name.as_ptr().cast_mut()),
        pPortName: PWSTR(port.as_ptr().cast_mut()),
        pDriverName: PWSTR(driver.as_ptr().cast_mut()),
        pComment: PWSTR(comment.as_ptr().cast_mut()),
        pPrintProcessor: PWSTR(w!("WinPrint").as_ptr().cast_mut()),
        pDatatype: PWSTR(w!("RAW").as_ptr().cast_mut()),
        Attributes: PRINTER_ATTRIBUTE_LOCAL | PRINTER_ATTRIBUTE_KEEPPRINTEDJOBS,
        Priority: DEFAULT_PRIORITY,
        DefaultPriority: DEFAULT_PRIORITY,
        ..Default::default()
    };
    let handle = unsafe {
        AddPrinterW(
            PCWSTR::null(),
            QUEUE_INFO_LEVEL,
            (&info as *const PRINTER_INFO_2W).cast(),
        )?
    };
    let _printer = Printer(PRINTER_HANDLE { Value: handle.0 });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, resume_unwind};
    use windows::Win32::Graphics::Printing::{
        EndDocPrinter, StartDocPrinterW, WritePrinter, DOC_INFO_1W, PRINTER_CONTROL_PAUSE,
    };

    const DOCUMENT_INFO_LEVEL: u32 = 1;
    const CAPTURE_HANDLE_RELEASE_DELAY: Duration = Duration::from_millis(500);

    #[test]
    #[ignore = "Requires administrator privileges and Microsoft Print to PDF; creates a temporary queue"]
    fn uninstall_cancels_pending_jobs() {
        let instance = hbb_common::uuid::Uuid::new_v4().simple().to_string();
        let app = format!("RustDesk uninstall test {instance}");
        let queue = install(&app, &instance).unwrap();
        let result = catch_unwind(|| {
            queue_pending_document(&queue);
            assert_eq!(queues::get(&queue.name).unwrap().unwrap().jobs, 1);
            let capture = Printer::open(&queue.name, PRINTER_ALL_ACCESS).unwrap();
            let removing_app = app.clone();
            let removal = thread::spawn(move || uninstall(&removing_app, |_| Ok(())));
            thread::sleep(CAPTURE_HANDLE_RELEASE_DELAY);
            drop(capture);
            removal.join().unwrap().unwrap();
            assert!(queues::get(&queue.name).unwrap().is_none());
            assert!(!ports::exists(&queue.port).unwrap());
        });
        if queues::get(&queue.name).unwrap().is_some() {
            let printer = Printer::open(&queue.name, PRINTER_ALL_ACCESS).unwrap();
            unsafe {
                SetPrinterW(printer.0, CONTROL_LEVEL, None, PRINTER_CONTROL_PURGE).unwrap();
                DeletePrinter(printer.0).unwrap();
            }
        }
        ports::remove(&queue.port).unwrap();
        if let Err(error) = result {
            resume_unwind(error);
        }
    }

    fn queue_pending_document(queue: &Queue) {
        let printer = Printer::open(&queue.name, PRINTER_ALL_ACCESS).unwrap();
        let document = DOC_INFO_1W {
            pDocName: PWSTR(w!("Pending uninstall regression").as_ptr().cast_mut()),
            pDatatype: PWSTR(w!("RAW").as_ptr().cast_mut()),
            ..Default::default()
        };
        let data = b"Pending uninstall regression";
        let mut written = 0;
        unsafe {
            SetPrinterW(printer.0, CONTROL_LEVEL, None, PRINTER_CONTROL_PAUSE).unwrap();
            assert_ne!(
                StartDocPrinterW(printer.0, DOCUMENT_INFO_LEVEL, &document),
                0
            );
            WritePrinter(
                printer.0,
                data.as_ptr().cast(),
                data.len() as u32,
                &mut written,
            )
            .ok()
            .unwrap();
            EndDocPrinter(printer.0).ok().unwrap();
        }
        assert_eq!(written as usize, data.len());
    }
}
