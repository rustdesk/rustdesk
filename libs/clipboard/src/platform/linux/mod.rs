use std::{
    collections::HashSet,
    fs::File,
    os::unix::prelude::FileExt,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use dashmap::DashMap;
use fuser::MountOption;
use hbb_common::{
    bytes::{BufMut, BytesMut},
    log,
};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use utf16string::WString;

use crate::{
    platform::fuse::FileDescription, send_data, ClipboardFile, CliprdrError, CliprdrServiceContext,
};

use super::{fuse::FuseServer, LDAP_EPOCH_DELTA};

#[cfg(not(feature = "wayland"))]
pub mod x11;

// not actual format id, just a placeholder
const FILEDESCRIPTOR_FORMAT_ID: i32 = 49334;
const FILEDESCRIPTORW_FORMAT_NAME: &str = "FileGroupDescriptorW";
// not actual format id, just a placeholder
const FILECONTENTS_FORMAT_ID: i32 = 49267;
const FILECONTENTS_FORMAT_NAME: &str = "FileContents";

lazy_static! {
    static ref REMOTE_FORMAT_MAP: DashMap<i32, String> = DashMap::new();
}

fn get_local_format(remote_id: i32) -> Option<String> {
    REMOTE_FORMAT_MAP.get(&remote_id).map(|s| s.clone())
}

fn add_remote_format(local_name: &str, remote_id: i32) {
    REMOTE_FORMAT_MAP.insert(remote_id, local_name.to_string());
}

trait SysClipboard: Send + Sync {
    fn start(&self);
    fn stop(&self);

    fn set_file_list(&self, paths: &[PathBuf]) -> Result<(), CliprdrError>;
    fn get_file_list(&self) -> Result<Vec<LocalFile>, CliprdrError>;
}

fn get_sys_clipboard(ignore_path: &PathBuf) -> Result<Box<dyn SysClipboard>, CliprdrError> {
    #[cfg(feature = "wayland")]
    {
        unimplemented!()
    }
    #[cfg(not(feature = "wayland"))]
    {
        pub use x11::*;
        let x11_clip = X11Clipboard::new(ignore_path)?;
        Ok(Box::new(x11_clip) as Box<_>)
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
// convert 'text/uri-list' data to a list of valid Paths
// # Note
// - none utf8 data will lead to error
fn parse_uri_list(text: &str) -> Result<Vec<PathBuf>, CliprdrError> {
    let mut list = Vec::new();

    for line in text.lines() {
        if !line.starts_with("file://") {
            continue;
        }
        let decoded = parse_uri_to_path(line)?;
        list.push(decoded)
    }
    Ok(list)
}

#[derive(Debug)]
struct LocalFile {
    pub path: PathBuf,
    pub handle: Option<File>,

    pub name: String,
    pub size: u64,
    pub last_write_time: SystemTime,
    pub is_dir: bool,
    pub read_only: bool,
    pub hidden: bool,
    pub system: bool,
    pub archive: bool,
    pub normal: bool,
}

impl LocalFile {
    pub fn try_open(path: &PathBuf) -> Result<Self, CliprdrError> {
        let mt = std::fs::metadata(path).map_err(|e| CliprdrError::FileError {
            path: path.clone(),
            err: e,
        })?;
        let size = mt.len() as u64;
        let is_dir = mt.is_dir();
        let read_only = mt.permissions().readonly();
        let system = false;
        let hidden = false;
        let archive = false;
        let normal = !is_dir;
        let last_write_time = mt.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        let name = path
            .display()
            .to_string()
            .trim_start_matches('/')
            .replace('/', "\\");

        let handle = if is_dir {
            None
        } else {
            let file = std::fs::File::open(path).map_err(|e| CliprdrError::FileError {
                path: path.clone(),
                err: e,
            })?;
            let reader = file;
            Some(reader)
        };

        Ok(Self {
            name,
            path: path.clone(),
            handle,
            size,
            last_write_time,
            is_dir,
            read_only,
            system,
            hidden,
            archive,
            normal,
        })
    }
    pub fn as_bin(&self) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(592);

        let read_only_flag = if self.read_only { 0x1 } else { 0 };
        let hidden_flag = if self.hidden { 0x2 } else { 0 };
        let system_flag = if self.system { 0x4 } else { 0 };
        let directory_flag = if self.is_dir { 0x10 } else { 0 };
        let archive_flag = if self.archive { 0x20 } else { 0 };
        let normal_flag = if self.normal { 0x80 } else { 0 };

        let file_attributes: u32 = read_only_flag
            | hidden_flag
            | system_flag
            | directory_flag
            | archive_flag
            | normal_flag;

        let win32_time = self
            .last_write_time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
            / 100
            + LDAP_EPOCH_DELTA;

        let size_high = (self.size >> 32) as u32;
        let size_low = (self.size & (u32::MAX as u64)) as u32;

        let wstr: WString<utf16string::LE> = WString::from(&self.name);
        let name = wstr.as_bytes();

        log::debug!(
            "put file to list: name_len {}, name {}",
            name.len(),
            &self.name
        );

        let flags = 0x4064;

        // flags, 4 bytes
        buf.put_u32_le(flags);
        // 32 bytes reserved
        buf.put(&[0u8; 32][..]);
        // file attributes, 4 bytes
        buf.put_u32_le(file_attributes);
        // 16 bytes reserved
        buf.put(&[0u8; 16][..]);
        // last write time, 8 bytes
        buf.put_u64_le(win32_time);
        // file size (high)
        buf.put_u32_le(size_high);
        // file size (low)
        buf.put_u32_le(size_low);
        // put name and padding to 520 bytes
        let name_len = name.len();
        buf.put(name);
        buf.put(&vec![0u8; 520 - name_len][..]);

        buf.to_vec()
    }
}

fn construct_file_list(paths: &[PathBuf]) -> Result<Vec<LocalFile>, CliprdrError> {
    fn constr_file_lst(
        path: &PathBuf,
        file_list: &mut Vec<LocalFile>,
        visited: &mut HashSet<PathBuf>,
    ) -> Result<(), CliprdrError> {
        // prevent fs loop
        if visited.contains(path) {
            return Ok(());
        }
        visited.insert(path.clone());

        let local_file = LocalFile::try_open(path)?;
        file_list.push(local_file);

        let mt = std::fs::metadata(path).map_err(|e| CliprdrError::FileError {
            path: path.clone(),
            err: e,
        })?;
        if mt.is_dir() {
            let dir = std::fs::read_dir(path).unwrap();
            for entry in dir {
                let entry = entry.unwrap();
                let path = entry.path();
                constr_file_lst(&path, file_list, visited)?;
            }
        }
        Ok(())
    }

    let mut file_list = Vec::new();
    let mut visited = HashSet::new();

    for path in paths {
        constr_file_lst(path, &mut file_list, &mut visited)?;
    }
    Ok(file_list)
}

#[derive(Debug)]
enum FileContentsRequest {
    Size {
        stream_id: i32,
        file_idx: usize,
    },

    Range {
        stream_id: i32,
        file_idx: usize,
        offset: u64,
        length: u64,
    },
}

pub struct ClipboardContext {
    pub fuse_mount_point: PathBuf,
    fuse_handle: Mutex<Option<fuser::BackgroundSession>>,

    fuse_server: Arc<Mutex<FuseServer>>,

    clipboard: Arc<dyn SysClipboard>,
}

impl ClipboardContext {
    pub fn new(timeout: Duration, mount_path: PathBuf) -> Result<Self, CliprdrError> {
        // assert mount path exists
        let fuse_mount_point = mount_path.canonicalize().map_err(|e| {
            log::error!("failed to canonicalize mount path: {:?}", e);
            CliprdrError::CliprdrInit
        })?;

        let fuse_server = Arc::new(Mutex::new(FuseServer::new(timeout)));

        let clipboard = get_sys_clipboard(&fuse_mount_point)?;
        let clipboard = Arc::from(clipboard) as Arc<_>;

        Ok(Self {
            fuse_mount_point,
            fuse_server,
            fuse_handle: Mutex::new(None),
            clipboard,
        })
    }

    pub fn run(&self) -> Result<(), CliprdrError> {
        if !self.is_stopped() {
            return Ok(());
        }

        let mut fuse_handle = self.fuse_handle.lock();

        let mount_path = &self.fuse_mount_point;

        let mnt_opts = [
            MountOption::FSName("rustdesk-cliprdr-fs".to_string()),
            MountOption::RO,
            MountOption::NoAtime,
        ];
        log::info!(
            "mounting clipboard FUSE to {}",
            self.fuse_mount_point.display()
        );

        let new_handle = fuser::spawn_mount2(
            FuseServer::client(self.fuse_server.clone()),
            mount_path,
            &mnt_opts,
        )
        .map_err(|e| {
            log::error!("failed to mount cliprdr fuse: {:?}", e);
            CliprdrError::CliprdrInit
        })?;
        *fuse_handle = Some(new_handle);

        let clipboard = self.clipboard.clone();

        std::thread::spawn(move || {
            log::debug!("start listening clipboard");
            clipboard.start();
        });

        Ok(())
    }

    /// set clipboard data from file list
    pub fn set_clipboard(&self, paths: &[PathBuf]) -> Result<(), CliprdrError> {
        let prefix = self.fuse_mount_point.clone();
        let paths: Vec<PathBuf> = paths.iter().cloned().map(|p| prefix.join(p)).collect();
        log::debug!("setting clipboard with paths: {:?}", paths);
        self.clipboard.set_file_list(&paths)?;
        log::debug!("clipboard set, paths: {:?}", paths);
        Ok(())
    }

    fn serve_file_contents(
        &self,
        conn_id: i32,
        request: FileContentsRequest,
    ) -> Result<(), CliprdrError> {
        log::debug!("file contents (range) requested from conn: {}", conn_id);
        let file_contents_req = match request {
            FileContentsRequest::Size {
                stream_id,
                file_idx,
            } => {
                let file_list = self.clipboard.get_file_list()?;
                let Some(file) = file_list.get(file_idx) else {
                    log::error!(
                        "invalid file index {} requested from conn: {}",
                        file_idx,
                        conn_id
                    );
                    resp_file_contents_fail(conn_id, stream_id);

                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "invalid file index {} requested from conn: {}",
                            file_idx, conn_id
                        ),
                    });
                };

                log::debug!("conn {} requested file {}", conn_id, file.name);

                let size = file.size;
                ClipboardFile::FileContentsResponse {
                    msg_flags: 0x1,
                    stream_id,
                    requested_data: size.to_le_bytes().to_vec(),
                }
            }
            FileContentsRequest::Range {
                stream_id,
                file_idx,
                offset,
                length,
            } => {
                let file_list = self.clipboard.get_file_list()?;
                let Some(file) = file_list.get(file_idx) else {
                    log::error!(
                        "invalid file index {} requested from conn: {}",
                        file_idx,
                        conn_id
                    );
                    resp_file_contents_fail(conn_id, stream_id);
                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "invalid file index {} requested from conn: {}",
                            file_idx, conn_id
                        ),
                    });
                };
                log::debug!("conn {} requested file {}", conn_id, file.name);

                let Some(handle) = &file.handle else {
                    log::error!(
                        "invalid file index {} requested from conn: {}",
                        file_idx,
                        conn_id
                    );
                    resp_file_contents_fail(conn_id, stream_id);

                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "request to read directory on index {} as file from conn: {}",
                            file_idx, conn_id
                        ),
                    });
                };

                if offset > file.size {
                    log::error!("invalid reading offset requested from conn: {}", conn_id);
                    resp_file_contents_fail(conn_id, stream_id);

                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "invalid reading offset requested from conn: {}",
                            conn_id
                        ),
                    });
                }
                let read_size = if offset + length > file.size {
                    file.size - offset
                } else {
                    length
                };

                let mut buf = vec![0u8; read_size as usize];

                handle
                    .read_exact_at(&mut buf, offset)
                    .map_err(|e| CliprdrError::FileError {
                        path: file.path.clone(),
                        err: e,
                    })?;

                ClipboardFile::FileContentsResponse {
                    msg_flags: 0x1,
                    stream_id,
                    requested_data: buf,
                }
            }
        };

        send_data(conn_id, file_contents_req);
        log::debug!("file contents sent to conn: {}", conn_id);
        Ok(())
    }
}

