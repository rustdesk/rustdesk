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

//! This contains all the code for using webcams in the browser.
//!
//! Anything starting with `js` is meant as a binding, a.k.a. not meant for consumption.
//!
//! This assumes that you are running a modern browser on the desktop.

use image::{buffer::ConvertBuffer, ImageBuffer, Rgb, RgbImage, Rgba};
use js_sys::{Array, JsString, Map, Object, Promise};
use nokhwa_core::{
    error::NokhwaError,
    types::{CameraIndex, CameraInfo, Resolution},
};
use std::{
    borrow::{Borrow, Cow},
    convert::TryFrom,
    fmt::{Debug, Display, Formatter},
    ops::Deref,
};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    console::log_1, CanvasRenderingContext2d, Document, Element, HtmlCanvasElement,
    HtmlVideoElement, ImageData, MediaDeviceInfo, MediaDeviceKind, MediaDevices, MediaStream,
    MediaStreamConstraints, MediaStreamTrack, MediaStreamTrackState, Navigator, Node, Window,
};
#[cfg(feature = "output-wgpu")]
use wgpu::{
    Device, Extent3d, ImageCopyTexture, ImageDataLayout, Queue, Texture, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

// why no code completion
// big sadger

// intellij 2021.2 review: i like structure window, 4 pengs / 5 pengs

macro_rules! jsv {
    ($value:expr) => {{
        JsValue::from($value)
    }};
}

macro_rules! obj {
    ($(($key:expr, $value:expr)),+ ) => {{
        use js_sys::{Map, Object};
        use wasm_bindgen::JsValue;

        let map = Map::new();
        $(
            map.set(&jsv!($key), &jsv!($value));
        )+
        Object::from(map)
    }};
    ($object:expr, $(($key:expr, $value:expr)),+ ) => {{
        use js_sys::{Map, Object};
        use wasm_bindgen::JsValue;

        let map = Map::new();
        $(
            map.set(&jsv!($key), &jsv!($value));
        )+
        let o = Object::from(map);
        Object::assign(&$object, &o)
    }};
}

fn window() -> Result<Window, NokhwaError> {
    match web_sys::window() {
        Some(win) => Ok(win),
        None => Err(NokhwaError::StructureError {
            structure: "web_sys Window".to_string(),
            error: "None".to_string(),
        }),
    }
}

fn media_devices(navigator: &Navigator) -> Result<MediaDevices, NokhwaError> {
    match navigator.media_devices() {
        Ok(media) => Ok(media),
        Err(why) => Err(NokhwaError::StructureError {
            structure: "MediaDevices".to_string(),
            error: format!("{why:?}"),
        }),
    }
}

fn document(window: &Window) -> Result<Document, NokhwaError> {
    match window.document() {
        Some(doc) => Ok(doc),
        None => Err(NokhwaError::StructureError {
            structure: "web_sys Document".to_string(),
            error: "None".to_string(),
        }),
    }
}

fn document_select_elem(doc: &Document, element: &str) -> Result<Element, NokhwaError> {
    match doc.get_element_by_id(element) {
        Some(elem) => Ok(elem),
        None => {
            return Err(NokhwaError::StructureError {
                structure: format!("Document {element}"),
                error: "None".to_string(),
            })
        }
    }
}

fn element_cast<T: JsCast, U: JsCast>(from: T, name: &str) -> Result<U, NokhwaError> {
    if !from.has_type::<U>() {
        return Err(NokhwaError::StructureError {
            structure: name.to_string(),
            error: "Cannot Cast - No Subtype".to_string(),
        });
    }

    let casted = match from.dyn_into::<U>() {
        Ok(cast) => cast,
        Err(_) => {
            return Err(NokhwaError::StructureError {
                structure: name.to_string(),
                error: "Casting Error".to_string(),
            });
        }
    };
    Ok(casted)
}

fn element_cast_ref<'a, T: JsCast, U: JsCast>(
    from: &'a T,
    name: &'a str,
) -> Result<&'a U, NokhwaError> {
    if !from.has_type::<U>() {
        return Err(NokhwaError::StructureError {
            structure: name.to_string(),
            error: "Cannot Cast - No Subtype".to_string(),
        });
    }

    match from.dyn_ref::<U>() {
        Some(v_e) => Ok(v_e),
        None => Err(NokhwaError::StructureError {
            structure: name.to_string(),
            error: "Cannot Cast".to_string(),
        }),
    }
}

fn create_element(doc: &Document, element: &str) -> Result<Element, NokhwaError> {
    match Document::create_element(doc, element) {
        // ???? thank you intellij
        Ok(new_element) => Ok(new_element),
        Err(why) => Err(NokhwaError::StructureError {
            structure: "Document Video Element".to_string(),
            error: format!("{:?}", why.as_string()),
        }),
    }
}

fn set_autoplay_inline(element: &Element) -> Result<(), NokhwaError> {
    if let Err(why) = element.set_attribute("autoplay", "autoplay") {
        return Err(NokhwaError::SetPropertyError {
            property: "Video-autoplay".to_string(),
            value: "autoplay".to_string(),
            error: format!("{why:?}"),
        });
    }

    if let Err(why) = element.set_attribute("playsinline", "playsinline") {
        return Err(NokhwaError::SetPropertyError {
            property: "Video-playsinline".to_string(),
            value: "playsinline".to_string(),
            error: format!("{why:?}"),
        });
    }

    Ok(())
}

/// Requests Webcam permissions from the browser using [`MediaDevices::get_user_media()`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaDevices.html#method.get_user_media) [MDN](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia)
/// # Errors
/// This will error if there is no valid web context or the web API is not supported
pub async fn request_permission() -> Result<(), NokhwaError> {
    let window: Window = window()?;
    let navigator = window.navigator();
    let media_devices = media_devices(&navigator)?;

    match media_devices.get_user_media_with_constraints(
        MediaStreamConstraints::new()
            .video(&JsValue::from_bool(true))
            .audio(&JsValue::from_bool(false)),
    ) {
        Ok(promise) => {
            let js_future = JsFuture::from(promise);
            match js_future.await {
                Ok(stream) => {
                    let media_stream = MediaStream::from(stream);
                    media_stream
                        .get_tracks()
                        .iter()
                        .for_each(|track| MediaStreamTrack::from(track).stop());
                    Ok(())
                }
                Err(why) => Err(NokhwaError::OpenStreamError(format!("{why:?}"))),
            }
        }
        Err(why) => Err(NokhwaError::StructureError {
            structure: "UserMediaPermission".to_string(),
            error: format!("{why:?}"),
        }),
    }
}

/// Requests Webcam permissions from the browser using [`MediaDevices::get_user_media()`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaDevices.html#method.get_user_media) [MDN](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia)
/// # Errors
/// This will error if there is no valid web context or the web API is not supported
/// # JS-WASM
/// In exported JS bindings, the name of the function is `requestPermissions`. It may throw an exception.
#[cfg(feature = "output-wasm")]
#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = requestPermissions))]
pub async fn js_request_permission() -> Result<(), JsValue> {
    if let Err(why) = request_permission().await {
        return Err(JsValue::from(why.to_string()));
    }
    Ok(())
}

