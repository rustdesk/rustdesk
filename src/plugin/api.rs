use std::ffi::{c_char};

use lazy_static::lazy_static;

use crate::{
    flutter::{FlutterHandler, SESSIONS},
    plugins::PLUGIN_REGISTRAR,
    ui_session_interface::Session,
};

pub trait Callable {
    // Call 
    fn onCall(method_name: String) -> bool;
}