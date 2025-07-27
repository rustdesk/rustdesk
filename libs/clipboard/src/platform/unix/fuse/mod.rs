mod cs;

use super::filetype::FileDescription;
use crate::{ClipboardFile, CliprdrError};
use cs::FuseServer;
use fuser::MountOption;
use hbb_common::{config::APP_NAME, log};
use parking_lot::Mutex;
use std::{
    path::PathBuf,
    sync::{mpsc::Sender, Arc},
    time::Duration,
};

lazy_static::lazy_static! {
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

pub fn get_exclude_paths(is_client: bool) -> Arc<String> {
    if is_client {
        FUSE_MOUNT_POINT_CLIENT.clone()
    } else {
        FUSE_MOUNT_POINT_SERVER.clone()
    }
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
    // to-do: ignore the error if the mount point is already mounted
    // Because the sciter version uses separate processes as the controlling side.
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
        conn_id: 0,
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
    let mut ctx = if is_client {
        FUSE_CONTEXT_CLIENT.lock()
    } else {
        FUSE_CONTEXT_SERVER.lock()
    };
    ctx.as_mut()
        .ok_or(CliprdrError::CliprdrInit)?
        .format_data_response_to_urls(format_data, conn_id)
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

pub fn empty_local_files(is_client: bool, conn_id: i32) -> bool {
    let ctx = if is_client {
        FUSE_CONTEXT_CLIENT.lock()
    } else {
        FUSE_CONTEXT_SERVER.lock()
    };
    ctx.as_ref()
        .map(|c| c.empty_local_files(conn_id))
        .unwrap_or(false)
}

struct FuseContext {
    server: Arc<Mutex<FuseServer>>,
    tx: Sender<ClipboardFile>,
    mount_point: PathBuf,
    // stores fuse background session handle
    session: Mutex<Option<fuser::BackgroundSession>>,
    // Indicates the connection ID of that set the clipboard content
    conn_id: i32,
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
        log::info!("unmounting clipboard FUSE from {}", self.mount_point.display());
    }
}

impl FuseContext {
    pub fn empty_local_files(&self, conn_id: i32) -> bool {
        if conn_id != 0 && self.conn_id != conn_id {
            return false;
        }
        let mut fuse_guard = self.server.lock();
        let _ = fuse_guard.load_file_list(vec![]);
        true
    }

    pub fn format_data_response_to_urls(
        &mut self,
        format_data: Vec<u8>,
        conn_id: i32,
    ) -> Result<Vec<String>, CliprdrError> {
        let files = FileDescription::parse_file_descriptors(format_data, conn_id)?;

        let paths = {
            let mut fuse_guard = self.server.lock();
            fuse_guard.load_file_list(files)?;
            self.conn_id = conn_id;

            fuse_guard.list_root()
        };

        let prefix = self.mount_point.clone();
        Ok(paths
            .into_iter()
            .map(|p| prefix.join(p).to_string_lossy().to_string())
            .collect())
    }
}