/// Queries Cameras using [`MediaDevices::enumerate_devices()`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaDevices.html#method.enumerate_devices) [MDN](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/enumerateDevices)
/// # Errors
/// This will error if there is no valid web context or the web API is not supported
pub async fn query_js_cameras() -> Result<Vec<CameraInfo>, NokhwaError> {
    let window: Window = window()?;
    let navigator = window.navigator();
    let media_devices = media_devices(&navigator)?;

    match media_devices.enumerate_devices() {
        Ok(prom) => {
            let prom: Promise = prom;
            let future = JsFuture::from(prom);
            match future.await {
                Ok(v) => {
                    let array: Array = Array::from(&v);
                    let mut device_list = vec![];
                    request_permission().await.unwrap_or(()); // swallow errors
                    for idx_device in 0_u32..array.length() {
                        if MediaDeviceInfo::instanceof(&array.get(idx_device)) {
                            let media_device_info =
                                MediaDeviceInfo::unchecked_from_js(array.get(idx_device));

                            if media_device_info.kind() == MediaDeviceKind::Videoinput {
                                match media_devices.get_user_media_with_constraints(
                                    MediaStreamConstraints::new()
                                        .audio(&jsv!(false))
                                        .video(&jsv!(obj!((
                                            "deviceId",
                                            media_device_info.device_id()
                                        )))),
                                ) {
                                    Ok(promised_stream) => {
                                        let future_stream = JsFuture::from(promised_stream);
                                        if let Ok(stream) = future_stream.await {
                                            let stream = MediaStream::from(stream);
                                            let tracks = stream.get_video_tracks();
                                            let first = tracks.get(0);
                                            let name = if first.is_undefined() {
                                                format!(
                                                    "{:?}#{}",
                                                    media_device_info.kind(),
                                                    idx_device
                                                )
                                            } else {
                                                MediaStreamTrack::from(first).label()
                                            };
                                            device_list.push(CameraInfo::new(
                                                &name,
                                                &format!("{:?}", media_device_info.kind()),
                                                &format!(
                                                    "{} {}",
                                                    media_device_info.group_id(),
                                                    media_device_info.device_id()
                                                ),
                                                CameraIndex::String(format!(
                                                    "{} {}",
                                                    media_device_info.group_id(),
                                                    media_device_info.device_id()
                                                )),
                                            ));
                                            tracks
                                                .iter()
                                                .for_each(|t| MediaStreamTrack::from(t).stop());
                                        }
                                    }
                                    Err(_) => {
                                        device_list.push(CameraInfo::new(
                                            &format!(
                                                "{:?}#{}",
                                                media_device_info.kind(),
                                                idx_device
                                            ),
                                            &format!("{:?}", media_device_info.kind()),
                                            &format!(
                                                "{} {}",
                                                media_device_info.group_id(),
                                                media_device_info.device_id()
                                            ),
                                            CameraIndex::String(format!(
                                                "{} {}",
                                                media_device_info.group_id(),
                                                media_device_info.device_id()
                                            )),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Ok(device_list)
                }
                Err(why) => Err(NokhwaError::StructureError {
                    structure: "EnumerateDevicesFuture".to_string(),
                    error: format!("{why:?}"),
                }),
            }
        }
        Err(why) => Err(NokhwaError::StructureError {
            structure: "EnumerateDevices".to_string(),
            error: format!("{why:?}"),
        }),
    }
}

/// Queries Cameras using [`MediaDevices::enumerate_devices()`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaDevices.html#method.enumerate_devices) [MDN](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/enumerateDevices)
/// # Errors
/// This will error if there is no valid web context or the web API is not supported
/// # JS-WASM
/// This is exported as `queryCameras`. It may throw an exception.
#[cfg(feature = "output-wasm")]
#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = queryCameras))]
pub async fn js_query_js_cameras() -> Result<Array, JsValue> {
    match query_js_cameras().await {
        Ok(cameras) => Ok(cameras.into_iter().map(JsValue::from).collect()),
        Err(why) => Err(JsValue::from(why.to_string())),
    }
}

/// Queries the browser's supported constraints using [`navigator.mediaDevices.getSupportedConstraints()`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getSupportedConstraints)
/// # Errors
/// This will error if there is no valid web context or the web API is not supported
pub fn query_supported_constraints() -> Result<Vec<JSCameraSupportedCapabilities>, NokhwaError> {
    let window: Window = window()?;
    let navigator = window.navigator();
    let media_devices = media_devices(&navigator)?;

    let supported_constraints = JsValue::from(media_devices.get_supported_constraints());
    let dict_supported_constraints = Object::from(supported_constraints);

    let mut capabilities_vec = vec![];
    for constraint in Object::keys(&dict_supported_constraints).iter() {
        let constraint_str = JsValue::from(JsString::from(constraint))
            .as_string()
            .unwrap_or_default();

        // swallow errors
        if let Ok(cap) = JSCameraSupportedCapabilities::try_from(constraint_str) {
            capabilities_vec.push(cap);
        }
    }
    Ok(capabilities_vec)
}

/// Queries the browser's supported constraints using [`navigator.mediaDevices.getSupportedConstraints()`](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getSupportedConstraints)
/// # Errors
/// This will error if there is no valid web context or the web API is not supported
/// # JS-WASM
/// This is exported as `queryConstraints` and returns an array of strings.
#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = queryConstraints))]
pub fn query_supported_constraints_js() -> Result<Array, JsValue> {
    match query_supported_constraints() {
        Ok(constraints) => Ok(constraints
            .into_iter()
            .map(|c| JsValue::from(c.to_string()))
            .collect()),
        Err(why) => Err(JsValue::from(why.to_string())),
    }
}

/// The enum describing the possible constraints for video in the browser.
/// - `DeviceID`: The ID of the device
/// - `GroupID`: The ID of the group that the device is in
/// - `AspectRatio`: The Aspect Ratio of the final stream
/// - `FacingMode`: What direction the camera is facing. This is more common on mobile. See [`JSCameraFacingMode`]
/// - `FrameRate`: The Frame Rate of the final stream
/// - `Height`: The height of the final stream in pixels
/// - `Width`: The width of the final stream in pixels
/// - `ResizeMode`: Whether the client can crop and/or scale the stream to match the resolution (width, height). See [`JSCameraResizeMode`]
/// See More: [`MediaTrackConstraints`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints) [`Capabilities, constraints, and settings`](https://developer.mozilla.org/en-US/docs/Web/API/Media_Streams_API/Constraints)
/// # JS-WASM
/// This is exported as `CameraSupportedCapabilities`.
#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = CameraSupportedCapabilities))]
#[derive(Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum JSCameraSupportedCapabilities {
    DeviceID,
    GroupID,
    AspectRatio,
    FacingMode,
    FrameRate,
    Height,
    Width,
    ResizeMode,
}

impl Display for JSCameraSupportedCapabilities {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let cap = match self {
            JSCameraSupportedCapabilities::DeviceID => "deviceId",
            JSCameraSupportedCapabilities::GroupID => "groupId",
            JSCameraSupportedCapabilities::AspectRatio => "aspectRatio",
            JSCameraSupportedCapabilities::FacingMode => "facingMode",
            JSCameraSupportedCapabilities::FrameRate => "frameRate",
            JSCameraSupportedCapabilities::Height => "height",
            JSCameraSupportedCapabilities::Width => "width",
            JSCameraSupportedCapabilities::ResizeMode => "resizeMode",
        };

        write!(f, "{cap}")
    }
}

impl Debug for JSCameraSupportedCapabilities {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = self.to_string();
        write!(f, "{str}")
    }
}

impl TryFrom<String> for JSCameraSupportedCapabilities {
    type Error = NokhwaError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value = value.as_str();
        let result = match value {
            "deviceId" => JSCameraSupportedCapabilities::DeviceID,
            "groupId" => JSCameraSupportedCapabilities::GroupID,
            "aspectRatio" => JSCameraSupportedCapabilities::AspectRatio,
            "facingMode" => JSCameraSupportedCapabilities::FacingMode,
            "frameRate" => JSCameraSupportedCapabilities::FrameRate,
            "height" => JSCameraSupportedCapabilities::Height,
            "width" => JSCameraSupportedCapabilities::Width,
            "resizeMode" => JSCameraSupportedCapabilities::ResizeMode,
            _ => {
                return Err(NokhwaError::StructureError {
                    structure: "JSCameraSupportedCapabilities".to_string(),
                    error: "No Match Str".to_string(),
                })
            }
        };
        Ok(result)
    }
}

/// The Facing Mode of the camera
/// - Any: Make no particular choice.
/// - Environment: The camera that shows the user's environment, such as the back camera of a smartphone
/// - User: The camera that shows the user, such as the front camera of a smartphone
/// - Left: The camera that shows the user but to their left, such as a camera that shows a user but to their left shoulder
/// - Right: The camera that shows the user but to their right, such as a camera that shows a user but to their right shoulder
/// See More: [`facingMode`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/facingMode)
/// # JS-WASM
/// This is exported as `CameraFacingMode`.
#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = CameraFacingMode))]
#[derive(Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum JSCameraFacingMode {
    Any,
    Environment,
    User,
    Left,
    Right,
}

impl Display for JSCameraFacingMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let cap = match self {
            JSCameraFacingMode::Environment => "environment",
            JSCameraFacingMode::User => "user",
            JSCameraFacingMode::Left => "left",
            JSCameraFacingMode::Right => "right",
            JSCameraFacingMode::Any => "any",
        };
        write!(f, "{cap}")
    }
}

impl Debug for JSCameraFacingMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = self.to_string();
        write!(f, "{str}")
    }
}

/// Whether the browser can crop and/or scale to match the requested resolution.
/// - `Any`: Make no particular choice.
/// - `None`: Do not crop and/or scale.
/// - `CropAndScale`: Crop and/or scale to match the requested resolution.
/// See More: [`resizeMode`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints#resizemode)
/// # JS-WASM
/// This is exported as `CameraResizeMode`.
#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = CameraResizeMode))]
#[derive(Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum JSCameraResizeMode {
    Any,
    None,
    CropAndScale,
}

impl Display for JSCameraResizeMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let cap = match self {
            JSCameraResizeMode::None => "none",
            JSCameraResizeMode::CropAndScale => "crop-and-scale",
            JSCameraResizeMode::Any => "",
        };

        write!(f, "{cap}")
    }
}

impl Debug for JSCameraResizeMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = self.to_string();
        write!(f, "{str}")
    }
}

/// A builder that builds a [`JSCameraConstraints`] that is used to construct a [`JSCamera`].
/// See More: [`Constraints MDN`](https://developer.mozilla.org/en-US/docs/Web/API/Media_Streams_API/Constraints), [`Properties of Media Tracks MDN`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints)
/// # JS-WASM
/// This is exported as `CameraConstraintsBuilder`.
#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = CameraConstraintsBuilder))]
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct JSCameraConstraintsBuilder {
    pub(crate) min_resolution: Option<Resolution>,
    pub(crate) preferred_resolution: Resolution,
    pub(crate) max_resolution: Option<Resolution>,
    pub(crate) resolution_exact: bool,
    pub(crate) min_aspect_ratio: Option<f64>,
    pub(crate) aspect_ratio: f64,
    pub(crate) max_aspect_ratio: Option<f64>,
    pub(crate) aspect_ratio_exact: bool,
    pub(crate) facing_mode: JSCameraFacingMode,
    pub(crate) facing_mode_exact: bool,
    pub(crate) min_frame_rate: Option<u32>,
    pub(crate) frame_rate: u32,
    pub(crate) max_frame_rate: Option<u32>,
    pub(crate) frame_rate_exact: bool,
    pub(crate) resize_mode: JSCameraResizeMode,
    pub(crate) resize_mode_exact: bool,
    pub(crate) device_id: String,
    pub(crate) device_id_exact: bool,
    pub(crate) group_id: String,
    pub(crate) group_id_exact: bool,
}

