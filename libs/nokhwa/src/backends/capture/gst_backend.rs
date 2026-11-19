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

use crate::{
    mjpeg_to_rgb, yuyv422_to_rgb, ApiBackend, CameraControl, CameraFormat, CameraInfo,
    CaptureBackendTrait, FrameFormat, KnownCameraControl, NokhwaError, Resolution,
};
use glib::Quark;
use gstreamer::{
    element_error,
    glib::Cast,
    prelude::{DeviceExt, DeviceMonitorExt, DeviceMonitorExtManual, ElementExt, GstBinExt},
    Bin, Caps, ClockTime, DeviceMonitor, Element, FlowError, FlowSuccess, MessageView,
    ResourceError, State,
};
use gstreamer_app::{AppSink, AppSinkCallbacks};
use gstreamer_video::{VideoFormat, VideoInfo};
use image::{ImageBuffer, Rgb};
use parking_lot::Mutex;
use regex::Regex;
use std::{any::Any, borrow::Cow, collections::HashMap, str::FromStr, sync::Arc};

type PipelineGenRet = (Element, AppSink, Arc<Mutex<ImageBuffer<Rgb<u8>, Vec<u8>>>>);

/// The backend struct that interfaces with `GStreamer`.
/// To see what this does, please see [`CaptureBackendTrait`].
/// # Quirks
/// - `Drop`-ing this may cause a `panic`.
/// - Setting controls is not supported.
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-gst")))]
#[deprecated(
    since = "0.10",
    note = "Use one of the native backends instead(V4L, AVF, MSMF) or OpenCV"
)]
pub struct GStreamerCaptureDevice {
    pipeline: Element,
    app_sink: AppSink,
    camera_format: CameraFormat,
    camera_info: CameraInfo,
    image_lock: Arc<Mutex<ImageBuffer<Rgb<u8>, Vec<u8>>>>,
    caps: Option<Caps>,
}

impl GStreamerCaptureDevice {
    /// Creates a new capture device using the `GStreamer` backend. Indexes are gives to devices by the OS, and usually numbered by order of discovery.
    ///
    /// `GStreamer` uses `v4l2src` on linux, `ksvideosrc` on windows, and `autovideosrc` on mac.
    ///
    /// If `camera_format` is `None`, it will be spawned with with 640x480@15 FPS, MJPEG [`CameraFormat`] default.
    /// # Errors
    /// This function will error if the camera is currently busy or if `GStreamer` can't read device information.
    pub fn new(index: usize, cam_fmt: Option<CameraFormat>) -> Result<Self, NokhwaError> {
        let camera_format = match cam_fmt {
            Some(fmt) => fmt,
            None => CameraFormat::default(),
        };

        let index = index.as_index()?;

        if let Err(why) = gstreamer::init() {
            return Err(NokhwaError::InitializeError {
                backend: ApiBackend::GStreamer,
                error: why.to_string(),
            });
        }

        let (camera_info, caps) = {
            let device_monitor = DeviceMonitor::new();
            let video_caps = match Caps::from_str("video/x-raw") {
                Ok(cap) => cap,
                Err(why) => {
                    return Err(NokhwaError::GeneralError(format!(
                        "Failed to generate caps: {}",
                        why
                    )))
                }
            };
            let _video_filter_id =
                match device_monitor.add_filter(Some("Video/Source"), Some(&video_caps)) {
                    Some(id) => id,
                    None => {
                        return Err(NokhwaError::StructureError {
                            structure: "Video Filter ID Video/Source".to_string(),
                            error: "Null".to_string(),
                        })
                    }
                };
            if let Err(why) = device_monitor.start() {
                return Err(NokhwaError::StructureError {
                    structure: "Device Monitor".to_string(),
                    error: format!("Not started, {}", why),
                });
            }
            let device = match device_monitor.devices().get(index as usize) {
                Some(dev) => dev.clone(),
                None => {
                    return Err(NokhwaError::OpenDeviceError(
                        index.to_string(),
                        "No device".to_string(),
                    ))
                }
            };
            device_monitor.stop();
            let caps = device.caps();
            (
                CameraInfo::new(
                    &DeviceExt::display_name(&device),
                    &DeviceExt::device_class(&device),
                    &"",
                    index,
                ),
                caps,
            )
        };

        let (pipeline, app_sink, receiver) = generate_pipeline(camera_format, index as usize)?;

        Ok(GStreamerCaptureDevice {
            pipeline,
            app_sink,
            camera_format,
            camera_info,
            image_lock: receiver,
            caps,
        })
    }

