#![allow(dead_code)]

pub const ERR_SUCCESS: i32 = 0;

// ======================================================
// Errors from the plugins, must be handled by RustDesk

pub const ERR_RUSTDESK_HANDLE_BASE: i32 = 10000;

// not loaded
pub const ERR_PLUGIN_LOAD: i32 = 10001;
// not initialized
pub const ERR_PLUGIN_MSG_INIT: i32 = 10101;
pub const ERR_PLUGIN_MSG_INIT_INVALID: i32 = 10102;
pub const ERR_PLUGIN_MSG_GET_LOCAL_PEER_ID: i32 = 10103;
pub const ERR_PLUGIN_SIGNATURE_NOT_VERIFIED: i32 = 10104;
pub const ERR_PLUGIN_SIGNATURE_VERIFICATION_FAILED: i32 = 10105;
// invalid
pub const ERR_CALL_UNIMPLEMENTED: i32 = 10201;
pub const ERR_CALL_INVALID_METHOD: i32 = 10202;
pub const ERR_CALL_NOT_SUPPORTED_METHOD: i32 = 10203;
pub const ERR_CALL_INVALID_PEER: i32 = 10204;
// failed on calling
pub const ERR_CALL_INVALID_ARGS: i32 = 10301;
pub const ERR_PEER_ID_MISMATCH: i32 = 10302;
pub const ERR_CALL_CONFIG_VALUE: i32 = 10303;
// no handlers on calling
pub const ERR_NOT_HANDLED: i32 = 10401;

// ======================================================
// Errors from RustDesk callbacks.

pub const ERR_CALLBACK_HANDLE_BASE: i32 = 20000;
pub const ERR_CALLBACK_PLUGIN_ID: i32 = 20001;
pub const ERR_CALLBACK_INVALID_ARGS: i32 = 20002;
pub const ERR_CALLBACK_INVALID_MSG: i32 = 20003;
pub const ERR_CALLBACK_TARGET: i32 = 20004;
pub const ERR_CALLBACK_TARGET_TYPE: i32 = 20005;
pub const ERR_CALLBACK_PEER_NOT_FOUND: i32 = 20006;

pub const ERR_CALLBACK_FAILED: i32 = 21001;

// ======================================================
// Errors from the plugins, should be handled by the plugins.

pub const ERR_PLUGIN_HANDLE_BASE: i32 = 30000;

pub const EER_CALL_FAILED: i32 = 30021;
pub const ERR_PEER_ON_FAILED: i32 = 40012;
pub const ERR_PEER_OFF_FAILED: i32 = 40012;
