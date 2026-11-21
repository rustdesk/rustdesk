use super::{
    pdf_printer_target,
    service::{EmptyExtraFieldService, GenericService, Service},
    Server,
};
use crate::remote_print::files::PdfWriter;
use hbb_common::{anyhow::Context, bail, log, ResultType};
use remote_printer::pdf::{self, Job, Pipe, Queue};
use std::{thread, time::Duration};

const SERVICE_NAME: &str = "remote-pdf-printer";
const QUEUE_POLL: Duration = Duration::from_secs(1);
const ERROR_LOG_INTERVAL: Duration = Duration::from_secs(60);
const COPY_BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn register(server: &mut Server) {
    if !crate::is_server() || !crate::platform::is_installed() {
        return;
    }
    match pdf::is_elevated() {
        Ok(true) => {
            let service = EmptyExtraFieldService::new(SERVICE_NAME.to_owned(), false);
            GenericService::run(&service, run);
            server.add_service(Box::new(service.sp));
        }
        Ok(false) => log::error!("PDF capture requires the installed privileged server process"),
        Err(error) => log::error!("Cannot verify PDF capture service privileges: {error:#}"),
    }
}

fn run(service: EmptyExtraFieldService) -> ResultType<()> {
    while service.ok() {
        if let Err(error) = capture_queue(&service) {
            hbb_common::throttled_log!(ERROR_LOG_INTERVAL, error, "PDF capture failed: {error:#}");
        }
        if service.ok() {
            thread::sleep(QUEUE_POLL);
        }
    }
    Ok(())
}

fn capture_queue(service: &EmptyExtraFieldService) -> ResultType<()> {
    if !crate::remote_print::setup::installed_marker()? {
        return Ok(());
    }
    let Some(queue) = pdf::installed(&crate::get_app_name())? else {
        return Ok(());
    };
    pdf::verify_namespace(&uuid::Uuid::new_v4().simple().to_string())?;
    let pipe = Pipe::new(&queue.port)?;
    while service.ok() {
        pipe.connect_queue(&queue, &|| service.ok())?;
        let result = capture_connection(service, &queue, &pipe);
        let disconnected = pipe.disconnect();
        if let Err(error) = result {
            hbb_common::throttled_log!(ERROR_LOG_INTERVAL, error, "PDF job rejected: {error:#}");
        }
        disconnected?;
    }
    Ok(())
}

fn spooler_pid() -> ResultType<u32> {
    use windows_service::{
        service::{ServiceAccess, ServiceState},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service("Spooler", ServiceAccess::QUERY_STATUS)?;
    let status = service.query_status()?;
    match (status.current_state, status.process_id) {
        (ServiceState::Running, Some(pid)) if pid != 0 => Ok(pid),
        _ => bail!("The Windows Print Spooler is not running"),
    }
}

fn capture_connection(
    service: &EmptyExtraFieldService,
    queue: &Queue,
    pipe: &Pipe,
) -> ResultType<()> {
    if pipe.client_pid()? != spooler_pid()? {
        bail!("The PDF pipe client is not the Windows Print Spooler");
    }
    let owner = pdf::controlled_user_sid()?;
    let Some(job) = Job::bind(queue, &owner)? else {
        let mut byte = [0];
        if pipe.read(&mut byte, &|| service.ok())? != 0 {
            bail!("PDF output has no unique Spooler job");
        }
        return Ok(());
    };
    let result = capture_job(service, pipe, &job);
    let cleanup = job.remove();
    if let Err(error) = &cleanup {
        hbb_common::throttled_log!(
            ERROR_LOG_INTERVAL,
            error,
            "PDF source job {} cleanup failed: {error:#}",
            job.id()
        );
    }
    result?;
    cleanup.context(
        "The PDF was handed to its connection, but its retained source job could not be removed",
    )
}

fn capture_job(service: &EmptyExtraFieldService, pipe: &Pipe, job: &Job) -> ResultType<()> {
    let owner = pdf::controlled_user_sid()?;
    if owner != job.owner() {
        bail!("The controlled Windows user changed before PDF capture");
    }
    let target = pdf_printer_target()?;
    let active = || service.ok() && target.authorized();
    let result = (|| {
        let mut writer = PdfWriter::create()?;
        copy_output(pipe, &mut writer, &active)?;
        job.wait_completed(&active)?;
        if pdf::controlled_user_sid()? != owner {
            bail!("The controlled Windows user changed during PDF capture");
        }
        let document = writer.finish()?;
        target.send(document.read()?)?;
        log::trace!("PDF source job {} handed to its bound connection", job.id());
        Ok(())
    })();
    if let Err(error) = &result {
        target.report_error(&format!("PDF capture failed: {error:#}"));
    }
    result
}

fn copy_output(pipe: &Pipe, writer: &mut PdfWriter, active: &impl Fn() -> bool) -> ResultType<()> {
    let mut buffer = [0; COPY_BUFFER_BYTES];
    loop {
        let count = pipe.read(&mut buffer, active)?;
        if count == 0 {
            return Ok(());
        }
        writer.write(&buffer[..count])?;
    }
}
