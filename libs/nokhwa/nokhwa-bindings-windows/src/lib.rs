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

#![deny(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::too_many_lines)]

//! # nokhwa-bindings-windows
//! This crate is the `MediaFoundation` bindings for the `nokhwa` crate.
//!
//! It is not meant for general consumption. If you are looking for a Windows camera capture crate, consider using `nokhwa` with feature `input-msmf`.
//!
//! No support or API stability will be given. Subject to change at any time.

#[cfg(all(windows, not(feature = "docs-only")))]
mod mf_wrapper;

#[cfg(all(windows, not(feature = "docs-only")))]
pub mod wmf {
    use super::mf_wrapper::*;
    use nokhwa_core::error::NokhwaError;
    use nokhwa_core::types::{
        ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueDescription,
        ControlValueSetter, FrameFormat, KnownCameraControl, KnownCameraControlFlag, Resolution,
    };
    use once_cell::sync::Lazy;
    use std::ffi::c_void;
    use std::{
        borrow::Cow,
        cell::Cell,
        mem::MaybeUninit,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
    };
    use windows::Win32::Media::DirectShow::{CameraControl_Flags_Auto, CameraControl_Flags_Manual};
    use windows::Win32::Media::MediaFoundation::{
        IMFMediaType, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    };
    use windows::{
        core::{Interface, GUID, PWSTR},
        Win32::{
            Media::{
                DirectShow::{
                    CameraControl_Exposure, CameraControl_Focus, CameraControl_Iris,
                    CameraControl_Pan, CameraControl_Tilt, CameraControl_Zoom, IAMCameraControl,
                    IAMVideoProcAmp, VideoProcAmp_BacklightCompensation, VideoProcAmp_Brightness,
                    VideoProcAmp_ColorEnable, VideoProcAmp_Contrast, VideoProcAmp_Gain,
                    VideoProcAmp_Gamma, VideoProcAmp_Hue, VideoProcAmp_Saturation,
                    VideoProcAmp_Sharpness, VideoProcAmp_WhiteBalance,
                },
                KernelStreaming::GUID_NULL,
                MediaFoundation::{
                    IMFActivate, IMFAttributes, IMFMediaSource, IMFSample, IMFSourceReader,
                    MFMediaType_Video, MFSTARTUP_NOSOCKET, MF_API_VERSION,
                    MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
                    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_MT_FRAME_RATE,
                    MF_MT_FRAME_RATE_RANGE_MAX, MF_MT_FRAME_RATE_RANGE_MIN, MF_MT_FRAME_SIZE,
                    MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_READWRITE_DISABLE_CONVERTERS,
                },
            },
            System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT},
        },
    };

    static INITIALIZED: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));
    static CAMERA_REFCNT: Lazy<Arc<AtomicUsize>> = Lazy::new(|| Arc::new(AtomicUsize::new(0)));

    // See: https://stackoverflow.com/questions/80160/what-does-coinit-speed-over-memory-do
    const CO_INIT_APARTMENT_THREADED: COINIT = COINIT(0x2);
    const CO_INIT_DISABLE_OLE1DDE: COINIT = COINIT(0x4);

    // See: https://gix.github.io/media-types/#major-types
    const MF_VIDEO_FORMAT_YUY2: GUID = GUID::from_values(
        0x3259_5559,
        0x0000,
        0x0010,
        [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
    );
    const MF_VIDEO_FORMAT_MJPEG: GUID = GUID::from_values(
        0x4750_4A4D,
        0x0000,
        0x0010,
        [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
    );
    const MF_VIDEO_FORMAT_GRAY: GUID = GUID::from_values(
        0x3030_3859,
        0x0000,
        0x0010,
        [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
    );
    const MF_VIDEO_FORMAT_NV12: GUID = GUID::from_values(
        0x3231_564E,
        0x0000,
        0x0010,
        [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
    );
    const MF_VIDEO_FORMAT_RGB24: GUID = GUID::from_values(
        0x0000_0014,
        0x0000,
        0x0010,
        [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
    );

    const MEDIA_FOUNDATION_FIRST_VIDEO_STREAM: u32 = 0xFFFF_FFFC;
    const MF_SOURCE_READER_MEDIASOURCE: u32 = 0xFFFF_FFFF;

    // const CAM_CTRL_AUTO: i32 = 0x0001;
    // const CAM_CTRL_MANUAL: i32 = 0x0002;

    // macro_rules! define_controls {
    //     ( $( ($key:expr => ($property:ident, $min:ident, $max:ident, $step:ident, $default:ident, $flag:ident)) )* ) => {
    //         $(
    //         $key => {
    //             if let Err(why) = unsafe {
    //                     video_proc_amp.GetRange(
    //                         $property.0,
    //                         &mut $min,
    //                         &mut $max,
    //                         &mut $step,
    //                         &mut $default,
    //                         &mut $flag,
    //                     )
    //                 } {
    //                     return Err(NokhwaError::GetPropertyError {
    //                         property: stringify!($key).to_string(),
    //                         error: why.to_string()
    //                     });
    //                 }
    //         }
    //         )*
    //     };
    //     ( $( ($key:expr : ($property:ident, $value:ident, $flag:ident)) )* ) => {
    //         $(
    //         $key => {
    //             if let Err(why) = unsafe {
    //                 video_proc_amp.Get($property.0, &mut $value, &mut $flag)
    //                 } {
    //                     return Err(NokhwaError::GetPropertyError {
    //                         property: stringify!($key).to_string(),
    //                         error: why.to_string()
    //                     });
    //                 }
    //         }
    //         )*
    //     };
    // }

    fn guid_to_frameformat(guid: GUID) -> Option<FrameFormat> {
        match guid {
            MF_VIDEO_FORMAT_NV12 => Some(FrameFormat::NV12),
            MF_VIDEO_FORMAT_RGB24 => Some(FrameFormat::RAWRGB),
            MF_VIDEO_FORMAT_GRAY => Some(FrameFormat::GRAY),
            MF_VIDEO_FORMAT_YUY2 => Some(FrameFormat::YUYV),
            MF_VIDEO_FORMAT_MJPEG => Some(FrameFormat::MJPEG),
            _ => None,
        }
    }

    fn frameformat_to_guid(frameformat: FrameFormat) -> GUID {
        match frameformat {
            FrameFormat::MJPEG => MF_VIDEO_FORMAT_MJPEG,
            FrameFormat::YUYV => MF_VIDEO_FORMAT_YUY2,
            FrameFormat::NV12 => MF_VIDEO_FORMAT_NV12,
            FrameFormat::GRAY => MF_VIDEO_FORMAT_GRAY,
            FrameFormat::RAWRGB => MF_VIDEO_FORMAT_RGB24,
        }
    }

    pub fn initialize_mf() -> Result<(), NokhwaError> {
        if !(INITIALIZED.load(Ordering::SeqCst)) {
            if let Err(why) = unsafe {
                CoInitializeEx(None, CO_INIT_APARTMENT_THREADED | CO_INIT_DISABLE_OLE1DDE)
            } {
                return Err(NokhwaError::InitializeError {
                    backend: ApiBackend::MediaFoundation,
                    error: why.to_string(),
                });
            }

            if let Err(why) = unsafe { MFStartup(MF_API_VERSION, MFSTARTUP_NOSOCKET) } {
                unsafe {
                    CoUninitialize();
                }
                return Err(NokhwaError::InitializeError {
                    backend: ApiBackend::MediaFoundation,
                    error: why.to_string(),
                });
            }
            INITIALIZED.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn de_initialize_mf() -> Result<(), NokhwaError> {
        if INITIALIZED.load(Ordering::SeqCst) {
            unsafe {
                if let Err(why) = MFShutdown() {
                    return Err(NokhwaError::ShutdownError {
                        backend: ApiBackend::MediaFoundation,
                        error: why.to_string(),
                    });
                }
                CoUninitialize();
                INITIALIZED.store(false, Ordering::SeqCst);
            }
        }
        Ok(())
    }

    pub(crate) struct ActivateList {
        ptr: *mut Option<IMFActivate>,
        count: usize,
    }

    impl ActivateList {
        pub(crate) unsafe fn new(ptr: *mut Option<IMFActivate>, count: usize) -> Option<Self> {
            if ptr.is_null() || count == 0 {
                return None;
            }

            Some(Self { ptr, count })
        }

        /// Safety: The pointer returned by this method MUST NOT be used after
        /// `self` has gone out of scope. This pointer may also be null.
        pub(crate) unsafe fn iter(&self) -> ActivateIterator {
            ActivateIterator {
                current: self.ptr,
                remaining: self.count,
            }
        }
    }

    pub(crate) struct ActivateIterator {
        current: *mut Option<IMFActivate>,
        remaining: usize,
    }

    impl Iterator for ActivateIterator {
        type Item = Option<IMFActivate>;

        fn next(&mut self) -> Option<Self::Item> {
            if self.remaining == 0 || self.current.is_null() {
                return None;
            }

            // Safety: We check for null and bounds
            let result = unsafe { (*self.current).clone() };
            self.current = unsafe { self.current.add(1) };
            self.remaining -= 1;

            Some(result)
        }
    }

    impl Drop for ActivateList {
        fn drop(&mut self) {
            if !self.ptr.is_null() && self.count > 0 {
                unsafe {
                    // Free the memory allocated by MFEnumDeviceSources
                    CoTaskMemFree(Some(self.ptr as *mut _));
                }
            }
        }
    }

    fn query_activate_pointers() -> Result<Vec<IMFActivate>, NokhwaError> {
        initialize_mf()?;

        let mut attributes: Option<IMFAttributes> = None;
        if let Err(why) = unsafe { MFCreateAttributes(&mut attributes, 1) } {
            return Err(NokhwaError::GetPropertyError {
                property: "IMFAttributes".to_string(),
                error: why.to_string(),
            });
        }

        let attributes = match attributes {
            Some(attr) => {
                if let Err(why) = unsafe {
                    attr.SetGUID(
                        &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                        &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
                    )
                } {
                    return Err(NokhwaError::SetPropertyError {
                        property: "GUID MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE".to_string(),
                        value: "MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID".to_string(),
                        error: why.to_string(),
                    });
                }
                attr
            }
            None => {
                return Err(NokhwaError::SetPropertyError {
                    property: "GUID MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE".to_string(),
                    value: "MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID".to_string(),
                    error: "Call to IMFAttributes::SetGUID failed - IMFAttributes is None"
                        .to_string(),
                });
            }
        };

        let mut count: u32 = 0;
        let mut unused_mf_activate: MaybeUninit<*mut Option<IMFActivate>> = MaybeUninit::uninit();

        if let Err(why) =
            unsafe { MFEnumDeviceSources(&attributes, unused_mf_activate.as_mut_ptr(), &mut count) }
        {
            return Err(NokhwaError::StructureError {
                structure: "MFEnumDeviceSources".to_string(),
                error: why.to_string(),
            });
        }

        let activate_list = unsafe {
            ActivateList::new(unused_mf_activate.assume_init(), count as usize).ok_or_else(
                || NokhwaError::StructureError {
                    structure: "ActivateList".to_string(),
                    error: "Invalid pointer alignment or null pointer".to_string(),
                },
            )?
        };

        let mut device_list = Vec::new();

        unsafe {
            for activate in activate_list.iter() {
                if let Some(imf_activate) = activate {
                    device_list.push(imf_activate);
                }
            }
        }

        Ok(device_list)
    }

    fn activate_to_descriptors(
        index: CameraIndex,
        imf_activate: &IMFActivate,
    ) -> Result<CameraInfo, NokhwaError> {
        let mut pwstr_name = PWSTR(&mut 0_u16);
        let mut len_pwstrname = 0;
        let mut pwstr_symlink = PWSTR(&mut 0_u16);
        let mut len_pwstrsymlink = 0;

        if let Err(why) = unsafe {
            imf_activate.GetAllocatedString(
                &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME,
                &mut pwstr_name,
                &mut len_pwstrname,
            )
        } {
            return Err(NokhwaError::GetPropertyError {
                property: "MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME".to_string(),
                error: why.to_string(),
            });
        }

        if let Err(why) = unsafe {
            imf_activate.GetAllocatedString(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
                &mut pwstr_symlink,
                &mut len_pwstrsymlink,
            )
        } {
            return Err(NokhwaError::GetPropertyError {
                property: "MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK".to_string(),
                error: why.to_string(),
            });
        }

        if pwstr_name.is_null() {
            return Err(NokhwaError::GetPropertyError {
                property: "MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME".to_string(),
                error: "Call to IMFActivate::GetAllocatedString failed - PWSTR is null".to_string(),
            });
        }
        if pwstr_symlink.is_null() {
            return Err(NokhwaError::GetPropertyError {
                property: "MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK".to_string(),
                error: "Call to IMFActivate::GetAllocatedString failed - PWSTR is null".to_string(),
            });
        }

        let name = unsafe {
            pwstr_name
                .to_string()
                .map_err(|x| NokhwaError::StructureError {
                    structure: "PWSTR/String - Name".to_string(),
                    error: x.to_string(),
                })?
        };
        let symlink = unsafe {
            pwstr_symlink
                .to_string()
                .map_err(|x| NokhwaError::StructureError {
                    structure: "PWSTR/String - Symlink".to_string(),
                    error: x.to_string(),
                })?
        };

        Ok(CameraInfo::new(
            &name,
            "MediaFoundation Camera",
            &symlink,
            index,
        ))
    }

    pub fn query_media_foundation_descriptors() -> Result<Vec<CameraInfo>, NokhwaError> {
        let mut device_list = vec![];

        for (index, activate_ptr) in query_activate_pointers()?.into_iter().enumerate() {
            device_list.push(activate_to_descriptors(
                CameraIndex::Index(index as u32),
                &activate_ptr,
            )?);
        }
        Ok(device_list)
    }

    #[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Eq)]
    enum MFControlId {
        ProcAmpBoolean(i32),
        ProcAmpRange(i32),
        CCValue(i32),
        CCRange(i32),
    }

    #[allow(clippy::cast_sign_loss)]
    fn kcc_to_i32(kcc: KnownCameraControl) -> Option<MFControlId> {
        let control_id = match kcc {
            KnownCameraControl::Brightness => MFControlId::ProcAmpRange(VideoProcAmp_Brightness.0),
            KnownCameraControl::Contrast => MFControlId::ProcAmpRange(VideoProcAmp_Contrast.0),
            KnownCameraControl::Hue => MFControlId::ProcAmpRange(VideoProcAmp_Hue.0),
            KnownCameraControl::Saturation => MFControlId::ProcAmpRange(VideoProcAmp_Saturation.0),
            KnownCameraControl::Sharpness => MFControlId::ProcAmpRange(VideoProcAmp_Sharpness.0),
            KnownCameraControl::Gamma => MFControlId::ProcAmpRange(VideoProcAmp_Gamma.0),
            KnownCameraControl::WhiteBalance => {
                MFControlId::ProcAmpRange(VideoProcAmp_WhiteBalance.0)
            }
            KnownCameraControl::BacklightComp => {
                MFControlId::ProcAmpBoolean(VideoProcAmp_BacklightCompensation.0)
            }
            KnownCameraControl::Gain => MFControlId::ProcAmpRange(VideoProcAmp_Gain.0),
            KnownCameraControl::Pan => MFControlId::CCRange(CameraControl_Pan.0),
            KnownCameraControl::Tilt => MFControlId::CCRange(CameraControl_Tilt.0),
            KnownCameraControl::Zoom => MFControlId::CCRange(CameraControl_Zoom.0),
            KnownCameraControl::Exposure => MFControlId::CCValue(CameraControl_Exposure.0),
            KnownCameraControl::Iris => MFControlId::CCValue(CameraControl_Iris.0),
            KnownCameraControl::Focus => MFControlId::CCValue(CameraControl_Focus.0),
            KnownCameraControl::Other(o) => {
                if o == VideoProcAmp_ColorEnable.0 as u128 {
                    MFControlId::ProcAmpRange(o as i32)
                } else {
                    return None;
                }
            }
        };

        Some(control_id)
    }

    pub struct MediaFoundationDevice {
        is_open: Cell<bool>,
        device_specifier: CameraInfo,
        device_format: CameraFormat,
        source_reader: IMFSourceReader,
    }

    impl MediaFoundationDevice {
        pub fn new(index: CameraIndex) -> Result<Self, NokhwaError> {
            initialize_mf()?;
            match index {
                CameraIndex::Index(i) => {
                    let (media_source, device_descriptor) =
                        match query_activate_pointers()?.into_iter().nth(i as usize) {
                            Some(activate) => {
                                match unsafe { activate.ActivateObject::<IMFMediaSource>() } {
                                    Ok(media_source) => {
                                        (media_source, activate_to_descriptors(index, &activate)?)
                                    }
                                    Err(why) => {
                                        return Err(NokhwaError::OpenDeviceError(
                                            index.to_string(),
                                            why.to_string(),
                                        ))
                                    }
                                }
                            }
                            None => {
                                return Err(NokhwaError::OpenDeviceError(
                                    index.to_string(),
                                    "No device".to_string(),
                                ))
                            }
                        };

                    let source_reader_attr = {
                        let attr = match {
                            let mut attr: Option<IMFAttributes> = None;

                            if let Err(why) = unsafe { MFCreateAttributes(&mut attr, 3) } {
                                return Err(NokhwaError::StructureError {
                                    structure: "MFCreateAttributes".to_string(),
                                    error: why.to_string(),
                                });
                            }
                            attr
                        } {
                            Some(imf_attr) => imf_attr,
                            None => {
                                return Err(NokhwaError::StructureError {
                                    structure: "MFCreateAttributes".to_string(),
                                    error: "Attributee Alloc Failure".to_string(),
                                });
                            }
                        };

                        if let Err(why) = unsafe {
                            attr.SetUINT32(&MF_READWRITE_DISABLE_CONVERTERS, u32::from(true))
                        } {
                            return Err(NokhwaError::SetPropertyError {
                                property: "MF_READWRITE_DISABLE_CONVERTERS".to_string(),
                                value: u32::from(true).to_string(),
                                error: why.to_string(),
                            });
                        }

                        attr
                    };

                    let source_reader = match unsafe {
                        MFCreateSourceReaderFromMediaSource(&media_source, &source_reader_attr)
                    } {
                        Ok(sr) => sr,
                        Err(why) => {
                            return Err(NokhwaError::StructureError {
                                structure: "MFCreateSourceReaderFromMediaSource".to_string(),
                                error: why.to_string(),
                            })
                        }
                    };

                    // increment refcnt
                    CAMERA_REFCNT.store(CAMERA_REFCNT.load(Ordering::SeqCst) + 1, Ordering::SeqCst);

                    Ok(MediaFoundationDevice {
                        is_open: Cell::new(false),
                        device_specifier: device_descriptor,
                        device_format: CameraFormat::default(),
                        source_reader,
                    })
                }
                CameraIndex::String(s) => {
                    let devicelist = query_media_foundation_descriptors()?;
                    let mut id_eq = None;

                    for mfdev in devicelist {
                        if mfdev.misc() == s {
                            id_eq = Some(mfdev.index().as_index()?);
                            break;
                        }
                    }

                    match id_eq {
                        Some(index) => Self::new(CameraIndex::Index(index)),
                        None => Err(NokhwaError::OpenDeviceError(s, "Not Found".to_string())),
                    }
                }
            }
        }
        //
        // pub fn with_string(unique_id: &[u16]) -> Result<Self, NokhwaError> {
        //     let devicelist = query_media_foundation_descriptors()?;
        //     let mut id_eq = None;
        //
        //     for mfdev in devicelist {
        //         if (mfdev.symlink() as &[u16]) == unique_id {
        //             id_eq = Some(mfdev.index().as_index()?);
        //             break;
        //         }
        //     }
        //
        //     match id_eq {
        //         Some(index) => Self::new(index),
        //         None => {
        //             return Err(BindingError::DeviceOpenFailError(
        //                 std::str::from_utf8(
        //                     &unique_id.iter().map(|x| *x as u8).collect::<Vec<u8>>(),
        //                 )
        //                 .unwrap_or("")
        //                 .to_string(),
        //                 "Not Found".to_string(),
        //             ))
        //         }
        //     }
        // }

        pub fn index(&self) -> &CameraIndex {
            self.device_specifier.index()
        }

        pub fn name(&self) -> String {
            self.device_specifier.human_name()
        }

        pub fn symlink(&self) -> String {
            self.device_specifier.misc()
        }

        pub fn compatible_format_list(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
            let mut camera_format_list = vec![];
            let mut index: u32 = 0;

            // https://learn.microsoft.com/en-us/windows/win32/api/mfreadwrite/nf-mfreadwrite-imfsourcereader-getnativemediatype
            // https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/MediaFoundation/struct.IMFSourceReader.html#method.GetNativeMediaType
            while let Ok(media_type) = unsafe {
                self.source_reader
                    .GetNativeMediaType(MEDIA_FOUNDATION_FIRST_VIDEO_STREAM, index)
            } {
                let fourcc = match unsafe { media_type.GetGUID(&MF_MT_SUBTYPE) } {
                    Ok(fcc) => fcc,
                    Err(why) => {
                        return Err(NokhwaError::GetPropertyError {
                            property: "MF_MT_SUBTYPE".to_string(),
                            error: why.to_string(),
                        })
                    }
                };

                let (width, height) = match unsafe { media_type.GetUINT64(&MF_MT_FRAME_SIZE) } {
                    Ok(res_u64) => {
                        let width = (res_u64 >> 32) as u32;
                        let height = res_u64 as u32; // the cast will truncate the upper bits
                        (width, height)
                    }
                    Err(why) => {
                        return Err(NokhwaError::GetPropertyError {
                            property: "MF_MT_FRAME_SIZE".to_string(),
                            error: why.to_string(),
                        })
                    }
                };

                // MFRatio is represented as 2 u32s in memory. This means we cann convert it to 2
                let framerate_list = {
                    let mut framerates = vec![0_u32; 3];
                    if let Ok(fraction_u64) =
                        unsafe { media_type.GetUINT64(&MF_MT_FRAME_RATE_RANGE_MAX) }
                    {
                        let mut numerator = (fraction_u64 >> 32) as u32;
                        let denominator = fraction_u64 as u32;
                        if denominator != 1 {
                            numerator = 0;
                        }
                        framerates.push(numerator);
                    };
                    if let Ok(fraction_u64) =
                        unsafe { media_type.GetUINT64(&MF_MT_FRAME_RATE_RANGE_MAX) }
                    {
                        let mut numerator = (fraction_u64 >> 32) as u32;
                        let denominator = fraction_u64 as u32;
                        if denominator != 1 {
                            numerator = 0;
                        }
                        framerates.push(numerator);
                    };
                    if let Ok(fraction_u64) = unsafe { media_type.GetUINT64(&MF_MT_FRAME_RATE) } {
                        let mut numerator = (fraction_u64 >> 32) as u32;
                        let denominator = fraction_u64 as u32;
                        if denominator != 1 {
                            numerator = 0;
                        }
                        framerates.push(numerator);
                    };
                    if let Ok(fraction_u64) =
                        unsafe { media_type.GetUINT64(&MF_MT_FRAME_RATE_RANGE_MIN) }
                    {
                        let mut numerator = (fraction_u64 >> 32) as u32;
                        let denominator = fraction_u64 as u32;
                        if denominator != 1 {
                            numerator = 0;
                        }
                        framerates.push(numerator);
                    };
                    framerates
                };

                let frame_fmt = match guid_to_frameformat(fourcc) {
                    Some(fcc) => fcc,
                    None => {
                        index += 1;
                        // This condition should not be hit in normal cases.
                        // But we still add the check to ensure we can break out of the loop.
                        if index == 0 {
                            break;
                        }
                        continue;
                    }
                };

                for frame_rate in framerate_list {
                    if frame_rate != 0 {
                        camera_format_list.push(CameraFormat::new(
                            Resolution::new(width, height),
                            frame_fmt,
                            frame_rate,
                        ));
                    }
                }

                index += 1;
                // This condition should not be hit in normal cases.
                // But we still add the check to ensure we can break out of the loop.
                if index == 0 {
                    break;
                }
            }
            Ok(camera_format_list)
        }

        pub fn control(&self, control: KnownCameraControl) -> Result<CameraControl, NokhwaError> {
            let camera_control = unsafe {
                let mut receiver: MaybeUninit<IAMCameraControl> = MaybeUninit::uninit();
                let ptr_receiver = receiver.as_mut_ptr();
                if let Err(why) = self.source_reader.GetServiceForStream(
                    MF_SOURCE_READER_MEDIASOURCE,
                    &GUID_NULL,
                    &IAMCameraControl::IID,
                    ptr_receiver
                        .cast::<IAMCameraControl>()
                        .cast::<*mut c_void>(),
                ) {
                    return Err(NokhwaError::SetPropertyError {
                        property: "MF_SOURCE_READER_MEDIASOURCE".to_string(),
                        value: "IAMCameraControl".to_string(),
                        error: why.to_string(),
                    });
                }
                receiver.assume_init()
            };
            let video_proc_amp = unsafe {
                let mut receiver: MaybeUninit<IAMVideoProcAmp> = MaybeUninit::uninit();
                let ptr_receiver = receiver.as_mut_ptr();
                if let Err(why) = self.source_reader.GetServiceForStream(
                    MF_SOURCE_READER_MEDIASOURCE,
                    &GUID_NULL,
                    &IAMVideoProcAmp::IID,
                    ptr_receiver.cast::<IAMVideoProcAmp>().cast::<*mut c_void>(),
                ) {
                    return Err(NokhwaError::SetPropertyError {
                        property: "MF_SOURCE_READER_MEDIASOURCE".to_string(),
                        value: "IAMVideoProcAmp".to_string(),
                        error: why.to_string(),
                    });
                }
                receiver.assume_init()
            };

            let mut min = 0;
            let mut max = 0;
            let mut step = 0;
            let mut default = 0;
            let mut value = 0;
            let mut flag = 0;

            let control_id = kcc_to_i32(control).ok_or(NokhwaError::SetPropertyError {
                property: "CameraControl".to_string(),
                value: control.to_string(),
                error: "Does not exist".to_string(),
            })?;

            let ctrl_value_set = match control_id {
                MFControlId::ProcAmpBoolean(id) => unsafe {
                    if let Err(why) = video_proc_amp.GetRange(
                        id,
                        &mut min,
                        &mut max,
                        &mut step,
                        &mut default,
                        &mut flag,
                    ) {
                        return Err(NokhwaError::GetPropertyError {
                            property: format!("{:?}: {} - Range", control_id, control),
                            error: why.to_string(),
                        });
                    }
                    if let Err(why) = video_proc_amp.Get(id, &mut value, &mut flag) {
                        return Err(NokhwaError::GetPropertyError {
                            property: format!("{:?}: {} - Value", control_id, control),
                            error: why.to_string(),
                        });
                    }

                    let boolval = value != 0;
                    let booldef = default != 0;
                    ControlValueDescription::Boolean {
                        value: boolval,
                        default: booldef,
                    }
                },
                MFControlId::ProcAmpRange(id) => unsafe {
                    if let Err(why) = video_proc_amp.GetRange(
                        id,
                        &mut min,
                        &mut max,
                        &mut step,
                        &mut default,
                        &mut flag,
                    ) {
                        return Err(NokhwaError::GetPropertyError {
                            property: format!("{:?}: {} - Range", control_id, control),
                            error: why.to_string(),
                        });
                    }
                    if let Err(why) = video_proc_amp.Get(id, &mut value, &mut flag) {
                        return Err(NokhwaError::GetPropertyError {
                            property: format!("{:?}: {} - Value", control_id, control),
                            error: why.to_string(),
                        });
                    }
                    ControlValueDescription::IntegerRange {
                        min: i64::from(min),
                        max: i64::from(max),
                        value: i64::from(value),
                        step: i64::from(step),
                        default: i64::from(default),
                    }
                },
                MFControlId::CCValue(id) => unsafe {
                    if let Err(why) = camera_control.GetRange(
                        id,
                        &mut min,
                        &mut max,
                        &mut step,
                        &mut default,
                        &mut flag,
                    ) {
                        return Err(NokhwaError::GetPropertyError {
                            property: format!("{:?}: {} - Range", control_id, control),
                            error: why.to_string(),
                        });
                    }
                    if let Err(why) = camera_control.Get(id, &mut value, &mut flag) {
                        return Err(NokhwaError::GetPropertyError {
                            property: format!("{:?}: {} - Value", control_id, control),
                            error: why.to_string(),
                        });
                    }

                    ControlValueDescription::Integer {
                        value: i64::from(value),
                        default: i64::from(default),
                        step: i64::from(step),
                    }
                },
                MFControlId::CCRange(id) => unsafe {
                    if let Err(why) = camera_control.GetRange(
                        id,
                        &mut min,
                        &mut max,
                        &mut step,
                        &mut default,
                        &mut flag,
                    ) {
                        return Err(NokhwaError::GetPropertyError {
                            property: format!("{:?}: {} - Range", control_id, control),
                            error: why.to_string(),
                        });
                    }
                    if let Err(why) = camera_control.Get(id, &mut value, &mut flag) {
                        return Err(NokhwaError::GetPropertyError {
                            property: format!("{:?}: {} - Value", control_id, control),
                            error: why.to_string(),
                        });
                    }
                    ControlValueDescription::IntegerRange {
                        min: i64::from(min),
                        max: i64::from(max),
                        value: i64::from(value),
                        step: i64::from(step),
                        default: i64::from(default),
                    }
                },
            };

            let is_manual = if flag == CameraControl_Flags_Manual.0 {
                KnownCameraControlFlag::Manual
            } else {
                KnownCameraControlFlag::Automatic
            };

            Ok(CameraControl::new(
                control,
                control.to_string(),
                ctrl_value_set,
                vec![is_manual],
                true,
            ))
        }

        pub fn set_control(
            &mut self,
            control: KnownCameraControl,
            value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            let current_value = self.control(control)?;

            let camera_control = unsafe {
                let mut receiver: MaybeUninit<IAMCameraControl> = MaybeUninit::uninit();
                let ptr_receiver = receiver.as_mut_ptr();
                if let Err(why) = self.source_reader.GetServiceForStream(
                    MF_SOURCE_READER_MEDIASOURCE,
                    &GUID_NULL,
                    &IAMCameraControl::IID,
                    ptr_receiver
                        .cast::<IAMCameraControl>()
                        .cast::<*mut c_void>(),
                ) {
                    return Err(NokhwaError::SetPropertyError {
                        property: "MF_SOURCE_READER_MEDIASOURCE".to_string(),
                        value: "IAMCameraControl".to_string(),
                        error: why.to_string(),
                    });
                }
                receiver.assume_init()
            };
            let video_proc_amp = unsafe {
                let mut receiver: MaybeUninit<IAMVideoProcAmp> = MaybeUninit::uninit();
                let ptr_receiver = receiver.as_mut_ptr();
                if let Err(why) = self.source_reader.GetServiceForStream(
                    MF_SOURCE_READER_MEDIASOURCE,
                    &GUID_NULL,
                    &IAMVideoProcAmp::IID,
                    ptr_receiver.cast::<IAMVideoProcAmp>().cast::<*mut c_void>(),
                ) {
                    return Err(NokhwaError::SetPropertyError {
                        property: "MF_SOURCE_READER_MEDIASOURCE".to_string(),
                        value: "IAMVideoProcAmp".to_string(),
                        error: why.to_string(),
                    });
                }
                receiver.assume_init()
            };

            let control_id = kcc_to_i32(control).ok_or(NokhwaError::SetPropertyError {
                property: "CameraControl".to_string(),
                value: control.to_string(),
                error: "Does not exist".to_string(),
            })?;

            let ctrl_value = match value {
                ControlValueSetter::Integer(i) => i as i32,
                ControlValueSetter::Boolean(b) => i32::from(b),
                v => {
                    return Err(NokhwaError::StructureError {
                        structure: format!("ControlValueSetter {}", v),
                        error: "invalid value type".to_string(),
                    })
                }
            };

            let flag = current_value
                .flag()
                .get(0)
                .map(|x| {
                    if *x == KnownCameraControlFlag::Automatic {
                        CameraControl_Flags_Auto
                    } else {
                        CameraControl_Flags_Manual
                    }
                })
                .ok_or(NokhwaError::StructureError {
                    structure: "KnownCameraControlFlag".to_string(),
                    error: "could not cast to i32".to_string(),
                })?;

            match control_id {
                MFControlId::ProcAmpBoolean(id) | MFControlId::ProcAmpRange(id) => unsafe {
                    if let Err(why) = video_proc_amp.Set(id, ctrl_value, flag.0) {
                        return Err(NokhwaError::SetPropertyError {
                            property: control.to_string(),
                            value: ctrl_value.to_string(),
                            error: why.to_string(),
                        });
                    }
                },
                MFControlId::CCValue(id) | MFControlId::CCRange(id) => unsafe {
                    if let Err(why) = camera_control.Set(id, ctrl_value, flag.0) {
                        return Err(NokhwaError::SetPropertyError {
                            property: control.to_string(),
                            value: ctrl_value.to_string(),
                            error: why.to_string(),
                        });
                    }
                },
            }

            Ok(())
        }

        #[allow(clippy::cast_sign_loss)]
        pub fn format_refreshed(&mut self) -> Result<CameraFormat, NokhwaError> {
            match unsafe {
                self.source_reader
                    .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
            } {
                Ok(media_type) => {
                    let resolution = match unsafe { media_type.GetUINT64(&MF_MT_FRAME_SIZE) } {
                        Ok(res) => {
                            let width = (res >> 32) as u32;
                            let height = ((res << 32) >> 32) as u32;

                            Resolution {
                                width_x: width,
                                height_y: height,
                            }
                        }
                        Err(why) => {
                            return Err(NokhwaError::GetPropertyError {
                                property: "MF_MT_FRAME_SIZE".to_string(),
                                error: why.to_string(),
                            })
                        }
                    };

                    let frame_rate = match unsafe { media_type.GetUINT64(&MF_MT_FRAME_RATE) } {
                        Ok(fps) => fps as u32,
                        Err(why) => {
                            return Err(NokhwaError::GetPropertyError {
                                property: "MF_MT_FRAME_RATE".to_string(),
                                error: why.to_string(),
                            })
                        }
                    };

                    let format = match unsafe { media_type.GetGUID(&MF_MT_SUBTYPE) } {
                        Ok(fcc) => match guid_to_frameformat(fcc) {
                            Some(ff) => ff,
                            None => {
                                return Err(NokhwaError::GetPropertyError {
                                    property: "MF_MT_SUBTYPE".to_string(),
                                    error: "Unknown".to_string(),
                                })
                            }
                        },
                        Err(why) => {
                            return Err(NokhwaError::GetPropertyError {
                                property: "MF_MT_SUBTYPE".to_string(),
                                error: why.to_string(),
                            })
                        }
                    };

                    let cfmt = CameraFormat::new(resolution, format, frame_rate);
                    self.device_format = cfmt;

                    Ok(cfmt)
                }
                Err(why) => Err(NokhwaError::GetPropertyError {
                    property: "MF_SOURCE_READER_FIRST_VIDEO_STREAM".to_string(),
                    error: why.to_string(),
                }),
            }
        }

        pub fn format(&self) -> CameraFormat {
            self.device_format
        }

        pub fn set_format(&mut self, format: CameraFormat) -> Result<(), NokhwaError> {
            // convert to media_type
            let media_type: IMFMediaType = match unsafe { MFCreateMediaType() } {
                Ok(mt) => mt,
                Err(why) => {
                    return Err(NokhwaError::StructureError {
                        structure: "IMFMediaType".to_string(),
                        error: why.to_string(),
                    })
                }
            };

            // set relevant things
            let resolution = (u64::from(format.resolution().width_x) << 32_u64)
                + u64::from(format.resolution().height_y);
            let fps = {
                let frame_rate_u64 = 0_u64;
                let mut bytes: [u8; 8] = frame_rate_u64.to_le_bytes();
                bytes[7] = format.frame_rate() as u8;
                bytes[3] = 0x01;
                u64::from_le_bytes(bytes)
            };
            let fourcc = frameformat_to_guid(format.format());
            // setting to the new media_type
            if let Err(why) = unsafe { media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video) } {
                return Err(NokhwaError::SetPropertyError {
                    property: "MF_MT_MAJOR_TYPE".to_string(),
                    value: "MFMediaType_Video".to_string(),
                    error: why.to_string(),
                });
            }
            if let Err(why) = unsafe { media_type.SetGUID(&MF_MT_SUBTYPE, &fourcc) } {
                return Err(NokhwaError::SetPropertyError {
                    property: "MF_MT_SUBTYPE".to_string(),
                    value: format!("{:?}", fourcc),
                    error: why.to_string(),
                });
            }
            if let Err(why) = unsafe { media_type.SetUINT64(&MF_MT_FRAME_SIZE, resolution) } {
                return Err(NokhwaError::SetPropertyError {
                    property: "MF_MT_FRAME_SIZE".to_string(),
                    value: resolution.to_string(),
                    error: why.to_string(),
                });
            }
            if let Err(why) = unsafe { media_type.SetUINT64(&MF_MT_FRAME_RATE, fps) } {
                return Err(NokhwaError::SetPropertyError {
                    property: "MF_MT_FRAME_RATE".to_string(),
                    value: fps.to_string(),
                    error: why.to_string(),
                });
            }
            if let Err(why) = unsafe { media_type.SetUINT64(&MF_MT_FRAME_RATE_RANGE_MIN, fps) } {
                return Err(NokhwaError::SetPropertyError {
                    property: "MF_MT_FRAME_RATE_RANGE_MIN".to_string(),
                    value: fps.to_string(),
                    error: why.to_string(),
                });
            }
            if let Err(why) = unsafe { media_type.SetUINT64(&MF_MT_FRAME_RATE_RANGE_MAX, fps) } {
                return Err(NokhwaError::SetPropertyError {
                    property: "MF_MT_FRAME_RATE_RANGE_MAX".to_string(),
                    value: fps.to_string(),
                    error: why.to_string(),
                });
            }

            if let Err(why) = unsafe {
                self.source_reader.SetCurrentMediaType(
                    MEDIA_FOUNDATION_FIRST_VIDEO_STREAM,
                    None,
                    &media_type,
                )
            } {
                return Err(NokhwaError::SetPropertyError {
                    property: "MEDIA_FOUNDATION_FIRST_VIDEO_STREAM".to_string(),
                    value: format!("{media_type:?}"),
                    error: why.to_string(),
                });
            }
            self.device_format = format;
            self.format_refreshed()?;
            Ok(())
        }

        pub fn is_stream_open(&self) -> bool {
            self.is_open.get()
        }

        pub fn start_stream(&mut self) -> Result<(), NokhwaError> {
            if let Err(why) = unsafe {
                self.source_reader
                    .SetStreamSelection(MEDIA_FOUNDATION_FIRST_VIDEO_STREAM, true)
            } {
                return Err(NokhwaError::OpenStreamError(why.to_string()));
            }

            self.is_open.set(true);
            Ok(())
        }

        pub fn raw_bytes(&mut self) -> Result<Cow<[u8]>, NokhwaError> {
            let mut imf_sample: Option<IMFSample> = match unsafe { MFCreateSample() } {
                Ok(sample) => Some(sample),
                Err(why) => {
                    return Err(NokhwaError::ReadFrameError(why.to_string()));
                }
            };
            let mut stream_flags = 0;
            {
                loop {
                    if let Err(why) = unsafe {
                        self.source_reader.ReadSample(
                            MEDIA_FOUNDATION_FIRST_VIDEO_STREAM,
                            0,
                            None,
                            Some(&mut stream_flags),
                            None,
                            Some(&mut imf_sample),
                        )
                    } {
                        return Err(NokhwaError::ReadFrameError(why.to_string()));
                    }

                    if imf_sample.is_some() {
                        break;
                    }
                }
            }

            let imf_sample = match imf_sample {
                Some(sample) => sample,
                None => {
                    // shouldn't happen
                    return Err(NokhwaError::ReadFrameError("No sample".to_string()));
                }
            };

            let buffer = match unsafe { imf_sample.ConvertToContiguousBuffer() } {
                Ok(buf) => buf,
                Err(why) => return Err(NokhwaError::ReadFrameError(why.to_string())),
            };

            let mut buffer_valid_length = 0;
            let mut buffer_start_ptr = std::ptr::null_mut::<u8>();

            if let Err(why) =
                unsafe { buffer.Lock(&mut buffer_start_ptr, None, Some(&mut buffer_valid_length)) }
            {
                return Err(NokhwaError::ReadFrameError(why.to_string()));
            }

            if buffer_start_ptr.is_null() {
                return Err(NokhwaError::ReadFrameError(
                    "Buffer Pointer Null".to_string(),
                ));
            }

            if buffer_valid_length == 0 {
                return Err(NokhwaError::ReadFrameError("Buffer Size is 0".to_string()));
            }

            let mut data_slice = Vec::with_capacity(buffer_valid_length as usize);

            unsafe {
                // Copy pointer because we're bout to drop IMFSample
                data_slice.extend_from_slice(std::slice::from_raw_parts_mut(
                    buffer_start_ptr,
                    buffer_valid_length as usize,
                ) as &[u8]);
            }

            Ok(Cow::from(data_slice))
        }

        pub fn stop_stream(&mut self) {
            self.is_open.set(false);
        }
    }

    impl Drop for MediaFoundationDevice {
        fn drop(&mut self) {
            // swallow errors
            unsafe {
                if self
                    .source_reader
                    .Flush(MEDIA_FOUNDATION_FIRST_VIDEO_STREAM)
                    .is_ok()
                {}

                // decrement refcnt
                if CAMERA_REFCNT.load(Ordering::SeqCst) > 0 {
                    CAMERA_REFCNT.store(CAMERA_REFCNT.load(Ordering::SeqCst) - 1, Ordering::SeqCst);
                }
                if CAMERA_REFCNT.load(Ordering::SeqCst) == 0 {
                    #[allow(clippy::let_underscore_drop)]
                    let _ = de_initialize_mf();
                }
            }
        }
    }
}

#[cfg(any(not(windows), feature = "docs-only"))]
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::unused_self)]
#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::must_use_candidate)]
pub mod wmf {
    use nokhwa_core::error::NokhwaError;
    use nokhwa_core::types::{
        CameraControl, CameraFormat, CameraIndex, CameraInfo, ControlValueSetter,
        KnownCameraControl,
    };
    use std::borrow::Cow;

    pub fn initialize_mf() -> Result<(), NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            "Not on windows".to_string(),
        ))
    }

    pub fn de_initialize_mf() -> Result<(), NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            "Not on windows".to_string(),
        ))
    }

    pub fn query_msmf() -> Result<Vec<CameraInfo>, NokhwaError> {
        Err(NokhwaError::NotImplementedError(
            "Not on windows".to_string(),
        ))
    }

    pub struct MediaFoundationDevice {
        camera: CameraIndex,
    }

    impl MediaFoundationDevice {
        pub fn new(_index: CameraIndex) -> Result<Self, NokhwaError> {
            Ok(MediaFoundationDevice {
                camera: CameraIndex::Index(0),
            })
        }

        pub fn index(&self) -> &CameraIndex {
            &self.camera
        }

        pub fn name(&self) -> String {
            String::new()
        }

        pub fn symlink(&self) -> String {
            String::new()
        }

        pub fn compatible_format_list(&mut self) -> Result<Vec<CameraFormat>, NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "Only on Windows".to_string(),
            ))
        }

        pub fn control(&self, _control: KnownCameraControl) -> Result<CameraControl, NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "Only on Windows".to_string(),
            ))
        }

        pub fn set_control(
            &mut self,
            _control: KnownCameraControl,
            _value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "Only on Windows".to_string(),
            ))
        }

        pub fn format_refreshed(&mut self) -> Result<CameraFormat, NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "Only on Windows".to_string(),
            ))
        }

        pub fn format(&self) -> CameraFormat {
            CameraFormat::default()
        }

        pub fn set_format(&mut self, _format: CameraFormat) -> Result<(), NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "Only on Windows".to_string(),
            ))
        }

        pub fn is_stream_open(&self) -> bool {
            false
        }

        pub fn start_stream(&mut self) -> Result<(), NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "Only on Windows".to_string(),
            ))
        }

        pub fn raw_bytes(&mut self) -> Result<Cow<[u8]>, NokhwaError> {
            Err(NokhwaError::NotImplementedError(
                "Only on Windows".to_string(),
            ))
        }

        pub fn stop_stream(&mut self) {}
    }

    impl Drop for MediaFoundationDevice {
        fn drop(&mut self) {}
    }
}
