mod cs;

use super::filetype::FileDescription;
use crate::{ClipboardFile, CliprdrError};
use cs::FuseServer;
use fuser::MountOption;
use hbb_common::{config::Config, log};
use parking_lot::Mutex;
use std::{
    io,
    path::{Path, PathBuf},
    sync::{mpsc::Sender, Arc},
    time::Duration,
};

lazy_static::lazy_static! {
    static ref FUSE_MOUNT_POINT_CLIENT: Arc<String> = {
        let mnt_path = fuse_mount_point("cliprdr-client");
        // No need to run `canonicalize()` here.
        Arc::new(mnt_path)
    };

    static ref FUSE_MOUNT_POINT_SERVER: Arc<String> = {
        let mnt_path = fuse_mount_point("cliprdr-server");
        // No need to run `canonicalize()` here.
        Arc::new(mnt_path)
    };

    static ref FUSE_CONTEXT_CLIENT: Arc<Mutex<Option<FuseContext>>> = Arc::new(Mutex::new(None));
    static ref FUSE_CONTEXT_SERVER: Arc<Mutex<Option<FuseContext>>> = Arc::new(Mutex::new(None));
}

static FUSE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, PartialEq, Eq)]
enum MountPointState {
    HealthyMount,
    NotMounted,
    StaleMount,
    Unknown,
}

fn fuse_mount_point(name: &str) -> String {
    let mut path = PathBuf::from(Config::ipc_path(""));
    path.pop();
    path.push(name);
    path.to_string_lossy().to_string()
}

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
        match inspect_mount_point_state(&ctx.mount_point) {
            MountPointState::HealthyMount => return Ok(()),
            MountPointState::StaleMount | MountPointState::NotMounted => {
                log::warn!(
                    "clipboard FUSE mount {} is disconnected, remounting",
                    ctx.mount_point.display()
                );
                let stale_context = fuse_context_lock.take();
                drop(fuse_context_lock);
                drop(stale_context);
                return init_fuse_context(is_client);
            }
            MountPointState::Unknown => {
                log::warn!(
                    "failed to verify clipboard FUSE mount {}",
                    ctx.mount_point.display()
                );
                return Err(CliprdrError::CliprdrInit);
            }
        }
    }
    let mount_point = if is_client {
        FUSE_MOUNT_POINT_CLIENT.clone()
    } else {
        FUSE_MOUNT_POINT_SERVER.clone()
    };

    let mount_point = std::path::PathBuf::from(&*mount_point);
    match inspect_mount_point_state(&mount_point) {
        MountPointState::HealthyMount => {
            log::warn!(
                "clipboard FUSE mount {} is already active in another context",
                mount_point.display()
            );
            return Err(CliprdrError::ClipboardOccupied);
        }
        MountPointState::StaleMount => {
            log::warn!(
                "clipboard FUSE mount {} is stale, cleaning up before remount",
                mount_point.display()
            );
            unmount_fuse_mount_point(&mount_point);
            validate_mount_state_after_stale_cleanup(
                &mount_point,
                inspect_mount_point_state(&mount_point),
            )?;
        }
        MountPointState::Unknown => return Err(CliprdrError::CliprdrInit),
        MountPointState::NotMounted => {}
    }
    let (server, tx) = FuseServer::new(FUSE_TIMEOUT);
    let server = Arc::new(Mutex::new(server));

    prepare_fuse_mount_point(&mount_point)?;
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
fn prepare_fuse_mount_point(mount_point: &PathBuf) -> Result<(), CliprdrError> {
    use std::{
        fs::{self, Permissions},
        os::unix::prelude::PermissionsExt,
    };

    if let Some(parent) = mount_point.parent() {
        reject_symlink_path(parent)?;
        if let Err(e) = fs::create_dir_all(parent) {
            log::warn!("failed to create FUSE mount parent {:?}: {:?}", parent, e);
            return Err(CliprdrError::CliprdrInit);
        }
    }

    reject_symlink_path(mount_point)?;

    let recovered_stale_mount = if let Err(e) = fs::create_dir_all(mount_point) {
        log::warn!(
            "failed to create clipboard FUSE mount point {}, trying stale mount cleanup: {:?}",
            mount_point.display(),
            e
        );
        unmount_fuse_mount_point(mount_point);
        fs::create_dir_all(mount_point).map_err(|e| {
            log::error!(
                "failed to create clipboard FUSE mount point {} after cleanup: {:?}",
                mount_point.display(),
                e
            );
            CliprdrError::CliprdrInit
        })?;
        true
    } else {
        false
    };
    if let Err(e) = fs::set_permissions(mount_point, Permissions::from_mode(0o777)) {
        log::warn!(
            "failed to set clipboard FUSE mount point permissions {}: {:?}",
            mount_point.display(),
            e
        );
    }

    if !recovered_stale_mount {
        unmount_fuse_mount_point(mount_point);
    }
    Ok(())
}

