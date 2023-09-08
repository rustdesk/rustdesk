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

#[cfg(any(target_os = "linux", target_os = "macos"))]
/// use FUSE for file pasting on these platforms
pub mod fuse;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub fn create_cliprdr_context(
    enable_files: bool,
    _enable_others: bool,
    response_wait_timeout_secs: u32,
) -> crate::ResultType<Box<dyn crate::CliprdrServiceContext>> {
    use std::sync::Arc;

    if !enable_files {
        return Ok(Box::new(DummyCliprdrContext {}) as Box<_>);
    }

    let timeout = std::time::Duration::from_secs(response_wait_timeout_secs as u64);
    let mut tmp_path = std::env::temp_dir();
    tmp_path.push("rustdesk-cliprdr");
    let rd_mnt = tmp_path;
    std::fs::create_dir(rd_mnt.clone())?;
    let linux_ctx = Arc::new(linux::ClipboardContext::new(timeout, rd_mnt)?);

    let fuse_ctx = linux_ctx.clone();
    std::thread::spawn(move || fuse_ctx.mount());
    let clipboard_listen_ctx = linux_ctx.clone();
    std::thread::spawn(move || clipboard_listen_ctx.listen_clipboard());

    Ok(Box::new(linux_ctx.client()) as Box<_>)
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

// begin of epoch used by microsoft
// 1601-01-01 00:00:00 + LDAP_EPOCH_DELTA*(100 ns) = 1970-01-01 00:00:00
#[cfg(target_os = "linux")]
const LDAP_EPOCH_DELTA: u64 = 116444772610000000;