fn resp_file_contents_fail(conn_id: i32, stream_id: i32) {
    let resp = ClipboardFile::FileContentsResponse {
        msg_flags: 0x2,
        stream_id,
        requested_data: vec![],
    };
    send_data(conn_id, resp)
}

impl ClipboardContext {
    pub fn is_stopped(&self) -> bool {
        self.fuse_handle.lock().is_none()
    }

    pub fn serve(&self, conn_id: i32, msg: ClipboardFile) -> Result<(), CliprdrError> {
        if self.is_stopped() {
            log::debug!("cliprdr stopped, restart it");
            self.run()?;
        }
        match msg {
            ClipboardFile::NotifyCallback { .. } => {
                unreachable!()
            }
            ClipboardFile::MonitorReady => {
                log::debug!("server_monitor_ready called");

                self.send_file_list(conn_id)?;

                Ok(())
            }

            ClipboardFile::FormatList { format_list } => {
                log::debug!("server_format_list called");
                // filter out "FileGroupDescriptorW" and "FileContents"
                let fmt_lst: Vec<(i32, String)> = format_list
                    .into_iter()
                    .filter(|(_, name)| {
                        name == FILEDESCRIPTORW_FORMAT_NAME || name == FILECONTENTS_FORMAT_NAME
                    })
                    .collect();
                if fmt_lst.len() != 2 {
                    log::debug!("no supported formats");
                    return Ok(());
                }
                log::debug!("supported formats: {:?}", fmt_lst);
                let file_contents_id = fmt_lst
                    .iter()
                    .find(|(_, name)| name == FILECONTENTS_FORMAT_NAME)
                    .map(|(id, _)| *id)
                    .unwrap();
                let file_descriptor_id = fmt_lst
                    .iter()
                    .find(|(_, name)| name == FILEDESCRIPTORW_FORMAT_NAME)
                    .map(|(id, _)| *id)
                    .unwrap();

                add_remote_format(FILECONTENTS_FORMAT_NAME, file_contents_id);
                add_remote_format(FILEDESCRIPTORW_FORMAT_NAME, file_descriptor_id);

                // sync file system from peer
                let data = ClipboardFile::FormatDataRequest {
                    requested_format_id: file_descriptor_id,
                };
                send_data(conn_id, data);

                Ok(())
            }
            ClipboardFile::FormatListResponse { msg_flags } => {
                log::debug!("server_format_list_response called");
                if msg_flags != 0x1 {
                    send_format_list(conn_id)
                } else {
                    Ok(())
                }
            }
            ClipboardFile::FormatDataRequest {
                requested_format_id,
            } => {
                log::debug!("server_format_data_request called");
                let Some(format) = get_local_format(requested_format_id) else {
                    log::error!(
                        "got unsupported format data request: id={} from conn={}",
                        requested_format_id,
                        conn_id
                    );
                    resp_format_data_failure(conn_id);
                    return Ok(());
                };

                if format == FILEDESCRIPTORW_FORMAT_NAME {
                    self.send_file_list(conn_id)?;
                } else if format == FILECONTENTS_FORMAT_NAME {
                    log::error!(
                        "try to read file contents with FormatDataRequest from conn={}",
                        conn_id
                    );
                    resp_format_data_failure(conn_id);
                } else {
                    log::error!(
                        "got unsupported format data request: id={} from conn={}",
                        requested_format_id,
                        conn_id
                    );
                    resp_format_data_failure(conn_id);
                }
                Ok(())
            }
            ClipboardFile::FormatDataResponse {
                msg_flags,
                format_data,
            } => {
                log::debug!("server_format_data_response called");

                if msg_flags != 0x1 {
                    resp_format_data_failure(conn_id);
                    return Ok(());
                }

                // this must be a file descriptor format data
                let files = FileDescription::parse_file_descriptors(format_data.into(), conn_id)?;

                let paths = {
                    let mut fuse_guard = self.fuse_server.lock();
                    fuse_guard.load_file_list(files)?;

                    fuse_guard.list_root()
                };

                self.set_clipboard(&paths)?;
                Ok(())
            }
            ClipboardFile::FileContentsResponse { .. } => {
                log::debug!("server_file_contents_response called");
                // we don't know its corresponding request, no resend can be performed
                self.fuse_server.lock().serve(msg)?;
                Ok(())
            }
            ClipboardFile::FileContentsRequest {
                stream_id,
                list_index,
                dw_flags,
                n_position_low,
                n_position_high,
                cb_requested,
                ..
            } => {
                log::debug!("server_file_contents_request called");
                let fcr = if dw_flags == 0x1 {
                    FileContentsRequest::Size {
                        stream_id,
                        file_idx: list_index as usize,
                    }
                } else if dw_flags == 0x2 {
                    let offset = (n_position_high as u64) << 32 | n_position_low as u64;
                    let length = cb_requested as u64;

                    FileContentsRequest::Range {
                        stream_id,
                        file_idx: list_index as usize,
                        offset,
                        length,
                    }
                } else {
                    log::error!("got invalid FileContentsRequest from conn={}", conn_id);
                    resp_file_contents_fail(conn_id, stream_id);
                    return Ok(());
                };

                self.serve_file_contents(conn_id, fcr)
            }
        }
    }

