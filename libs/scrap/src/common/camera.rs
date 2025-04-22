use std::{
    io,
    sync::{Arc, Mutex},
};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use nokhwa::{
    pixel_format::RgbAFormat,
    query,
    utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};

use hbb_common::message_proto::{DisplayInfo, Resolution};

#[cfg(feature = "vram")]
use crate::AdapterDevice;

use crate::common::{bail, ResultType};
use crate::{Frame, PixelBuffer, Pixfmt, TraitCapturer};

pub const PRIMARY_CAMERA_IDX: usize = 0;
lazy_static::lazy_static! {
    static ref SYNC_CAMERA_DISPLAYS: Arc<Mutex<Vec<DisplayInfo>>> = Arc::new(Mutex::new(Vec::new()));
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
const CAMERA_NOT_SUPPORTED: &str = "This platform doesn't support camera yet";

pub struct Cameras;

// pre-condition
pub fn primary_camera_exists() -> bool {
    Cameras::exists(PRIMARY_CAMERA_IDX)
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl Cameras {
    pub fn all_info() -> ResultType<Vec<DisplayInfo>> {
        match query(ApiBackend::Auto) {
            Ok(cameras) => {
                let mut camera_displays = SYNC_CAMERA_DISPLAYS.lock().unwrap();
                camera_displays.clear();
                // FIXME: nokhwa returns duplicate info for one physical camera on linux for now.
                // issue: https://github.com/l1npengtul/nokhwa/issues/171
                // Use only one camera as a temporary hack.
                cfg_if::cfg_if! {
                    if #[cfg(target_os = "linux")] {
                        let Some(info) = cameras.first() else {
                            bail!("No camera found")
                        };
                        let camera = Self::create_camera(info.index())?;
                        let resolution = camera.resolution();
                        let (width, height) = (resolution.width() as i32, resolution.height() as i32);
                        camera_displays.push(DisplayInfo {
                            x: 0,
                            y: 0,
                            name: info.human_name().clone(),
                            width,
                            height,
                            online: true,
                            cursor_embedded: false,
                            scale:1.0,
                            original_resolution: Some(Resolution {
                                width,
                                height,
                                ..Default::default()
                            }).into(),
                            ..Default::default()
                        });
                    } else {
                        let mut x = 0;
                        for info in &cameras {
                            let camera = Self::create_camera(info.index())?;
                            let resolution = camera.resolution();
                            let (width, height) = (resolution.width() as i32, resolution.height() as i32);
                            camera_displays.push(DisplayInfo {
                                x,
                                y: 0,
                                name: info.human_name().clone(),
                                width,
                                height,
                                online: true,
                                cursor_embedded: false,
                                scale:1.0,
                                original_resolution: Some(Resolution {
                                    width,
                                    height,
                                    ..Default::default()
                                }).into(),
                                ..Default::default()
                            });
                            x += width;
                        }
                    }
                }
                Ok(camera_displays.clone())
            }
            Err(e) => {
                bail!("Query cameras error: {}", e)
            }
        }
    }

    pub fn exists(index: usize) -> bool {
        match query(ApiBackend::Auto) {
            Ok(cameras) => index < cameras.len(),
            _ => return false,
        }
    }

    fn create_camera(index: &CameraIndex) -> ResultType<Camera> {
        let result = Camera::new(
            index.clone(),
            RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestResolution),
        );
        match result {
            Ok(camera) => Ok(camera),
            Err(e) => bail!("create camera{} error:  {}", index, e),
        }
    }

    pub fn get_camera_resolution(index: usize) -> ResultType<Resolution> {
        let index = CameraIndex::Index(index as u32);
        let camera = Self::create_camera(&index)?;
        let resolution = camera.resolution();
        Ok(Resolution {
            width: resolution.width() as i32,
            height: resolution.height() as i32,
            ..Default::default()
        })
    }

    pub fn get_sync_cameras() -> Vec<DisplayInfo> {
        SYNC_CAMERA_DISPLAYS.lock().unwrap().clone()
    }

    pub fn get_capturer(current: usize) -> ResultType<Box<dyn TraitCapturer>> {
        Ok(Box::new(CameraCapturer::new(current)?))
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
impl Cameras {
    pub fn all_info() -> ResultType<Vec<DisplayInfo>> {
        return Ok(Vec::new());
    }

    pub fn exists(index: usize) -> bool {
        false
    }

    pub fn get_camera_resolution(index: usize) -> ResultType<Resolution> {
        bail!(CAMERA_NOT_SUPPORTED);
    }

    pub fn get_sync_cameras() -> Vec<DisplayInfo> {
        vec![]
    }

    pub fn get_capturer(current: usize) -> ResultType<Box<dyn TraitCapturer>> {
        bail!(CAMERA_NOT_SUPPORTED);
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub struct CameraCapturer {
    camera: Camera,
    data: Vec<u8>,
    last_data: Vec<u8>, // for faster compare and copy
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub struct CameraCapturer;

impl CameraCapturer {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn new(current: usize) -> ResultType<Self> {
        let index = CameraIndex::Index(current as u32);
        let camera = Cameras::create_camera(&index)?;
        Ok(CameraCapturer {
            camera,
            data: Vec::new(),
            last_data: Vec::new(),
        })
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn new(_current: usize) -> ResultType<Self> {
        bail!(CAMERA_NOT_SUPPORTED);
    }
}

impl TraitCapturer for CameraCapturer {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn frame<'a>(&'a mut self, _timeout: std::time::Duration) -> std::io::Result<Frame<'a>> {
        // TODO: move this check outside `frame`.
        if !self.camera.is_stream_open() {
            if let Err(e) = self.camera.open_stream() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Camera open stream error: {}", e),
                ));
            }
        }
        match self.camera.frame() {
            Ok(buffer) => {
                match buffer.decode_image::<RgbAFormat>() {
                    Ok(decoded) => {
                        self.data = decoded.as_raw().to_vec();
                        crate::would_block_if_equal(&mut self.last_data, &self.data)?;
                        // FIXME: macos's PixelBuffer cannot be directly created from bytes slice.
                        cfg_if::cfg_if! {
                            if #[cfg(any(target_os = "linux", target_os = "windows"))] {
                                Ok(Frame::PixelBuffer(PixelBuffer::new(
                                    &self.data,
                                    Pixfmt::RGBA,
                                    decoded.width() as usize,
                                    decoded.height() as usize,
                                )))
                            } else {
                                Err(io::Error::new(
                                    io::ErrorKind::Other,
                                    format!("Camera is not supported on this platform yet"),
                                ))
                            }
                        }
                    }
                    Err(e) => Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Camera frame decode error: {}", e),
                    )),
                }
            }
            Err(e) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Camera frame error: {}", e),
            )),
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    fn frame<'a>(&'a mut self, _timeout: std::time::Duration) -> std::io::Result<Frame<'a>> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            CAMERA_NOT_SUPPORTED.to_string(),
        ))
    }

    #[cfg(windows)]
    fn is_gdi(&self) -> bool {
        false
    }

    #[cfg(windows)]
    fn set_gdi(&mut self) -> bool {
        false
    }

    #[cfg(feature = "vram")]
    fn device(&self) -> AdapterDevice {
        AdapterDevice::default()
    }

    #[cfg(feature = "vram")]
    fn set_output_texture(&mut self, _texture: bool) {}
}