    /// Creates a new capture device using the `GStreamer` backend. Indexes are gives to devices by the OS, and usually numbered by order of discovery.
    ///
    /// `GStreamer` uses `v4l2src` on linux, `ksvideosrc` on windows, and `autovideosrc` on mac.
    /// # Errors
    /// This function will error if the camera is currently busy or if `GStreamer` can't read device information.
    pub fn new_with(index: usize, width: u32, height: u32, fps: u32) -> Result<Self, NokhwaError> {
        let cam_fmt = CameraFormat::new(Resolution::new(width, height));
        GStreamerCaptureDevice::new(index, Some(cam_fmt))
    }
}

impl GStreamerCaptureDevice {
    fn backend(&self) -> ApiBackend {
        ApiBackend::GStreamer
    }

    fn camera_info(&self) -> &CameraInfo {
        &self.camera_info
    }

    fn camera_format(&self) -> CameraFormat {
        self.camera_format
    }

    fn set_camera_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
        let mut reopen = false;
        if self.is_stream_open() {
            self.stop_stream()?;
            reopen = true;
        }
        let (pipeline, app_sink, receiver) =
            generate_pipeline(new_fmt, self.camera_info.index_num()? as usize)?;
        self.pipeline = pipeline;
        self.app_sink = app_sink;
        self.image_lock = receiver;
        if reopen {
            self.open_stream()?;
        }
        self.camera_format = new_fmt;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_sign_loss)]
    fn compatible_list_by_resolution(
        &mut self,
        fourcc: FrameFormat,
    ) -> Result<HashMap<Resolution, Vec<u32>>, NokhwaError> {
        let mut resolution_map = HashMap::new();

        let frame_regex = Regex::new(r"(\d+/1)|((\d+/\d)+(\d/1)*)").unwrap();

        match self.caps.clone() {
            Some(c) => {
                for capability in c.iter() {
                    match fourcc {
                        FrameFormat::MJPEG => {
                            if capability.name() == "image/jpeg" {
                                let mut fps_vec = vec![];

                                let width = match capability.get::<i32>("width") {
                                    Ok(w) => w,
                                    Err(why) => {
                                        return Err(NokhwaError::GetPropertyError {
                                            property: "Capibilities by Resolution: Width"
                                                .to_string(),
                                            error: why.to_string(),
                                        })
                                    }
                                };
                                let height = match capability.get::<i32>("height") {
                                    Ok(w) => w,
                                    Err(why) => {
                                        return Err(NokhwaError::GetPropertyError {
                                            property: "Capibilities by Resolution: Height"
                                                .to_string(),
                                            error: why.to_string(),
                                        })
                                    }
                                };
                                let value = match capability
                                    .value_by_quark(Quark::from_string("framerate"))
                                {
                                    Ok(v) => match v.transform::<String>() {
                                        Ok(s) => {
                                            format!("{:?}", s)
                                        }
                                        Err(why) => {
                                            return Err(NokhwaError::GetPropertyError {
                                                property: "Framerates".to_string(),
                                                error: format!(
                                                    "Failed to make framerates into string: {}",
                                                    why
                                                ),
                                            });
                                        }
                                    },
                                    Err(_) => {
                                        return Err(NokhwaError::GetPropertyError {
                                            property: "Framerates".to_string(),
                                            error: "Failed to get framerates: doesnt exist!"
                                                .to_string(),
                                        })
                                    }
                                };

                                for m in frame_regex.find_iter(&value) {
                                    let fraction_string: Vec<&str> =
                                        m.as_str().split('/').collect();
                                    if fraction_string.len() != 2 {
                                        return Err(NokhwaError::GetPropertyError { property: "Framerates".to_string(), error: format!("Fraction framerate had more than one demoninator: {:?}", fraction_string) });
                                    }

                                    if let Some(v) = fraction_string.get(1) {
                                        if *v != "1" {
                                            continue; // swallow error
                                        }
                                    } else {
                                        return Err(NokhwaError::GetPropertyError { property: "Framerates".to_string(), error: "No framerate denominator? Shouldn't happen, please report!".to_string() });
                                    }

                                    if let Some(numerator) = fraction_string.get(0) {
                                        match numerator.parse::<u32>() {
                                            Ok(fps) => fps_vec.push(fps),
                                            Err(why) => {
                                                return Err(NokhwaError::GetPropertyError {
                                                    property: "Framerates".to_string(),
                                                    error: format!(
                                                        "Failed to parse numerator: {}",
                                                        why
                                                    ),
                                                });
                                            }
                                        }
                                    } else {
                                        return Err(NokhwaError::GetPropertyError { property: "Framerates".to_string(), error: "No framerate numerator? Shouldn't happen, please report!".to_string() });
                                    }
                                }
                                resolution_map
                                    .insert(Resolution::new(width as u32, height as u32), fps_vec);
                            }
                        }
                        FrameFormat::YUYV => {
                            if capability.name() == "video/x-raw"
                                && capability.get::<String>("format").unwrap_or_default() == *"YUY2"
                            {
                                let mut fps_vec = vec![];

                                let width = match capability.get::<i32>("width") {
                                    Ok(w) => w,
                                    Err(why) => {
                                        return Err(NokhwaError::GetPropertyError {
                                            property: "Capibilities by Resolution: Width"
                                                .to_string(),
                                            error: why.to_string(),
                                        })
                                    }
                                };
                                let height = match capability.get::<i32>("height") {
                                    Ok(w) => w,
                                    Err(why) => {
                                        return Err(NokhwaError::GetPropertyError {
                                            property: "Capibilities by Resolution: Height"
                                                .to_string(),
                                            error: why.to_string(),
                                        })
                                    }
                                };
                                let value = match capability
                                    .value_by_quark(Quark::from_string("framerate"))
                                {
                                    Ok(v) => match v.transform::<String>() {
                                        Ok(s) => {
                                            format!("{:?}", s)
                                        }
                                        Err(why) => {
                                            return Err(NokhwaError::GetPropertyError {
                                                property: "Framerates".to_string(),
                                                error: format!(
                                                    "Failed to make framerates into string: {}",
                                                    why
                                                ),
                                            });
                                        }
                                    },
                                    Err(_) => {
                                        return Err(NokhwaError::GetPropertyError {
                                            property: "Framerates".to_string(),
                                            error: "Failed to get framerates: doesnt exist!"
                                                .to_string(),
                                        })
                                    }
                                };

                                for m in frame_regex.find_iter(&value) {
                                    let fraction_string: Vec<&str> =
                                        m.as_str().split('/').collect();
                                    if fraction_string.len() != 2 {
                                        return Err(NokhwaError::GetPropertyError { property: "Framerates".to_string(), error: format!("Fraction framerate had more than one demoninator: {:?}", fraction_string) });
                                    }

                                    if let Some(v) = fraction_string.get(1) {
                                        if *v != "1" {
                                            continue; // swallow error
                                        }
                                    } else {
                                        return Err(NokhwaError::GetPropertyError { property: "Framerates".to_string(), error: "No framerate denominator? Shouldn't happen, please report!".to_string() });
                                    }

                                    if let Some(numerator) = fraction_string.get(0) {
                                        match numerator.parse::<u32>() {
                                            Ok(fps) => fps_vec.push(fps),
                                            Err(why) => {
                                                return Err(NokhwaError::GetPropertyError {
                                                    property: "Framerates".to_string(),
                                                    error: format!(
                                                        "Failed to parse numerator: {}",
                                                        why
                                                    ),
                                                });
                                            }
                                        }
                                    } else {
                                        return Err(NokhwaError::GetPropertyError { property: "Framerates".to_string(), error: "No framerate numerator? Shouldn't happen, please report!".to_string() });
                                    }
                                }
                                resolution_map
                                    .insert(Resolution::new(width as u32, height as u32), fps_vec);
                            }
                        }
                        unsupported => {
                            return Err(NokhwaError::NotImplementedError(format!(
                                "Not supported frame format {unsupported:?}"
                            )))
                        }
                    }
                }
            }
            None => {
                return Err(NokhwaError::GetPropertyError {
                    property: "Device Caps".to_string(),
                    error: "No device caps!".to_string(),
                })
            }
        }

        Ok(resolution_map)
    }

    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        let mut format_vec = vec![];
        match self.caps.clone() {
            Some(c) => {
                for capability in c.iter() {
                    if capability.name() == "image/jpeg" {
                        format_vec.push(FrameFormat::MJPEG);
                    } else if capability.name() == "video/x-raw"
                        && capability.get::<String>("format").unwrap_or_default() == *"YUY2"
                    {
                        format_vec.push(FrameFormat::YUYV);
                    }
                }
            }
            None => {
                return Err(NokhwaError::GetPropertyError {
                    property: "Device Caps".to_string(),
                    error: "No device caps!".to_string(),
                })
            }
        }
        format_vec.sort();
        format_vec.dedup();
        Ok(format_vec)
    }

    fn resolution(&self) -> Resolution {
        self.camera_format.resolution()
    }

    fn set_resolution(&mut self, new_res: Resolution) -> Result<(), NokhwaError> {
        let mut new_fmt = self.camera_format;
        new_fmt.set_resolution(new_res);
        self.set_camera_format(new_fmt)
    }

    fn frame_rate(&self) -> u32 {
        self.camera_format.frame_rate()
    }

    fn set_frame_rate(&mut self, new_fps: u32) -> Result<(), NokhwaError> {
        let mut new_fmt = self.camera_format;
        new_fmt.set_frame_rate(new_fps);
        self.set_camera_format(new_fmt)
    }

    fn frame_format(&self) -> FrameFormat {
        self.camera_format.format()
    }

    fn set_frame_format(&mut self, _fourcc: FrameFormat) -> Result<(), NokhwaError> {
        Err(NokhwaError::UnsupportedOperationError(
            ApiBackend::GStreamer,
        ))
    }

    fn supported_camera_controls(&self) -> Result<Vec<KnownCameraControl>, NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            ApiBackend::GStreamer.to_string(),
        ))
    }

    fn camera_control(&self, _control: KnownCameraControl) -> Result<CameraControl, NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            ApiBackend::GStreamer.to_string(),
        ))
    }

    fn set_camera_control(&mut self, _control: CameraControl) -> Result<(), NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            ApiBackend::GStreamer.to_string(),
        ))
    }

    fn raw_supported_camera_controls(&self) -> Result<Vec<Box<dyn Any>>, NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            ApiBackend::GStreamer.to_string(),
        ))
    }

    fn raw_camera_control(&self, _control: &dyn Any) -> Result<Box<dyn Any>, NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            ApiBackend::GStreamer.to_string(),
        ))
    }

    fn set_raw_camera_control(
        &mut self,
        _control: &dyn Any,
        _value: &dyn Any,
    ) -> Result<(), NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            ApiBackend::GStreamer.to_string(),
        ))
    }

    fn open_stream(&mut self) -> Result<(), NokhwaError> {
        if let Err(why) = self.pipeline.set_state(State::Playing) {
            return Err(NokhwaError::OpenStreamError(format!(
                "Failed to set appsink to playing: {}",
                why
            )));
        }
        Ok(())
    }

    // TODO: someone validate this
    fn is_stream_open(&self) -> bool {
        let (res, state_from, state_to) = self.pipeline.state(ClockTime::from_mseconds(16));
        if res.is_ok() {
            if state_to == State::Playing {
                return true;
            }
            false
        } else {
            if state_from == State::Playing {
                return true;
            }
            false
        }
    }

    fn frame(&mut self) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>, NokhwaError> {
        let cam_fmt = self.camera_format;
        let image_data = self.frame_raw()?;
        let imagebuf =
            match ImageBuffer::from_vec(cam_fmt.width(), cam_fmt.height(), image_data.to_vec()) {
                Some(buf) => {
                    let rgbbuf: ImageBuffer<Rgb<u8>, Vec<u8>> = buf;
                    rgbbuf
                }
                None => return Err(NokhwaError::ReadFrameError(
                    "Imagebuffer is not large enough! This is probably a bug, please report it!"
                        .to_string(),
                )),
            };
        Ok(imagebuf)
    }

    fn frame_raw(&mut self) -> Result<Cow<[u8]>, NokhwaError> {
        let bus = match self.pipeline.bus() {
            Some(bus) => bus,
            None => {
                return Err(NokhwaError::ReadFrameError(
                    "The pipeline has no bus!".to_string(),
                ))
            }
        };

        if let Some(message) = bus.timed_pop(ClockTime::from_seconds(0)) {
            match message.view() {
                MessageView::Eos(..) => {
                    return Err(NokhwaError::ReadFrameError("Stream is ended!".to_string()))
                }
                MessageView::Error(err) => {
                    return Err(NokhwaError::ReadFrameError(format!(
                        "Bus error: {}",
                        err.error()
                    )));
                }
                _ => {}
            }
        }

        Ok(Cow::from(self.image_lock.lock().to_vec()))
    }

    fn stop_stream(&mut self) -> Result<(), NokhwaError> {
        if let Err(why) = self.pipeline.set_state(State::Null) {
            return Err(NokhwaError::StreamShutdownError(format!(
                "Could not change state: {}",
                why
            )));
        }
        Ok(())
    }
}

