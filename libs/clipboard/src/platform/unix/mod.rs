use std::{
    path::{Path, PathBuf},
    sync::{mpsc::Sender, Arc},
    time::Duration,
};

use dashmap::DashMap;
use fuser::MountOption;
use hbb_common::{
    bytes::{BufMut, BytesMut},
    log,
};
use lazy_static::lazy_static;
use parking_lot::Mutex;

use crate::{
    platform::{fuse::FileDescription, unix::local_file::construct_file_list},
    send_data, ClipboardFile, CliprdrError, CliprdrServiceContext,
};

use self::local_file::LocalFile;
#[cfg(target_os = "linux")]
use self::url::{encode_path_to_uri, parse_plain_uri_list};

use super::fuse::FuseServer;

#[cfg(target_os = "linux")]
/// clipboard implementation of x11
pub mod x11;

#[cfg(target_os = "macos")]
/// clipboard implementation of macos
pub mod ns_clipboard;

pub mod local_file;

#[cfg(target_os = "linux")]
pub mod url;

// not actual format id, just a placeholder
const FILEDESCRIPTOR_FORMAT_ID: i32 = 49334;
const FILEDESCRIPTORW_FORMAT_NAME: &str = "FileGroupDescriptorW";
// not actual format id, just a placeholder
const FILECONTENTS_FORMAT_ID: i32 = 49267;
const FILECONTENTS_FORMAT_NAME: &str = "FileContents";

lazy_static! {
    static ref REMOTE_FORMAT_MAP: DashMap<i32, String> = DashMap::from_iter(
        [
            (
                FILEDESCRIPTOR_FORMAT_ID,
                FILEDESCRIPTORW_FORMAT_NAME.to_string()
            ),
            (FILECONTENTS_FORMAT_ID, FILECONTENTS_FORMAT_NAME.to_string())
        ]
        .iter()
        .cloned()
    );
}

fn get_local_format(remote_id: i32) -> Option<String> {
    REMOTE_FORMAT_MAP.get(&remote_id).map(|s| s.clone())
}

fn add_remote_format(local_name: &str, remote_id: i32) {
    REMOTE_FORMAT_MAP.insert(remote_id, local_name.to_string());
}

trait SysClipboard: Send + Sync {
    fn start(&self);

    fn set_file_list(&self, paths: &[PathBuf]) -> Result<(), CliprdrError>;
    fn get_file_list(&self) -> Vec<PathBuf>;
}

#[cfg(target_os = "linux")]
fn get_sys_clipboard(ignore_path: &Path) -> Result<Box<dyn SysClipboard>, CliprdrError> {
    #[cfg(feature = "wayland")]
    {
        unimplemented!()
    }
    #[cfg(not(feature = "wayland"))]
    {
        use x11::*;
        let x11_clip = X11Clipboard::new(ignore_path)?;
        Ok(Box::new(x11_clip) as Box<_>)
    }
}

#[cfg(target_os = "macos")]
fn get_sys_clipboard(ignore_path: &Path) -> Result<Box<dyn SysClipboard>, CliprdrError> {
    use ns_clipboard::*;
    let ns_pb = NsPasteboard::new(ignore_path)?;
    Ok(Box::new(ns_pb) as Box<_>)
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
    /// stores fuse background session handle
    fuse_handle: Mutex<Option<fuser::BackgroundSession>>,

    /// a sender of clipboard file contents pdu to fuse server
    fuse_tx: Sender<ClipboardFile>,
    fuse_server: Arc<Mutex<FuseServer>>,

    clipboard: Arc<dyn SysClipboard>,
    local_files: Mutex<Vec<LocalFile>>,
}

