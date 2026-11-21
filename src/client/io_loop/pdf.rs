use super::{
    file_transfer_send_request, fs, keys, FileTransferSendRequest, InvokeUiSession, LocalConfig,
    Remote,
};
use crate::{
    client::Interface,
    remote_print::{job, receiver},
};
use hbb_common::{
    anyhow::{anyhow, Context},
    bail, ResultType,
};

impl<T: InvokeUiSession> Remote<T> {
    pub(super) fn pdf_offer(&mut self, request: FileTransferSendRequest) {
        if let Err(error) = self.accept_pdf_offer(request) {
            self.pdf_error(error);
        }
    }

    fn accept_pdf_offer(&mut self, request: FileTransferSendRequest) -> ResultType<()> {
        if job::print_format(&request)? != file_transfer_send_request::PrintFormat::Pdf {
            bail!("The print request does not declare PDF");
        }
        if !self.is_connected || !self.handler.is_default() || !receiver::available() {
            bail!("This session does not support PDF printing");
        }
        let action = LocalConfig::get_option(keys::OPTION_PRINTER_INCOMING_JOB_ACTION);
        if action == "dismiss" {
            return Ok(());
        }
        if !action.is_empty() && action != "selected" {
            bail!("Unknown incoming PDF print action: {action}");
        }
        let automatic = LocalConfig::get_bool_option(keys::OPTION_PRINTER_ALLOW_AUTO_PRINT);
        let printer = LocalConfig::get_option(keys::OPTION_PRINTER_SELECTED_NAME);
        if automatic && action == "selected" && printer.is_empty() {
            bail!("Select a printer before enabling automatic PDF printing");
        }
        let id = fs::get_next_job_id();
        self.pdf_jobs.insert(id, request.path.clone())?;
        if automatic {
            let printer = if action.is_empty() {
                String::new()
            } else {
                printer
            };
            self.handler.printer_response(id, request.path, printer);
        } else {
            self.handler.printer_request(id, request.path);
        }
        Ok(())
    }

    pub(super) fn approve_pdf(&mut self, id: i32, path: &str) -> bool {
        if !job::is_pdf_request_path(path) && !self.pdf_jobs.contains(id) {
            return true;
        }
        let result = if self.is_connected {
            self.pdf_jobs.start(id, path)
        } else {
            Err(anyhow!("The PDF request belongs to a disconnected session"))
        };
        if let Err(error) = result {
            self.discard_pdf(id);
            self.pdf_error(error);
            return false;
        }
        true
    }

    pub(super) async fn finish_pdf(&mut self, id: i32) {
        let result = self.take_pdf(id).await;
        let printer = self.handler.printer_names.write().unwrap().remove(&id);
        let data = match result {
            Ok(data) => data,
            Err(error) => {
                self.pdf_error(error);
                return;
            }
        };
        let Some(printer) = printer else {
            self.pdf_error(anyhow!("No printer was approved for this PDF request"));
            return;
        };
        let handler = self.handler.clone();
        std::thread::spawn(move || match receiver::print(&data, &printer) {
            Ok(job) => {
                hbb_common::log::trace!("PDF accepted by the local print queue as job {job}")
            }
            Err(error) => handler.msgbox(
                "custom-error",
                "Remote printing",
                &format!("PDF printing failed: {error:#}"),
                "",
            ),
        });
    }

    async fn take_pdf(&mut self, id: i32) -> ResultType<Vec<u8>> {
        let approved = self.pdf_jobs.finish(id);
        let job =
            fs::remove_job(id, &mut self.write_jobs).context("The PDF transfer does not exist")?;
        approved?;
        if job.r#type != fs::JobType::Printer {
            bail!("The PDF response has the wrong transfer type");
        }
        job.finish_download()?;
        if let Some(error) = job.job_error() {
            bail!("PDF transfer failed: {error}");
        }
        job.get_buf_data()
            .await?
            .context("The PDF transfer has no document data")
    }

    pub(super) fn pdf_error(&self, error: hbb_common::anyhow::Error) {
        self.handler.msgbox(
            "custom-error",
            "Remote printing",
            &format!("PDF printing failed: {error:#}"),
            "",
        );
    }

    pub(super) fn discard_pdf(&mut self, id: i32) {
        if self.pdf_jobs.contains(id) {
            self.pdf_jobs.remove(id);
            self.handler.printer_names.write().unwrap().remove(&id);
        }
    }

    pub(super) fn clear_pdf_jobs(&mut self) {
        for id in self.pdf_jobs.clear() {
            fs::remove_job(id, &mut self.write_jobs);
            self.handler.printer_names.write().unwrap().remove(&id);
        }
    }
}

pub(super) fn is_explicit_format(request: &FileTransferSendRequest) -> bool {
    request.file_type == file_transfer_send_request::FileType::Printer.into()
        && request.print_format.value() != 0
}