impl Drop for GStreamerCaptureDevice {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(State::Null);
    }
}

#[cfg(target_os = "macos")]
fn webcam_pipeline(device: &str, camera_format: CameraFormat) -> String {
    match camera_format.format() {
        FrameFormat::MJPEG => {
            format!("autovideosrc location=/dev/video{} ! image/jpeg,width={},height={},framerate={}/1 ! appsink name=appsink async=false sync=false", device, camera_format.width(), camera_format.height(), camera_format.frame_rate())
        }
        FrameFormat::YUYV => {
            format!("autovideosrc location=/dev/video{} ! video/x-raw,format=YUY2,width={},height={},framerate={}/1 ! appsink name=appsink async=false sync=false", device, camera_format.width(), camera_format.height(), camera_format.frame_rate())
        }
        _ => {
            format!("unsupproted! if you see this, switch to something else!")
        }
    }
}

#[cfg(target_os = "linux")]
fn webcam_pipeline(device: &str, camera_format: CameraFormat) -> String {
    match camera_format.format() {
        FrameFormat::MJPEG => {
            format!("v4l2src device=/dev/video{} ! image/jpeg, width={},height={},framerate={}/1 ! appsink name=appsink async=false sync=false", device, camera_format.width(), camera_format.height(), camera_format.frame_rate())
        }
        FrameFormat::YUYV => {
            format!("v4l2src device=/dev/video{} ! video/x-raw,format=YUY2,width={},height={},framerate={}/1 ! appsink name=appsink async=false sync=false", device, camera_format.width(), camera_format.height(), camera_format.frame_rate())
        }
        _ => {
            format!("unsupproted! if you see this, switch to something else!")
        }
    }
}