fn inspect_mount_point_state(mount_point: &Path) -> MountPointState {
    if ensure_mount_point_path_is_safe(mount_point).is_err() {
        return MountPointState::Unknown;
    }
    inspect_mount_point_state_with(
        mount_point,
        std::fs::metadata(mount_point),
        std::fs::read_to_string("/proc/self/mountinfo"),
    )
}

fn validate_mount_state_after_stale_cleanup(
    mount_point: &Path,
    mount_state: MountPointState,
) -> Result<(), CliprdrError> {
    match mount_state {
        MountPointState::NotMounted => Ok(()),
        MountPointState::HealthyMount => {
            log::warn!(
                "clipboard FUSE mount {} is still active after stale cleanup",
                mount_point.display()
            );
            Err(CliprdrError::ClipboardOccupied)
        }
        MountPointState::StaleMount => {
            log::warn!(
                "clipboard FUSE mount {} is still stale after cleanup",
                mount_point.display()
            );
            Err(CliprdrError::CliprdrInit)
        }
        MountPointState::Unknown => {
            log::warn!(
                "failed to verify clipboard FUSE mount {} after cleanup",
                mount_point.display()
            );
            Err(CliprdrError::CliprdrInit)
        }
    }
}

fn inspect_mount_point_state_with<T>(
    mount_point: &Path,
    metadata_result: io::Result<T>,
    mountinfo_result: io::Result<String>,
) -> MountPointState {
    match metadata_result {
        Ok(_) => match mountinfo_result {
            Ok(mountinfo) => {
                if is_mount_point_listed_in_mountinfo(mount_point, &mountinfo) {
                    MountPointState::HealthyMount
                } else {
                    MountPointState::NotMounted
                }
            }
            Err(e) => {
                log::warn!("failed to read mountinfo for {:?}: {:?}", mount_point, e);
                MountPointState::Unknown
            }
        },
        Err(e) if e.raw_os_error() == Some(libc::ENOTCONN) => MountPointState::StaleMount,
        Err(e) if e.kind() == io::ErrorKind::NotFound => MountPointState::NotMounted,
        Err(e) => {
            log::warn!("failed to inspect FUSE mount {:?}: {:?}", mount_point, e);
            MountPointState::Unknown
        }
    }
}

fn is_mount_point_listed_in_mountinfo(mount_point: &Path, mountinfo: &str) -> bool {
    let mount_point = mount_point.to_string_lossy();
    mountinfo.lines().any(|line| {
        let mut fields = line.split_whitespace();
        let _mount_id = fields.next();
        let _parent_id = fields.next();
        let _major_minor = fields.next();
        let _root = fields.next();
        let mount_path = fields.next();
        mount_path == Some(mount_point.as_ref())
    })
}

fn reject_symlink_metadata_result(
    path: &Path,
    metadata_result: io::Result<std::fs::Metadata>,
    allow_disconnected_mount: bool,
) -> Result<(), CliprdrError> {
    match metadata_result {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            log::warn!("refusing to use symlinked FUSE path {:?}", path);
            Err(CliprdrError::CliprdrInit)
        }
        Ok(_) => Ok(()),
        Err(e) if allow_disconnected_mount && e.raw_os_error() == Some(libc::ENOTCONN) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => {
            log::warn!("failed to inspect FUSE path {:?}: {:?}", path, e);
            Err(CliprdrError::CliprdrInit)
        }
    }
}

