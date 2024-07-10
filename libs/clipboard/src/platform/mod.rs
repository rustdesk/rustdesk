use crate::{CliprdrError, CliprdrServiceContext};

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub fn create_cliprdr_context(
    enable_files: bool,
    enable_others: bool,
    response_wait_timeout_secs: u32,
) -> crate::ResultType<Box<dyn crate::CliprdrServiceContext>> {
    let boxed =
        windows::create_cliprdr_context(enable_files, enable_others, response_wait_timeout_secs)?
            as Box<_>;
    Ok(boxed)
}

#[cfg(feature = "unix-file-copy-paste")]
#[cfg(any(target_os = "linux", target_os = "macos"))]
/// use FUSE for file pasting on these platforms
pub mod fuse;
#[cfg(feature = "unix-file-copy-paste")]
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod unix;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn create_cliprdr_context(
    _enable_files: bool,
    _enable_others: bool,
    _response_wait_timeout_secs: u32,
) -> crate::ResultType<Box<dyn crate::CliprdrServiceContext>> {
    #[cfg(feature = "unix-file-copy-paste")]
    {
        use std::{fs::Permissions, os::unix::prelude::PermissionsExt};

        use hbb_common::{config::APP_NAME, log};

        if !_enable_files {
            return Ok(Box::new(DummyCliprdrContext {}) as Box<_>);
        }

        let timeout = std::time::Duration::from_secs(_response_wait_timeout_secs as u64);

        let app_name = APP_NAME.read().unwrap().clone();

        let mnt_path = format!("/tmp/{}/{}", app_name, "cliprdr");

        // this function must be called after the main IPC is up
        std::fs::create_dir(&mnt_path).ok();
        std::fs::set_permissions(&mnt_path, Permissions::from_mode(0o777)).ok();

        log::info!("clear previously mounted cliprdr FUSE");
        if let Err(e) = std::process::Command::new("umount").arg(&mnt_path).status() {
            log::warn!("umount {:?} may fail: {:?}", mnt_path, e);
        }

        let unix_ctx = unix::ClipboardContext::new(timeout, mnt_path.parse()?)?;
        log::debug!("start cliprdr FUSE");
        unix_ctx.run()?;

        Ok(Box::new(unix_ctx) as Box<_>)
    }

    #[cfg(not(feature = "unix-file-copy-paste"))]
    return Ok(Box::new(DummyCliprdrContext {}) as Box<_>);
}

struct DummyCliprdrContext {}

impl CliprdrServiceContext for DummyCliprdrContext {
    fn set_is_stopped(&mut self) -> Result<(), CliprdrError> {
        Ok(())
    }
    fn empty_clipboard(&mut self, _conn_id: i32) -> Result<bool, CliprdrError> {
        Ok(true)
    }
    fn server_clip_file(
        &mut self,
        _conn_id: i32,
        _msg: crate::ClipboardFile,
    ) -> Result<(), crate::CliprdrError> {
        Ok(())
    }
}

#[cfg(feature = "unix-file-copy-paste")]
#[cfg(any(target_os = "linux", target_os = "macos"))]
// begin of epoch used by microsoft
// 1601-01-01 00:00:00 + LDAP_EPOCH_DELTA*(100 ns) = 1970-01-01 00:00:00
const LDAP_EPOCH_DELTA: u64 = 116444772610000000;
