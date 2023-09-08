use std::path::PathBuf;

use x11_clipboard::Clipboard;
use x11rb::protocol::xproto::Atom;

use crate::CliprdrError;

use super::{encode_path_to_uri, parse_plain_uri_list, SysClipboard};

pub struct X11Clipboard {
    text_uri_list: Atom,
    gnome_copied_files: Atom,
    clipboard: Clipboard,
}

impl X11Clipboard {
    pub fn new() -> Result<Self, CliprdrError> {
        let clipboard = Clipboard::new().map_err(|_| CliprdrError::CliprdrInit)?;
        let text_uri_list = clipboard
            .setter
            .get_atom("text/uri-list")
            .map_err(|_| CliprdrError::CliprdrInit)?;
        let gnome_copied_files = clipboard
            .setter
            .get_atom("x-special/gnome-copied-files")
            .map_err(|_| CliprdrError::CliprdrInit)?;
        Ok(Self {
            text_uri_list,
            gnome_copied_files,
            clipboard,
        })
    }

    fn load(&self, target: Atom) -> Result<Vec<u8>, CliprdrError> {
        let clip = self.clipboard.setter.atoms.clipboard;
        let prop = self.clipboard.setter.atoms.property;
        self.clipboard
            .load_wait(clip, target, prop)
            .map_err(|_| CliprdrError::ConversionFailure)
    }

    fn store_batch(&self, batch: Vec<(Atom, Vec<u8>)>) -> Result<(), CliprdrError> {
        let clip = self.clipboard.setter.atoms.clipboard;
        self.clipboard
            .store_batch(clip, batch)
            .map_err(|_| CliprdrError::ClipboardInternalError)
    }
}

impl SysClipboard for X11Clipboard {
    fn wait_file_list(&self) -> Result<Vec<PathBuf>, CliprdrError> {
        let v = self.load(self.text_uri_list)?;
        // loading 'text/uri-list' should be enough?
        parse_plain_uri_list(v)
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
}
