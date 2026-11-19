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

#[cfg(target_os = "linux")]
mod internal {
    use nokhwa_core::{
        buffer::Buffer,
        error::NokhwaError,
        traits::CaptureBackendTrait,
        types::{
            ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo,
            ControlValueDescription, ControlValueSetter, FrameFormat, KnownCameraControl,
            KnownCameraControlFlag, RequestedFormat, RequestedFormatType, Resolution,
        },
    };
    use std::{
        borrow::Cow,
        collections::HashMap,
        io::{self, ErrorKind},
    };
    use v4l::{
        control::{Control, Flags, Type, Value},
        frameinterval::FrameIntervalEnum,
        framesize::FrameSizeEnum,
        io::traits::CaptureStream,
        prelude::MmapStream,
        video::{capture::Parameters, Capture},
        Device, Format, FourCC,
    };
    use v4l::v4l_sys::{
        V4L2_CID_BACKLIGHT_COMPENSATION, V4L2_CID_BRIGHTNESS, V4L2_CID_CONTRAST, V4L2_CID_EXPOSURE,
        V4L2_CID_FOCUS_RELATIVE, V4L2_CID_GAIN, V4L2_CID_GAMMA, V4L2_CID_HUE,
        V4L2_CID_IRIS_RELATIVE, V4L2_CID_PAN_RELATIVE, V4L2_CID_SATURATION, V4L2_CID_SHARPNESS,
        V4L2_CID_TILT_RELATIVE, V4L2_CID_WHITE_BALANCE_TEMPERATURE, V4L2_CID_ZOOM_RELATIVE,
    };

    /// Attempts to convert a [`KnownCameraControl`] into a V4L2 Control ID.
    /// If the associated control is not found, this will return `None` (`ColorEnable`, `Roll`)
    #[allow(clippy::cast_possible_truncation)]
    pub fn known_camera_control_to_id(ctrl: KnownCameraControl) -> u32 {
        match ctrl {
            KnownCameraControl::Brightness => V4L2_CID_BRIGHTNESS,
            KnownCameraControl::Contrast => V4L2_CID_CONTRAST,
            KnownCameraControl::Hue => V4L2_CID_HUE,
            KnownCameraControl::Saturation => V4L2_CID_SATURATION,
            KnownCameraControl::Sharpness => V4L2_CID_SHARPNESS,
            KnownCameraControl::Gamma => V4L2_CID_GAMMA,
            KnownCameraControl::WhiteBalance => V4L2_CID_WHITE_BALANCE_TEMPERATURE,
            KnownCameraControl::BacklightComp => V4L2_CID_BACKLIGHT_COMPENSATION,
            KnownCameraControl::Gain => V4L2_CID_GAIN,
            KnownCameraControl::Pan => V4L2_CID_PAN_RELATIVE,
            KnownCameraControl::Tilt => V4L2_CID_TILT_RELATIVE,
            KnownCameraControl::Zoom => V4L2_CID_ZOOM_RELATIVE,
            KnownCameraControl::Exposure => V4L2_CID_EXPOSURE,
            KnownCameraControl::Iris => V4L2_CID_IRIS_RELATIVE,
            KnownCameraControl::Focus => V4L2_CID_FOCUS_RELATIVE,
            KnownCameraControl::Other(id) => id as u32,
        }
    }

    /// Attempts to convert a [`u32`] V4L2 Control ID into a [`KnownCameraControl`]
    /// If the associated control is not found, this will return `None` (`ColorEnable`, `Roll`)
    #[allow(clippy::cast_lossless)]
    pub fn id_to_known_camera_control(id: u32) -> KnownCameraControl {
        match id {
            V4L2_CID_BRIGHTNESS => KnownCameraControl::Brightness,
            V4L2_CID_CONTRAST => KnownCameraControl::Contrast,
            V4L2_CID_HUE => KnownCameraControl::Hue,
            V4L2_CID_SATURATION => KnownCameraControl::Saturation,
            V4L2_CID_SHARPNESS => KnownCameraControl::Sharpness,
            V4L2_CID_GAMMA => KnownCameraControl::Gamma,
            V4L2_CID_WHITE_BALANCE_TEMPERATURE => KnownCameraControl::WhiteBalance,
            V4L2_CID_BACKLIGHT_COMPENSATION => KnownCameraControl::BacklightComp,
            V4L2_CID_GAIN => KnownCameraControl::Gain,
            V4L2_CID_PAN_RELATIVE => KnownCameraControl::Pan,
            V4L2_CID_TILT_RELATIVE => KnownCameraControl::Tilt,
            V4L2_CID_ZOOM_RELATIVE => KnownCameraControl::Zoom,
            V4L2_CID_EXPOSURE => KnownCameraControl::Exposure,
            V4L2_CID_IRIS_RELATIVE => KnownCameraControl::Iris,
            V4L2_CID_FOCUS_RELATIVE => KnownCameraControl::Focus,
            id => KnownCameraControl::Other(id as u128),
        }
    }

