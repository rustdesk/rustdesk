use std::{collections::BTreeSet, path::PathBuf};

use hbb_common::log;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use x11_clipboard::Clipboard;
use x11rb::protocol::xproto::Atom;

use crate::{platform::unix::send_format_list, CliprdrError};

use super::{encode_path_to_uri, parse_plain_uri_list, SysClipboard};

static X11_CLIPBOARD: OnceCell<Clipboard> = OnceCell::new();

fn get_clip() -> Result<&'static Clipboard, CliprdrError> {
    X11_CLIPBOARD.get_or_try_init(|| Clipboard::new().map_err(|_| CliprdrError::CliprdrInit))
}

pub struct X11Clipboard {
    ignore_path: PathBuf,
    text_uri_list: Atom,
    gnome_copied_files: Atom,
    nautilus_clipboard: Atom,

    former_file_list: Mutex<Vec<PathBuf>>,
}

impl X11Clipboard {
    pub fn new(ignore_path: &PathBuf) -> Result<Self, CliprdrError> {
        let clipboard = get_clip()?;
        let text_uri_list = clipboard
            .setter
            .get_atom("text/uri-list")
            .map_err(|_| CliprdrError::CliprdrInit)?;
        let gnome_copied_files = clipboard
            .setter
            .get_atom("x-special/gnome-copied-files")
            .map_err(|_| CliprdrError::CliprdrInit)?;
        let nautilus_clipboard = clipboard
            .setter
            .get_atom("x-special/nautilus-clipboard")
            .map_err(|_| CliprdrError::CliprdrInit)?;
        Ok(Self {
            ignore_path: ignore_path.to_owned(),
            text_uri_list,
            gnome_copied_files,
            nautilus_clipboard,
            former_file_list: Mutex::new(vec![]),
        })
    }

    fn load(&self, target: Atom) -> Result<Vec<u8>, CliprdrError> {
        let clip = get_clip()?.setter.atoms.clipboard;
        let prop = get_clip()?.setter.atoms.property;
        // NOTE:
        // # why not use `load_wait`
        // load_wait is likely to wait forever, which is not what we want
        let res = get_clip()?.load_wait(clip, target, prop);
        match res {
            Ok(res) => Ok(res),
            Err(x11_clipboard::error::Error::UnexpectedType(_)) => Ok(vec![]),
            Err(x11_clipboard::error::Error::Timeout) => {
                log::debug!("x11 clipboard get content timeout.");
                Err(CliprdrError::ClipboardInternalError)
            }
            Err(e) => {
                log::debug!("x11 clipboard get content fail: {:?}", e);
                Err(CliprdrError::ClipboardInternalError)
            }
        }
    }

    fn store_batch(&self, batch: Vec<(Atom, Vec<u8>)>) -> Result<(), CliprdrError> {
        let clip = get_clip()?.setter.atoms.clipboard;
        log::debug!("try to store clipboard content");
        get_clip()?
            .store_batch(clip, batch)
            .map_err(|_| CliprdrError::ClipboardInternalError)
    }

    fn wait_file_list(&self) -> Result<Option<Vec<PathBuf>>, CliprdrError> {
        let v = self.load(self.text_uri_list)?;
        let p = parse_plain_uri_list(v)?;
        Ok(Some(p))
    }
}

impl SysClipboard for X11Clipboard {
    fn set_file_list(&self, paths: &[PathBuf]) -> Result<(), CliprdrError> {
        *self.former_file_list.lock() = paths.to_vec();

        let uri_list: Vec<String> = {
            let mut v = Vec::new();
            for path in paths {
                v.push(encode_path_to_uri(path)?);
            }
            v
        };
        let uri_list = uri_list.join("\n");
        let text_uri_list_data = uri_list.as_bytes().to_vec();
        let gnome_copied_files_data = ["copy\n".as_bytes(), uri_list.as_bytes()].concat();
        let batch = vec![
            (self.text_uri_list, text_uri_list_data),
            (self.gnome_copied_files, gnome_copied_files_data.clone()),
            (self.nautilus_clipboard, gnome_copied_files_data),
        ];
        self.store_batch(batch)
            .map_err(|_| CliprdrError::ClipboardInternalError)
    }

    fn start(&self) {
        {
            // clear cached file list
            *self.former_file_list.lock() = vec![];
        }
        loop {
            let sth = match self.wait_file_list() {
                Ok(sth) => sth,
                Err(e) => {
                    log::warn!("failed to get file list from clipboard: {}", e);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
            };

            let Some(paths) = sth else {
                // just sleep
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
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

    fn get_file_list(&self) -> Vec<PathBuf> {
        self.former_file_list.lock().clone()
    }
}
