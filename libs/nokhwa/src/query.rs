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

use nokhwa_core::{
    error::NokhwaError,
    types::{ApiBackend, CameraInfo},
};

/// Gets the native [`ApiBackend`]
#[must_use]
pub fn native_api_backend() -> Option<ApiBackend> {
    match std::env::consts::OS {
        "linux" => Some(ApiBackend::Video4Linux),
        "macos" | "ios" => Some(ApiBackend::AVFoundation),
        "windows" => Some(ApiBackend::MediaFoundation),
        _ => None,
    }
}

// TODO: Update as this goes
/// Query the system for a list of available devices. Please refer to the API Backends that support `Query`) <br>
/// Usually the order goes Native -> UVC -> Gstreamer.
/// # Quirks
/// - `Media Foundation`: The symbolic link for the device is listed in the `misc` attribute of the [`CameraInfo`].
/// - `Media Foundation`: The names may contain invalid characters since they were converted from UTF16.
/// - `AVFoundation`: The ID of the device is stored in the `misc` attribute of the [`CameraInfo`].
/// - `AVFoundation`: There is lots of miscellaneous info in the `desc` attribute.
/// - `WASM`: The `misc` field contains the device ID and group ID are seperated by a space (' ')
/// # Errors
/// If you use an unsupported API (check the README or crate root for more info), incompatible backend for current platform, incompatible platform, or insufficient permissions, etc
/// this will error.
pub fn query(api: ApiBackend) -> Result<Vec<CameraInfo>, NokhwaError> {
    match api {
        ApiBackend::Auto => {
            // determine platform
            match std::env::consts::OS {
                "linux" => {
                    if cfg!(feature = "input-v4l") && cfg!(target_os = "linux") {
                        query(ApiBackend::Video4Linux)
                    } else if cfg!(feature = "input-opencv") {
                        query(ApiBackend::OpenCv)
                    } else {
                        dbg!("Error: No suitable Backends available. Perhaps you meant to enable one of the backends such as `input-v4l`? (Please read the docs.)");
                        Err(NokhwaError::UnsupportedOperationError(ApiBackend::Auto))
                    }
                }
                "windows" => {
                    if cfg!(feature = "input-msmf") && cfg!(target_os = "windows") {
                        query(ApiBackend::MediaFoundation)
                    } else if cfg!(feature = "input-opencv") {
                        query(ApiBackend::OpenCv)
                    } else {
                        dbg!("Error: No suitable Backends available. Perhaps you meant to enable one of the backends such as `input-msmf`? (Please read the docs.)");
                        Err(NokhwaError::UnsupportedOperationError(ApiBackend::Auto))
                    }
                }
                "macos" => {
                    if cfg!(feature = "input-avfoundation") {
                        query(ApiBackend::AVFoundation)
                    } else if cfg!(feature = "input-opencv") {
                        query(ApiBackend::OpenCv)
                    } else {
                        dbg!("Error: No suitable Backends available. Perhaps you meant to enable one of the backends such as `input-avfoundation`? (Please read the docs.)");
                        Err(NokhwaError::UnsupportedOperationError(ApiBackend::Auto))
                    }
                }
                "ios" => {
                    if cfg!(feature = "input-avfoundation") {
                        query(ApiBackend::AVFoundation)
                    } else {
                        dbg!("Error: No suitable Backends available. Perhaps you meant to enable one of the backends such as `input-avfoundation`? (Please read the docs.)");
                        Err(NokhwaError::UnsupportedOperationError(ApiBackend::Auto))
                    }
                }
                _ => {
                    dbg!("Error: No suitable Backends available. You are on an unsupported platform.");
                    Err(NokhwaError::NotImplementedError("Bad Platform".to_string()))
                }
            }
        }
        ApiBackend::AVFoundation => query_avfoundation(),
        ApiBackend::Video4Linux => query_v4l(),
        ApiBackend::MediaFoundation => query_msmf(),
        ApiBackend::OpenCv | ApiBackend::Network => {
            Err(NokhwaError::UnsupportedOperationError(api))
        }
        ApiBackend::Browser => query_wasm(),
        _ => Err(NokhwaError::UnsupportedOperationError(api)),
    }
}

// TODO: More

#[cfg(all(feature = "input-v4l", target_os = "linux"))]
fn query_v4l() -> Result<Vec<CameraInfo>, NokhwaError> {
    nokhwa_bindings_linux::query()
}

#[cfg(any(not(feature = "input-v4l"), not(target_os = "linux")))]
fn query_v4l() -> Result<Vec<CameraInfo>, NokhwaError> {
    Err(NokhwaError::UnsupportedOperationError(
        ApiBackend::Video4Linux,
    ))
}