    /// query v4l2 cameras
    #[allow(clippy::unnecessary_wraps)]
    #[allow(clippy::cast_possible_truncation)]
    pub fn query() -> Result<Vec<CameraInfo>, NokhwaError> {
        Ok({
            let camera_info: Vec<CameraInfo> = v4l::context::enum_devices()
                .iter()
                .map(|node| {
                    CameraInfo::new(
                        &node
                            .name()
                            .unwrap_or(format!("{}", node.path().to_string_lossy())),
                        &format!("Video4Linux Device @ {}", node.path().to_string_lossy()),
                        "",
                        CameraIndex::Index(node.index() as u32),
                    )
                })
                .collect();
            camera_info
        })
    }

    type SharedDevice = std::sync::Arc<std::sync::Mutex<Device>>;
    type WeakSharedDevice = std::sync::Weak<std::sync::Mutex<Device>>;

    struct WeakSharedDeviceEntry {
        device: WeakSharedDevice,
        index: usize,
    }

    type SharedDeviceList = std::sync::OnceLock<std::sync::Mutex<Vec<WeakSharedDeviceEntry>>>;

    /// Global array that keep track of every Device that are currently open.
    /// This is used to open multiple handle to the same device.
    /// This is a workaround for the fact that the v4l2 backend does not support multiple handles to the same device.
    /// This replicate behavior of MF backend.
    /// The reference is a reference of Weak<Mutex<Device>>, so that the Device can be dropped when the last handle is closed.
    /// This list might need some cleanup because it will accumulate every device that is opened, this should not be a problem because the list should not grow too much.
    /// The list is also protected by a mutex, so it should be thread safe.
    static DEVICES: SharedDeviceList = std::sync::OnceLock::new();

    fn cleanup_dropped_devices(devices: &mut Vec<WeakSharedDeviceEntry>) {
        devices.retain(|entry| entry.device.strong_count() > 0);
    }

    fn new_shared_device(index: usize) -> Result<SharedDevice, NokhwaError> {
        let mut devices = DEVICES
            .get_or_init(|| std::sync::Mutex::new(Vec::new()))
            .lock()
            .map_err(|e| NokhwaError::InitializeError {
                backend: ApiBackend::Video4Linux,
                error: format!("Fail to lock global device list mutex: {}", e),
            })?;

        // do some cleanup, this will avoid here memory to grow forever
        // if for some reason someone has tons of camera plugged in
        cleanup_dropped_devices(&mut devices);

        if let Some(entry) = devices.iter().find(|entry| entry.index == index) {
            if let Some(device) = entry.device.upgrade() {
                return Ok(device);
            }
        }

        // Cleanup a second, the device we are interested might have been dropped during before upgrade call
        // For this point on we are assured that the device is not in the list
        cleanup_dropped_devices(&mut devices);

        // Let's be extra sure, this code should never panic, but maybe will help catch some race condition
        assert!(
            devices.iter().find(|entry| entry.index == index).is_none(),
            "Device {index} should not be in the list"
        );

        // Now we can open the device, and never run into a busy io error,
        // as long as the device isn't opened by other programs.
        let device = match Device::new(index) {
            Ok(dev) => dev,
            Err(why) => {
                return Err(NokhwaError::OpenDeviceError(
                    index.to_string(),
                    format!("V4L2 Error: {}", why),
                ))
            }
        };

        let device = std::sync::Arc::new(std::sync::Mutex::new(device));
        devices.push(WeakSharedDeviceEntry {
            device: std::sync::Arc::downgrade(&device),
            index,
        });

        // Last check to be sure that every devices have a unique index
        // and that the data isn't corrupted
        if devices.len() > 1 {
            assert_eq!(
                devices
                    .windows(2)
                    .filter(|window| window[0].index == window[1].index)
                    .count(),
                devices.len(),
                "Device list should not contain duplicate indexes"
            );
        }

        Ok(device)
    }

