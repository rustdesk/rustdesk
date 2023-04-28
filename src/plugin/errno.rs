#![allow(dead_code)]

pub const ERR_SUCCESS: i32 = 0;

// ======================================================
// errors that will be handled by RustDesk

pub const ERR_RUSTDESK_HANDLE_BASE: i32 = 10000;

// not loaded
pub const ERR_PLUGIN_LOAD: i32 = 10001;
// not initialized
pub const ERR_PLUGIN_MSG_CB: i32 = 10101;
// invalid
pub const ERR_CALL_INVALID_METHOD: i32 = 10201;
pub const ERR_CALL_NOT_SUPPORTED_METHOD: i32 = 10202;
// failed on calling
pub const ERR_CALL_INVALID_ARGS: i32 = 10301;
pub const ERR_PEER_ID_MISMATCH: i32 = 10302;
// no handlers on calling
pub const ERR_NOT_HANDLED: i32 = 10401;

// ======================================================
// errors that should be handled by the plugin

pub const ERR_PLUGIN_HANDLE_BASE: i32 = 20000;

pub const EER_CALL_FAILED: i32 = 200021;
pub const ERR_PEER_ON_FAILED: i32 = 30012;
pub const ERR_PEER_OFF_FAILED: i32 = 30012;