impl ClipboardContext {
    pub fn new(timeout: Duration, mount_path: PathBuf) -> Result<Self, CliprdrError> {
        // assert mount path exists
        let fuse_mount_point = mount_path.canonicalize().map_err(|e| {
            log::error!("failed to canonicalize mount path: {:?}", e);
            CliprdrError::CliprdrInit
        })?;

        let (fuse_server, fuse_tx) = FuseServer::new(timeout);

        let fuse_server = Arc::new(Mutex::new(fuse_server));

        let clipboard = get_sys_clipboard(&fuse_mount_point)?;
        let clipboard = Arc::from(clipboard) as Arc<_>;
        let local_files = Mutex::new(vec![]);

        Ok(Self {
            fuse_mount_point,
            fuse_server,
            fuse_tx,
            fuse_handle: Mutex::new(None),
            clipboard,
            local_files,
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
            MountOption::NoAtime,
            MountOption::RO,
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
        let mut file_list = self.local_files.lock();

        let (file_idx, file_contents_resp) = match request {
            FileContentsRequest::Size {
                stream_id,
                file_idx,
            } => {
                log::debug!("file contents (size) requested from conn: {}", conn_id);
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

                log::debug!(
                    "conn {} requested file-{}: {}",
                    conn_id,
                    file_idx,
                    file.name
                );

                let size = file.size;
                (
                    file_idx,
                    ClipboardFile::FileContentsResponse {
                        msg_flags: 0x1,
                        stream_id,
                        requested_data: size.to_le_bytes().to_vec(),
                    },
                )
            }
            FileContentsRequest::Range {
                stream_id,
                file_idx,
                offset,
                length,
            } => {
                log::debug!(
                    "file contents (range from {} length {}) request from conn: {}",
                    offset,
                    length,
                    conn_id
                );
                let Some(file) = file_list.get_mut(file_idx) else {
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
                log::debug!(
                    "conn {} requested file-{}: {}",
                    conn_id,
                    file_idx,
                    file.name
                );

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

                file.read_exact_at(&mut buf, offset)?;

                (
                    file_idx,
                    ClipboardFile::FileContentsResponse {
                        msg_flags: 0x1,
                        stream_id,
                        requested_data: buf,
                    },
                )
            }
        };

        send_data(conn_id, file_contents_resp);
        log::debug!("file contents sent to conn: {}", conn_id);
        // hot reload next file
        for next_file in file_list.iter_mut().skip(file_idx + 1) {
            if !next_file.is_dir {
                next_file.load_handle()?;
                break;
            }
        }
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

    pub fn sync_local_files(&self) -> Result<(), CliprdrError> {
        let mut local_files = self.local_files.lock();
        let clipboard_files = self.clipboard.get_file_list();
        let local_file_list: Vec<PathBuf> = local_files.iter().map(|f| f.path.clone()).collect();
        if local_file_list == clipboard_files {
            return Ok(());
        }
        let new_files = construct_file_list(&clipboard_files)?;
        *local_files = new_files;
        Ok(())
    }

    pub fn serve(&self, conn_id: i32, msg: ClipboardFile) -> Result<(), CliprdrError> {
        log::debug!("serve clipboard file from conn: {}", conn_id);
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
                    .map(|(id, _)| *id)?;
                let file_descriptor_id = fmt_lst
                    .iter()
                    .find(|(_, name)| name == FILEDESCRIPTORW_FORMAT_NAME)
                    .map(|(id, _)| *id)?;

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
                log::debug!(
                    "server_format_data_response called, msg_flags={}",
                    msg_flags
                );

                if msg_flags != 0x1 {
                    resp_format_data_failure(conn_id);
                    return Ok(());
                }

                log::debug!("parsing file descriptors");
                // this must be a file descriptor format data
                let files = FileDescription::parse_file_descriptors(format_data, conn_id)?;

                let paths = {
                    let mut fuse_guard = self.fuse_server.lock();
                    fuse_guard.load_file_list(files)?;

                    fuse_guard.list_root()
                };

                log::debug!("load file list: {:?}", paths);
                self.set_clipboard(&paths)?;
                Ok(())
            }
            ClipboardFile::FileContentsResponse { .. } => {
                log::debug!("server_file_contents_response called");
                // we don't know its corresponding request, no resend can be performed
                self.fuse_tx.send(msg).map_err(|e| {
                    log::error!("failed to send file contents response to fuse: {:?}", e);
                    CliprdrError::ClipboardInternalError
                })?;
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
        self.sync_local_files()?;

        let file_list = self.local_files.lock();
        send_file_list(&*file_list, conn_id)
    }
}

impl CliprdrServiceContext for ClipboardContext {
    fn set_is_stopped(&mut self) -> Result<(), CliprdrError> {
        // unmount the fuse
        if let Some(fuse_handle) = self.fuse_handle.lock().take() {
            fuse_handle.join();
        }
        // we don't stop the clipboard, keep listening in case of restart
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

fn build_file_list_pdu(files: &[LocalFile]) -> Vec<u8> {
    let mut data = BytesMut::with_capacity(4 + 592 * files.len());
    data.put_u32_le(files.len() as u32);
    for file in files.iter() {
        data.put(file.as_bin().as_slice());
    }

    data.to_vec()
}

fn send_file_list(files: &[LocalFile], conn_id: i32) -> Result<(), CliprdrError> {
    log::debug!(
        "send file list to remote, conn={}, list={:?}",
        conn_id,
        files.iter().map(|f| f.path.display()).collect::<Vec<_>>()
    );

    let format_data = build_file_list_pdu(files);

    send_data(
        conn_id,
        ClipboardFile::FormatDataResponse {
            msg_flags: 1,
            format_data,
        },
    );
    Ok(())
}