#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_class = CameraConstraintsBuilder))]
impl JSCameraConstraintsBuilder {
    /// Constructs a default [`JSCameraConstraintsBuilder`].
    /// The constructed default [`JSCameraConstraintsBuilder`] has these settings:
    /// - 480x234 min, 640x360 ideal, 1920x1080 max
    /// - 10 FPS min, 15 FPS ideal, 30 FPS max
    /// - 1.0 aspect ratio min, 1.77777777778 aspect ratio ideal, 2.0 aspect ratio max
    /// - No `exact`s
    /// # JS-WASM
    /// This is exported as a constructor.
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(constructor))]
    pub fn new() -> Self {
        JSCameraConstraintsBuilder::default()
    }

    /// Sets the minimum resolution for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`width`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/width) and [`height`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/height).
    /// # JS-WASM
    /// This is exported as `set_MinResolution`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = MinResolution)
    )]
    pub fn min_resolution(mut self, min_resolution: Resolution) -> JSCameraConstraintsBuilder {
        self.min_resolution = Some(min_resolution);
        self
    }

    /// Sets the preferred resolution for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`width`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/width) and [`height`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/height).
    /// # JS-WASM
    /// This is exported as `set_Resolution`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = Resolution)
    )]
    pub fn resolution(mut self, new_resolution: Resolution) -> JSCameraConstraintsBuilder {
        self.preferred_resolution = new_resolution;
        self
    }

    /// Sets the maximum resolution for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`width`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/width) and [`height`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/height).
    /// # JS-WASM
    /// This is exported as `set_MaxResolution`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = MaxResolution)
    )]
    pub fn max_resolution(mut self, max_resolution: Resolution) -> JSCameraConstraintsBuilder {
        self.min_resolution = Some(max_resolution);
        self
    }

    /// Sets whether the resolution fields ([`width`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/width), [`height`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/height)/[`resolution`](crate::js_camera::JSCameraConstraintsBuilder::resolution))
    /// should use [`exact`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints#constraints).
    /// Note that this will make the builder ignore [`min_resolution`](crate::js_camera::JSCameraConstraintsBuilder::min_resolution) and [`max_resolution`](crate::js_camera::JSCameraConstraintsBuilder::max_resolution).
    /// # JS-WASM
    /// This is exported as `set_ResolutionExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = ResolutionExact)
    )]
    pub fn resolution_exact(mut self, value: bool) -> JSCameraConstraintsBuilder {
        self.resolution_exact = value;
        self
    }

    /// Sets the minimum aspect ratio of the resulting constraint for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`aspectRatio`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/aspectRatio).
    /// # JS-WASM
    /// This is exported as `set_MinAspectRatio`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = MinAspectRatio)
    )]
    pub fn min_aspect_ratio(mut self, ratio: f64) -> JSCameraConstraintsBuilder {
        self.min_aspect_ratio = Some(ratio);
        self
    }

    /// Sets the aspect ratio of the resulting constraint for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`aspectRatio`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/aspectRatio).
    /// # JS-WASM
    /// This is exported as `set_AspectRatio`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = AspectRatio)
    )]
    pub fn aspect_ratio(mut self, ratio: f64) -> JSCameraConstraintsBuilder {
        self.aspect_ratio = ratio;
        self
    }

    /// Sets the maximum aspect ratio of the resulting constraint for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`aspectRatio`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/aspectRatio).
    /// # JS-WASM
    /// This is exported as `set_MaxAspectRatio`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = MaxAspectRatio)
    )]
    pub fn max_aspect_ratio(mut self, ratio: f64) -> JSCameraConstraintsBuilder {
        self.max_aspect_ratio = Some(ratio);
        self
    }

    /// Sets whether the [`aspect_ratio`](crate::js_camera::JSCameraConstraintsBuilder::aspect_ratio) field should use [`exact`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints#constraints).
    /// Note that this will make the builder ignore [`min_aspect_ratio`](crate::js_camera::JSCameraConstraintsBuilder::min_aspect_ratio) and [`max_aspect_ratio`](crate::js_camera::JSCameraConstraintsBuilder::max_aspect_ratio).
    /// # JS-WASM
    /// This is exported as `set_AspectRatioExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = AspectRatioExact)
    )]
    pub fn aspect_ratio_exact(mut self, value: bool) -> JSCameraConstraintsBuilder {
        self.aspect_ratio_exact = value;
        self
    }

    /// Sets the facing mode of the resulting constraint for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`facingMode`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/facingMode).
    /// # JS-WASM
    /// This is exported as `set_FacingMode`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = FacingMode)
    )]
    pub fn facing_mode(mut self, facing_mode: JSCameraFacingMode) -> JSCameraConstraintsBuilder {
        self.facing_mode = facing_mode;
        self
    }

    /// Sets whether the [`facing_mode`](crate::js_camera::JSCameraConstraintsBuilder::facing_mode) field should use [`exact`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints#constraints).
    /// # JS-WASM
    /// This is exported as `set_FacingModeExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = FacingModeExact)
    )]
    pub fn facing_mode_exact(mut self, value: bool) -> JSCameraConstraintsBuilder {
        self.facing_mode_exact = value;
        self
    }

    /// Sets the minimum frame rate of the resulting constraint for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`frameRate`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/frameRate).
    /// # JS-WASM
    /// This is exported as `set_MinFrameRate`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = MinFrameRate)
    )]
    pub fn min_frame_rate(mut self, fps: u32) -> JSCameraConstraintsBuilder {
        self.min_frame_rate = Some(fps);
        self
    }

    /// Sets the frame rate of the resulting constraint for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`frameRate`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/frameRate).
    /// # JS-WASM
    /// This is exported as `set_FrameRate`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = FrameRate)
    )]
    pub fn frame_rate(mut self, fps: u32) -> JSCameraConstraintsBuilder {
        self.frame_rate = fps;
        self
    }

    /// Sets the maximum frame rate of the resulting constraint for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`frameRate`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/frameRate).
    /// # JS-WASM
    /// This is exported as `set_MaxFrameRate`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = MaxFrameRate)
    )]
    pub fn max_frame_rate(mut self, fps: u32) -> JSCameraConstraintsBuilder {
        self.max_frame_rate = Some(fps);
        self
    }

    /// Sets whether the [`frame_rate`](crate::js_camera::JSCameraConstraintsBuilder::frame_rate) field should use [`exact`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints#constraints).
    /// Note that this will make the builder ignore [`min_frame_rate`](crate::js_camera::JSCameraConstraintsBuilder::min_frame_rate) and [`max_frame_rate`](crate::js_camera::JSCameraConstraintsBuilder::max_frame_rate).
    /// # JS-WASM
    /// This is exported as `set_FrameRateExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = FrameRateExact)
    )]
    pub fn frame_rate_exact(mut self, value: bool) -> JSCameraConstraintsBuilder {
        self.frame_rate_exact = value;
        self
    }

    /// Sets the resize mode of the resulting constraint for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`resizeMode`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints#resizemode).
    /// # JS-WASM
    /// This is exported as `set_ResizeMode`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = ResizeMode)
    )]
    pub fn resize_mode(mut self, resize_mode: JSCameraResizeMode) -> JSCameraConstraintsBuilder {
        self.resize_mode = resize_mode;
        self
    }

    /// Sets whether the [`resize_mode`](crate::js_camera::JSCameraConstraintsBuilder::resize_mode) field should use [`exact`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints#constraints).
    /// # JS-WASM
    /// This is exported as `set_ResizeModeExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = ResizeModeExact)
    )]
    pub fn resize_mode_exact(mut self, value: bool) -> JSCameraConstraintsBuilder {
        self.resize_mode_exact = value;
        self
    }

    /// Sets the device ID of the resulting constraint for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`deviceId`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/deviceId).
    /// # JS-WASM
    /// This is exported as `set_DeviceId`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = DeviceId)
    )]
    pub fn device_id(mut self, id: &str) -> JSCameraConstraintsBuilder {
        self.device_id = id.to_string();
        self
    }

    /// Sets whether the [`device_id`](crate::js_camera::JSCameraConstraintsBuilder::device_id) field should use [`exact`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints#constraints).
    /// # JS-WASM
    /// This is exported as `set_DeviceIdExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = DeviceIdExact)
    )]
    pub fn device_id_exact(mut self, value: bool) -> JSCameraConstraintsBuilder {
        self.device_id_exact = value;
        self
    }

    /// Sets the group ID of the resulting constraint for the [`JSCameraConstraintsBuilder`].
    ///
    /// Sets [`groupId`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints/groupId).
    /// # JS-WASM
    /// This is exported as `set_GroupId`.
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = GroupId))]
    pub fn group_id(mut self, id: &str) -> JSCameraConstraintsBuilder {
        self.group_id = id.to_string();
        self
    }

    /// Sets whether the [`group_id`](crate::js_camera::JSCameraConstraintsBuilder::group_id) field should use [`exact`](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints#constraints).
    /// # JS-WASM
    /// This is exported as `set_GroupIdExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = GroupIdExact)
    )]
    pub fn group_id_exact(mut self, value: bool) -> JSCameraConstraintsBuilder {
        self.group_id_exact = value;
        self
    }

    /// Builds the [`JSCameraConstraints`]. Wrapper for [`build`](crate::js_camera::JSCameraConstraintsBuilder::build)
    ///
    /// Fields that use exact are marked `exact`, otherwise are marked with `ideal`. If min-max are involved, they will use `min` and `max` accordingly.
    /// # JS-WASM
    /// This is exported as `buildCameraConstraints`.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(js_name = buildCameraConstraints)
    )]
    #[must_use]
    pub fn js_build(self) -> JSCameraConstraints {
        self.build()
    }
}

impl JSCameraConstraintsBuilder {
    /// Builds the [`JSCameraConstraints`]
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn build(self) -> JSCameraConstraints {
        let null_resolution = Resolution::default();
        let null_string = String::new();

        let mut video_object = Object::new();

        // width
        if self.resolution_exact {
            if self.preferred_resolution != null_resolution {
                video_object = obj!(
                    video_object,
                    ("width", obj!(("exact", self.preferred_resolution.width())))
                );
            }
        } else {
            let mut width_object = Object::new();

            if let Some(min_res) = self.min_resolution {
                width_object = obj!(width_object, ("min", min_res.width()));
            }

            width_object = obj!(width_object, ("ideal", self.preferred_resolution.width()));
            if let Some(max_res) = self.max_resolution {
                width_object = obj!(width_object, ("max", max_res.width()));
            }

            video_object = obj!(video_object, ("width", width_object));
        }

        // height
        if self.resolution_exact {
            if self.preferred_resolution != null_resolution {
                video_object = obj!(
                    video_object,
                    (
                        "height",
                        obj!(("exact", self.preferred_resolution.height()))
                    )
                );
            }
        } else {
            let mut height_object = Object::new();

            if let Some(min_res) = self.min_resolution {
                height_object = obj!(height_object, ("min", min_res.height()));
            }

            height_object = obj!(height_object, ("ideal", self.preferred_resolution.height()));
            if let Some(max_res) = self.max_resolution {
                height_object = obj!(height_object, ("max", max_res.height()));
            }

            video_object = obj!(video_object, ("height", height_object));
        }

        // aspect ratio
        if self.aspect_ratio_exact {
            if self.aspect_ratio != 0_f64 {
                video_object = obj!(
                    video_object,
                    ("aspectRatio", obj!(("exact", self.aspect_ratio)))
                );
            }
        } else {
            let mut aspect_ratio_object = Object::new();

            if let Some(min_ratio) = self.min_aspect_ratio {
                aspect_ratio_object = obj!(aspect_ratio_object, ("min", min_ratio));
            }

            aspect_ratio_object = obj!(aspect_ratio_object, ("ideal", self.aspect_ratio));
            if let Some(max_ratio) = self.max_aspect_ratio {
                aspect_ratio_object = obj!(aspect_ratio_object, ("max", max_ratio));
            }

            video_object = obj!(video_object, ("aspectRatio", aspect_ratio_object));
        }

        if self.facing_mode != JSCameraFacingMode::Any && self.facing_mode_exact {
            video_object = obj!(
                video_object,
                ("facingMode", obj!(("exact", self.facing_mode.to_string())))
            );
        } else if self.facing_mode != JSCameraFacingMode::Any {
            video_object = obj!(
                video_object,
                ("facingMode", obj!(("ideal", self.facing_mode.to_string())))
            );
        }

        // aspect ratio
        if self.frame_rate_exact {
            if self.frame_rate != 0 {
                video_object = obj!(
                    video_object,
                    ("frameRate", obj!(("exact", self.frame_rate)))
                );
            }
        } else {
            let mut frame_rate_object = Object::new();

            if let Some(min_frame_rate) = self.min_frame_rate {
                frame_rate_object = obj!(frame_rate_object, ("min", min_frame_rate));
            }

            frame_rate_object = obj!(frame_rate_object, ("ideal", self.frame_rate));
            if let Some(max_frame_rate) = self.max_frame_rate {
                frame_rate_object = obj!(frame_rate_object, ("max", max_frame_rate));
            }

            video_object = obj!(video_object, ("frameRate", frame_rate_object));
        }

        if self.resize_mode != JSCameraResizeMode::Any && self.resize_mode_exact {
            video_object = obj!(
                video_object,
                ("resizeMode", obj!(("exact", self.resize_mode.to_string())))
            );
        } else if self.resize_mode != JSCameraResizeMode::Any {
            video_object = obj!(
                video_object,
                ("resizeMode", obj!(("ideal", self.resize_mode.to_string())))
            );
        }

        if self.device_id != null_string && self.device_id_exact {
            video_object = obj!(video_object, ("deviceId", obj!(("exact", &self.device_id))));
        } else if self.device_id != null_string {
            video_object = obj!(video_object, ("deviceId", obj!(("ideal", &self.device_id))));
        }

        if self.group_id != null_string && self.group_id_exact {
            video_object = obj!(video_object, ("groupId", obj!(("exact", &self.group_id))));
        } else if self.group_id != null_string {
            video_object = obj!(video_object, ("groupId", obj!(("ideal", &self.group_id))));
        }

        let media_stream_constraints = MediaStreamConstraints::new()
            .audio(&jsv!(false))
            .video(&jsv!(video_object))
            .clone();

        JSCameraConstraints {
            media_constraints: media_stream_constraints,
            min_resolution: self.min_resolution,
            preferred_resolution: self.preferred_resolution,
            max_resolution: self.max_resolution,
            resolution_exact: self.resolution_exact,
            min_aspect_ratio: self.min_aspect_ratio,
            aspect_ratio: self.aspect_ratio,
            max_aspect_ratio: self.max_aspect_ratio,
            aspect_ratio_exact: self.aspect_ratio_exact,
            facing_mode: self.facing_mode,
            facing_mode_exact: self.facing_mode_exact,
            min_frame_rate: self.min_frame_rate,
            frame_rate: self.frame_rate,
            max_frame_rate: self.max_frame_rate,
            frame_rate_exact: self.frame_rate_exact,
            resize_mode: self.resize_mode,
            resize_mode_exact: self.resize_mode_exact,
            device_id: self.device_id,
            device_id_exact: self.device_id_exact,
            group_id: self.group_id,
            group_id_exact: self.device_id_exact,
        }
    }
}