#[cfg(target_os = "windows")]
fn webcam_pipeline(device: &str, camera_format: CameraFormat) -> String {
    match camera_format.format() {
        FrameFormat::MJPEG => {
            format!("ksvideosrc device_index={} ! image/jpeg, width={},height={},framerate={}/1 ! appsink name=appsink async=false sync=false", device, camera_format.width(), camera_format.height(), camera_format.frame_rate())
        }
        FrameFormat::YUYV => {
            format!("ksvideosrc device_index={} ! video/x-raw,format=YUY2,width={},height={},framerate={}/1 ! appsink name=appsink async=false sync=false", device, camera_format.width(), camera_format.height(), camera_format.frame_rate())
        }
        _ => {
            format!("unsupproted! if you see this, switch to something else!")
        }
    }
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::let_and_return)]
fn generate_pipeline(fmt: CameraFormat, index: usize) -> Result<PipelineGenRet, NokhwaError> {
    let pipeline =
        match gstreamer::parse_launch(webcam_pipeline(format!("{}", index).as_str(), fmt).as_str())
        {
            Ok(p) => p,
            Err(why) => {
                return Err(NokhwaError::OpenDeviceError(
                    index.to_string(),
                    format!(
                        "Failed to open pipeline with args {}: {}",
                        webcam_pipeline(format!("{}", index).as_str(), fmt),
                        why
                    ),
                ))
            }
        };

    let sink = match pipeline
        .clone()
        .dynamic_cast::<Bin>()
        .unwrap()
        .by_name("appsink")
    {
        Some(s) => s,
        None => {
            return Err(NokhwaError::OpenDeviceError(
                index.to_string(),
                "Failed to get sink element!".to_string(),
            ))
        }
    };

    let appsink = match sink.dynamic_cast::<AppSink>() {
        Ok(aps) => aps,
        Err(_) => {
            return Err(NokhwaError::OpenDeviceError(
                index.to_string(),
                "Failed to get sink element as appsink".to_string(),
            ))
        }
    };

    pipeline.set_state(State::Playing).unwrap();

    let image_lock = Arc::new(Mutex::new(ImageBuffer::default()));
    let img_lck_clone = image_lock.clone();

    appsink.set_callbacks(
        AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                let sample = appsink.pull_sample().map_err(|_| FlowError::Eos)?;
                let sample_caps = if let Some(c) = sample.caps() {
                    c
                } else {
                    element_error!(
                        appsink,
                        ResourceError::Failed,
                        ("Failed to get caps of sample")
                    );
                    return Err(FlowError::Error);
                };

                let video_info = match VideoInfo::from_caps(sample_caps) {
                    Ok(vi) => vi, // help let me outtttttt
                    Err(why) => {
                        element_error!(
                            appsink,
                            ResourceError::Failed,
                            (format!("Failed to get videoinfo from caps: {}", why).as_str())
                        );

                        return Err(FlowError::Error);
                    }
                };

                let buffer = if let Some(buf) = sample.buffer() {
                    buf
                } else {
                    element_error!(
                        appsink,
                        ResourceError::Failed,
                        ("Failed to get buffer from sample")
                    );
                    return Err(FlowError::Error);
                };

                let buffer_map = match buffer.map_readable() {
                    Ok(m) => m,
                    Err(why) => {
                        element_error!(
                            appsink,
                            ResourceError::Failed,
                            (format!("Failed to map buffer to readablemap: {}", why).as_str())
                        );

                        return Err(FlowError::Error);
                    }
                };

                let channels = if video_info.has_alpha() { 4 } else { 3 };

                let image_buffer = match video_info.format() {
                    VideoFormat::Yuy2 => {
                        let mut decoded_buffer = match yuyv422_to_rgb(&buffer_map, false) {
                            Ok(buf) => buf,
                            Err(why) => {
                                element_error!(
                                    appsink,
                                    ResourceError::Failed,
                                    (format!("Failed to make yuy2 into rgb888: {}", why).as_str())
                                );

                                return Err(FlowError::Error);
                            }
                        };

                        decoded_buffer.resize(
                            (video_info.width() * video_info.height() * channels) as usize,
                            0_u8,
                        );

                        let image = if let Some(i) = ImageBuffer::from_vec(
                            video_info.width(),
                            video_info.height(),
                            decoded_buffer,
                        ) {
                            let rgb: ImageBuffer<Rgb<u8>, Vec<u8>> = i;
                            rgb
                        } else {
                            element_error!(
                                appsink,
                                ResourceError::Failed,
                                ("Failed to make rgb buffer into imagebuffer")
                            );

                            return Err(FlowError::Error);
                        };
                        image
                    }
                    VideoFormat::Rgb => {
                        let mut decoded_buffer = buffer_map.as_slice().to_vec();
                        decoded_buffer.resize(
                            (video_info.width() * video_info.height() * channels) as usize,
                            0_u8,
                        );
                        let image = if let Some(i) = ImageBuffer::from_vec(
                            video_info.width(),
                            video_info.height(),
                            decoded_buffer,
                        ) {
                            let rgb: ImageBuffer<Rgb<u8>, Vec<u8>> = i;
                            rgb
                        } else {
                            element_error!(
                                appsink,
                                ResourceError::Failed,
                                ("Failed to make rgb buffer into imagebuffer")
                            );

                            return Err(FlowError::Error);
                        };
                        image
                    }
                    // MJPEG
                    VideoFormat::Encoded => {
                        let mut decoded_buffer = match mjpeg_to_rgb(&buffer_map, false) {
                            Ok(buf) => buf,
                            Err(why) => {
                                element_error!(
                                    appsink,
                                    ResourceError::Failed,
                                    (format!("Failed to make yuy2 into rgb888: {}", why).as_str())
                                );

                                return Err(FlowError::Error);
                            }
                        };

                        decoded_buffer.resize(
                            (video_info.width() * video_info.height() * channels) as usize,
                            0_u8,
                        );

                        let image = if let Some(i) = ImageBuffer::from_vec(
                            video_info.width(),
                            video_info.height(),
                            decoded_buffer,
                        ) {
                            let rgb: ImageBuffer<Rgb<u8>, Vec<u8>> = i;
                            rgb
                        } else {
                            element_error!(
                                appsink,
                                ResourceError::Failed,
                                ("Failed to make rgb buffer into imagebuffer")
                            );

                            return Err(FlowError::Error);
                        };
                        image
                    }
                    _ => {
                        element_error!(
                            appsink,
                            ResourceError::Failed,
                            ("Unsupported video format")
                        );
                        return Err(FlowError::Error);
                    }
                };

                *img_lck_clone.lock() = image_buffer;

                Ok(FlowSuccess::Ok)
            })
            .build(),
    );
    Ok((pipeline, appsink, image_lock))
}