    fn send_file_list(&self, conn_id: i32) -> Result<(), CliprdrError> {
        let file_list = self.clipboard.get_file_list()?;
        let paths = file_list.into_iter().map(|lf| lf.path).collect();

        send_file_list(paths, conn_id)
    }
}

impl CliprdrServiceContext for ClipboardContext {
    fn set_is_stopped(&mut self) -> Result<(), CliprdrError> {
        // unmount the fuse
        if let Some(fuse_handle) = self.fuse_handle.lock().take() {
            fuse_handle.join();
        }
        self.clipboard.stop();
        Ok(())
    }

    fn empty_clipboard(&mut self, _conn_id: i32) -> Result<bool, CliprdrError> {
        self.clipboard.set_file_list(&[])?;
        Ok(true)
    }

    fn server_clip_file(&mut self, conn_id: i32, msg: ClipboardFile) -> Result<(), CliprdrError> {
        self.serve(conn_id, msg)
    }
}

fn resp_format_data_failure(conn_id: i32) {
    let data = ClipboardFile::FormatDataResponse {
        msg_flags: 0x2,
        format_data: vec![],
    };
    send_data(conn_id, data)
}

fn send_format_list(conn_id: i32) -> Result<(), CliprdrError> {
    log::debug!("send format list to remote, conn={}", conn_id);
    let fd_format_name = get_local_format(FILEDESCRIPTOR_FORMAT_ID)
        .unwrap_or(FILEDESCRIPTORW_FORMAT_NAME.to_string());
    let fc_format_name =
        get_local_format(FILECONTENTS_FORMAT_ID).unwrap_or(FILECONTENTS_FORMAT_NAME.to_string());
    let format_list = ClipboardFile::FormatList {
        format_list: vec![
            (FILEDESCRIPTOR_FORMAT_ID, fd_format_name),
            (FILECONTENTS_FORMAT_ID, fc_format_name),
        ],
    };

    send_data(conn_id, format_list);
    log::debug!("format list to remote dispatched, conn={}", conn_id);
    Ok(())
}

fn send_file_list(paths: Vec<PathBuf>, conn_id: i32) -> Result<(), CliprdrError> {
    log::debug!(
        "send file list to remote, conn={}, list={:?}",
        conn_id,
        paths
    );
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
