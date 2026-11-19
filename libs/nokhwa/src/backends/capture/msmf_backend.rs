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
use nokhwa_bindings_windows::wmf::MediaFoundationDevice;
use nokhwa_core::{
    buffer::Buffer,
    error::NokhwaError,
    pixel_format::RgbFormat,
    traits::CaptureBackendTrait,
    types::{
        all_known_camera_controls, ApiBackend, CameraControl, CameraFormat, CameraIndex,
        CameraInfo, ControlValueSetter, FrameFormat, KnownCameraControl, RequestedFormat,
        RequestedFormatType, Resolution,
    },
};
use std::{borrow::Cow, collections::HashMap};

/// The backend that deals with Media Foundation on Windows.
/// To see what this does, please see [`CaptureBackendTrait`].
///
/// Note: This requires Windows 7 or newer to work.
/// # Quirks
/// - This does build on non-windows platforms, however when you do the backend will be empty and will return an error for any given operation.
/// - Please check [`nokhwa-bindings-windows`](https://github.com/l1npengtul/nokhwa/tree/senpai/nokhwa-bindings-windows) source code to see the internal raw interface.
/// - The symbolic link for the device is listed in the `misc` attribute of the [`CameraInfo`].
/// - The names may contain invalid characters since they were converted from UTF16.
/// - When you call new or drop the struct, `initialize`/`de_initialize` will automatically be called.
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-msmf")))]
pub struct MediaFoundationCaptureDevice {
    inner: MediaFoundationDevice,
    info: CameraInfo,
}

impl MediaFoundationCaptureDevice {
    /// Creates a new capture device using the Media Foundation backend. Indexes are gives to devices by the OS, and usually numbered by order of discovery.
    /// # Errors
    /// This function will error if Media Foundation fails to get the device.
    pub fn new(index: &CameraIndex, camera_fmt: RequestedFormat) -> Result<Self, NokhwaError> {
        let mut mf_device = MediaFoundationDevice::new(index.clone())?;

        let info = CameraInfo::new(
            &mf_device.name(),
            "MediaFoundation Camera Device",
            &mf_device.symlink(),
            index.clone(),
        );

        let availible = mf_device.compatible_format_list()?;

        let desired = camera_fmt
            .fulfill(&availible)
            .ok_or(NokhwaError::InitializeError {
                backend: ApiBackend::MediaFoundation,
                error: "Failed to fulfill requested format".to_string(),
            })?;

        mf_device.set_format(desired)?;

        let mut new_cam = MediaFoundationCaptureDevice {
            inner: mf_device,
            info,
        };
        new_cam.refresh_camera_format()?;
        Ok(new_cam)
    }

    /// Create a new Media Foundation Device with desired settings.
    /// # Errors
    /// This function will error if Media Foundation fails to get the device.
    #[deprecated(since = "0.10.0", note = "please use `new` instead.")]
    pub fn new_with(
        index: &CameraIndex,
        width: u32,
        height: u32,
        fps: u32,
        fourcc: FrameFormat,
    ) -> Result<Self, NokhwaError> {
        let camera_format = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(
            CameraFormat::new_from(width, height, fourcc, fps),
        ));
        MediaFoundationCaptureDevice::new(index, camera_format)
    }

    /// Gets the list of supported [`KnownCameraControl`]s
    /// # Errors
    /// May error if there is an error from `MediaFoundation`.
    pub fn supported_camera_controls(&self) -> Vec<KnownCameraControl> {
        let mut supported_camera_controls: Vec<KnownCameraControl> = vec![];

        for camera_control in all_known_camera_controls() {
            if let Ok(supported) = self.inner.control(camera_control) {
                supported_camera_controls.push(supported.control());
            }
        }
        supported_camera_controls
    }
}

impl CaptureBackendTrait for MediaFoundationCaptureDevice {
    fn backend(&self) -> ApiBackend {
        ApiBackend::MediaFoundation
    }

    fn camera_info(&self) -> &CameraInfo {
        &self.info
    }

    fn refresh_camera_format(&mut self) -> Result<(), NokhwaError> {
        let _ = self.inner.format_refreshed()?;
        Ok(())
    }

