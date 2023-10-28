use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use cacao::pasteboard::{Pasteboard, PasteboardName};
use hbb_common::log;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

use crate::{platform::unix::send_format_list, CliprdrError};

use super::SysClipboard;

static NS_PASTEBOARD: Lazy<Pasteboard> = Lazy::new(|| Pasteboard::named(PasteboardName::General));

#[inline]
fn wait_file_list() -> Option<Vec<PathBuf>> {
    NS_PASTEBOARD
        .get_file_urls()
        .ok()
        .map(|v| v.into_iter().map(|nsurl| nsurl.to_path_buf()).collect())
}
pub struct NsPasteboard {
    ignore_path: PathBuf,

    former_file_list: Mutex<Vec<PathBuf>>,
}

impl NsPasteboard {
    pub fn new(ignore_path: &PathBuf) -> Result<Self, CliprdrError> {
        Ok(Self {
            stopped: AtomicBool::new(false),
            ignore_path: ignore_path.to_owned(),
            former_file_list: Mutex::new(vec![]),
        })
    }

    #[inline]
    fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }
}

impl SysClipboard for NsPasteboard {
    fn set_file_list(&self, paths: &[PathBuf]) -> Result<(), CliprdrError> {
        *self.former_file_list.lock() = paths.to_vec();
        NS_PASTEBOARD
            .set_file_urls(paths)
            .map_err(|_| CliprdrError::ClipboardInternalError)
    }

    fn start(&self) {
        self.stopped.store(false, Ordering::Relaxed);

        loop {
            if self.is_stopped() {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            let file_list = match wait_file_list() {
                Some(v) => v,
                None => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
            };

            let filtered = paths
                .into_iter()
                .filter(|pb| !pb.starts_with(&self.ignore_path))
                .collect::<Vec<_>>();

            if filtered.is_empty() {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }

            {
                let mut former = self.former_file_list.lock();

                let filtered_st: BTreeSet<_> = filtered.iter().collect();
                let former_st = former.iter().collect::<BTreeSet<_>>();
                if filtered_st == former_st {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }

                *former = filtered;
            }

            if let Err(e) = send_format_list(0) {
                log::warn!("failed to send format list: {}", e);
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        log::debug!("stop listening file related atoms on clipboard");
    }

    fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    fn get_file_list(&self) -> Vec<PathBuf> {
        self.former_file_list.lock().clone()
    }
}
