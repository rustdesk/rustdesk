use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use once_cell::sync::OnceCell;
use x11_clipboard::Clipboard;
use x11rb::protocol::xproto::Atom;

use crate::CliprdrError;

use super::{encode_path_to_uri, parse_plain_uri_list, SysClipboard};

static X11_CLIPBOARD: OnceCell<Clipboard> = OnceCell::new();

fn get_clip() -> Result<&'static Clipboard, CliprdrError> {
    X11_CLIPBOARD.get_or_try_init(|| Clipboard::new().map_err(|_| CliprdrError::CliprdrInit))
}

pub struct X11Clipboard {
    stop: AtomicBool,
    text_uri_list: Atom,
    gnome_copied_files: Atom,
}

impl X11Clipboard {
    pub fn new() -> Result<Self, CliprdrError> {
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
            stop: AtomicBool::new(false),
            text_uri_list,
            gnome_copied_files,
        })
    }

    fn load(&self, target: Atom) -> Result<Vec<u8>, CliprdrError> {
        let clip = get_clip()?.setter.atoms.clipboard;
        let prop = get_clip()?.setter.atoms.property;
        get_clip()?
            .load_wait(clip, target, prop)
            .map_err(|_| CliprdrError::ConversionFailure)
    }

    fn store_batch(&self, batch: Vec<(Atom, Vec<u8>)>) -> Result<(), CliprdrError> {
        let clip = get_clip()?.setter.atoms.clipboard;
        get_clip()?
            .store_batch(clip, batch)
            .map_err(|_| CliprdrError::ClipboardInternalError)
    }
}

impl SysClipboard for X11Clipboard {
    fn wait_file_list(&self) -> Result<Option<Vec<PathBuf>>, CliprdrError> {
        if self.stop.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let v = self.load(self.text_uri_list)?;
        // loading 'text/uri-list' should be enough?
        let p = parse_plain_uri_list(v)?;
        Ok(Some(p))
    }

    fn set_file_list(&self, paths: &[PathBuf]) -> Result<(), CliprdrError> {
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
    }
}