impl Default for JSCameraConstraintsBuilder {
    fn default() -> Self {
        JSCameraConstraintsBuilder {
            min_resolution: Some(Resolution::new(480, 234)),
            preferred_resolution: Resolution::new(640, 360),
            max_resolution: Some(Resolution::new(1920, 1080)),
            resolution_exact: false,
            min_aspect_ratio: Some(1_f64),
            aspect_ratio: 1.777_777_777_78_f64,
            max_aspect_ratio: Some(2_f64),
            aspect_ratio_exact: false,
            facing_mode: JSCameraFacingMode::Any,
            facing_mode_exact: false,
            min_frame_rate: Some(10),
            frame_rate: 15,
            max_frame_rate: Some(30),
            frame_rate_exact: false,
            resize_mode: JSCameraResizeMode::Any,
            resize_mode_exact: false,
            device_id: String::new(),
            device_id_exact: false,
            group_id: String::new(),
            group_id_exact: false,
        }
    }
}

/// Constraints to create a [`JSCamera`]
///
/// If you want more options, see [`JSCameraConstraintsBuilder`]
/// # JS-WASM
/// This is exported as `CameraConstraints`.
#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = CameraConstraints))]
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct JSCameraConstraints {
    pub(crate) media_constraints: MediaStreamConstraints,
    pub(crate) min_resolution: Option<Resolution>,
    pub(crate) preferred_resolution: Resolution,
    pub(crate) max_resolution: Option<Resolution>,
    pub(crate) resolution_exact: bool,
    pub(crate) min_aspect_ratio: Option<f64>,
    pub(crate) aspect_ratio: f64,
    pub(crate) max_aspect_ratio: Option<f64>,
    pub(crate) aspect_ratio_exact: bool,
    pub(crate) facing_mode: JSCameraFacingMode,
    pub(crate) facing_mode_exact: bool,
    pub(crate) min_frame_rate: Option<u32>,
    pub(crate) frame_rate: u32,
    pub(crate) max_frame_rate: Option<u32>,
    pub(crate) frame_rate_exact: bool,
    pub(crate) resize_mode: JSCameraResizeMode,
    pub(crate) resize_mode_exact: bool,
    pub(crate) device_id: String,
    pub(crate) device_id_exact: bool,
    pub(crate) group_id: String,
    pub(crate) group_id_exact: bool,
}

#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_class = CameraConstraints))]
impl JSCameraConstraints {
    /// Gets the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html)
    /// # JS-WASM
    /// This is exported as `get_MediaStreamConstraints`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = MediaStreamConstraints)
    )]
    pub fn media_constraints(&self) -> MediaStreamConstraints {
        self.media_constraints.clone()
    }

    /// Gets the minimum [`Resolution`].
    /// # JS-WASM
    /// This is exported as `get_MinResolution`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = MinResolution)
    )]
    #[must_use]
    pub fn min_resolution(&self) -> Option<Resolution> {
        self.min_resolution
    }

    /// Gets the minimum [`Resolution`].
    /// # JS-WASM
    /// This is exported as `set_MinResolution`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = MinResolution)
    )]
    pub fn set_min_resolution(&mut self, min_resolution: Resolution) {
        self.min_resolution = Some(min_resolution);
    }

    /// Gets the internal [`Resolution`]
    /// # JS-WASM
    /// This is exported as `get_Resolution`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = Resolution)
    )]
    pub fn resolution(&self) -> Resolution {
        self.preferred_resolution
    }

    /// Sets the internal [`Resolution`]
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_Resolution`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = Resolution)
    )]
    pub fn set_resolution(&mut self, preferred_resolution: Resolution) {
        self.preferred_resolution = preferred_resolution;
    }

    /// Gets the maximum [`Resolution`].
    /// # JS-WASM
    /// This is exported as `get_MaxResolution`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = MaxResolution)
    )]
    #[must_use]
    pub fn max_resolution(&self) -> Option<Resolution> {
        self.max_resolution
    }

    /// Gets the maximum [`Resolution`].
    /// # JS-WASM
    /// This is exported as `set_MaxResolution`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = MaxResolution)
    )]
    pub fn set_max_resolution(&mut self, max_resolution: Resolution) {
        self.max_resolution = Some(max_resolution);
    }

    /// Gets the internal resolution exact.
    /// # JS-WASM
    /// This is exported as `get_ResolutionExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = ResolutionExact)
    )]
    pub fn resolution_exact(&self) -> bool {
        self.resolution_exact
    }

    /// Sets the internal resolution exact.
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_ResolutionExact`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = ResolutionExact)
    )]
    pub fn set_resolution_exact(&mut self, resolution_exact: bool) {
        self.resolution_exact = resolution_exact;
    }

    /// Gets the minimum aspect ratio of the [`JSCameraConstraints`].
    /// # JS-WASM
    /// This is exported as `get_MinAspectRatio`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = MinAspectRatio)
    )]
    pub fn min_aspect_ratio(&self) -> Option<f64> {
        self.min_aspect_ratio
    }

    /// Sets the minimum aspect ratio of the [`JSCameraConstraints`].
    /// # JS-WASM
    /// This is exported as `set_MinAspectRatio`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = MinAspectRatio)
    )]
    pub fn set_min_aspect_ratio(&mut self, min_aspect_ratio: f64) {
        self.min_aspect_ratio = Some(min_aspect_ratio);
    }

    /// Gets the internal aspect ratio.
    /// # JS-WASM
    /// This is exported as `get_AspectRatio`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = AspectRatio)
    )]
    pub fn aspect_ratio(&self) -> f64 {
        self.aspect_ratio
    }

    /// Sets the internal aspect ratio.
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_AspectRatio`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = AspectRatio)
    )]
    pub fn set_aspect_ratio(&mut self, aspect_ratio: f64) {
        self.aspect_ratio = aspect_ratio;
    }

    /// Gets the maximum aspect ratio.
    /// # JS-WASM
    /// This is exported as `get_MaxAspectRatio`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = MaxAspectRatio)
    )]
    #[must_use]
    pub fn max_aspect_ratio(&self) -> Option<f64> {
        self.max_aspect_ratio
    }

    /// Sets the maximum internal aspect ratio.
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_MaxAspectRatio`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = MaxAspectRatio)
    )]
    pub fn set_max_aspect_ratio(&mut self, max_aspect_ratio: f64) {
        self.max_aspect_ratio = Some(max_aspect_ratio);
    }

    /// Gets the internal aspect ratio exact.
    /// # JS-WASM
    /// This is exported as `get_AspectRatioExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = AspectRatioExact)
    )]
    pub fn aspect_ratio_exact(&self) -> bool {
        self.aspect_ratio_exact
    }

    /// Sets the internal aspect ratio exact.
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_AspectRatioExact`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = AspectRatioExact)
    )]
    pub fn set_aspect_ratio_exact(&mut self, aspect_ratio_exact: bool) {
        self.aspect_ratio_exact = aspect_ratio_exact;
    }

    /// Gets the internal [`JSCameraFacingMode`].
    /// # JS-WASM
    /// This is exported as `get_FacingMode`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = FacingMode)
    )]
    pub fn facing_mode(&self) -> JSCameraFacingMode {
        self.facing_mode
    }

    /// Sets the internal [`JSCameraFacingMode`]
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_FacingMode`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = FacingMode)
    )]
    pub fn set_facing_mode(&mut self, facing_mode: JSCameraFacingMode) {
        self.facing_mode = facing_mode;
    }

    /// Gets the internal facing mode exact.
    /// # JS-WASM
    /// This is exported as `get_FacingModeExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = FacingModeExact)
    )]
    pub fn facing_mode_exact(&self) -> bool {
        self.facing_mode_exact
    }

    /// Sets the internal facing mode exact
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_FacingModeExact`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = FacingModeExact)
    )]
    pub fn set_facing_mode_exact(&mut self, facing_mode_exact: bool) {
        self.facing_mode_exact = facing_mode_exact;
    }

    /// Gets the minimum internal frame rate.
    /// # JS-WASM
    /// This is exported as `get_MinFrameRate`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = MinFrameRate)
    )]
    #[must_use]
    pub fn min_frame_rate(&self) -> Option<u32> {
        self.min_frame_rate
    }

    /// Sets the minimum internal frame rate
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_MinFrameRate`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = MinFrameRate)
    )]
    pub fn set_min_frame_rate(&mut self, min_frame_rate: u32) {
        self.min_frame_rate = Some(min_frame_rate);
    }

    /// Gets the internal frame rate.
    /// # JS-WASM
    /// This is exported as `get_FrameRate`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = FrameRate)
    )]
    pub fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    /// Sets the internal frame rate
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_FrameRate`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = FrameRate)
    )]
    pub fn set_frame_rate(&mut self, frame_rate: u32) {
        self.frame_rate = frame_rate;
    }

    /// Gets the maximum internal frame rate.
    /// # JS-WASM
    /// This is exported as `get_MaxFrameRate`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = MaxFrameRate)
    )]
    #[must_use]
    pub fn max_frame_rate(&self) -> Option<u32> {
        self.max_frame_rate
    }

    /// Sets the maximum internal frame rate
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_MaxFrameRate`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = MaxFrameRate)
    )]
    pub fn set_max_frame_rate(&mut self, max_frame_rate: u32) {
        self.max_frame_rate = Some(max_frame_rate);
    }

    /// Gets the internal frame rate exact.
    /// # JS-WASM
    /// This is exported as `get_FrameRateExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = FrameRateExact)
    )]
    pub fn frame_rate_exact(&self) -> bool {
        self.frame_rate_exact
    }

    /// Sets the internal frame rate exact.
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_FrameRateExact`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = FrameRateExact)
    )]
    pub fn set_frame_rate_exact(&mut self, frame_rate_exact: bool) {
        self.frame_rate_exact = frame_rate_exact;
    }

    /// Gets the internal [`JSCameraResizeMode`].
    /// # JS-WASM
    /// This is exported as `get_ResizeMode`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = ResizeMode)
    )]
    pub fn resize_mode(&self) -> JSCameraResizeMode {
        self.resize_mode
    }

    /// Sets the internal [`JSCameraResizeMode`]
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_ResizeMode`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = ResizeMode)
    )]
    pub fn set_resize_mode(&mut self, resize_mode: JSCameraResizeMode) {
        self.resize_mode = resize_mode;
    }

    /// Gets the internal resize mode exact.
    /// # JS-WASM
    /// This is exported as `get_ResizeModeExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = ResizeModeExact)
    )]
    pub fn resize_mode_exact(&self) -> bool {
        self.resize_mode_exact
    }

    /// Sets the internal resize mode exact.
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_ResizeModeExact`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = ResizeModeExact)
    )]
    pub fn set_resize_mode_exact(&mut self, resize_mode_exact: bool) {
        self.resize_mode_exact = resize_mode_exact;
    }

    /// Gets the internal device id.
    /// # JS-WASM
    /// This is exported as `get_DeviceId`.
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(getter = DeviceId))]
    pub fn device_id(&self) -> String {
        self.device_id.to_string()
    }

    /// Sets the internal device ID.
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_DeviceId`.
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(setter = DeviceId))]
    pub fn set_device_id(&mut self, device_id: String) {
        self.device_id = device_id;
    }

    /// Gets the internal device id exact.
    /// # JS-WASM
    /// This is exported as `get_DeviceIdExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = DeviceIdExact)
    )]
    pub fn device_id_exact(&self) -> bool {
        self.device_id_exact
    }

    /// Sets the internal device ID exact.
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_DeviceIdExact`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = DeviceIdExact)
    )]
    pub fn set_device_id_exact(&mut self, device_id_exact: bool) {
        self.device_id_exact = device_id_exact;
    }

    /// Gets the internal group id.
    /// # JS-WASM
    /// This is exported as `get_GroupId`.
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(getter = GroupId))]
    pub fn group_id(&self) -> String {
        self.group_id.to_string()
    }

    /// Sets the internal group ID.
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_GroupId`.
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(setter = GroupId))]
    pub fn set_group_id(&mut self, group_id: String) {
        self.group_id = group_id;
    }

    /// Gets the internal group id exact.
    /// # JS-WASM
    /// This is exported as `get_GroupIdExact`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = GroupIdExact)
    )]
    pub fn group_id_exact(&self) -> bool {
        self.group_id_exact
    }

    /// Sets the internal group ID exact.
    /// Note that this doesn't affect the internal [`MediaStreamConstraints`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStreamConstraints.html) until you call
    /// [`apply_constraints()`](crate::js_camera::JSCameraConstraints::apply_constraints)
    /// # JS-WASM
    /// This is exported as `set_GroupIdExact`.
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = GroupIdExact)
    )]
    pub fn set_group_id_exact(&mut self, group_id_exact: bool) {
        self.group_id_exact = group_id_exact;
    }

    /// Applies any modified constraints.
    /// # JS-WASM
    /// This is exported as `applyConstraints`.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = applyConstraints))]
    pub fn js_apply_constraints(&mut self) {
        self.apply_constraints();
    }
}

