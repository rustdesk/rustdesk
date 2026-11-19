/*
 * Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use nokhwa_core::types::RequestedFormatType;
use nokhwa_core::{
    buffer::Buffer,
    error::NokhwaError,
    traits::CaptureBackendTrait,
    types::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueDescription,
        ControlValueSetter, FrameFormat, KnownCameraControl, RequestedFormat, Resolution,
    },
};
use opencv::{
    core::{Mat, MatTraitConst, MatTraitConstManual, Vec3b},
    videoio::{
        VideoCapture, VideoCaptureProperties, VideoCaptureTrait, VideoCaptureTraitConst, CAP_ANY,
        CAP_AVFOUNDATION, CAP_MSMF, CAP_PROP_FPS, CAP_PROP_FRAME_HEIGHT, CAP_PROP_FRAME_WIDTH,
        CAP_V4L2,
    },
};
use std::{borrow::Cow, collections::HashMap};

/// Attempts to convert a [`KnownCameraControl`] into a `OpenCV` video capture property.
/// If the associated control is not found, this will return `Err`
pub fn known_camera_control_to_video_capture_property(
    ctrl: KnownCameraControl,
) -> Result<VideoCaptureProperties, NokhwaError> {
    match ctrl {
        KnownCameraControl::Brightness => Ok(VideoCaptureProperties::CAP_PROP_BRIGHTNESS),
        KnownCameraControl::Contrast => Ok(VideoCaptureProperties::CAP_PROP_CONTRAST),
        KnownCameraControl::Hue => Ok(VideoCaptureProperties::CAP_PROP_HUE),
        KnownCameraControl::Saturation => Ok(VideoCaptureProperties::CAP_PROP_SATURATION),
        KnownCameraControl::Sharpness => Ok(VideoCaptureProperties::CAP_PROP_SHARPNESS),
        KnownCameraControl::Gamma => Ok(VideoCaptureProperties::CAP_PROP_GAMMA),
        KnownCameraControl::BacklightComp => Ok(VideoCaptureProperties::CAP_PROP_BACKLIGHT),
        KnownCameraControl::Gain => Ok(VideoCaptureProperties::CAP_PROP_GAIN),
        KnownCameraControl::Pan => Ok(VideoCaptureProperties::CAP_PROP_PAN),
        KnownCameraControl::Tilt => Ok(VideoCaptureProperties::CAP_PROP_TILT),
        KnownCameraControl::Zoom => Ok(VideoCaptureProperties::CAP_PROP_ZOOM),
        KnownCameraControl::Exposure => Ok(VideoCaptureProperties::CAP_PROP_EXPOSURE),
        KnownCameraControl::Iris => Ok(VideoCaptureProperties::CAP_PROP_IRIS),
        KnownCameraControl::Focus => Ok(VideoCaptureProperties::CAP_PROP_FOCUS),
        _ => Err(NokhwaError::UnsupportedOperationError(ApiBackend::OpenCv)),
    }
}

/// The backend struct that interfaces with `OpenCV`. Note that an `opencv` matching the version that this was either compiled on must be present on the user's machine. (usually 4.5.2 or greater)
/// For more information, please see [`opencv-rust`](https://github.com/twistedfall/opencv-rust) and [`OpenCV VideoCapture Docs`](https://docs.opencv.org/4.5.2/d8/dfe/classcv_1_1VideoCapture.html).
///
/// To see what this does, please see [`CaptureBackendTrait`]
/// # Quirks
///  - **Some features don't work properly on this backend (yet)! Setting [`Resolution`], FPS, [`FrameFormat`] does not work and will default to 640x480 30FPS. This is being worked on.**
///  - This is a **cross-platform** backend. This means that it will work on most platforms given that `OpenCV` is present.
///  - This backend can also do IP Camera input.
///  - The backend's backend will default to system level APIs on Linux(V4L2), Mac(AVFoundation), and Windows(Media Foundation). Otherwise, it will decide for itself.
///  - If the [`OpenCvCaptureDevice`] is initialized as a `IPCamera`, the [`CameraFormat`]'s `index` value will be [`u32::MAX`](std::u32::MAX) (4294967295).
///  - `OpenCV` does not support camera querying. Camera Name and Camera supported resolution/fps/fourcc is a [`UnsupportedOperationError`](NokhwaError::UnsupportedOperationError).
/// Note: [`resolution()`](crate::camera_traits::CaptureBackendTrait::resolution()), [`frame_format()`](crate::camera_traits::CaptureBackendTrait::frame_format()), and [`frame_rate()`](crate::camera_traits::CaptureBackendTrait::frame_rate()) is not affected.
///  - [`CameraInfo`]'s human name will be "`OpenCV` Capture Device {location}"
///  - [`CameraInfo`]'s description will contain the Camera's Index or IP.
///  - The API Preference order is the native OS API (linux => `v4l2`, mac => `AVFoundation`, windows => `MSMF`) than [`CAP_AUTO`](https://docs.opencv.org/4.5.2/d4/d15/group__videoio__flags__base.html#gga023786be1ee68a9105bf2e48c700294da77ab1fe260fd182f8ec7655fab27a31d)
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-opencv")))]
pub struct OpenCvCaptureDevice {
    camera_format: CameraFormat,
    camera_location: CameraIndex,
    camera_info: CameraInfo,
    api_preference: i32,
    video_capture: VideoCapture,
}

#[allow(clippy::must_use_candidate)]
impl OpenCvCaptureDevice {
    /// Creates a new capture device using the `OpenCV` backend.
    ///
    /// Indexes are gives to devices by the OS, and usually numbered by order of discovery.
    ///
    /// `IPCameras` follow the format
    /// ```.ignore
    /// <protocol>://<IP>:<port>/
    /// ```
    /// , but please refer to the manufacturer for the actual IP format.
    ///
    /// # Errors
    /// If the backend fails to open the camera (e.g. Device does not exist at specified index/ip), Camera does not support specified [`CameraFormat`], and/or other `OpenCV` Error, this will error.
    /// # Panics
    /// If the API u32 -> i32 fails this will error
    #[allow(clippy::cast_possible_wrap)]
    pub fn new(index: &CameraIndex, cam_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
        let api_pref = if index.is_string() {
            CAP_ANY
        } else {
            get_api_pref_int()
        };

        let mut video_capture = match &index {
            CameraIndex::Index(idx) => VideoCapture::new(*idx as i32, api_pref),
            CameraIndex::String(ip) => VideoCapture::from_file(ip.as_str(), api_pref),
        }
        .map_err(|why| {
            NokhwaError::OpenDeviceError(format!("Failed to open {index}"), why.to_string())
        })?;

        let camera_format =
            if let RequestedFormatType::Exact(exact) = cam_fmt.requested_format_type() {
                exact
            } else {
                return Err(NokhwaError::UnsupportedOperationError(ApiBackend::OpenCv));
            };

        set_properties(&mut video_capture, camera_format)?;

        let camera_info = CameraInfo::new(
            format!("OpenCV Capture Device {index}").as_str(),
            index.to_string().as_str(),
            "",
            index.clone(),
        );

        Ok(OpenCvCaptureDevice {
            camera_format,
            camera_location: index.clone(),
            camera_info,
            api_preference: api_pref,
            video_capture,
        })
    }

    /// Gets weather said capture device is an `IPCamera`.
    pub fn is_ip_camera(&self) -> bool {
        match self.camera_location {
            CameraIndex::Index(_) => false,
            CameraIndex::String(_) => true,
        }
    }

    /// Gets weather said capture device is an OS-based indexed camera.
    pub fn is_index_camera(&self) -> bool {
        match self.camera_location {
            CameraIndex::Index(_) => true,
            CameraIndex::String(_) => false,
        }
    }

    /// Gets the camera location
    pub fn camera_location(&self) -> &CameraIndex {
        &self.camera_location
    }

    /// Gets the `OpenCV` API Preference number. Please refer to [`OpenCV VideoCapture Flag Docs`](https://docs.opencv.org/4.5.2/d4/d15/group__videoio__flags__base.html).
    pub fn opencv_preference(&self) -> i32 {
        self.api_preference
    }

    /// Gets the RGB24 frame directly read from `OpenCV` without any additional processing.
    /// # Errors
    /// If the frame is failed to be read, this will error.
    #[allow(clippy::cast_sign_loss)]
    pub fn raw_frame_vec(&mut self) -> Result<Cow<[u8]>, NokhwaError> {
        if !self.is_stream_open() {
            return Err(NokhwaError::ReadFrameError(
                "Stream is not open!".to_string(),
            ));
        }

        let mut frame = Mat::default();
        match self.video_capture.read(&mut frame) {
            Ok(a) => {
                if !a {
                    return Err(NokhwaError::ReadFrameError(
                        "Failed to read frame from videocapture: OpenCV return false, camera disconnected?".to_string(),
                    ));
                }
            }
            Err(why) => {
                return Err(NokhwaError::ReadFrameError(format!(
                    "Failed to read frame from videocapture: {}",
                    why
                )))
            }
        }

        if frame.empty() {
            return Err(NokhwaError::ReadFrameError("Frame Empty!".to_string()));
        }

        match frame.size() {
            Ok(size) => {
                if size.width > 0 {
                    return if frame.is_continuous() {
                        let mut raw_vec: Vec<u8> = Vec::new();

                        let frame_data_vec = match Mat::data_typed::<Vec3b>(&frame) {
                            Ok(v) => v,
                            Err(why) => {
                                return Err(NokhwaError::ReadFrameError(format!(
                                    "Failed to convert frame into raw Vec3b: {}",
                                    why
                                )))
                            }
                        };

                        for pixel in frame_data_vec.iter() {
                            let pixel_slice: &[u8; 3] = pixel;
                            raw_vec.push(pixel_slice[2]);
                            raw_vec.push(pixel_slice[1]);
                            raw_vec.push(pixel_slice[0]);
                        }

                        Ok(Cow::from(raw_vec))
                    } else {
                        Err(NokhwaError::ReadFrameError(
                            "Failed to read frame from videocapture: not cont".to_string(),
                        ))
                    };
                }
                Err(NokhwaError::ReadFrameError(
                    "Frame width is less than zero!".to_string(),
                ))
            }
            Err(why) => Err(NokhwaError::ReadFrameError(format!(
                "Failed to read frame from videocapture: failed to read size: {}",
                why
            ))),
        }
    }

    /// Gets the resolution raw as read by `OpenCV`.
    /// # Errors
    /// If the resolution is failed to be read (e.g. invalid or not supported), this will error.
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    pub fn raw_resolution(&self) -> Result<Resolution, NokhwaError> {
        let width = match self.video_capture.get(CAP_PROP_FRAME_WIDTH) {
            Ok(width) => width as u32,
            Err(why) => {
                return Err(NokhwaError::GetPropertyError {
                    property: "Width".to_string(),
                    error: why.to_string(),
                })
            }
        };

        let height = match self.video_capture.get(CAP_PROP_FRAME_HEIGHT) {
            Ok(height) => height as u32,
            Err(why) => {
                return Err(NokhwaError::GetPropertyError {
                    property: "Height".to_string(),
                    error: why.to_string(),
                })
            }
        };

        Ok(Resolution::new(width, height))
    }

    /// Gets the framerate raw as read by `OpenCV`.
    /// # Errors
    /// If the framerate is failed to be read (e.g. invalid or not supported), this will error.
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    pub fn raw_framerate(&self) -> Result<u32, NokhwaError> {
        match self.video_capture.get(CAP_PROP_FPS) {
            Ok(fps) => Ok(fps as u32),
            Err(why) => Err(NokhwaError::GetPropertyError {
                property: "Framerate".to_string(),
                error: why.to_string(),
            }),
        }
    }
}

impl CaptureBackendTrait for OpenCvCaptureDevice {
    fn backend(&self) -> ApiBackend {
        ApiBackend::OpenCv
    }

    fn camera_info(&self) -> &CameraInfo {
        &self.camera_info
    }

    #[allow(clippy::cast_lossless)]
    fn refresh_camera_format(&mut self) -> Result<(), NokhwaError> {
        let width = u32::from(
            self.video_capture
                .set(CAP_PROP_FRAME_WIDTH, self.camera_format.width() as f64)
                .map_err(|why| NokhwaError::SetPropertyError {
                    property: "Resolution Width".to_string(),
                    value: self.camera_format.to_string(),
                    error: why.to_string(),
                })?,
        );
        let height = self
            .video_capture
            .set(CAP_PROP_FRAME_HEIGHT, self.camera_format.height() as f64)
            .map_err(|why| NokhwaError::SetPropertyError {
                property: "Resolution Height".to_string(),
                value: self.camera_format.to_string(),
                error: why.to_string(),
            })? as u32;
        let fps = self
            .video_capture
            .set(CAP_PROP_FPS, self.camera_format.frame_rate() as f64)
            .map_err(|why| NokhwaError::SetPropertyError {
                property: "FPS".to_string(),
                value: self.camera_format.to_string(),
                error: why.to_string(),
            })? as u32;

        let ffmt = self.frame_format();
        self.set_camera_format(CameraFormat::new_from(width, height, ffmt, fps))?;

        Ok(())
    }

    fn camera_format(&self) -> CameraFormat {
        self.camera_format
    }

    fn set_camera_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
        let current_format = self.camera_format;
        let is_opened = match self.video_capture.is_opened() {
            Ok(opened) => opened,
            Err(why) => {
                return Err(NokhwaError::GetPropertyError {
                    property: "Is Stream Open".to_string(),
                    error: why.to_string(),
                })
            }
        };

        self.camera_format = new_fmt;

        if let Err(why) = set_properties(&mut self.video_capture, new_fmt) {
            self.camera_format = current_format;
            return Err(why);
        }
        if is_opened {
            self.stop_stream()?;
            if let Err(why) = self.open_stream() {
                return Err(NokhwaError::OpenDeviceError(
                    self.camera_location.to_string(),
                    why.to_string(),
                ));
            }
        }
        Ok(())
    }

    fn compatible_list_by_resolution(
        &mut self,
        _fourcc: FrameFormat,
    ) -> Result<HashMap<Resolution, Vec<u32>>, NokhwaError> {
        Err(NokhwaError::UnsupportedOperationError(ApiBackend::OpenCv))
    }

    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        Err(NokhwaError::UnsupportedOperationError(ApiBackend::OpenCv))
    }

    fn resolution(&self) -> Resolution {
        self.raw_resolution()
            .unwrap_or_else(|_| Resolution::new(640, 480))
    }

    fn set_resolution(&mut self, new_res: Resolution) -> Result<(), NokhwaError> {
        let mut current_fmt = self.camera_format;
        current_fmt.set_resolution(new_res);
        self.set_camera_format(current_fmt)
    }

    fn frame_rate(&self) -> u32 {
        self.raw_framerate().unwrap_or(30)
    }

    fn set_frame_rate(&mut self, new_fps: u32) -> Result<(), NokhwaError> {
        let mut current_fmt = self.camera_format;
        current_fmt.set_frame_rate(new_fps);
        self.set_camera_format(current_fmt)
    }

    fn frame_format(&self) -> FrameFormat {
        self.camera_format.format()
    }

    fn set_frame_format(&mut self, fourcc: FrameFormat) -> Result<(), NokhwaError> {
        let mut current_fmt = self.camera_format;
        current_fmt.set_format(fourcc);
        self.set_camera_format(current_fmt)
    }

    fn camera_control(&self, control: KnownCameraControl) -> Result<CameraControl, NokhwaError> {
        let id = known_camera_control_to_video_capture_property(control)? as i32;
        let current = self
            .video_capture
            .get(id)
            .map_err(|why| NokhwaError::GetPropertyError {
                property: id.to_string(),
                error: why.to_string(),
            })?;
        Ok(CameraControl::new(
            control,
            id.to_string(),
            ControlValueDescription::Float {
                value: current,
                default: 0.0,
                step: 0.0,
            },
            vec![],
            true,
        ))
    }

    fn camera_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        Err(NokhwaError::UnsupportedOperationError(ApiBackend::OpenCv))
    }

    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_lossless)]
    fn set_camera_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        let control_val = match value {
            ControlValueSetter::Integer(i) => i as f64,
            ControlValueSetter::Float(f) => f,
            ControlValueSetter::Boolean(b) => u8::from(b) as f64,
            val => {
                return Err(NokhwaError::SetPropertyError {
                    property: "Camera Control".to_string(),
                    value: val.to_string(),
                    error: "unsupported value".to_string(),
                })
            }
        };

        if !self
            .video_capture
            .set(
                known_camera_control_to_video_capture_property(id)? as i32,
                control_val,
            )
            .map_err(|why| NokhwaError::SetPropertyError {
                property: "Camera Control".to_string(),
                value: control_val.to_string(),
                error: why.to_string(),
            })?
        {
            return Err(NokhwaError::SetPropertyError {
                property: "Camera Control".to_string(),
                value: control_val.to_string(),
                error: "false".to_string(),
            });
        }

        let set_value = self.camera_control(id)?.value();
        if set_value != value {
            return Err(NokhwaError::SetPropertyError {
                property: "Camera Control".to_string(),
                value: control_val.to_string(),
                error: "failed to set value: rejected".to_string(),
            });
        }

        Ok(())
    }

    #[allow(clippy::cast_possible_wrap)]
    fn open_stream(&mut self) -> Result<(), NokhwaError> {
        match self.camera_location.clone() {
            CameraIndex::Index(idx) => {
                match self.video_capture.open(idx as i32, get_api_pref_int()) {
                    Ok(open) => {
                        if open {
                            return Ok(());
                        }
                        Err(NokhwaError::OpenStreamError(
                            "Stream is not opened after stream open attempt opencv".to_string(),
                        ))
                    }
                    Err(why) => Err(NokhwaError::OpenDeviceError(
                        idx.to_string(),
                        format!("Failed to open device: {why}"),
                    )),
                }
            }
            CameraIndex::String(_) => Err(NokhwaError::OpenDeviceError(
                "Cannot open".to_string(),
                "String index not supported (try NetworkCamera instead)".to_string(),
            )),
        }?;

        match self.video_capture.is_opened() {
            Ok(open) => {
                if open {
                    return Ok(());
                }
                Err(NokhwaError::OpenStreamError(
                    "Stream is not opened after stream open attempt opencv".to_string(),
                ))
            }
            Err(why) => Err(NokhwaError::GetPropertyError {
                property: "Is Stream Open After Open Stream".to_string(),
                error: why.to_string(),
            }),
        }
    }

    fn is_stream_open(&self) -> bool {
        self.video_capture.is_opened().unwrap_or(false)
    }

    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        let camera_resolution = self.camera_format.resolution();
        let image_data = {
            let mut data = self.frame_raw()?.to_vec();
            data.resize(
                (camera_resolution.width() * camera_resolution.height() * 3) as usize,
                0_u8,
            );
            data
        };
        Ok(Buffer::new(
            camera_resolution,
            &image_data,
            self.camera_format.format(),
        ))
    }

    fn frame_raw(&mut self) -> Result<Cow<[u8]>, NokhwaError> {
        let cow = self.raw_frame_vec()?;
        Ok(cow)
    }

    fn stop_stream(&mut self) -> Result<(), NokhwaError> {
        match self.video_capture.release() {
            Ok(_) => Ok(()),
            Err(why) => Err(NokhwaError::StreamShutdownError(why.to_string())),
        }
    }
}

fn get_api_pref_int() -> i32 {
    match std::env::consts::OS {
        "linux" => CAP_V4L2,
        "windows" => CAP_MSMF,
        "mac" => CAP_AVFOUNDATION,
        &_ => CAP_ANY,
    }
}

// I'm done. This stupid POS refuses to actually do anything useful with camera settings
// If anyone else wants to tackle this monster, please do.
fn set_properties(vc: &mut VideoCapture, camera_format: CameraFormat) -> Result<(), NokhwaError> {
    if !vc
        .set(CAP_PROP_FRAME_WIDTH, f64::from(camera_format.width()))
        .map_err(|why| NokhwaError::SetPropertyError {
            property: "Resolution Width".to_string(),
            value: camera_format.to_string(),
            error: why.to_string(),
        })?
    {
        return Err(NokhwaError::SetPropertyError {
            property: "Resolution Width".to_string(),
            value: camera_format.to_string(),
            error: "false".to_string(),
        });
    }
    if !vc
        .set(CAP_PROP_FRAME_HEIGHT, f64::from(camera_format.height()))
        .map_err(|why| NokhwaError::SetPropertyError {
            property: "Resolution Height".to_string(),
            value: camera_format.to_string(),
            error: why.to_string(),
        })?
    {
        return Err(NokhwaError::SetPropertyError {
            property: "Resolution Height".to_string(),
            value: camera_format.to_string(),
            error: "false".to_string(),
        });
    }
    if !vc
        .set(CAP_PROP_FPS, f64::from(camera_format.frame_rate()))
        .map_err(|why| NokhwaError::SetPropertyError {
            property: "FPS".to_string(),
            value: camera_format.to_string(),
            error: why.to_string(),
        })?
    {
        return Err(NokhwaError::SetPropertyError {
            property: "FPS".to_string(),
            value: camera_format.to_string(),
            error: "false".to_string(),
        });
    }
    Ok(())
}