fn reject_symlink_path(path: &Path) -> Result<(), CliprdrError> {
    reject_symlink_metadata_result(path, std::fs::symlink_metadata(path), false)
}

fn ensure_mount_point_path_is_safe(mount_point: &Path) -> Result<(), CliprdrError> {
    if let Some(parent) = mount_point.parent() {
        reject_symlink_path(parent)?;
    }
    reject_symlink_metadata_result(mount_point, std::fs::symlink_metadata(mount_point), true)
}

fn unmount_fuse_mount_point(mount_point: &Path) {
    if ensure_mount_point_path_is_safe(mount_point).is_err() {
        log::warn!(
            "refusing to unmount unsafe clipboard FUSE mount point {:?}",
            mount_point
        );
        return;
    }
    if inspect_mount_point_state_with(
        mount_point,
        std::fs::metadata(mount_point),
        std::fs::read_to_string("/proc/self/mountinfo"),
    ) == MountPointState::NotMounted
    {
        return;
    }
    for (program, args) in unmount_command_candidates() {
        if run_unmount_command(program, args, mount_point) {
            return;
        }
    }
    log::warn!(
        "failed to unmount clipboard FUSE mount point {:?}",
        mount_point
    );
}

fn unmount_command_candidates() -> [(&'static str, &'static [&'static str]); 3] {
    [
        ("fusermount3", &["-uz"]),
        ("fusermount", &["-uz"]),
        ("umount", &["-l"]),
    ]
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
    let ctx = {
        let mut fuse_context_lock = if is_client {
            FUSE_CONTEXT_CLIENT.lock()
        } else {
            FUSE_CONTEXT_SERVER.lock()
        };
        fuse_context_lock.take()
    };
    drop(ctx);
}

impl Drop for FuseContext {
    fn drop(&mut self) {
        self.session.lock().take().map(|s| s.join());
        log::info!(
            "unmounting clipboard FUSE from {}",
            self.mount_point.display()
        );
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

    #[cfg(target_family = "unix")]
    use std::os::unix::fs::symlink;

    #[test]
    fn classifies_mount_point_state_from_metadata_and_mountinfo() {
        let mount_point = std::env::temp_dir().join(format!(
            "rustdesk-fuse-mount-state-test-{}-{}",
            std::process::id(),
            line!()
        ));
        let mountinfo = format!(
            "123 1 0:45 / {} rw,nosuid,nodev - fuse.rustdesk rustdesk rw\n",
            mount_point.display()
        );

        assert_eq!(
            inspect_mount_point_state_with(&mount_point, Ok(()), Ok(mountinfo)),
            MountPointState::HealthyMount
        );
        assert_eq!(
            inspect_mount_point_state_with(&mount_point, Ok(()), Ok(String::new())),
            MountPointState::NotMounted
        );

        let disconnected_metadata: io::Result<()> =
            Err(io::Error::from_raw_os_error(libc::ENOTCONN));
        assert_eq!(
            inspect_mount_point_state_with(&mount_point, disconnected_metadata, Ok(String::new())),
            MountPointState::StaleMount
        );
    }

    #[test]
    #[cfg(target_family = "unix")]
    fn rejects_symlink_mount_point() {
        let base = std::env::temp_dir().join(format!(
            "rustdesk-fuse-symlink-test-{}-{}",
            std::process::id(),
            line!()
        ));
        let mount_parent = base.join("parent");
        let mount_point = mount_parent.join("cliprdr-client");
        let symlink_target = base.join("symlink-target");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        fs::create_dir_all(&mount_parent).unwrap();
        fs::create_dir_all(&symlink_target).unwrap();
        symlink(&symlink_target, &mount_point).unwrap();

        assert!(matches!(
            prepare_fuse_mount_point(&mount_point),
            Err(CliprdrError::CliprdrInit)
        ));

        let _ = fs::remove_dir_all(&base);
    }
}