impl JSCameraConstraints {
    /// Applies any modified constraints.
    pub fn apply_constraints(&mut self) {
        let new_constraints = JSCameraConstraintsBuilder {
            min_resolution: self.min_resolution(),
            preferred_resolution: self.resolution(),
            max_resolution: self.max_resolution(),
            resolution_exact: self.resolution_exact(),
            min_aspect_ratio: self.min_aspect_ratio(),
            aspect_ratio: self.aspect_ratio(),
            max_aspect_ratio: self.max_aspect_ratio(),
            aspect_ratio_exact: self.aspect_ratio_exact(),
            facing_mode: self.facing_mode(),
            facing_mode_exact: self.facing_mode_exact(),
            min_frame_rate: self.min_frame_rate(),
            frame_rate: self.frame_rate(),
            max_frame_rate: self.max_frame_rate(),
            frame_rate_exact: self.frame_rate_exact(),
            resize_mode: self.resize_mode(),
            resize_mode_exact: self.resize_mode_exact(),
            device_id: self.device_id(),
            device_id_exact: self.device_id_exact(),
            group_id: self.group_id(),
            group_id_exact: self.group_id_exact(),
        }
        .build();

        self.media_constraints = new_constraints.media_constraints;
    }
}

impl Deref for JSCameraConstraints {
    type Target = MediaStreamConstraints;

    fn deref(&self) -> &Self::Target {
        &self.media_constraints
    }
}

/// A wrapper around a [`MediaStream`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStream.html)
/// # JS-WASM
/// This is exported as `NokhwaCamera`.
#[cfg(feature = "input-jscam")]
#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = NokhwaCamera))]
#[cfg_attr(feature = "docs-features", doc(cfg(feature = "input-jscam")))]
pub struct JSCamera {
    media_stream: MediaStream,
    constraints: JSCameraConstraints,
    attached: bool,
    attached_node: Option<Node>,
    measured_resolution: Resolution,
    attached_canvas: Option<HtmlCanvasElement>,
    canvas_context: Option<CanvasRenderingContext2d>,
}

#[cfg(feature = "input-jscam")]
#[cfg_attr(feature = "output-wasm", wasm_bindgen(js_class = NokhwaCamera))]
impl JSCamera {
    /// Creates a new [`JSCamera`] using [`JSCameraConstraints`].
    ///
    /// # Errors
    /// This may error if permission is not granted, or the constraints are invalid.
    /// # JS-WASM
    /// This is the constructor for `NokhwaCamera`. It returns a promise and may throw an error.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(constructor))]
    pub async fn js_new(constraints: JSCameraConstraints) -> Result<JSCamera, JsValue> {
        match JSCamera::new(constraints).await {
            Ok(camera) => Ok(camera),
            Err(why) => Err(JsValue::from(why.to_string())),
        }
    }

    /// Gets the internal [`JSCameraConstraints`].
    /// Most likely, you will edit this value by taking ownership of it, then feed it back into [`set_constraints`](crate::js_camera::JSCamera::set_constraints).
    /// # JS-WASM
    /// This is exported as `get_Constraints`.
    #[must_use]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(getter = Constraints))]
    pub fn constraints(&self) -> JSCameraConstraints {
        self.constraints.clone()
    }

