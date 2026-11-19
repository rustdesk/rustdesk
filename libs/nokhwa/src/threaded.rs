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

use crate::Camera;
use nokhwa_core::{
    buffer::Buffer,
    error::NokhwaError,
    types::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueSetter,
        FrameFormat, KnownCameraControl, RequestedFormat, RequestedFormatType, Resolution,
    },
};
use std::thread::JoinHandle;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

type AtomicLock<T> = Arc<Mutex<T>>;
type HeldCallbackType = Arc<Mutex<Box<dyn FnMut(Buffer) + Send + 'static>>>;

/// Creates a camera that runs in a different thread that you can use a callback to access the frames of.
/// It uses a `Arc` and a `Mutex` to ensure that this feels like a normal camera, but callback based.
/// See [`Camera`] for more details on the camera itself.
///
/// Your function is called every time there is a new frame. In order to avoid frame loss, it should
/// complete before a new frame is available. If you need to do heavy image processing, it may be
/// beneficial to directly pipe the data to a new thread to process it there.
///
/// Note that this does not have `WGPU` capabilities. This should be implemented in your callback.
/// # SAFETY
/// The `Mutex` guarantees exclusive access to the underlying camera struct. They should be safe to
/// impl `Send` on.
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "output-threaded")))]
pub struct CallbackCamera {
    // Important: this needs a fair mutex so that the capture loop doesn't block other accessors from touching the camera forever.
    camera: Arc<parking_lot::FairMutex<Camera>>,
    frame_callback: HeldCallbackType,
    last_frame_captured: AtomicLock<Buffer>,
    die_bool: Arc<AtomicBool>,
    current_camera: CameraInfo,
    handle: AtomicLock<Option<JoinHandle<()>>>,
}

impl CallbackCamera {
    /// Create a new `ThreadedCamera` from a [`CameraIndex`] and [`format`]
    ///
    /// # Errors
    /// This will error if you either have a bad platform configuration (e.g. `input-v4l` but not on linux) or the backend cannot create the camera (e.g. permission denied).
    pub fn new(
        index: CameraIndex,
        format: RequestedFormat,
        callback: impl FnMut(Buffer) + Send + 'static,
    ) -> Result<Self, NokhwaError> {
        Ok(Self::with_custom(Camera::new(index, format)?, callback))
    }

