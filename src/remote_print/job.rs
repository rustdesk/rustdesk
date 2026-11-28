use base::message_proto::{file_transfer_send_request::PrintFormat, FileTransferSendRequest};
use hbb_common::{
    anyhow::{anyhow, Context},
    bail, ResultType,
};
use std::collections::HashMap;
use uuid::Uuid;

const PDF_JOB_PREFIX: &str = "RustDesk://FsJob//PdfPrinter/";

pub(crate) fn print_format(request: &FileTransferSendRequest) -> ResultType<PrintFormat> {
    request
        .print_format
        .enum_value()
        .map_err(|value| anyhow!("Unsupported print format: {value}"))
}

struct Offer {
    path: String,
    receiving: bool,
}

#[derive(Default)]
pub(crate) struct Offers {
    jobs: HashMap<i32, Offer>,
}

impl Offers {
    pub(crate) fn insert(&mut self, id: i32, path: String) -> ResultType<()> {
        let identifier = path
            .strip_prefix(PDF_JOB_PREFIX)
            .context("Invalid PDF print request identifier")?;
        Uuid::parse_str(identifier).map_err(|_| anyhow!("Invalid PDF print request identifier"))?;
        if self.jobs.contains_key(&id) || self.jobs.values().any(|offer| offer.path == path) {
            bail!("Duplicate PDF print request");
        }
        self.jobs.insert(
            id,
            Offer {
                path,
                receiving: false,
            },
        );
        Ok(())
    }

    pub(crate) fn start(&mut self, id: i32, path: &str) -> ResultType<()> {
        let offer = self
            .jobs
            .get_mut(&id)
            .filter(|offer| offer.path == path && !offer.receiving)
            .context("The PDF print response does not match a pending request")?;
        offer.receiving = true;
        Ok(())
    }

    pub(crate) fn finish(&mut self, id: i32) -> ResultType<()> {
        match self.jobs.remove(&id) {
            Some(Offer {
                receiving: true, ..
            }) => Ok(()),
            _ => bail!("The PDF transfer was not approved for this session"),
        }
    }

    pub(crate) fn contains(&self, id: i32) -> bool {
        self.jobs.contains_key(&id)
    }

    pub(crate) fn remove(&mut self, id: i32) {
        self.jobs.remove(&id);
    }

    pub(crate) fn clear(&mut self) -> Vec<i32> {
        self.jobs.drain().map(|(id, _)| id).collect()
    }
}

pub(crate) fn new_request_path() -> String {
    format!("{PDF_JOB_PREFIX}{}", Uuid::new_v4())
}

pub(crate) fn is_pdf_request_path(path: &str) -> bool {
    path.starts_with(PDF_JOB_PREFIX)
}

pub(crate) fn unique_target<T>(mut targets: impl Iterator<Item = T>) -> ResultType<T> {
    let target = targets
        .next()
        .context("No authorized PDF print receiver is connected")?;
    if targets.next().is_some() {
        bail!("More than one PDF print receiver is connected");
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::message_proto::LoginRequest;
    use hbb_common::protobuf::Message;

    const LEGACY_PRINTER_REQUEST: &[u8] = &[0x28, 0x01];
    const FUTURE_PRINTER_REQUEST: &[u8] = &[0x28, 0x01, 0x30, 0x63];

    #[test]
    fn print_format_defaults_to_legacy_and_rejects_unknown_values() {
        let legacy = FileTransferSendRequest::parse_from_bytes(LEGACY_PRINTER_REQUEST).unwrap();
        assert_eq!(print_format(&legacy).unwrap(), PrintFormat::LegacyXps);
        assert!(!LoginRequest::new().pdf_printing_supported);
        let pdf = FileTransferSendRequest {
            print_format: PrintFormat::Pdf.into(),
            ..Default::default()
        };
        assert_eq!(print_format(&pdf).unwrap(), PrintFormat::Pdf);
        let future = FileTransferSendRequest::parse_from_bytes(FUTURE_PRINTER_REQUEST).unwrap();
        assert_eq!(
            print_format(&future).unwrap_err().to_string(),
            "Unsupported print format: 99"
        );
    }

    #[test]
    fn pdf_response_must_match_a_pending_request_in_this_session() {
        let path = new_request_path();
        let mut offers = Offers::default();
        offers.insert(1, path.clone()).unwrap();
        assert!(offers.insert(2, path.clone()).is_err());
        assert!(offers.start(1, &new_request_path()).is_err());
        offers.start(1, &path).unwrap();
        assert!(offers.start(1, &path).is_err());
        offers.finish(1).unwrap();
        assert!(offers.finish(1).is_err());
        offers.insert(2, new_request_path()).unwrap();
        assert!(offers.finish(2).is_err());
    }

    #[test]
    fn pdf_capture_requires_exactly_one_receiver() {
        assert!(unique_target(std::iter::empty::<i32>()).is_err());
        assert!(unique_target([7, 8].into_iter()).is_err());
        assert_eq!(unique_target([7].into_iter()).unwrap(), 7);
    }
}
