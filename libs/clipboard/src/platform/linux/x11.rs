use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use hbb_common::{
    bytes::{BufMut, BytesMut},
    log,
};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use x11_clipboard::Clipboard;
use x11rb::protocol::xproto::Atom;

use crate::{
    platform::linux::{
        construct_file_list, FILECONTENTS_FORMAT_ID, FILECONTENTS_FORMAT_NAME,
        FILEDESCRIPTORW_FORMAT_NAME, FILEDESCRIPTOR_FORMAT_ID,
    },
    send_data, ClipboardFile, CliprdrError,
};

use super::{encode_path_to_uri, parse_plain_uri_list, SysClipboard};

static X11_CLIPBOARD: OnceCell<Clipboard> = OnceCell::new();

// this is tested on an Arch Linux with X11
const X11_CLIPBOARD_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(70);

fn get_clip() -> Result<&'static Clipboard, CliprdrError> {
    X11_CLIPBOARD.get_or_try_init(|| Clipboard::new().map_err(|_| CliprdrError::CliprdrInit))
}

pub struct X11Clipboard {
    stop: AtomicBool,
    ignore_path: PathBuf,
    text_uri_list: Atom,
    gnome_copied_files: Atom,

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
        Ok(Self {
            ignore_path: ignore_path.to_owned(),
            stop: AtomicBool::new(false),
            text_uri_list,
            gnome_copied_files,
            former_file_list: Mutex::new(vec![]),
        })
    }

    fn load(&self, target: Atom) -> Result<Vec<u8>, CliprdrError> {
        let clip = get_clip()?.setter.atoms.clipboard;
        let prop = get_clip()?.setter.atoms.property;
        // NOTE:
        // # why not use `load_wait`
        // load_wait is likely to wait forever, which is not what we want
        get_clip()?
            .load(clip, target, prop, X11_CLIPBOARD_TIMEOUT)
            .map_err(|_| CliprdrError::ConversionFailure)
    }

    fn store_batch(&self, batch: Vec<(Atom, Vec<u8>)>) -> Result<(), CliprdrError> {
        let clip = get_clip()?.setter.atoms.clipboard;
        log::debug!("try to store clipboard content");
        get_clip()?
            .store_batch(clip, batch)
            .map_err(|_| CliprdrError::ClipboardInternalError)
    }

    fn wait_file_list(&self) -> Result<Option<Vec<PathBuf>>, CliprdrError> {
        if self.stop.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let v = self.load(self.text_uri_list)?;
        // loading 'text/uri-list' should be enough?
        let p = parse_plain_uri_list(v)?;
        Ok(Some(p))
    }
}

impl X11Clipboard {
    #[inline]
    fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

impl SysClipboard for X11Clipboard {
    fn set_file_list(&self, paths: &[PathBuf]) -> Result<(), CliprdrError> {
        *self.former_file_list.lock() = paths.to_vec();

        let uri_list: Vec<String> = paths.iter().map(|pb| encode_path_to_uri(pb)).collect();
        let uri_list = uri_list.join("\n");
        let text_uri_list_data = uri_list.as_bytes().to_vec();
        let gnome_copied_files_data = vec!["copy\n".as_bytes(), uri_list.as_bytes()].concat();
        let batch = vec![
            (self.text_uri_list, text_uri_list_data),
            (self.gnome_copied_files, gnome_copied_files_data),
        ];
        self.store_batch(batch)
            .map_err(|_| CliprdrError::ClipboardInternalError)
    }

    fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    fn start(&self) {
        self.stop.store(false, Ordering::Relaxed);

        loop {
            let sth = match self.wait_file_list() {
                Ok(sth) => sth,
                Err(e) => {
                    log::warn!("failed to get file list from clipboard: {}", e);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
            };

            if self.is_stopped() {
                break;
            }

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
                let former_st = former.iter().collect();
                if filtered_st == former_st {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }

                // send update to server
                log::debug!("clipboard updated: {:?}", filtered);
                *former = filtered;
            }

            if let Err(e) = send_format_list(0) {
                log::warn!("failed to send format list: {}", e);
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        log::debug!("stop listening file related atoms on clipboard");
    }

    fn send_format_list(&self, conn_id: i32) -> Result<(), CliprdrError> {
        if self.is_stopped() {
            log::debug!("clipboard stopped, skip sending");
            return Ok(());
        }

        let Some(paths) = self.wait_file_list()? else {
            log::debug!("no files in format list, skip sending");
            return Ok(());
        };

        let filtered: Vec<_> = paths
            .into_iter()
            .filter(|pb| !pb.starts_with(&self.ignore_path))
            .collect();

        if filtered.is_empty() {
            log::debug!("no files in format list, skip sending");
            return Ok(());
        }

        send_format_list(conn_id)
    }

    fn send_file_list(&self, conn_id: i32) -> Result<(), CliprdrError> {
        if self.is_stopped() {
            log::debug!("clipboard stopped, skip sending");
            return Ok(());
        }
        let Some(paths) = self.wait_file_list()? else {
            log::debug!("no files in format list, skip sending");
            return Ok(());
        };

        let filtered: Vec<_> = paths
            .into_iter()
            .filter(|pb| !pb.starts_with(&self.ignore_path))
            .collect();

        if filtered.is_empty() {
            log::debug!("no files in format list, skip sending");
            return Ok(());
        }

        send_file_list(filtered, conn_id)
    }
}

fn send_format_list(conn_id: i32) -> Result<(), CliprdrError> {
    log::debug!("send format list to remote, conn={}", conn_id);
    let format_list = ClipboardFile::FormatList {
        format_list: vec![
            (
                FILEDESCRIPTOR_FORMAT_ID,
                FILEDESCRIPTORW_FORMAT_NAME.to_string(),
            ),
            (FILECONTENTS_FORMAT_ID, FILECONTENTS_FORMAT_NAME.to_string()),
        ],
    };

    send_data(conn_id, format_list);
    log::debug!("format list to remote dispatched, conn={}", conn_id);
    Ok(())
}

fn send_file_list(paths: Vec<PathBuf>, conn_id: i32) -> Result<(), CliprdrError> {
    log::debug!("send file list to remote, conn={}", conn_id);
    let files = construct_file_list(paths.as_slice())?;

    let mut data = BytesMut::with_capacity(4 + 592 * files.len());
    data.put_u32_le(paths.len() as u32);
    for file in files.iter() {
        data.put(file.as_bin().as_slice());
    }

    let format_data = data.to_vec();

    send_data(
        conn_id,
        ClipboardFile::FormatDataResponse {
            msg_flags: 1,
            format_data,
        },
    );
    Ok(())
}
