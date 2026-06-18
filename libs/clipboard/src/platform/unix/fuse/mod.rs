mod cs;

use super::filetype::FileDescription;
use crate::{ClipboardFile, CliprdrError};
use cs::FuseServer;
use fuser::MountOption;
use hbb_common::{config::APP_NAME, log};
use parking_lot::Mutex;
use std::{
    io,
    path::{Path, PathBuf},
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
    if let Some(ctx) = fuse_context_lock.as_ref() {
        if is_mount_point_healthy(&ctx.mount_point) {
            return Ok(());
        }
        log::warn!(
            "clipboard FUSE mount {} is disconnected, remounting",
            ctx.mount_point.display()
        );
        let stale_context = fuse_context_lock.take();
        drop(stale_context);
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
fn prepare_fuse_mount_point(mount_point: &Path) {
    use std::{
        fs::{self, Permissions},
        os::unix::prelude::PermissionsExt,
    };

    if let Some(parent) = mount_point.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            log::warn!("failed to create FUSE mount parent {:?}: {:?}", parent, e);
        }
    }

    unmount_fuse_mount_point(mount_point);

    if let Err(e) = fs::create_dir_all(mount_point) {
        log::warn!(
            "failed to create FUSE mount point {:?}: {:?}",
            mount_point,
            e
        );
    }
    if let Err(e) = fs::set_permissions(mount_point, Permissions::from_mode(0o777)) {
        log::warn!(
            "failed to set FUSE mount point permissions {:?}: {:?}",
            mount_point,
            e
        );
    }
}

fn is_mount_point_healthy(mount_point: &Path) -> bool {
    is_mount_point_healthy_result(std::fs::metadata(mount_point))
}

fn is_mount_point_healthy_result<T>(result: io::Result<T>) -> bool {
    match result {
        Ok(_) => true,
        Err(e) => {
            e.raw_os_error() != Some(libc::ENOTCONN) && e.kind() != io::ErrorKind::NotFound
        }
    }
}

fn unmount_fuse_mount_point(mount_point: &Path) {
    if run_unmount_command("umount", &["-l"], mount_point) {
        return;
    }
    if run_unmount_command("fusermount3", &["-uz"], mount_point) {
        return;
    }
    run_unmount_command("fusermount", &["-uz"], mount_point);
}

fn run_unmount_command(program: &str, args: &[&str], mount_point: &Path) -> bool {
    match std::process::Command::new(program)
        .args(args)
        .arg(mount_point)
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => {
            log::debug!(
                "{} {:?} exited with status {:?}",
                program,
                mount_point,
                status.code()
            );
            return false;
        }
        Err(e) => {
            log::debug!("failed to run {} for {:?}: {:?}", program, mount_point, e);
            return false;
        }
    }
    true
}

fn uninit_fuse_context_(is_client: bool) {
    let mut fuse_context_lock = if is_client {
        FUSE_CONTEXT_CLIENT.lock()
    } else {
        FUSE_CONTEXT_SERVER.lock()
    };
    let ctx = fuse_context_lock.take();
    drop(ctx);
}

impl Drop for FuseContext {
    fn drop(&mut self) {
        log::info!("unmounting clipboard FUSE from {}", self.mount_point.display());
        unmount_fuse_mount_point(&self.mount_point);
        if let Some(session) = self.session.lock().take() {
            session.join();
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io};

    #[test]
    fn reports_disconnected_fuse_mount_as_unhealthy() {
        let err = io::Error::from_raw_os_error(libc::ENOTCONN);

        assert!(!is_mount_point_healthy_result::<()>(Err(err)));
    }

    #[test]
    fn reports_existing_directory_mount_point_as_healthy() {
        let mount_point = std::env::temp_dir().join(format!(
            "rustdesk-fuse-mount-health-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&mount_point);
        fs::create_dir(&mount_point).unwrap();

        assert!(is_mount_point_healthy(&mount_point));

        let _ = fs::remove_dir_all(&mount_point);
    }
}
