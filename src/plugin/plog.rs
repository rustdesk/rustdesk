use hbb_common::log;
use std::ffi::c_char;

const LOG_LEVEL_TRACE: &[u8; 6] = b"trace\0";
const LOG_LEVEL_DEBUG: &[u8; 6] = b"debug\0";
const LOG_LEVEL_INFO: &[u8; 5] = b"info\0";
const LOG_LEVEL_WARN: &[u8; 5] = b"warn\0";
const LOG_LEVEL_ERROR: &[u8; 6] = b"error\0";

#[inline]
fn is_level(level: *const c_char, level_bytes: &[u8]) -> bool {
    level_bytes == unsafe { std::slice::from_raw_parts(level as *const u8, level_bytes.len()) }
}

#[no_mangle]
pub(super) extern "C" fn plugin_log(level: *const c_char, msg: *const c_char) {
    if level.is_null() || msg.is_null() {
        return;
    }

    if let Ok(msg) = super::cstr_to_string(msg) {
        if is_level(level, LOG_LEVEL_TRACE) {
            log::trace!("{}", msg);
        } else if is_level(level, LOG_LEVEL_DEBUG) {
            log::debug!("{}", msg);
        } else if is_level(level, LOG_LEVEL_INFO) {
            log::info!("{}", msg);
        } else if is_level(level, LOG_LEVEL_WARN) {
            log::warn!("{}", msg);
        } else if is_level(level, LOG_LEVEL_ERROR) {
            log::error!("{}", msg);
        }
    }
}
