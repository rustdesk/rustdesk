use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use cacao::pasteboard::{Pasteboard, PasteboardName};
use hbb_common::log;
use parking_lot::Mutex;

use crate::{platform::unix::send_format_list, CliprdrError};

use super::SysClipboard;

#[inline]
fn wait_file_list() -> Option<Vec<PathBuf>> {
    let pb = Pasteboard::named(PasteboardName::General);
    pb.get_file_urls()
        .ok()
        .map(|v| v.into_iter().map(|nsurl| nsurl.pathbuf()).collect())
}

#[inline]
fn set_file_list(file_list: &[PathBuf]) -> Result<(), CliprdrError> {
    let pb = Pasteboard::named(PasteboardName::General);
    pb.set_files(file_list.to_vec())
        .map_err(|_| CliprdrError::ClipboardInternalError)
}

pub struct NsPasteboard {
    ignore_path: PathBuf,

    former_file_list: Mutex<Vec<PathBuf>>,
}

impl NsPasteboard {
    pub fn new(ignore_path: &Path) -> Result<Self, CliprdrError> {
        Ok(Self {
            ignore_path: ignore_path.to_owned(),
            former_file_list: Mutex::new(vec![]),
        })
    }
}

impl SysClipboard for NsPasteboard {
    fn set_file_list(&self, paths: &[PathBuf]) -> Result<(), CliprdrError> {
        *self.former_file_list.lock() = paths.to_vec();
        set_file_list(paths)
    }

    fn start(&self) {
        {
            *self.former_file_list.lock() = vec![];
        }

        loop {
            let file_list = match wait_file_list() {
                Some(v) => v,
                None => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
            };

            let filtered = file_list
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

    fn get_file_list(&self) -> Vec<PathBuf> {
        self.former_file_list.lock().clone()
    }
}