    /// Sets the [`JSCameraConstraints`]. This calls [`apply_constraints`](crate::js_camera::JSCamera::apply_constraints) internally.
    ///
    /// # Errors
    /// See [`apply_constraints`](crate::js_camera::JSCamera::apply_constraints).
    /// # JS-WASM
    /// This is exported as `set_Constraints`. It may throw an error.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(setter = Constraints)
    )]
    pub fn js_set_constraints(&mut self, constraints: JSCameraConstraints) -> Result<(), JsValue> {
        match self.set_constraints(constraints) {
            Ok(_) => Ok(()),
            Err(why) => Err(JsValue::from(why.to_string())),
        }
    }

    /// Gets the internal [`Resolution`].
    ///
    /// Note: This value is only updated after you call [`measure_resolution`](crate::js_camera::JSCamera::measure_resolution)
    /// # JS-WASM
    /// This is exported as `get_Resolution`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = Resolution)
    )]
    pub fn resolution(&self) -> Resolution {
        self.measured_resolution
    }

    /// Measures the [`Resolution`] of the internal stream. You usually do not need to call this.
    ///
    /// # Errors
    /// If the camera fails to attach to the created `<video>`, this will error.
    ///
    /// # JS-WASM
    /// This is exported as `measureResolution`. It may throw an error.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(
    feature = "output-wasm",
    wasm_bindgen(js_name = measureResolution)
    )]
    pub fn js_measure_resolution(&mut self) -> Result<(), JsValue> {
        if let Err(why) = self.measure_resolution() {
            return Err(JsValue::from(why.to_string()));
        }
        Ok(())
    }

    /// Applies any modified constraints.
    /// # Errors
    /// This function may return an error on failing to measure the resolution. Please check [`measure_resolution()`](crate::js_camera::JSCamera::measure_resolution) for details.
    /// # JS-WASM
    /// This is exported as `applyConstraints`. It may throw an error.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = applyConstraints))]
    pub fn js_apply_constraints(&mut self) -> Result<(), JsValue> {
        if let Err(why) = self.apply_constraints() {
            return Err(JsValue::from(why.to_string()));
        }
        Ok(())
    }

    /// Gets the internal [`MediaStream`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.MediaStream.html) [`MDN`](https://developer.mozilla.org/en-US/docs/Web/API/MediaStream)
    /// # JS-WASM
    /// This is exported as `MediaStream`.
    #[must_use]
    #[cfg_attr(
        feature = "output-wasm",
        wasm_bindgen(getter = MediaStream)
    )]
    pub fn media_stream(&self) -> MediaStream {
        self.media_stream.clone()
    }

    /// Captures an [`ImageData`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.ImageData.html) [`MDN`](https://developer.mozilla.org/en-US/docs/Web/API/ImageData) by drawing the image to a non-existent canvas.
    ///
    /// # Errors
    /// If drawing to the canvas fails this will error.
    /// # JS-WASM
    /// This is exported as `captureImageData`. It may throw an error.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = captureImageData))]
    pub fn js_frame_image_data(&mut self) -> Result<ImageData, JsValue> {
        match self.frame_image_data() {
            Ok(img) => Ok(img),
            Err(why) => Err(JsValue::from(why.to_string())),
        }
    }

    /// Captures an [`ImageData`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.ImageData.html) [`MDN`](https://developer.mozilla.org/en-US/docs/Web/API/ImageData) and then returns its `URL` as a string.
    /// - `mime_type`: The mime type of the resulting URI. It is `image/png` by default (lossless) but can be set to `image/jpeg` or `image/webp` (lossy). Anything else is ignored.
    /// - `image_quality`: A number between `0` and `1` indicating the resulting image quality in case you are using a lossy image mime type. The default value is 0.92, and all other values are ignored.
    ///
    /// # Errors
    /// If drawing to the canvas fails or URI generation is not supported or fails this will error.
    /// # JS-WASM
    /// This is exported as `captureImageURI`. It may throw an error
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = captureImageURI))]
    pub fn js_frame_image_data_uri(
        &mut self,
        mime_type: &str,
        image_quality: f64,
    ) -> Result<String, JsValue> {
        match self.frame_uri(Some(mime_type), Some(image_quality)) {
            Ok(uri) => Ok(uri),
            Err(why) => Err(JsValue::from(why.to_string())),
        }
    }

    /// Creates an off-screen canvas and a `<video>` element (if not already attached) and returns a raw `Cow<[u8]>` RGBA frame.
    /// # Errors
    /// If a cast fails, the camera fails to attach, the currently attached node is invalid, or writing/reading from the canvas fails, this will error.
    /// # JS-WASM
    /// This is exported as `captureFrameRawData`. This may throw an error.
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = captureFrameRawData))]
    pub fn js_frame_raw(&mut self) -> Result<Box<[u8]>, JsValue> {
        match self.frame_raw() {
            Ok(frame) => Ok(frame.iter().copied().collect()),
            Err(why) => Err(JsValue::from(why.to_string())),
        }
    }

    /// Copies camera frame to a `html_id`(by-id, canvas).
    ///
    /// If `generate_new` is true, the generated element will have an Id of `html_id`+`-canvas`. For example, if you pass "nokhwaisbest" for `html_id`, the new `<canvas>`'s ID will be "nokhwaisbest-canvas".
    /// # Errors
    /// If the internal canvas is not here, drawing fails, or a cast fails, this will error.
    /// # JS-WASM
    /// This is exported as `copyToCanvas`. It may error.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = copyToCanvas))]
    pub fn js_frame_canvas_copy(
        &mut self,
        html_id: &str,
        generate_new: bool,
    ) -> Result<(), JsValue> {
        match self.frame_canvas_copy(html_id, generate_new) {
            Ok(_) => Ok(()),
            Err(why) => Err(JsValue::from(why.to_string())),
        }
    }

    /// Attaches camera to a `html_id`(by-id).
    ///
    /// If `generate_new` is true, the generated element will have an Id of `html_id`+`-video`. For example, if you pass "nokhwaisbest" for `html_id`, the new `<video>`'s ID will be "nokhwaisbest-video".
    /// # Errors
    /// If the camera fails to attach, fails to generate the video element, or a cast fails, this will error.
    /// # JS-WASM
    /// This is exported as `attachToElement`. It may throw an error.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = attachToElement))]
    pub fn js_attach(&mut self, html_id: &str, generate_new: bool) -> Result<(), JsValue> {
        if let Err(why) = self.attach(html_id, generate_new) {
            return Err(JsValue::from(why.to_string()));
        }
        Ok(())
    }

    /// Detaches the camera from the `<video>` node.
    /// # Errors
    /// If the casting fails (the stored node is not a `<video>`) this will error.
    /// # JS-WASM
    /// This is exported as `detachCamera`. This may throw an error.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = detachCamera))]
    pub fn js_detach(&mut self) -> Result<(), JsValue> {
        if let Err(why) = self.detach() {
            return Err(JsValue::from(why.to_string()));
        }
        Ok(())
    }

    /// Stops all streams and detaches the camera.
    /// # Errors
    /// There may be an error while detaching the camera. Please see [`detach()`](crate::js_camera::JSCamera::detach) for more details.
    #[cfg(feature = "output-wasm")]
    #[cfg_attr(feature = "output-wasm", wasm_bindgen(js_name = stopAll))]
    pub fn js_stop_all(&mut self) -> Result<(), JsValue> {
        if let Err(why) = self.stop_all() {
            return Err(JsValue::from(why.to_string()));
        }
        Ok(())
    }
}

impl JSCamera {
    /// Creates a new [`JSCamera`] using [`JSCameraConstraints`].
    ///
    /// # Errors
    /// This may error if permission is not granted, or the constraints are invalid.
    pub async fn new(constraints: JSCameraConstraints) -> Result<Self, NokhwaError> {
        let window: Window = window()?;
        let navigator = window.navigator();
        let media_devices = media_devices(&navigator)?;

        let stream: MediaStream = match media_devices.get_user_media_with_constraints(&constraints)
        {
            Ok(promise) => {
                let future = JsFuture::from(promise);
                match future.await {
                    Ok(stream) => {
                        let media_stream: MediaStream = MediaStream::from(stream);
                        media_stream
                    }
                    Err(why) => {
                        return Err(NokhwaError::StructureError {
                            structure: "MediaDevicesGetUserMediaJsFuture".to_string(),
                            error: format!("{why:?}"),
                        })
                    }
                }
            }
            Err(why) => {
                return Err(NokhwaError::StructureError {
                    structure: "MediaDevicesGetUserMedia".to_string(),
                    error: format!("{why:?}"),
                })
            }
        };

        let mut js_camera = JSCamera {
            media_stream: stream,
            constraints,
            attached: false,
            attached_node: None,
            measured_resolution: Resolution::new(0, 0),
            attached_canvas: None,
            canvas_context: None,
        };
        js_camera.measure_resolution()?;

        Ok(js_camera)
    }

    /// Applies any modified constraints.
    /// # Errors
    /// This function may return an error on failing to measure the resolution. Please check [`measure_resolution()`](crate::js_camera::JSCamera::measure_resolution) for details.
    pub fn apply_constraints(&mut self) -> Result<(), NokhwaError> {
        let new_constraints = JSCameraConstraintsBuilder {
            min_resolution: self.constraints.min_resolution(),
            preferred_resolution: self.constraints.resolution(),
            max_resolution: self.constraints.max_resolution(),
            resolution_exact: self.constraints.resolution_exact(),
            min_aspect_ratio: self.constraints.min_aspect_ratio(),
            aspect_ratio: self.constraints.aspect_ratio(),
            max_aspect_ratio: self.constraints.max_aspect_ratio(),
            aspect_ratio_exact: self.constraints.aspect_ratio_exact(),
            facing_mode: self.constraints.facing_mode(),
            facing_mode_exact: self.constraints.facing_mode_exact(),
            min_frame_rate: self.constraints.min_frame_rate(),
            frame_rate: self.constraints.frame_rate(),
            max_frame_rate: self.constraints.max_frame_rate(),
            frame_rate_exact: self.constraints.frame_rate_exact(),
            resize_mode: self.constraints.resize_mode(),
            resize_mode_exact: self.constraints.resize_mode_exact(),
            device_id: self.constraints.device_id(),
            device_id_exact: self.constraints.device_id_exact(),
            group_id: self.constraints.group_id(),
            group_id_exact: self.constraints.group_id_exact(),
        }
        .build();

        self.constraints.media_constraints = new_constraints.media_constraints;
        self.measure_resolution()?;
        Ok(())
    }

    /// Sets the [`JSCameraConstraints`]. This calls [`apply_constraints`](crate::js_camera::JSCamera::apply_constraints) internally.
    ///
    /// # Errors
    /// See [`apply_constraints`](crate::js_camera::JSCamera::apply_constraints).
    pub fn set_constraints(&mut self, constraints: JSCameraConstraints) -> Result<(), NokhwaError> {
        let current = std::mem::replace(&mut self.constraints, constraints);
        if let Err(why) = self.apply_constraints() {
            self.constraints = current;
            return Err(why);
        }
        Ok(())
    }