    fn camera_format(&self) -> CameraFormat {
        self.inner.format()
    }

    fn set_camera_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
        self.inner.set_format(new_fmt)
    }

    fn compatible_list_by_resolution(
        &mut self,
        fourcc: FrameFormat,
    ) -> Result<HashMap<Resolution, Vec<u32>>, NokhwaError> {
        let mf_camera_format_list = self.inner.compatible_format_list()?;
        let mut resolution_map: HashMap<Resolution, Vec<u32>> = HashMap::new();

        for camera_format in mf_camera_format_list {
            // check fcc
            if camera_format.format() != fourcc {
                continue;
            }

            match resolution_map.get_mut(&camera_format.resolution()) {
                Some(fps_list) => {
                    fps_list.push(camera_format.frame_rate());
                }
                None => {
                    if let Some(mut wtf_why_we_here_list) = resolution_map
                        .insert(camera_format.resolution(), vec![camera_format.frame_rate()])
                    {
                        wtf_why_we_here_list.push(camera_format.frame_rate());
                        resolution_map.insert(camera_format.resolution(), wtf_why_we_here_list);
                    }
                }
            }
        }
        Ok(resolution_map)
    }

    fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        let mf_camera_format_list = self.inner.compatible_format_list()?;
        let mut frame_format_list = vec![];

        for camera_format in mf_camera_format_list {
            if !frame_format_list.contains(&camera_format.format()) {
                frame_format_list.push(camera_format.format());
            }

            // TODO: Update as we get more frame formats!
            if frame_format_list.len() == 2 {
                break;
            }
        }
        Ok(frame_format_list)
    }

    fn resolution(&self) -> Resolution {
        self.camera_format().resolution()
    }

    fn set_resolution(&mut self, new_res: Resolution) -> Result<(), NokhwaError> {
        let mut new_format = self.camera_format();
        new_format.set_resolution(new_res);
        self.set_camera_format(new_format)
    }

    fn frame_rate(&self) -> u32 {
        self.camera_format().frame_rate()
    }

    fn set_frame_rate(&mut self, new_fps: u32) -> Result<(), NokhwaError> {
        let mut new_format = self.camera_format();
        new_format.set_frame_rate(new_fps);
        self.set_camera_format(new_format)
    }

    fn frame_format(&self) -> FrameFormat {
        self.camera_format().format()
    }

    fn set_frame_format(&mut self, fourcc: FrameFormat) -> Result<(), NokhwaError> {
        let mut new_format = self.camera_format();
        new_format.set_format(fourcc);
        self.set_camera_format(new_format)
    }

    fn camera_control(&self, control: KnownCameraControl) -> Result<CameraControl, NokhwaError> {
        self.inner.control(control)
    }

    fn camera_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        let mut camera_ctrls = Vec::with_capacity(15);
        for ctrl_id in all_known_camera_controls() {
            let ctrl = match self.camera_control(ctrl_id) {
                Ok(v) => v,
                Err(_) => continue,
            };

            camera_ctrls.push(ctrl);
        }
        camera_ctrls.shrink_to_fit();
        Ok(camera_ctrls)
    }

    fn set_camera_control(
        &mut self,
        id: KnownCameraControl,
        value: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.inner.set_control(id, value)
    }

    fn open_stream(&mut self) -> Result<(), NokhwaError> {
        self.inner.start_stream()
    }

    fn is_stream_open(&self) -> bool {
        self.inner.is_stream_open()
    }

    fn frame(&mut self) -> Result<Buffer, NokhwaError> {
        self.refresh_camera_format()?;
        let self_ctrl = self.camera_format();
        Ok(Buffer::new(
            self_ctrl.resolution(),
            &self.inner.raw_bytes()?,
            self_ctrl.format(),
        ))
    }

    fn frame_raw(&mut self) -> Result<Cow<[u8]>, NokhwaError> {
        self.inner.raw_bytes()
    }

    fn stop_stream(&mut self) -> Result<(), NokhwaError> {
        self.inner.stop_stream();
        Ok(())
    }
}
