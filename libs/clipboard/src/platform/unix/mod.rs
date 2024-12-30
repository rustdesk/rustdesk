use std::{
    path::PathBuf,
    sync::{mpsc::Sender, Arc},
    time::Duration,
};

use dashmap::DashMap;
use fuser::MountOption;
use hbb_common::{
    bytes::{BufMut, BytesMut},
    config::APP_NAME,
    log,
};
use lazy_static::lazy_static;
use parking_lot::Mutex;

use crate::{
    platform::{fuse::FileDescription, unix::local_file::construct_file_list},
    send_data, ClipboardFile, CliprdrError,
};

use self::local_file::LocalFile;

use super::fuse::FuseServer;

pub mod local_file;

// not actual format id, just a placeholder
pub const FILEDESCRIPTOR_FORMAT_ID: i32 = 49334;
pub const FILEDESCRIPTORW_FORMAT_NAME: &str = "FileGroupDescriptorW";
// not actual format id, just a placeholder
pub const FILECONTENTS_FORMAT_ID: i32 = 49267;
pub const FILECONTENTS_FORMAT_NAME: &str = "FileContents";

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

    static ref FUSE_MOUNT_POINT_CLIENT: Arc<String> = {
        let mnt_path = format!("/tmp/{}/{}", APP_NAME.read().unwrap(), "cliprdr-client");
        // No need to run `canonicalize()` here.
        Arc::new(mnt_path)
    };

    static ref FUSE_MOUNT_POINT_SERVER: Arc<String> = {
        let mnt_path = format!("/tmp/{}/{}", APP_NAME.read().unwrap(), "cliprdr-server");
        // No need to run `canonicalize()` here.
        Arc::new(mnt_path)
    };

    static ref FUSE_CONTEXT_CLIENT: Arc<Mutex<Option<FuseContext>>> = Arc::new(Mutex::new(None));
    static ref FUSE_CONTEXT_SERVER: Arc<Mutex<Option<FuseContext>>> = Arc::new(Mutex::new(None));
}

static FUSE_TIMEOUT: Duration = Duration::from_secs(3);

pub fn get_exclude_paths() -> Vec<Arc<String>> {
    vec![
        FUSE_MOUNT_POINT_CLIENT.clone(),
        FUSE_MOUNT_POINT_SERVER.clone(),
    ]
}

pub fn is_fuse_context_inited(is_client: bool) -> bool {
    if is_client {
        FUSE_CONTEXT_CLIENT.lock().is_some()
    } else {
        FUSE_CONTEXT_SERVER.lock().is_some()
    }
}

pub fn init_fuse_context(is_client: bool) -> Result<(), CliprdrError> {
    let mut fuse_context_lock = if is_client {
        FUSE_CONTEXT_CLIENT.lock()
    } else {
        FUSE_CONTEXT_SERVER.lock()
    };
    if fuse_context_lock.is_some() {
        return Ok(());
    }
    let mount_point = if is_client {
        FUSE_MOUNT_POINT_CLIENT.clone()
    } else {
        FUSE_MOUNT_POINT_SERVER.clone()
    };

    let mount_point = std::path::PathBuf::from(&*mount_point);
    let (server, tx) = FuseServer::new(FUSE_TIMEOUT);
    let server = Arc::new(Mutex::new(server));

    prepare_fuse_mount_point(&mount_point);
    let mnt_opts = [
        MountOption::FSName("rustdesk-cliprdr-fs".to_string()),
        MountOption::NoAtime,
        MountOption::RO,
    ];
    log::info!("mounting clipboard FUSE to {}", mount_point.display());
    let session = fuser::spawn_mount2(
        FuseServer::client(server.clone()),
        mount_point.clone(),
        &mnt_opts,
    )
    .map_err(|e| {
        log::error!("failed to mount cliprdr fuse: {:?}", e);
        CliprdrError::CliprdrInit
    })?;
    let session = Mutex::new(Some(session));

    let ctx = FuseContext {
        server,
        tx,
        mount_point,
        session,
        local_files: Mutex::new(vec![]),
    };
    *fuse_context_lock = Some(ctx);
    Ok(())
}

pub fn uninit_fuse_context(is_client: bool) {
    uninit_fuse_context_(is_client)
}

pub fn format_data_response_to_urls(
    is_client: bool,
    format_data: Vec<u8>,
    conn_id: i32,
) -> Result<Vec<String>, CliprdrError> {
    let ctx = if is_client {
        FUSE_CONTEXT_CLIENT.lock()
    } else {
        FUSE_CONTEXT_SERVER.lock()
    };
    ctx.as_ref()
        .ok_or(CliprdrError::CliprdrInit)?
        .format_data_response_to_urls(format_data, conn_id)
}

pub fn read_file_contents(
    is_client: bool,
    conn_id: i32,
    stream_id: i32,
    list_index: i32,
    dw_flags: i32,
    n_position_low: i32,
    n_position_high: i32,
    cb_requested: i32,
) -> Result<ClipboardFile, CliprdrError> {
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
        return Err(CliprdrError::InvalidRequest {
            description: format!("got invalid FileContentsRequest, dw_flats: {dw_flags}"),
        });
    };

    let ctx = if is_client {
        FUSE_CONTEXT_CLIENT.lock()
    } else {
        FUSE_CONTEXT_SERVER.lock()
    };
    ctx.as_ref()
        .ok_or(CliprdrError::CliprdrInit)?
        .serve_file_contents(conn_id, fcr)
}