    fn get_device_format(device: &Device) -> Result<CameraFormat, NokhwaError> {
        match device.format() {
            Ok(format) => {
                let frame_format =
                    fourcc_to_frameformat(format.fourcc).ok_or(NokhwaError::GetPropertyError {
                        property: "FrameFormat".to_string(),
                        error: "unsupported".to_string(),
                    })?;

                let fps = match device.params() {
                    Ok(params) => {
                        if params.interval.numerator != 1
                            && params.interval.denominator % params.interval.numerator != 0
                        {
                            return Err(NokhwaError::GetPropertyError {
                                property: "V4L2 FrameRate".to_string(),
                                error: format!(
                                    "Framerate not whole number: {} / {}",
                                    params.interval.denominator, params.interval.numerator
                                ),
                            });
                        }

                        if params.interval.numerator == 1 {
                            params.interval.denominator
                        } else {
                            params.interval.denominator / params.interval.numerator
                        }
                    }
                    Err(why) => {
                        return Err(NokhwaError::GetPropertyError {
                            property: "V4L2 FrameRate".to_string(),
                            error: why.to_string(),
                        })
                    }
                };

                Ok(CameraFormat::new(
                    Resolution::new(format.width, format.height),
                    frame_format,
                    fps,
                ))
            }
            Err(why) => Err(NokhwaError::GetPropertyError {
                property: "parameters".to_string(),
                error: why.to_string(),
            }),
        }
    }