// #[cfg(feature = "input-uvc")]
// fn query_uvc() -> Result<Vec<CameraInfo>, NokhwaError> {
//     use crate::CameraIndex;
//     use uvc::Device;
//
//     let context = match uvc::Context::new() {
//         Ok(ctx) => ctx,
//         Err(why) => {
//             return Err(NokhwaError::GeneralError(format!(
//                 "UVC Context failure: {}",
//                 why
//             )))
//         }
//     };
//
//     let usb_devices = usb_enumeration::enumerate(None, None);
//     let uvc_devices = match context.devices() {
//         Ok(devs) => {
//             let device_vec: Vec<Device> = devs.collect();
//             device_vec
//         }
//         Err(why) => {
//             return Err(NokhwaError::GeneralError(format!(
//                 "UVC Context Devicelist failure: {}",
//                 why
//             )))
//         }
//     };
//
//     let mut camera_info_vec = vec![];
//     let mut counter = 0_usize;
//
//     // Optimize this O(n*m) algorithm
//     for usb_dev in &usb_devices {
//         for uvc_dev in &uvc_devices {
//             if let Ok(desc) = uvc_dev.description() {
//                 if desc.product_id == usb_dev.product_id && desc.vendor_id == usb_dev.vendor_id {
//                     let name = usb_dev
//                         .description
//                         .as_ref()
//                         .unwrap_or(&format!(
//                             "{}:{} {} {}",
//                             desc.vendor_id,
//                             desc.product_id,
//                             desc.manufacturer.unwrap_or_else(|| "Generic".to_string()),
//                             desc.product.unwrap_or_else(|| "Camera".to_string())
//                         ))
//                         .clone();
//
//                     camera_info_vec.push(CameraInfo::new(
//                         name.clone(),
//                         usb_dev
//                             .description
//                             .as_ref()
//                             .unwrap_or(&"".to_string())
//                             .clone(),
//                         format!(
//                             "{}:{} {}",
//                             desc.vendor_id,
//                             desc.product_id,
//                             desc.serial_number.unwrap_or_else(|| "".to_string())
//                         ),
//                         CameraIndex::Index(counter as u32),
//                     ));
//                     counter += 1;
//                 }
//             }
//         }
//     }
//     Ok(camera_info_vec)
// }
//
// #[cfg(not(feature = "input-uvc"))]
// #[allow(deprecated)]
// fn query_uvc() -> Result<Vec<CameraInfo>, NokhwaError> {
//     Err(NokhwaError::UnsupportedOperationError(
//         ApiBackend::UniversalVideoClass,
//     ))
// }
//
// #[cfg(feature = "input-gst")]
// fn query_gstreamer() -> Result<Vec<CameraInfo>, NokhwaError> {
//     use gstreamer::{
//         prelude::{DeviceExt, DeviceMonitorExt, DeviceMonitorExtManual},
//         Caps, DeviceMonitor,
//     };
//     use nokhwa_core::types::CameraIndex;
//     use std::str::FromStr;
//
//     if let Err(why) = gstreamer::init() {
//         return Err(NokhwaError::GeneralError(format!(
//             "Failed to init gstreamer: {}",
//             why
//         )));
//     }
//     let device_monitor = DeviceMonitor::new();
//     let video_caps = match Caps::from_str("video/x-raw") {
//         Ok(cap) => cap,
//         Err(why) => {
//             return Err(NokhwaError::GeneralError(format!(
//                 "Failed to generate caps: {}",
//                 why
//             )))
//         }
//     };
//     let _video_filter_id = match device_monitor.add_filter(Some("Video/Source"), Some(&video_caps))
//     {
//         Some(id) => id,
//         None => {
//             return Err(NokhwaError::StructureError {
//                 structure: "Video Filter ID Video/Source".to_string(),
//                 error: "Null".to_string(),
//             })
//         }
//     };
//     if let Err(why) = device_monitor.start() {
//         return Err(NokhwaError::GeneralError(format!(
//             "Failed to start device monitor: {}",
//             why
//         )));
//     }
//     let mut counter = 0;
//     let devices: Vec<CameraInfo> = device_monitor
//         .devices()
//         .iter_mut()
//         .map(|gst_dev| {
//             let name = DeviceExt::display_name(gst_dev);
//             let class = DeviceExt::device_class(gst_dev);
//             counter += 1;
//             CameraInfo::new(&name, &class, "", CameraIndex::Index(counter - 1))
//         })
//         .collect();
//     device_monitor.stop();
//     Ok(devices)
// }
//
// #[cfg(not(feature = "input-gst"))]
// #[allow(deprecated)]
// fn query_gstreamer() -> Result<Vec<CameraInfo>, NokhwaError> {
//     Err(NokhwaError::UnsupportedOperationError(
//         ApiBackend::GStreamer,
//     ))
// }

// please refer to https://docs.microsoft.com/en-us/windows/win32/medfound/enumerating-video-capture-devices
#[cfg(all(feature = "input-msmf", target_os = "windows"))]
fn query_msmf() -> Result<Vec<CameraInfo>, NokhwaError> {
    nokhwa_bindings_windows::wmf::query_media_foundation_descriptors()
}

#[cfg(any(not(feature = "input-msmf"), not(target_os = "windows")))]
fn query_msmf() -> Result<Vec<CameraInfo>, NokhwaError> {
    Err(NokhwaError::UnsupportedOperationError(
        ApiBackend::MediaFoundation,
    ))
}

#[cfg(all(
    feature = "input-avfoundation",
    any(target_os = "macos", target_os = "ios")
))]
fn query_avfoundation() -> Result<Vec<CameraInfo>, NokhwaError> {
    use nokhwa_bindings_macos::query_avfoundation;

    Ok(query_avfoundation()?
        .into_iter()
        .collect::<Vec<CameraInfo>>())
}

#[cfg(not(all(
    feature = "input-avfoundation",
    any(target_os = "macos", target_os = "ios")
)))]
fn query_avfoundation() -> Result<Vec<CameraInfo>, NokhwaError> {
    Err(NokhwaError::UnsupportedOperationError(
        ApiBackend::AVFoundation,
    ))
}

#[cfg(feature = "input-jscam")]
fn query_wasm() -> Result<Vec<CameraInfo>, NokhwaError> {
    use crate::js_camera::query_js_cameras;
    use wasm_rs_async_executor::single_threaded::block_on;

    block_on(query_js_cameras())
}

#[cfg(not(feature = "input-jscam"))]
fn query_wasm() -> Result<Vec<CameraInfo>, NokhwaError> {
    Err(NokhwaError::UnsupportedOperationError(ApiBackend::Browser))
}