pub fn handle_file_content_response(
    is_client: bool,
    clip: ClipboardFile,
) -> Result<(), CliprdrError> {
    // we don't know its corresponding request, no resend can be performed
    let ctx = if is_client {
        FUSE_CONTEXT_CLIENT.lock()
    } else {
        FUSE_CONTEXT_SERVER.lock()
    };
    ctx.as_ref()
        .ok_or(CliprdrError::CliprdrInit)?
        .tx
        .send(clip)
        .map_err(|e| {
            log::error!("failed to send file contents response to fuse: {:?}", e);
            CliprdrError::ClipboardInternalError
        })?;
    Ok(())
}

pub fn empty_local_files(is_client: bool) {
    let ctx = if is_client {
        FUSE_CONTEXT_CLIENT.lock()
    } else {
        FUSE_CONTEXT_SERVER.lock()
    };
    ctx.as_ref().map(|c| c.empty_local_files());
}

struct FuseContext {
    server: Arc<Mutex<FuseServer>>,
    tx: Sender<ClipboardFile>,
    mount_point: PathBuf,
    // stores fuse background session handle
    session: Mutex<Option<fuser::BackgroundSession>>,
    // local files are cached, this value should not be changed when copying files
    // Because `CliprdrFileContentsRequest` only contains the index of the file in the list.
    // We need to keep the file list in the same order as the remote side.
    // We may add a `FileId` field to `CliprdrFileContentsRequest` in the future.
    local_files: Mutex<Vec<LocalFile>>,
}

// this function must be called after the main IPC is up
fn prepare_fuse_mount_point(mount_point: &PathBuf) {
    use std::{
        fs::{self, Permissions},
        os::unix::prelude::PermissionsExt,
    };

    fs::create_dir(mount_point).ok();
    fs::set_permissions(mount_point, Permissions::from_mode(0o777)).ok();

    if let Err(e) = std::process::Command::new("umount")
        .arg(mount_point)
        .status()
    {
        log::warn!("umount {:?} may fail: {:?}", mount_point, e);
    }
}

fn uninit_fuse_context_(is_client: bool) {
    if is_client {
        let _ = FUSE_CONTEXT_CLIENT.lock().take();
    } else {
        let _ = FUSE_CONTEXT_SERVER.lock().take();
    }
}

impl Drop for FuseContext {
    fn drop(&mut self) {
        self.session.lock().take().map(|s| s.join());
    }
}

impl FuseContext {
    pub fn empty_local_files(&self) {
        let mut local_files = self.local_files.lock();
        *local_files = vec![];
        let mut fuse_guard = self.server.lock();
        let _ = fuse_guard.load_file_list(vec![]);
    }

    pub fn format_data_response_to_urls(
        &self,
        format_data: Vec<u8>,
        conn_id: i32,
    ) -> Result<Vec<String>, CliprdrError> {
        let files = FileDescription::parse_file_descriptors(format_data, conn_id)?;

        let paths = {
            let mut fuse_guard = self.server.lock();
            fuse_guard.load_file_list(files)?;

            fuse_guard.list_root()
        };

        let prefix = self.mount_point.clone();
        Ok(paths
            .into_iter()
            .map(|p| prefix.join(p).to_string_lossy().to_string())
            .collect())
    }

    fn sync_local_files(&self, clipboard_files: &[String]) -> Result<(), CliprdrError> {
        let clipboard_files = clipboard_files
            .iter()
            .map(|s| PathBuf::from(s))
            .collect::<Vec<_>>();
        let mut local_files = self.local_files.lock();
        let local_file_list: Vec<PathBuf> = local_files.iter().map(|f| f.path.clone()).collect();
        if local_file_list == clipboard_files {
            return Ok(());
        }
        let new_files = construct_file_list(&clipboard_files)?;
        *local_files = new_files;
        Ok(())
    }

    fn serve_file_contents(
        &self,
        conn_id: i32,
        request: FileContentsRequest,
    ) -> Result<ClipboardFile, CliprdrError> {
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
                    let _ = resp_file_contents_fail(conn_id, stream_id);

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
                    let _ = resp_file_contents_fail(conn_id, stream_id);
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
                    let _ = resp_file_contents_fail(conn_id, stream_id);

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

        log::debug!("file contents sent to conn: {}", conn_id);
        // hot reload next file
        for next_file in file_list.iter_mut().skip(file_idx + 1) {
            if !next_file.is_dir {
                next_file.load_handle()?;
                break;
            }
        }
        Ok(file_contents_resp)
    }
}

pub fn get_local_format(remote_id: i32) -> Option<String> {
    REMOTE_FORMAT_MAP.get(&remote_id).map(|s| s.clone())
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

fn resp_file_contents_fail(conn_id: i32, stream_id: i32) -> Result<(), CliprdrError> {
    let resp = ClipboardFile::FileContentsResponse {
        msg_flags: 0x2,
        stream_id,
        requested_data: vec![],
    };
    send_data(conn_id, resp)
}

pub fn send_format_list(conn_id: i32) -> Result<(), CliprdrError> {
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

    send_data(conn_id, format_list)?;
    log::debug!("format list to remote dispatched, conn={}", conn_id);
    Ok(())
}

pub fn build_file_list_format_data(
    is_client: bool,
    files: &[String],
) -> Result<Vec<u8>, CliprdrError> {
    let ctx = if is_client {
        FUSE_CONTEXT_CLIENT.lock()
    } else {
        FUSE_CONTEXT_SERVER.lock()
    };
    match &*ctx {
        None => Err(CliprdrError::CliprdrInit),
        Some(ctx) => {
            ctx.sync_local_files(files)?;
            Ok(build_file_list_pdu(&ctx.local_files.lock()))
        }
    }
}

fn build_file_list_pdu(files: &[LocalFile]) -> Vec<u8> {
    let mut data = BytesMut::with_capacity(4 + 592 * files.len());
    data.put_u32_le(files.len() as u32);
    for file in files.iter() {
        data.put(file.as_bin().as_slice());
    }

    data.to_vec()
}