    /// The backend struct that interfaces with V4L2.
    /// To see what this does, please see [`CaptureBackendTrait`].
    /// # Quirks
    /// - Calling [`set_resolution()`](CaptureBackendTrait::set_resolution), [`set_frame_rate()`](CaptureBackendTrait::set_frame_rate), or [`set_frame_format()`](CaptureBackendTrait::set_frame_format) each internally calls [`set_camera_format()`](CaptureBackendTrait::set_camera_format).
    pub struct V4LCaptureDevice<'a> {
        camera_format: CameraFormat,
        camera_info: CameraInfo,
        device: SharedDevice,
        stream_handle: Option<MmapStream<'a>>,
    }

    impl<'a> V4LCaptureDevice<'a> {
        /// Creates a new capture device using the `V4L2` backend. Indexes are gives to devices by the OS, and usually numbered by order of discovery.
        /// # Errors
        /// This function will error if the camera is currently busy or if `V4L2` can't read device information.
        #[allow(clippy::too_many_lines)]
        pub fn new(index: &CameraIndex, cam_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
            let index = index.clone();

            let shared_device = new_shared_device(index.as_index()? as usize)?;
            let device = shared_device
                .lock()
                .map_err(|e| NokhwaError::InitializeError {
                    backend: ApiBackend::Video4Linux,
                    error: format!("Fail to lock device mutex: {}", e),
                })?;

            // get all formats
            // get all fcc
            let mut camera_formats = vec![];

            let frame_formats = match device.enum_formats() {
                Ok(formats) => {
                    let mut frame_format_vec = vec![];
                    formats
                        .iter()
                        .for_each(|fmt| frame_format_vec.push(fmt.fourcc));
                    frame_format_vec.dedup();
                    Ok(frame_format_vec)
                }
                Err(why) => Err(NokhwaError::GetPropertyError {
                    property: "FrameFormat".to_string(),
                    error: why.to_string(),
                }),
            }?;

            for ff in frame_formats {
                let framefmt = match fourcc_to_frameformat(ff) {
                    Some(s) => s,
                    None => continue,
                };
                // i write unmaintainable blobs of code because i am so cute uwu~~
                let mut formats = device
                    .enum_framesizes(ff)
                    .map_err(|why| NokhwaError::GetPropertyError {
                        property: "ResolutionList".to_string(),
                        error: why.to_string(),
                    })?
                    .into_iter()
                    .flat_map(|x| {
                        match x.size {
                            FrameSizeEnum::Discrete(d) => {
                                [Resolution::new(d.width, d.height)].to_vec()
                            }
                            // we step over each step, getting a new resolution.
                            FrameSizeEnum::Stepwise(s) => (s.min_width..s.max_width)
                                .step_by(s.step_width as usize)
                                .zip((s.min_height..s.max_height).step_by(s.step_height as usize))
                                .map(|(x, y)| Resolution::new(x, y))
                                .collect(),
                        }
                    })
                    .flat_map(|res| {
                        device
                            .enum_frameintervals(ff, res.x(), res.y())
                            .unwrap_or_default()
                            .into_iter()
                            .flat_map(|x| match x.interval {
                                FrameIntervalEnum::Discrete(dis) => {
                                    if dis.numerator == 1 {
                                        vec![CameraFormat::new(
                                            Resolution::new(x.width, x.height),
                                            framefmt,
                                            dis.denominator,
                                        )]
                                    } else {
                                        vec![]
                                    }
                                }
                                FrameIntervalEnum::Stepwise(step) => {
                                    let mut intvec = vec![];
                                    for fstep in (step.min.numerator..=step.max.numerator)
                                        .step_by(step.step.numerator as usize)
                                    {
                                        if step.max.denominator != 1 || step.min.denominator != 1 {
                                            intvec.push(CameraFormat::new(
                                                Resolution::new(x.width, x.height),
                                                framefmt,
                                                fstep,
                                            ));
                                        }
                                    }
                                    intvec
                                }
                            })
                    })
                    .collect::<Vec<CameraFormat>>();
                camera_formats.append(&mut formats);
            }

            let mut format = cam_fmt
                .fulfill(&camera_formats)
                .ok_or(NokhwaError::GetPropertyError {
                    property: "CameraFormat".to_string(),
                    error: "Failed to Fufill".to_string(),
                })?;

            let current_format = get_device_format(&device)?;
            if cam_fmt.requested_format_type() == RequestedFormatType::None {
                format = current_format;
            } else {
                if current_format.width() != format.width()
                    || current_format.height() != format.height()
                    || current_format.format() != format.format()
                {
                    if let Err(why) = device.set_format(&Format::new(
                        format.width(),
                        format.height(),
                        frameformat_to_fourcc(format.format()),
                    )) {
                        return Err(NokhwaError::SetPropertyError {
                            property: "Resolution, FrameFormat".to_string(),
                            value: format.to_string(),
                            error: why.to_string(),
                        });
                    }
                }

                if current_format.frame_rate() != format.frame_rate() {
                    if let Err(why) = device.set_params(&Parameters::with_fps(format.frame_rate())) {
                        return Err(NokhwaError::SetPropertyError {
                            property: "Frame rate".to_string(),
                            value: format.frame_rate().to_string(),
                            error: why.to_string(),
                        });
                    }
                }
            }

            let device_caps = device
                .query_caps()
                .map_err(|why| NokhwaError::GetPropertyError {
                    property: "Device Capabilities".to_string(),
                    error: why.to_string(),
                })?;

            drop(device);

            let mut v4l2 = V4LCaptureDevice {
                camera_format: format,
                camera_info: CameraInfo::new(
                    &device_caps.card,
                    &device_caps.driver,
                    &format!("{} {:?}", device_caps.bus, device_caps.version),
                    index,
                ),
                device: shared_device,
                stream_handle: None,
            };

            v4l2.force_refresh_camera_format()?;
            if v4l2.camera_format() != format {
                return Err(NokhwaError::SetPropertyError {
                    property: "CameraFormat".to_string(),
                    value: String::new(),
                    error: "Not same/Rejected".to_string(),
                });
            }

            Ok(v4l2)
        }

        /// Create a new `V4L2` Camera with desired settings. This may or may not work.
        /// # Errors
        /// This function will error if the camera is currently busy or if `V4L2` can't read device information.
        #[deprecated(since = "0.10.0", note = "please use `new` instead.")]
        #[allow(clippy::needless_pass_by_value)]
        pub fn new_with(
            index: CameraIndex,
            width: u32,
            height: u32,
            fps: u32,
            fourcc: FrameFormat,
        ) -> Result<Self, NokhwaError> {
            let camera_format = CameraFormat::new_from(width, height, fourcc, fps);
            V4LCaptureDevice::new(
                &index,
                RequestedFormat::with_formats(
                    RequestedFormatType::Exact(camera_format),
                    vec![camera_format.format()].as_slice(),
                ),
            )
        }

        fn lock_device(&self) -> Result<std::sync::MutexGuard<'_, Device>, NokhwaError> {
            self.device
                .lock()
                .map_err(|e| NokhwaError::GeneralError(format!("Failed to lock device: {}", e)))
        }

        fn get_resolution_list(&self, fourcc: FrameFormat) -> Result<Vec<Resolution>, NokhwaError> {
            let format = frameformat_to_fourcc(fourcc);

            match self.lock_device()?.enum_framesizes(format) {
                Ok(frame_sizes) => {
                    let mut resolutions = vec![];
                    for frame_size in frame_sizes {
                        match frame_size.size {
                            FrameSizeEnum::Discrete(dis) => {
                                resolutions.push(Resolution::new(dis.width, dis.height));
                            }
                            FrameSizeEnum::Stepwise(step) => {
                                resolutions.push(Resolution::new(step.min_width, step.min_height));
                                resolutions.push(Resolution::new(step.max_width, step.max_height));
                                // TODO: Respect step size
                            }
                        }
                    }
                    Ok(resolutions)
                }
                Err(why) => Err(NokhwaError::GetPropertyError {
                    property: "Resolutions".to_string(),
                    error: why.to_string(),
                }),
            }
        }

        /// Force refreshes the inner [`CameraFormat`] state.
        /// # Errors
        /// If the internal representation in the driver is invalid, this will error.
        pub fn force_refresh_camera_format(&mut self) -> Result<(), NokhwaError> {
            let camera_format = get_device_format(&*self.lock_device()?)?;
            self.camera_format = camera_format;
            Ok(())
        }
    }

    impl<'a> CaptureBackendTrait for V4LCaptureDevice<'a> {
        fn backend(&self) -> ApiBackend {
            ApiBackend::Video4Linux
        }

        fn camera_info(&self) -> &CameraInfo {
            &self.camera_info
        }

        fn refresh_camera_format(&mut self) -> Result<(), NokhwaError> {
            self.force_refresh_camera_format()
        }

        fn camera_format(&self) -> CameraFormat {
            self.camera_format
        }

        fn set_camera_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
            let device = self.lock_device()?;
            let prev_format = match Capture::format(&*device) {
                Ok(fmt) => fmt,
                Err(why) => {
                    return Err(NokhwaError::GetPropertyError {
                        property: "Resolution, FrameFormat".to_string(),
                        error: why.to_string(),
                    })
                }
            };
            let prev_fps = match Capture::params(&*device) {
                Ok(fps) => fps,
                Err(why) => {
                    return Err(NokhwaError::GetPropertyError {
                        property: "Frame rate".to_string(),
                        error: why.to_string(),
                    })
                }
            };

            let v4l_fcc = match new_fmt.format() {
                FrameFormat::MJPEG => FourCC::new(b"MJPG"),
                FrameFormat::YUYV => FourCC::new(b"YUYV"),
                FrameFormat::GRAY => FourCC::new(b"GRAY"),
                FrameFormat::RAWRGB => FourCC::new(b"RGB3"),
                FrameFormat::NV12 => FourCC::new(b"NV12"),
            };

            let format = Format::new(new_fmt.width(), new_fmt.height(), v4l_fcc);
            let frame_rate = Parameters::with_fps(new_fmt.frame_rate());

            if let Err(why) = Capture::set_format(&*device, &format) {
                return Err(NokhwaError::SetPropertyError {
                    property: "Resolution, FrameFormat".to_string(),
                    value: format.to_string(),
                    error: why.to_string(),
                });
            }
            if let Err(why) = Capture::set_params(&*device, &frame_rate) {
                return Err(NokhwaError::SetPropertyError {
                    property: "Frame rate".to_string(),
                    value: frame_rate.to_string(),
                    error: why.to_string(),
                });
            }

            drop(device);

            if self.stream_handle.is_some() {
                return match self.open_stream() {
                    Ok(_) => Ok(()),
                    Err(why) => {
                        // undo
                        let device = self.lock_device()?;
                        if let Err(why) = Capture::set_format(&*device, &prev_format) {
                            return Err(NokhwaError::SetPropertyError {
                                property: format!("Attempt undo due to stream acquisition failure with error {}. Resolution, FrameFormat", why),
                                value: prev_format.to_string(),
                                error: why.to_string(),
                            });
                        }
                        if let Err(why) = Capture::set_params(&*device, &prev_fps) {
                            return Err(NokhwaError::SetPropertyError {
                                property:
                                format!("Attempt undo due to stream acquisition failure with error {}. Frame rate", why),
                                value: prev_fps.to_string(),
                                error: why.to_string(),
                            });
                        }
                        Err(why)
                    }
                };
            }
            self.camera_format = new_fmt;

            self.force_refresh_camera_format()?;
            if self.camera_format != new_fmt {
                return Err(NokhwaError::SetPropertyError {
                    property: "CameraFormat".to_string(),
                    value: new_fmt.to_string(),
                    error: "Rejected".to_string(),
                });
            }

            Ok(())
        }

        fn compatible_list_by_resolution(
            &mut self,
            fourcc: FrameFormat,
        ) -> Result<HashMap<Resolution, Vec<u32>>, NokhwaError> {
            let resolutions = self.get_resolution_list(fourcc)?;
            let format = frameformat_to_fourcc(fourcc);
            let mut res_map = HashMap::new();
            for res in resolutions {
                let mut compatible_fps = vec![];
                match self
                    .lock_device()?
                    .enum_frameintervals(format, res.width(), res.height())
                {
                    Ok(intervals) => {
                        for interval in intervals {
                            match interval.interval {
                                FrameIntervalEnum::Discrete(dis) => {
                                    compatible_fps.push(dis.denominator);
                                }
                                FrameIntervalEnum::Stepwise(step) => {
                                    for fstep in (step.min.numerator..step.max.numerator)
                                        .step_by(step.step.numerator as usize)
                                    {
                                        if step.max.denominator != 1 || step.min.denominator != 1 {
                                            compatible_fps.push(fstep);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(why) => {
                        return Err(NokhwaError::GetPropertyError {
                            property: "Frame rate".to_string(),
                            error: why.to_string(),
                        })
                    }
                }
                res_map.insert(res, compatible_fps);
            }
            Ok(res_map)
        }

        fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
            match self.lock_device()?.enum_formats() {
                Ok(formats) => {
                    let mut frame_format_vec = vec![];
                    for format in formats {
                        match fourcc_to_frameformat(format.fourcc) {
                            Some(ff) => frame_format_vec.push(ff),
                            None => continue,
                        }
                    }
                    frame_format_vec.sort();
                    frame_format_vec.dedup();
                    Ok(frame_format_vec)
                }
                Err(why) => Err(NokhwaError::GetPropertyError {
                    property: "FrameFormat".to_string(),
                    error: why.to_string(),
                }),
            }
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

        fn set_frame_format(&mut self, fourcc: FrameFormat) -> Result<(), NokhwaError> {
            let mut new_fmt = self.camera_format;
            new_fmt.set_format(fourcc);
            self.set_camera_format(new_fmt)
        }

        fn camera_control(
            &self,
            control: KnownCameraControl,
        ) -> Result<CameraControl, NokhwaError> {
            let controls = self.camera_controls()?;
            for supported_control in controls {
                if supported_control.control() == control {
                    return Ok(supported_control);
                }
            }
            Err(NokhwaError::GetPropertyError {
                property: control.to_string(),
                error: "not found/not supported".to_string(),
            })
        }

        #[allow(clippy::cast_possible_wrap)]
        fn camera_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
            let device = self.lock_device()?;
            device
                .query_controls()
                .map_err(|why| NokhwaError::GetPropertyError {
                    property: "V4L2 Controls".to_string(),
                    error: why.to_string(),
                })?
                .into_iter()
                .map(|desc| {
                    let id_as_kcc = id_to_known_camera_control(desc.id);
                    let ctrl_current = device.control(desc.id)?.value;

                    let ctrl_value_desc = match (desc.typ, ctrl_current) {
                        (
                            Type::Integer
                            | Type::Integer64
                            | Type::Menu
                            | Type::U8
                            | Type::U16
                            | Type::U32
                            | Type::IntegerMenu,
                            Value::Integer(current),
                        ) => ControlValueDescription::IntegerRange {
                            min: desc.minimum as i64,
                            max: desc.maximum,
                            value: current,
                            step: desc.step as i64,
                            default: desc.default,
                        },
                        (Type::Boolean, Value::Boolean(current)) => {
                            ControlValueDescription::Boolean {
                                value: current,
                                default: desc.default != 0,
                            }
                        }

                        (Type::String, Value::String(current)) => ControlValueDescription::String {
                            value: current,
                            default: None,
                        },
                        _ => {
                            return Err(io::Error::new(
                                ErrorKind::Unsupported,
                                "what is this?????? todo: support ig",
                            ))
                        }
                    };

                    let is_readonly = desc
                        .flags
                        .intersects(Flags::READ_ONLY)
                        .then_some(KnownCameraControlFlag::ReadOnly);
                    let is_writeonly = desc
                        .flags
                        .intersects(Flags::WRITE_ONLY)
                        .then_some(KnownCameraControlFlag::WriteOnly);
                    let is_disabled = desc
                        .flags
                        .intersects(Flags::DISABLED)
                        .then_some(KnownCameraControlFlag::Disabled);
                    let is_volatile = desc
                        .flags
                        .intersects(Flags::VOLATILE)
                        .then_some(KnownCameraControlFlag::Volatile);
                    let is_inactive = desc
                        .flags
                        .intersects(Flags::INACTIVE)
                        .then_some(KnownCameraControlFlag::Disabled);
                    let flags_vec = vec![
                        is_inactive,
                        is_readonly,
                        is_volatile,
                        is_disabled,
                        is_writeonly,
                    ]
                    .into_iter()
                    .filter(Option::is_some)
                    .collect::<Option<Vec<KnownCameraControlFlag>>>()
                    .unwrap_or_default();

                    Ok(CameraControl::new(
                        id_as_kcc,
                        desc.name,
                        ctrl_value_desc,
                        flags_vec,
                        !desc.flags.intersects(Flags::INACTIVE),
                    ))
                })
                .filter(Result::is_ok)
                .collect::<Result<Vec<CameraControl>, io::Error>>()
                .map_err(|x| NokhwaError::GetPropertyError {
                    property: "www".to_string(),
                    error: x.to_string(),
                })
        }

        fn set_camera_control(
            &mut self,
            id: KnownCameraControl,
            value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            let conv_value = match value.clone() {
                ControlValueSetter::None => Value::None,
                ControlValueSetter::Integer(i) => Value::Integer(i),
                ControlValueSetter::Boolean(b) => Value::Boolean(b),
                ControlValueSetter::String(s) => Value::String(s),
                ControlValueSetter::Bytes(b) => Value::CompoundU8(b),
                v => {
                    return Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: v.to_string(),
                        error: "not supported".to_string(),
                    })
                }
            };
            self.lock_device()?
                .set_control(Control {
                    id: known_camera_control_to_id(id),
                    value: conv_value,
                })
                .map_err(|why| NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: format!("{:?}", value),
                    error: why.to_string(),
                })?;
            // verify

            let control = self.camera_control(id)?;
            if control.value() != value {
                return Err(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: format!("{:?}", value),
                    error: "Rejected".to_string(),
                });
            }
            Ok(())
        }

        fn open_stream(&mut self) -> Result<(), NokhwaError> {
            // Disable mut warning, since mut is only required when not using arena buffers
            #[allow(unused_mut)]
            let mut stream =
                match MmapStream::new(&*self.lock_device()?, v4l::buffer::Type::VideoCapture) {
                    Ok(s) => s,
                    Err(why) => return Err(NokhwaError::OpenStreamError(why.to_string())),
                };

            // Explicitly start now, or won't work with the RPi. As a consequence, buffers will only be used as required.
            // WARNING: This will cause drop of half of the frames
            #[cfg(feature = "no-arena-buffer")]
            match stream.start() {
                Ok(s) => s,
                Err(why) => return Err(NokhwaError::OpenStreamError(why.to_string())),
            }
            self.stream_handle = Some(stream);
            Ok(())
        }

        fn is_stream_open(&self) -> bool {
            self.stream_handle.is_some()
        }

        fn frame(&mut self) -> Result<Buffer, NokhwaError> {
            let cam_fmt = self.camera_format;
            let raw_frame = self.frame_raw()?;
            Ok(Buffer::new(
                cam_fmt.resolution(),
                &raw_frame,
                cam_fmt.format(),
            ))
        }

        fn frame_raw(&mut self) -> Result<Cow<[u8]>, NokhwaError> {
            match &mut self.stream_handle {
                Some(sh) => match sh.next() {
                    Ok((data, _)) => Ok(Cow::Borrowed(data)),
                    Err(why) => Err(NokhwaError::ReadFrameError(why.to_string())),
                },
                None => Err(NokhwaError::ReadFrameError(
                    "Stream Not Started".to_string(),
                )),
            }
        }

        fn stop_stream(&mut self) -> Result<(), NokhwaError> {
            if self.stream_handle.is_some() {
                self.stream_handle = None;
            }
            Ok(())
        }
    }

    fn fourcc_to_frameformat(fourcc: FourCC) -> Option<FrameFormat> {
        match fourcc.str().ok()? {
            "YUYV" => Some(FrameFormat::YUYV),
            "MJPG" => Some(FrameFormat::MJPEG),
            "GRAY" => Some(FrameFormat::GRAY),
            "RGB3" => Some(FrameFormat::RAWRGB),
            "NV12" => Some(FrameFormat::NV12),
            _ => None,
        }
    }

    fn frameformat_to_fourcc(fourcc: FrameFormat) -> FourCC {
        match fourcc {
            FrameFormat::MJPEG => FourCC::new(b"MJPG"),
            FrameFormat::YUYV => FourCC::new(b"YUYV"),
            FrameFormat::GRAY => FourCC::new(b"GRAY"),
            FrameFormat::RAWRGB => FourCC::new(b"RGB3"),
            FrameFormat::NV12 => FourCC::new(b"NV12"),
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod internal {
    use nokhwa_core::buffer::Buffer;
    use nokhwa_core::error::NokhwaError;
    use nokhwa_core::traits::CaptureBackendTrait;
    use nokhwa_core::types::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueSetter,
        FrameFormat, KnownCameraControl, RequestedFormat, Resolution,
    };
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::marker::PhantomData;

    /// Attempts to convert a [`KnownCameraControl`] into a V4L2 Control ID.
    /// If the associated control is not found, this will return `None` (`ColorEnable`, `Roll`)
    #[allow(clippy::cast_possible_truncation)]
    pub fn known_camera_control_to_id(_ctrl: KnownCameraControl) -> u32 {
        0
    }

    /// Attempts to convert a [`u32`] V4L2 Control ID into a [`KnownCameraControl`]
    /// If the associated control is not found, this will return `None` (`ColorEnable`, `Roll`)
    #[allow(clippy::cast_lossless)]
    pub fn id_to_known_camera_control(id: u32) -> KnownCameraControl {
        KnownCameraControl::Other(id as u128)
    }

    /// The backend struct that interfaces with V4L2.
    /// To see what this does, please see [`CaptureBackendTrait`].
    /// # Quirks
    /// - Calling [`set_resolution()`](CaptureBackendTrait::set_resolution), [`set_frame_rate()`](CaptureBackendTrait::set_frame_rate), or [`set_frame_format()`](CaptureBackendTrait::set_frame_format) each internally calls [`set_camera_format()`](CaptureBackendTrait::set_camera_format).
    pub struct V4LCaptureDevice<'a> {
        __holder: PhantomData<&'a str>,
    }

    #[allow(unused_variables)]
    impl<'a> V4LCaptureDevice<'a> {
        /// Creates a new capture device using the `V4L2` backend. Indexes are gives to devices by the OS, and usually numbered by order of discovery.
        /// # Errors
        /// This function will error if the camera is currently busy or if `V4L2` can't read device information.
        #[allow(clippy::too_many_lines)]
        pub fn new(index: &CameraIndex, cam_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "V4L2 only on Linux".to_string(),
            ))
        }

        /// Create a new `V4L2` Camera with desired settings. This may or may not work.
        /// # Errors
        /// This function will error if the camera is currently busy or if `V4L2` can't read device information.
        #[deprecated(since = "0.10.0", note = "please use `new` instead.")]
        pub fn new_with(
            index: CameraIndex,
            width: u32,
            height: u32,
            fps: u32,
            fourcc: FrameFormat,
        ) -> Result<Self, NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "V4L2 only on Linux".to_string(),
            ))
        }

        /// Force refreshes the inner [`CameraFormat`] state.
        /// # Errors
        /// If the internal representation in the driver is invalid, this will error.
        pub fn force_refresh_camera_format(&mut self) -> Result<(), NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "V4L2 only on Linux".to_string(),
            ))
        }
    }

    #[allow(unused_variables)]
    impl<'a> CaptureBackendTrait for V4LCaptureDevice<'a> {
        fn backend(&self) -> ApiBackend {
            ApiBackend::Video4Linux
        }

        fn camera_info(&self) -> &CameraInfo {
            todo!()
        }

        fn refresh_camera_format(&mut self) -> Result<(), NokhwaError> {
            todo!()
        }

        fn camera_format(&self) -> CameraFormat {
            todo!()
        }

        fn set_camera_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
            todo!()
        }

        fn compatible_list_by_resolution(
            &mut self,
            fourcc: FrameFormat,
        ) -> Result<HashMap<Resolution, Vec<u32>>, NokhwaError> {
            todo!()
        }

        fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
            todo!()
        }

        fn resolution(&self) -> Resolution {
            todo!()
        }

        fn set_resolution(&mut self, new_res: Resolution) -> Result<(), NokhwaError> {
            todo!()
        }

        fn frame_rate(&self) -> u32 {
            todo!()
        }

        fn set_frame_rate(&mut self, new_fps: u32) -> Result<(), NokhwaError> {
            todo!()
        }

        fn frame_format(&self) -> FrameFormat {
            todo!()
        }

        fn set_frame_format(&mut self, fourcc: FrameFormat) -> Result<(), NokhwaError> {
            todo!()
        }

        fn camera_control(
            &self,
            control: KnownCameraControl,
        ) -> Result<CameraControl, NokhwaError> {
            todo!()
        }

        fn camera_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
            todo!()
        }

        fn set_camera_control(
            &mut self,
            id: KnownCameraControl,
            value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            todo!()
        }

        fn open_stream(&mut self) -> Result<(), NokhwaError> {
            todo!()
        }

        fn is_stream_open(&self) -> bool {
            todo!()
        }

        fn frame(&mut self) -> Result<Buffer, NokhwaError> {
            todo!()
        }

        fn frame_raw(&mut self) -> Result<Cow<[u8]>, NokhwaError> {
            todo!()
        }

        fn stop_stream(&mut self) -> Result<(), NokhwaError> {
            todo!()
        }
    }
}

pub use internal::*;
