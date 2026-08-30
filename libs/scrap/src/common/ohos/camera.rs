use std::io;

use hbb_common::message_proto::{DisplayInfo, Resolution};

use crate::common::{bail, ResultType};
#[cfg(feature = "vram")]
use crate::AdapterDevice;
use crate::{Frame, TraitCapturer};

pub const PRIMARY_CAMERA_IDX: usize = 0;
const CAMERA_NOT_SUPPORTED: &str = "This platform doesn't support camera yet";

pub struct Cameras;

pub fn primary_camera_exists() -> bool {
    false
}

impl Cameras {
    pub fn all_info() -> ResultType<Vec<DisplayInfo>> {
        Ok(Vec::new())
    }

    pub fn exists(_index: usize) -> bool {
        false
    }

    pub fn get_camera_resolution(_index: usize) -> ResultType<Resolution> {
        bail!(CAMERA_NOT_SUPPORTED);
    }

    pub fn get_sync_cameras() -> Vec<DisplayInfo> {
        Vec::new()
    }

    pub fn get_capturer(_current: usize) -> ResultType<Box<dyn TraitCapturer>> {
        bail!(CAMERA_NOT_SUPPORTED);
    }
}

pub struct CameraCapturer;

impl TraitCapturer for CameraCapturer {
    fn frame<'a>(&'a mut self, _timeout: std::time::Duration) -> io::Result<Frame<'a>> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            CAMERA_NOT_SUPPORTED.to_owned(),
        ))
    }

    #[cfg(feature = "vram")]
    fn device(&self) -> AdapterDevice {
        AdapterDevice::default()
    }

    #[cfg(feature = "vram")]
    fn set_output_texture(&mut self, _texture: bool) {}
}