    /// Measures the [`Resolution`] of the internal stream. You usually do not need to call this.
    ///
    /// # Errors
    /// If the camera fails to attach to the created `<video>`, this will error.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn measure_resolution(&mut self) -> Result<(), NokhwaError> {
        let stream = self
            .media_stream
            .get_video_tracks()
            .iter()
            .next()
            .unwrap_or_else(JsValue::undefined);
        if !stream.is_undefined() {
            let stream = MediaStreamTrack::from(stream);
            let settings_map = {
                let settings_array = Object::entries(&stream.get_settings());
                let settings_map = Map::new();
                settings_array.iter().for_each(|arr_elem| {
                    let arr_elem = Array::from(&arr_elem);
                    let key = arr_elem.get(0);
                    let value = arr_elem.get(1);
                    if key != JsValue::UNDEFINED && value != JsValue::UNDEFINED {
                        settings_map.set(&key, &value);
                    }
                });
                settings_map
            };

            self.measured_resolution = Resolution::new(
                settings_map.get(&jsv!("width")).as_f64().unwrap_or(0_f64) as u32,
                settings_map.get(&jsv!("height")).as_f64().unwrap_or(0_f64) as u32,
            );
            return Ok(());
        }
        Err(NokhwaError::ReadFrameError("Null Stream".to_string()))
    }

    /// Attaches camera to a `html_id`(by-id).
    ///
    /// If `generate_new` is true, the generated element will have an Id of `html_id`+`-video`. For example, if you pass "nokhwaisbest" for `html_id`, the new `<video>`'s ID will be "nokhwaisbest-video".
    /// # Errors
    /// If the camera fails to attach, fails to generate the video element, or a cast fails, this will error.
    pub fn attach(&mut self, html_id: &str, generate_new: bool) -> Result<(), NokhwaError> {
        let window: Window = window()?;
        let document: Document = document(&window)?;

        let selected_element: Element = document_select_elem(&document, html_id)?;
        self.measure_resolution()?;

        if generate_new {
            let video_element = create_element(&document, "video")?;

            set_autoplay_inline(&video_element)?;

            let video_element: HtmlVideoElement =
                element_cast::<Element, HtmlVideoElement>(video_element, "HtmlVideoElement")?;

            video_element.set_width(self.resolution().width());
            video_element.set_height(self.resolution().height());
            video_element.set_src_object(Some(&self.media_stream()));
            video_element.set_id(&format!("{html_id}-video"));

            return match selected_element.append_child(&Node::from(video_element)) {
                Ok(n) => {
                    self.attached_node = Some(n);
                    self.attached = true;
                    Ok(())
                }
                Err(why) => Err(NokhwaError::StructureError {
                    structure: "Attach Error".to_string(),
                    error: format!("{why:?}"),
                }),
            };
        }

        set_autoplay_inline(&selected_element)?;

        let selected_element =
            element_cast::<Element, HtmlVideoElement>(selected_element, "HtmlVideoElement")?;

        selected_element.set_width(self.resolution().width());
        selected_element.set_height(self.resolution().height());
        selected_element.set_src_object(Some(&self.media_stream()));

        self.attached_node = Some(Node::from(selected_element));
        self.attached = true;
        Ok(())
    }

    /// Detaches the camera from the `<video>` node.
    /// # Errors
    /// If the casting fails (the stored node is not a `<video>`) this will error.
    pub fn detach(&mut self) -> Result<(), NokhwaError> {
        if !self.attached {
            return Ok(());
        }

        let attached: &Node = match &self.attached_node {
            Some(node) => node,
            None => return Ok(()),
        };

        let attached = element_cast_ref::<Node, HtmlVideoElement>(attached, "HtmlVideoElement")?;

        attached.set_src_object(None);
        self.attached_node = None;
        self.attached = false;

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn draw_to_canvas(&mut self) -> Result<(), NokhwaError> {
        let window: Window = window()?;
        let document: Document = document(&window)?;
        self.measure_resolution()?;

        if self.attached_canvas.is_none() {
            let canvas = create_element(&document, "canvas")?;

            match document.body() {
                Some(body) => {
                    if let Err(why) = body.append_child(&canvas) {
                        return Err(NokhwaError::ReadFrameError(format!(
                            "Failed to attach canvas: {:?}",
                            why
                        )));
                    }
                }
                None => {
                    return Err(NokhwaError::ReadFrameError(
                        "Failed to get body".to_string(),
                    ))
                }
            }

            let canvas = element_cast::<Element, HtmlCanvasElement>(canvas, "HtmlCanvasElement")?;
            canvas.set_hidden(true);

            self.canvas_context = match canvas.get_context("2d") {
                Ok(maybe_ctx) => match maybe_ctx {
                    Some(ctx) => Some(element_cast::<Object, CanvasRenderingContext2d>(
                        ctx,
                        "CanvasRenderingContext2d",
                    )?),
                    None => {
                        return Err(NokhwaError::StructureError {
                            structure: "HtmlCanvasElement Context 2D".to_string(),
                            error: "None".to_string(),
                        });
                    }
                },
                Err(why) => {
                    return Err(NokhwaError::StructureError {
                        structure: "HtmlCanvasElement Context 2D".to_string(),
                        error: format!("{why:?}"),
                    });
                }
            };

            self.attached_canvas = Some(canvas);
        }

        let canvas = match &self.attached_canvas {
            Some(canvas) => canvas,
            None => {
                // shouldn't happen
                return Err(NokhwaError::GetPropertyError {
                    property: "Canvas".to_string(),
                    error: "None".to_string(),
                });
            }
        };

        canvas.set_width(self.resolution().width());
        canvas.set_height(self.resolution().height());

        let context = match &self.canvas_context {
            Some(cc) => cc,
            None => {
                return Err(NokhwaError::StructureError {
                    structure: "CanvasContext".to_string(),
                    error: "None".to_string(),
                })
            }
        };

        if self.attached && self.attached_node.is_some() {
            let video_element = match &self.attached_node {
                Some(n) => element_cast_ref::<Node, HtmlVideoElement>(n, "HtmlVideoElement")?,
                None => {
                    // this shouldn't happen
                    return Err(NokhwaError::StructureError {
                        structure: "Document Attached Video Element".to_string(),
                        error: "None".to_string(),
                    });
                }
            };

            if let Err(why) = context.draw_image_with_html_video_element_and_dw_and_dh(
                video_element,
                0_f64,
                0_f64,
                self.resolution().width().into(),
                self.resolution().height().into(),
            ) {
                return Err(NokhwaError::ReadFrameError(format!("{why:?}")));
            }

            match context.get_image_data(
                0_f64,
                0_f64,
                self.resolution().width().into(),
                self.resolution().height().into(),
            ) {
                Ok(data) => log_1(&jsv!(data)),
                Err(why) => {
                    return Err(NokhwaError::ReadFrameError(format!("{why:?}")));
                }
            };
        } else {
            let video_element = match document.create_element("video") {
                Ok(new_element) => new_element,
                Err(why) => {
                    return Err(NokhwaError::StructureError {
                        structure: "Document Video Element".to_string(),
                        error: format!("{why:?}"),
                    })
                }
            };

            set_autoplay_inline(&video_element)?;

            let video_element: HtmlVideoElement =
                element_cast::<Element, HtmlVideoElement>(video_element, "HtmlVideoElement")?;

            video_element.set_width(self.resolution().width());
            video_element.set_height(self.resolution().height());
            video_element.set_src_object(Some(&self.media_stream()));
            video_element.set_hidden(true);

            match document.body() {
                Some(body) => {
                    if let Err(why) = body.append_child(&video_element) {
                        return Err(NokhwaError::ReadFrameError(format!(
                            "Failed to attach video: {:?}",
                            why
                        )));
                    }
                }
                None => {
                    return Err(NokhwaError::ReadFrameError(
                        "Failed to get body".to_string(),
                    ))
                }
            }

            if let Err(why) = context.draw_image_with_html_video_element_and_dw_and_dh(
                &video_element,
                0_f64,
                0_f64,
                self.resolution().width().into(),
                self.resolution().height().into(),
            ) {
                return Err(NokhwaError::ReadFrameError(format!("{why:?}")));
            }

            match document.body() {
                Some(body) => {
                    if let Err(why) = body.remove_child(&video_element) {
                        return Err(NokhwaError::ReadFrameError(format!(
                            "Failed to remove video: {why:?}"
                        )));
                    }
                }
                None => {
                    return Err(NokhwaError::ReadFrameError(
                        "Failed to get body".to_string(),
                    ))
                }
            }
        }

        Ok(())
    }

    /// Copies camera frame to a `html_id`(by-id, canvas).
    ///
    /// If `generate_new` is true, the generated element will have an Id of `html_id`+`-canvas`. For example, if you pass "nokhwaisbest" for `html_id`, the new `<canvas>`'s ID will be "nokhwaisbest-canvas".
    /// # Errors
    /// If the internal canvas is not here, drawing fails, or a cast fails, this will error.
    #[allow(clippy::must_use_candidate)]
    #[allow(clippy::too_many_lines)]
    pub fn frame_canvas_copy(
        &mut self,
        html_id: &str,
        generate_new: bool,
    ) -> Result<(HtmlCanvasElement, CanvasRenderingContext2d), NokhwaError> {
        let window: Window = window()?;
        let document: Document = document(&window)?;

        let selected_element: Element = document_select_elem(&document, html_id)?;
        self.measure_resolution()?;
        self.draw_to_canvas()?;

        if generate_new {
            let new_canvas = create_element(&document, "canvas")?;
            let new_canvas =
                element_cast::<Element, HtmlCanvasElement>(new_canvas, "HtmlCanvasElement")?;

            new_canvas.set_width(self.resolution().width());
            new_canvas.set_height(self.resolution().height());
            new_canvas.set_id(&format!("{html_id}-canvas"));

            if let Err(why) = selected_element.append_child(&new_canvas) {
                return Err(NokhwaError::StructureError {
                    structure: "HtmlCanvasElement".to_string(),
                    error: format!("add child: {why:?}"),
                });
            }

            let context = match new_canvas.get_context("2d") {
                Ok(objcontext) => match objcontext {
                    Some(c2d) => c2d,
                    None => {
                        return Err(NokhwaError::StructureError {
                            structure: "CanvasRenderingContext2d".to_string(),
                            error: "No context".to_string(),
                        });
                    }
                },
                Err(why) => {
                    return Err(NokhwaError::StructureError {
                        structure: "CanvasRenderingContext2d".to_string(),
                        error: format!("context: {why:?}"),
                    });
                }
            };

            let self_canvas = match &self.attached_canvas {
                Some(c) => c,
                None => {
                    return Err(NokhwaError::StructureError {
                        structure: "HtmlCanvasElement".to_string(),
                        error: "Is None?".to_string(),
                    });
                }
            };

            let context = element_cast::<Object, CanvasRenderingContext2d>(
                context,
                "CanvasRenderingContext2d",
            )?;

            if let Err(why) = context.draw_image_with_html_canvas_element_and_dw_and_dh(
                self_canvas,
                0_f64,
                0_f64,
                self.resolution().width().into(),
                self.resolution().height().into(),
            ) {
                return Err(NokhwaError::ReadFrameError(format!(
                    "Failed to draw: {:?}",
                    why
                )));
            }

            Ok((new_canvas, context))
        } else {
            let canvas =
                element_cast::<Element, HtmlCanvasElement>(selected_element, "HtmlCanvasElement")?;

            let context = match canvas.get_context("2d") {
                Ok(objcontext) => match objcontext {
                    Some(c2d) => c2d,
                    None => {
                        return Err(NokhwaError::StructureError {
                            structure: "CanvasRenderingContext2d".to_string(),
                            error: "No context".to_string(),
                        });
                    }
                },
                Err(why) => {
                    return Err(NokhwaError::StructureError {
                        structure: "CanvasRenderingContext2d".to_string(),
                        error: format!("context: {why:?}"),
                    });
                }
            };

            let self_canvas = match &self.attached_canvas {
                Some(c) => c,
                None => {
                    return Err(NokhwaError::StructureError {
                        structure: "HtmlCanvasElement".to_string(),
                        error: "Is None?".to_string(),
                    });
                }
            };

            let context = element_cast::<Object, CanvasRenderingContext2d>(
                context,
                "CanvasRenderingContext2d",
            )?;

            if let Err(why) = context.draw_image_with_html_canvas_element_and_dw_and_dh(
                self_canvas,
                0_f64,
                0_f64,
                self.resolution().width().into(),
                self.resolution().height().into(),
            ) {
                return Err(NokhwaError::ReadFrameError(format!(
                    "Failed to draw: {:?}",
                    why
                )));
            }

            Ok((canvas, context))
        }
    }

    /// Captures an [`ImageData`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.ImageData.html) [`MDN`](https://developer.mozilla.org/en-US/docs/Web/API/ImageData) by drawing the image to a non-existent canvas.
    /// It is greatly advised to call this after calling attach to reduce DOM overhead.
    ///
    /// # Errors
    /// If drawing to the canvas fails this will error.
    pub fn frame_image_data(&mut self) -> Result<ImageData, NokhwaError> {
        self.draw_to_canvas()?;

        let context = match &self.canvas_context {
            Some(cc) => cc,
            None => {
                return Err(NokhwaError::StructureError {
                    structure: "CanvasContext".to_string(),
                    error: "None".to_string(),
                })
            }
        };

        let image_data = match context.get_image_data(
            0_f64,
            0_f64,
            self.resolution().width().into(),
            self.resolution().height().into(),
        ) {
            Ok(data) => data,
            Err(why) => {
                return Err(NokhwaError::ReadFrameError(format!("{why:?}")));
            }
        };

        Ok(image_data)
    }

    /// Captures an [`ImageData`](https://rustwasm.github.io/wasm-bindgen/api/web_sys/struct.ImageData.html) [`MDN`](https://developer.mozilla.org/en-US/docs/Web/API/ImageData) and then returns its `URL` as a string.
    /// - `mime_type`: The mime type of the resulting URI. It is `image/png` by default (lossless) but can be set to `image/jpeg` or `image/webp` (lossy). Anything else is ignored.
    /// - `image_quality`: A number between `0` and `1` indicating the resulting image quality in case you are using a lossy image mime type. The default value is 0.92, and all other values are ignored.
    ///
    /// # Errors
    /// If drawing to the canvas fails or URI generation is not supported or fails this will error.
    // TODO: Repleace with a data URI from base64!
    pub fn frame_uri(
        &mut self,
        mime_type: Option<&str>,
        image_quality: Option<f64>,
    ) -> Result<String, NokhwaError> {
        let mime_type = mime_type.unwrap_or("image/png");
        let image_quality = JsValue::from(image_quality.unwrap_or(0.92_f64));
        self.draw_to_canvas()?;

        let canvas = match &self.attached_canvas {
            Some(c) => c,
            None => return Err(NokhwaError::ReadFrameError("No Canvas".to_string())),
        };

        match canvas.to_data_url_with_type_and_encoder_options(mime_type, &image_quality) {
            Ok(uri) => Ok(uri),
            Err(why) => Err(NokhwaError::ReadFrameError(format!("{why:?}"))),
        }
    }

    /// Creates an off-screen canvas and a `<video>` element (if not already attached) and returns a raw `Cow<[u8]>` RGBA frame.
    /// # Errors
    /// If a cast fails, the camera fails to attach, the currently attached node is invalid, or writing/reading from the canvas fails, this will error.
    pub fn frame_raw(&mut self) -> Result<Cow<[u8]>, NokhwaError> {
        let image_data = self.frame_image_data()?.data().0;

        Ok(Cow::from(image_data))
    }

    /// This takes the output from [`frame_raw()`](crate::js_camera::JSCamera::frame_raw) and turns it into an `ImageBuffer<Rgb<u8>, Vec<u8>>`.
    /// # Errors
    /// This will error if the frame vec is too small(this is probably a bug, please report it!) or if the frame fails to capture. See [`frame_raw()`](crate::js_camera::JSCamera::frame_raw).
    pub fn frame(&mut self) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>, NokhwaError> {
        let raw_data = self.frame_raw()?.to_vec();
        let resolution = self.resolution();
        let image_buf =
            match ImageBuffer::from_vec(resolution.width(), resolution.height(), raw_data) {
                Some(buf) => {
                    let rgba_buf: ImageBuffer<Rgba<u8>, Vec<u8>> = buf;
                    let rgb_image_converted: ImageBuffer<Rgb<u8>, Vec<u8>> = rgba_buf.convert();
                    rgb_image_converted
                }
                None => return Err(NokhwaError::ReadFrameError(
                    "ImageBuffer is not large enough! This is probably a bug, please report it!"
                        .to_string(),
                )),
            };
        Ok(image_buf)
    }

    /// This takes the output from [`frame_raw()`](crate::js_camera::JSCamera::frame_raw) and turns it into an `ImageBuffer<Rgba<u8>, Vec<u8>>`.
    /// # Errors
    /// This will error if the frame vec is too small(this is probably a bug, please report it!) or if the frame fails to capture. See [`frame_raw()`](crate::js_camera::JSCamera::frame_raw).
    pub fn rgba_frame(&mut self) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, NokhwaError> {
        let raw_data = self.frame_raw()?.to_vec();
        let resolution = self.resolution();
        let image_buf =
            match ImageBuffer::from_vec(resolution.width(), resolution.height(), raw_data) {
                Some(buf) => {
                    let rgba_buf: ImageBuffer<Rgba<u8>, Vec<u8>> = buf;
                    rgba_buf
                }
                None => return Err(NokhwaError::ReadFrameError(
                    "ImageBuffer is not large enough! This is probably a bug, please report it!"
                        .to_string(),
                )),
            };
        Ok(image_buf)
    }

    /// The minimum buffer size needed to write the current frame (RGB24). If `use_rgba` is true, it will instead return the minimum size of the RGBA buffer needed.
    #[must_use]
    pub fn min_buffer_size(&self, use_rgba: bool) -> usize {
        let resolution = self.resolution();
        if use_rgba {
            (resolution.width() * resolution.height() * 4) as usize
        } else {
            (resolution.width() * resolution.height() * 3) as usize
        }
    }

    /// Directly writes the current frame(RGB24) into said `buffer`. If `convert_rgba` is true, the buffer written will be written as an RGBA frame instead of a RGB frame. Returns the amount of bytes written on successful capture.
    /// # Errors
    /// If reading the frame fails, this will error. See [`frame_raw()`](crate::js_camera::JSCamera::frame_raw).
    pub fn write_frame_to_buffer(
        &mut self,
        buffer: &mut [u8],
        convert_rgba: bool,
    ) -> Result<usize, NokhwaError> {
        let resolution = self.resolution();
        let frame = self.frame_raw()?;
        if convert_rgba {
            buffer.copy_from_slice(frame.borrow());
            return Ok(frame.len());
        }
        let image = match ImageBuffer::from_raw(resolution.width(), resolution.height(), frame) {
            Some(image) => {
                let image: ImageBuffer<Rgba<u8>, Cow<[u8]>> = image;
                let rgb_image: RgbImage = image.convert();
                rgb_image
            }
            None => {
                return Err(NokhwaError::ReadFrameError(
                    "Frame Cow Too Small".to_string(),
                ))
            }
        };

        buffer.copy_from_slice(image.as_raw());
        Ok(image.len())
    }

    #[cfg(feature = "output-wgpu")]
    /// Directly copies a frame to a Wgpu texture. This will automatically convert the frame into a RGBA frame.
    /// # Errors
    /// If the frame cannot be captured or the resolution is 0 on any axis, this will error.
    pub fn frame_texture<'a>(
        &mut self,
        device: &Device,
        queue: &Queue,
        label: Option<&'a str>,
    ) -> Result<Texture, NokhwaError> {
        use std::num::NonZeroU32;
        let resolution = self.resolution();
        let frame = self.frame_raw()?;

        let texture_size = Extent3d {
            width: resolution.width(),
            height: resolution.height(),
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&TextureDescriptor {
            label,
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        });

        let width_nonzero = match NonZeroU32::try_from(4 * resolution.width()) {
            Ok(w) => Some(w),
            Err(why) => return Err(NokhwaError::ReadFrameError(why.to_string())),
        };

        let height_nonzero = match NonZeroU32::try_from(resolution.height()) {
            Ok(h) => Some(h),
            Err(why) => return Err(NokhwaError::ReadFrameError(why.to_string())),
        };

        queue.write_texture(
            ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            frame.borrow(),
            ImageDataLayout {
                offset: 0,
                bytes_per_row: width_nonzero,
                rows_per_image: height_nonzero,
            },
            texture_size,
        );

        Ok(texture)
    }

    /// Checks if the stream is open.
    pub fn is_open(&self) -> bool {
        let stream = self
            .media_stream()
            .get_video_tracks()
            .iter()
            .next()
            .unwrap_or_else(JsValue::undefined);
        if !stream.is_undefined() {
            let stream = MediaStreamTrack::from(stream);
            if stream.ready_state() == MediaStreamTrackState::Live && stream.enabled() {
                return true;
            }
        }
        false
    }

    /// Restarts the stream.
    /// # Errors
    /// There may be errors when re-creating the camera, such as permission errors.
    pub async fn restart(&mut self) -> Result<(), NokhwaError> {
        let window: Window = window()?;
        let navigator = window.navigator();
        let media_devices = media_devices(&navigator)?;

        let stream: MediaStream = match media_devices
            .get_user_media_with_constraints(&self.constraints.media_constraints)
        {
            Ok(promise) => {
                let future = JsFuture::from(promise);
                match future.await {
                    Ok(stream) => {
                        let media_stream: MediaStream = MediaStream::from(stream);
                        media_stream
                    }
                    Err(why) => {
                        return Err(NokhwaError::StructureError {
                            structure: "MediaDevicesGetUserMediaJsFuture".to_string(),
                            error: format!("{why:?}"),
                        })
                    }
                }
            }
            Err(why) => {
                return Err(NokhwaError::StructureError {
                    structure: "MediaDevicesGetUserMedia".to_string(),
                    error: format!("{why:?}"),
                })
            }
        };

        self.media_stream = stream;
        Ok(())
    }

    /// Stops all streams and detaches the camera.
    /// # Errors
    /// There may be an error while detaching the camera. Please see [`detach()`](crate::js_camera::JSCamera::detach) for more details.
    pub fn stop_all(&mut self) -> Result<(), NokhwaError> {
        self.detach()?;
        self.media_stream.get_tracks().iter().for_each(|track| {
            let media_track = MediaStreamTrack::from(track);
            media_track.stop();
        });
        Ok(())
    }
}

impl Deref for JSCamera {
    type Target = MediaStream;

    fn deref(&self) -> &Self::Target {
        &self.media_stream
    }
}

impl Drop for JSCamera {
    fn drop(&mut self) {
        self.stop_all().unwrap_or(()); // swallow errors
    }
}

// SAFETY: JSCamera is used in WASM, it will never be sent to a different thread. This is only done to satisfy the compiler.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for JSCamera {}
