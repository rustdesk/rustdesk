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
pub mod unix;

#[cfg(target_os = "macos")]
pub fn create_cliprdr_context(
    _enable_files: bool,
    _enable_others: bool,
    _response_wait_timeout_secs: u32,
) -> crate::ResultType<Box<dyn crate::CliprdrServiceContext>> {
    let boxed = unix::macos::pasteboard_context::create_pasteboard_context()? as Box<_>;
    Ok(boxed)
}
