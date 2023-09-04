use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use crate::CliprdrError;

use super::fuse::{self, FuseServer};

#[cfg(not(feature = "wayland"))]
pub mod x11;

trait SysClipboard {
    fn wait_file_list(&self) -> Result<Vec<PathBuf>, CliprdrError>;
    fn set_file_list(&self, paths: &[PathBuf]) -> Result<(), CliprdrError>;
}

fn get_sys_clipboard() -> Box<dyn SysClipboard> {
    #[cfg(feature = "wayland")]
    {
        unimplemented!()
    }
    #[cfg(not(feature = "wayland"))]
    {
        pub use x11::*;
        X11Clipboard::new()
    }
}

// on x11, path will be encode as
// "/home/rustdesk/pictures/🖼️.png" -> "file:///home/rustdesk/pictures/%F0%9F%96%BC%EF%B8%8F.png"
// url encode and decode is needed
const ENCODE_SET: percent_encoding::AsciiSet = percent_encoding::CONTROLS.add(b' ').remove(b'/');

fn encode_path_to_uri(path: &PathBuf) -> String {
    let encoded = percent_encoding::percent_encode(path.to_str().unwrap().as_bytes(), &ENCODE_SET)
        .to_string();
    format!("file://{}", encoded)
}

fn parse_uri_to_path(encoded_uri: &str) -> Result<PathBuf, CliprdrError> {
    let encoded_path = encoded_uri.trim_start_matches("file://");
    let path_str = percent_encoding::percent_decode_str(encoded_path)
        .decode_utf8()
        .map_err(|_| CliprdrError::ConversionFailure)?;
    let path_str = path_str.to_string();

    Ok(Path::new(&path_str).to_path_buf())
}

#[cfg(test)]
mod uri_test {
    #[test]
    fn test_conversion() {
        let path = std::path::PathBuf::from("/home/rustdesk/pictures/🖼️.png");
        let uri = super::encode_path_to_uri(&path);
        assert_eq!(
            uri,
            "file:///home/rustdesk/pictures/%F0%9F%96%BC%EF%B8%8F.png"
        );
        let convert_back = super::parse_uri_to_path(&uri).unwrap();
        assert_eq!(path, convert_back);
    }
}

// helper parse function
// convert 'text/uri-list' data to a list of valid Paths
// # Note
// - none utf8 data will lead to error
fn parse_plain_uri_list(v: Vec<u8>) -> Result<Vec<PathBuf>, CliprdrError> {
    let text = String::from_utf8(v).map_err(|_| CliprdrError::ConversionFailure)?;
    parse_uri_list(&text)
}

// helper parse function
// convert "x-special/gnome-copied-files", "x-special/x-kde-cutselection" and "x-special/nautilus-clipboard" data to a list of valid Paths
// # Note
// - none utf8 data will lead to error
fn parse_de_uri_list(v: Vec<u8>) -> Result<Vec<PathBuf>, CliprdrError> {
    let text = String::from_utf8(v).map_err(|_| CliprdrError::ConversionFailure)?;
    let plain_list = text
        .trim_start_matches("copy\n")
        .trim_start_matches("cut\n");
    parse_uri_list(plain_list)
}

// helper parse function
// convert 'text/uri-list' data to a list of valid Paths
// # Note
// - none utf8 data will lead to error
fn parse_uri_list(text: &str) -> Result<Vec<PathBuf>, CliprdrError> {
    let mut list = Vec::new();

    for line in text.lines() {
        let decoded = parse_uri_to_path(line)?;
        list.push(decoded)
    }
    Ok(list)
}

#[derive(Debug)]
pub struct ClipboardContext {
    pub stop: bool,
    pub fuse_mount_point: PathBuf,
    pub fuse_server: FuseServer,
    pub file_list: HashSet<PathBuf>,
    pub clipboard: Clipboard,

    pub bkg_session: fuser::BackgroundSession,
}

impl ClipboardContext {
    fn new(timeout: Duration, mount_path: PathBuf) -> Result<Self, CliprdrError> {
        // assert mount path exists
        let mountpoint = mount_path
            .canonicalize()
            .map_err(|e| CliprdrError::Unknown {
                description: format!("invalid mount point: {:?}", e),
            })?;
        let fuse_server = FuseServer::new(timeout);
        let mnt_opts = [
            fuser::MountOption::FSName("clipboard".to_string()),
            fuser::MountOption::NoAtime,
            fuser::MountOption::RO,
            fuser::MountOption::NoExec,
        ];
        let bkg_session = fuser::spawn_mount2(fuse_server, mountpoint, &mnt_opts).map_err(|e| {
            CliprdrError::Unknown {
                description: format!("failed to mount fuse: {:?}", e),
            }
        })?;

        log::debug!("mounting clipboard fuse to {}", mount_path.display());
    }
}