    /// Allows creation of a [`Camera`] with a custom backend. This is useful if you are creating e.g. a custom module.
    ///
    /// You **must** have set a format beforehand.
    pub fn with_custom(camera: Camera, callback: impl FnMut(Buffer) + Send + 'static) -> Self {
        let current_camera = camera.info().clone();
        CallbackCamera {
            camera: Arc::new(parking_lot::FairMutex::new(camera)),
            frame_callback: Arc::new(Mutex::new(Box::new(callback))),
            last_frame_captured: Arc::new(Mutex::new(Buffer::new(
                Resolution::new(0, 0),
                &vec![],
                FrameFormat::GRAY,
            ))),
            die_bool: Arc::new(Default::default()),
            current_camera,
            handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Gets the current Camera's index.
    pub fn index(&self) -> &CameraIndex {
        &self.current_camera.index()
    }

    /// Sets the current Camera's index. Note that this re-initializes the camera.
    /// # Errors
    /// The Backend may fail to initialize.
    pub fn set_index(&mut self, new_idx: &CameraIndex) -> Result<(), NokhwaError> {
        let mut camera = self.camera.lock();
        camera.set_index(new_idx)?;
        self.current_camera = camera.info().clone();
        Ok(())
    }

    /// Gets the current Camera's backend
    pub fn backend(&self) -> Result<ApiBackend, NokhwaError> {
        Ok(self.camera.lock().backend())
    }

    /// Sets the current Camera's backend. Note that this re-initializes the camera.
    /// # Errors
    /// The new backend may not exist or may fail to initialize the new camera.
    pub fn set_backend(&mut self, new_backend: ApiBackend) -> Result<(), NokhwaError> {
        self.camera.lock().set_backend(new_backend)
    }

    /// Gets the camera information such as Name and Index as a [`CameraInfo`].
    pub fn info(&self) -> &CameraInfo {
        &self.current_camera
    }

    /// Gets the current [`CameraFormat`].
    pub fn camera_format(&self) -> Result<CameraFormat, NokhwaError> {
        Ok(self.camera.lock().camera_format())
    }

    /// Will set the current [`CameraFormat`]
    /// This will reset the current stream if used while stream is opened.
    /// # Errors
    /// If you started the stream and the camera rejects the new camera format, this will return an error.
    #[deprecated(since = "0.10.0", note = "please use `set_camera_requset` instead.")]
    pub fn set_camera_format(&mut self, new_fmt: CameraFormat) -> Result<(), NokhwaError> {
        *self
            .last_frame_captured
            .lock()
            .map_err(|why| NokhwaError::GeneralError(why.to_string()))? = Buffer::new(
            new_fmt.resolution(),
            &Vec::default(),
            self.camera_format()?.format(),
        );
        let formats = vec![new_fmt.format()];
        let request = RequestedFormat::with_formats(RequestedFormatType::Exact(new_fmt), &formats);
        let set_fmt = self.camera.lock().set_camera_requset(request)?;
        if new_fmt != set_fmt {
            return Err(NokhwaError::SetPropertyError {
                property: "CameraFormat".to_string(),
                value: "CameraFormat".to_string(),
                error: "Requested Format Not Consistant".to_string(),
            });
        }
        Ok(())
    }

    /// Will set the current [`CameraFormat`], using a [`RequestedFormat.`]
    /// This will reset the current stream if used while stream is opened.
    ///
    /// This will also update the cache.
    ///
    /// This will return the new [`CameraFormat`]
    /// # Errors
    /// If nothing fits the requested criteria, this will return an error.
    pub fn set_camera_requset(
        &mut self,
        request: RequestedFormat,
    ) -> Result<CameraFormat, NokhwaError> {
        self.camera.lock().set_camera_requset(request)
    }
    /// A hashmap of [`Resolution`]s mapped to framerates
    /// # Errors
    /// This will error if the camera is not queryable or a query operation has failed. Some backends will error this out as a [`UnsupportedOperationError`](crate::NokhwaError::UnsupportedOperationError).
    pub fn compatible_list_by_resolution(
        &mut self,
        fourcc: FrameFormat,
    ) -> Result<HashMap<Resolution, Vec<u32>>, NokhwaError> {
        self.camera.lock().compatible_list_by_resolution(fourcc)
    }

    /// A Vector of compatible [`FrameFormat`]s.
    /// # Errors
    /// This will error if the camera is not queryable or a query operation has failed. Some backends will error this out as a [`UnsupportedOperationError`](crate::NokhwaError::UnsupportedOperationError).
    pub fn compatible_fourcc(&mut self) -> Result<Vec<FrameFormat>, NokhwaError> {
        self.camera.lock().compatible_fourcc()
    }

    /// Gets the current camera resolution (See: [`Resolution`], [`CameraFormat`]).
    pub fn resolution(&self) -> Result<Resolution, NokhwaError> {
        Ok(self.camera.lock().resolution())
    }

    /// Will set the current [`Resolution`]
    /// This will reset the current stream if used while stream is opened.
    /// # Errors
    /// If you started the stream and the camera rejects the new resolution, this will return an error.
    pub fn set_resolution(&mut self, new_res: Resolution) -> Result<(), NokhwaError> {
        *self
            .last_frame_captured
            .lock()
            .map_err(|why| NokhwaError::GeneralError(why.to_string()))? =
            Buffer::new(new_res, &Vec::default(), self.camera_format()?.format());
        self.camera.lock().set_resolution(new_res)
    }

    /// Gets the current camera framerate (See: [`CameraFormat`]).
    pub fn frame_rate(&self) -> Result<u32, NokhwaError> {
        Ok(self.camera.lock().frame_rate())
    }

    /// Will set the current framerate
    /// This will reset the current stream if used while stream is opened.
    /// # Errors
    /// If you started the stream and the camera rejects the new framerate, this will return an error.
    pub fn set_frame_rate(&mut self, new_fps: u32) -> Result<(), NokhwaError> {
        self.camera.lock().set_frame_rate(new_fps)
    }

    /// Gets the current camera's frame format (See: [`FrameFormat`], [`CameraFormat`]).
    pub fn frame_format(&self) -> Result<FrameFormat, NokhwaError> {
        Ok(self.camera.lock().frame_format())
    }

    /// Will set the current [`FrameFormat`]
    /// This will reset the current stream if used while stream is opened.
    /// # Errors
    /// If you started the stream and the camera rejects the new frame format, this will return an error.
    pub fn set_frame_format(&mut self, fourcc: FrameFormat) -> Result<(), NokhwaError> {
        self.camera.lock().set_frame_format(fourcc)
    }

    /// Gets the current supported list of [`KnownCameraControl`]
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as a "nothing supported".
    pub fn supported_camera_controls(&self) -> Result<Vec<KnownCameraControl>, NokhwaError> {
        self.camera.lock().supported_camera_controls()
    }

    /// Gets the current supported list of [`CameraControl`]s keyed by its name as a `String`.
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as a "nothing supported".
    pub fn camera_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
        let known_controls = self.supported_camera_controls()?;
        let maybe_camera_controls = known_controls
            .iter()
            .map(|x| self.camera_control(*x))
            .filter(Result::is_ok)
            .map(Result::unwrap)
            .collect::<Vec<CameraControl>>();

        Ok(maybe_camera_controls)
    }

    /// Gets the current supported list of [`CameraControl`]s keyed by its name as a `String`.
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as a "nothing supported".
    pub fn camera_controls_string(&self) -> Result<HashMap<String, CameraControl>, NokhwaError> {
        let known_controls = self.supported_camera_controls()?;
        let maybe_camera_controls = known_controls
            .iter()
            .map(|x| (x.to_string(), self.camera_control(*x)))
            .filter(|(_, x)| x.is_ok())
            .map(|(c, x)| (c, Result::unwrap(x)))
            .collect::<Vec<(String, CameraControl)>>();
        let mut control_map = HashMap::with_capacity(maybe_camera_controls.len());

        for (kc, cc) in maybe_camera_controls {
            control_map.insert(kc, cc);
        }

        Ok(control_map)
    }

    /// Gets the current supported list of [`CameraControl`]s keyed by its name as a `String`.
    /// # Errors
    /// If the list cannot be collected, this will error. This can be treated as a "nothing supported".
    pub fn camera_controls_known_camera_controls(
        &self,
    ) -> Result<HashMap<KnownCameraControl, CameraControl>, NokhwaError> {
        let known_controls = self.supported_camera_controls()?;
        let maybe_camera_controls = known_controls
            .iter()
            .map(|x| (*x, self.camera_control(*x)))
            .filter(|(_, x)| x.is_ok())
            .map(|(c, x)| (c, Result::unwrap(x)))
            .collect::<Vec<(KnownCameraControl, CameraControl)>>();
        let mut control_map = HashMap::with_capacity(maybe_camera_controls.len());

        for (kc, cc) in maybe_camera_controls {
            control_map.insert(kc, cc);
        }

        Ok(control_map)
    }

    /// Gets the value of [`KnownCameraControl`].
    /// # Errors
    /// If the `control` is not supported or there is an error while getting the camera control values (e.g. unexpected value, too high, etc)
    /// this will error.
    pub fn camera_control(
        &self,
        control: KnownCameraControl,
    ) -> Result<CameraControl, NokhwaError> {
        self.camera.lock().camera_control(control)
    }

    /// Sets the control to `control` in the camera.
    /// Usually, the pipeline is calling [`camera_control()`](crate::camera_traits::CaptureBackendTrait::camera_control), getting a camera control that way
    /// then calling [`value()`](crate::utils::CameraControl::value()) to get a [`ControlValueSetter`](crate::utils::ControlValueSetter) and setting the value that way.
    /// # Errors
    /// If the `control` is not supported, the value is invalid (less than min, greater than max, not in step), or there was an error setting the control,
    /// this will error.
    pub fn set_camera_control(
        &mut self,
        id: KnownCameraControl,
        control: ControlValueSetter,
    ) -> Result<(), NokhwaError> {
        self.camera.lock().set_camera_control(id, control)
    }

    /// Will open the camera stream with set parameters. This will be called internally if you try and call [`frame()`](crate::Camera::frame()) before you call [`open_stream()`](crate::Camera::open_stream()).
    /// The callback will be called every frame.
    /// # Errors
    /// If the specific backend fails to open the camera (e.g. already taken, busy, doesn't exist anymore) this will error.
    pub fn open_stream(&mut self) -> Result<(), NokhwaError> {
        let mut handle_lock = self
            .handle
            .lock()
            .map_err(|why| NokhwaError::GetPropertyError {
                property: "thread handle".to_string(),
                error: why.to_string(),
            })?;
        if handle_lock.is_none() {
            self.camera.lock().open_stream()?;
            let die_bool_clone = self.die_bool.clone();
            let camera_clone = self.camera.clone();
            let last_frame = self.last_frame_captured.clone();
            let callback = self.frame_callback.clone();
            let handle = std::thread::spawn(move || {
                camera_frame_thread_loop(camera_clone, callback, last_frame, die_bool_clone)
            });
            *handle_lock = Some(handle);
            Ok(())
        } else {
            Err(NokhwaError::OpenStreamError(
                "Stream Already Open".to_string(),
            ))
        }
    }

    /// Sets the frame callback to the new specified function. This function will be called instead of the previous one(s).
    pub fn set_callback(
        &mut self,
        callback: impl FnMut(Buffer) + Send + 'static,
    ) -> Result<(), NokhwaError> {
        *self
            .frame_callback
            .lock()
            .map_err(|why| NokhwaError::GetPropertyError {
                property: "frame_callback".to_string(),
                error: why.to_string(),
            })? = Box::new(callback);
        Ok(())
    }

    /// Polls the camera for a frame, analogous to [`Camera::frame`](crate::Camera::frame)
    /// # Errors
    /// This will error if the camera fails to capture a frame.
    pub fn poll_frame(&mut self) -> Result<Buffer, NokhwaError> {
        let frame = self.camera.lock().frame()?;
        *self
            .last_frame_captured
            .lock()
            .map_err(|why| NokhwaError::GeneralError(why.to_string()))? = frame.clone();
        Ok(frame)
    }

    /// Gets the last frame captured by the camera.
    pub fn last_frame(&self) -> Result<Buffer, NokhwaError> {
        Ok(self
            .last_frame_captured
            .lock()
            .map_err(|why| NokhwaError::ReadFrameError(why.to_string()))?
            .clone())
    }

    /// Checks if stream if open. If it is, it will return true.
    pub fn is_stream_open(&self) -> Result<bool, NokhwaError> {
        Ok(self.camera.lock().is_stream_open())
    }

    /// Will drop the stream.
    /// # Errors
    /// Please check the `Quirks` section of each backend.
    pub fn stop_stream(&mut self) -> Result<(), NokhwaError> {
        self.camera.lock().stop_stream()
    }
}

impl Drop for CallbackCamera {
    fn drop(&mut self) {
        let _stop_stream_err = self.stop_stream();
        self.die_bool.store(true, Ordering::SeqCst);
    }
}

fn camera_frame_thread_loop(
    camera: Arc<parking_lot::FairMutex<Camera>>,
    frame_callback: HeldCallbackType,
    last_frame_captured: AtomicLock<Buffer>,
    die_bool: Arc<AtomicBool>,
) {
    loop {
        let mut camera = camera.lock();
        if let Ok(frame) = camera.frame() {
            if let Ok(mut last_frame) = last_frame_captured.lock() {
                *last_frame = frame.clone();
                if let Ok(mut cb) = frame_callback.lock() {
                    cb(frame);
                }
            }
        }
        if die_bool.load(Ordering::SeqCst) {
            break;
        }
    }
}
