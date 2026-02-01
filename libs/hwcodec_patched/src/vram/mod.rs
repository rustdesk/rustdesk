pub(crate) mod amf;
pub mod decode;
pub mod encode;
pub(crate) mod ffmpeg;
mod inner;
pub(crate) mod mfx;
pub(crate) mod nv;

pub(crate) const MAX_ADATERS: usize = 16;

use crate::common::{DataFormat, Driver};
pub use serde;
pub use serde_derive;
use serde_derive::{Deserialize, Serialize};
use std::ffi::c_void;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FeatureContext {
    pub driver: Driver,
    pub vendor: Driver,
    pub luid: i64,
    pub data_format: DataFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct DynamicContext {
    #[serde(skip)]
    pub device: Option<*mut c_void>,
    pub width: i32,
    pub height: i32,
    pub kbitrate: i32,
    pub framerate: i32,
    pub gop: i32,
}

unsafe impl Send for DynamicContext {}
unsafe impl Sync for DynamicContext {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct EncodeContext {
    pub f: FeatureContext,
    pub d: DynamicContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct DecodeContext {
    #[serde(skip)]
    pub device: Option<*mut c_void>,
    pub driver: Driver,
    pub vendor: Driver,
    pub luid: i64,
    pub data_format: DataFormat,
}

unsafe impl Send for DecodeContext {}
unsafe impl Sync for DecodeContext {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Available {
    pub e: Vec<FeatureContext>,
    pub d: Vec<DecodeContext>,
}

impl Available {
    pub fn serialize(&self) -> Result<String, ()> {
        match serde_json::to_string_pretty(self) {
            Ok(s) => Ok(s),
            Err(_) => Err(()),
        }
    }

    pub fn deserialize(s: &str) -> Result<Self, ()> {
        match serde_json::from_str(s) {
            Ok(c) => Ok(c),
            Err(_) => Err(()),
        }
    }

    pub fn contains(&self, encode: bool, vendor: Driver, data_format: DataFormat) -> bool {
        if encode {
            self.e
                .iter()
                .any(|f| f.vendor == vendor && f.data_format == data_format)
        } else {
            self.d
                .iter()
                .any(|d| d.vendor == vendor && d.data_format == data_format)
        }
    }
}
