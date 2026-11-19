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

// hello, future peng here
// whatever is written here will induce horrors uncomprehendable.
// save yourselves. write apple code in swift and bind it to rust.

// <some change so we can call this 0.10.4>

#![allow(clippy::not_unsafe_ptr_arg_deref)]

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[macro_use]
extern crate objc;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod internal {

    #[allow(non_snake_case)]
    pub mod core_media {
        // all of this is stolen from bindgen
        // steal it idc
        use crate::internal::CGFloat;
        use core_media_sys::{
            CMBlockBufferRef, CMFormatDescriptionRef, CMSampleBufferRef, CMTime, CMVideoDimensions,
            FourCharCode,
        };
        use objc::{runtime::Object, Message};
        use std::ops::Deref;

        pub type Id = *mut Object;

        #[repr(transparent)]
        #[derive(Clone)]
        pub struct NSObject(pub Id);
        impl Deref for NSObject {
            type Target = Object;
            fn deref(&self) -> &Self::Target {
                unsafe { &*self.0 }
            }
        }
        unsafe impl Message for NSObject {}
        impl NSObject {
            pub fn alloc() -> Self {
                Self(unsafe { msg_send!(objc::class!(NSObject), alloc) })
            }
        }

        #[repr(transparent)]
        #[derive(Clone)]
        pub struct NSString(pub Id);
        impl Deref for NSString {
            type Target = Object;
            fn deref(&self) -> &Self::Target {
                unsafe { &*self.0 }
            }
        }
        unsafe impl Message for NSString {}
        impl NSString {
            pub fn alloc() -> Self {
                Self(unsafe { msg_send!(objc::class!(NSString), alloc) })
            }
        }

        pub type AVMediaType = NSString;

        #[allow(non_snake_case)]
        #[link(name = "CoreMedia", kind = "framework")]
        extern "C" {
            pub fn CMVideoFormatDescriptionGetDimensions(
                videoDesc: CMFormatDescriptionRef,
            ) -> CMVideoDimensions;

            pub fn CMTimeMake(value: i64, scale: i32) -> CMTime;

            pub fn CMBlockBufferGetDataLength(theBuffer: CMBlockBufferRef) -> std::os::raw::c_int;

            pub fn CMBlockBufferCopyDataBytes(
                theSourceBuffer: CMBlockBufferRef,
                offsetToData: usize,
                dataLength: usize,
                destination: *mut std::os::raw::c_void,
            ) -> std::os::raw::c_int;

            pub fn CMSampleBufferGetDataBuffer(sbuf: CMSampleBufferRef) -> CMBlockBufferRef;

            pub fn dispatch_queue_create(
                label: *const std::os::raw::c_char,
                attr: NSObject,
            ) -> NSObject;

            pub fn dispatch_release(object: NSObject);

            pub fn CMSampleBufferGetImageBuffer(sbuf: CMSampleBufferRef) -> CVImageBufferRef;

            pub fn CVPixelBufferLockBaseAddress(
                pixelBuffer: CVPixelBufferRef,
                lockFlags: CVPixelBufferLockFlags,
            ) -> CVReturn;

            pub fn CVPixelBufferUnlockBaseAddress(
                pixelBuffer: CVPixelBufferRef,
                unlockFlags: CVPixelBufferLockFlags,
            ) -> CVReturn;

            pub fn CVPixelBufferGetDataSize(pixelBuffer: CVPixelBufferRef)
                -> std::os::raw::c_ulong;

            pub fn CVPixelBufferGetBaseAddress(
                pixelBuffer: CVPixelBufferRef,
            ) -> *mut std::os::raw::c_void;

            pub fn CVPixelBufferGetPixelFormatType(pixelBuffer: CVPixelBufferRef) -> OSType;

            pub fn CVPixelBufferGetPlaneCount(pixelBuffer: CVPixelBufferRef) -> usize;

            pub fn CVPixelBufferGetBaseAddressOfPlane(
                pixelBuffer: CVPixelBufferRef,
                planeIndex: usize,
            ) -> *mut std::os::raw::c_void;

            pub fn CVPixelBufferGetBytesPerRowOfPlane(
                pixelBuffer: CVPixelBufferRef,
                planeIndex: usize,
            ) -> usize;

            pub fn CVPixelBufferGetWidthOfPlane(
                pixelBuffer: CVPixelBufferRef,
                planeIndex: usize,
            ) -> usize;

            pub fn CVPixelBufferGetHeightOfPlane(
                pixelBuffer: CVPixelBufferRef,
                planeIndex: usize,
            ) -> usize;
        }

        #[repr(C)]
        #[derive(Clone, Debug, PartialEq, PartialOrd)]
        pub struct CGPoint {
            pub x: CGFloat,
            pub y: CGFloat,
        }

        #[repr(C)]
        #[derive(Debug, Copy, Clone)]
        pub struct __CVBuffer {
            _unused: [u8; 0],
        }

        #[allow(non_snake_case)]
        #[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
        #[repr(C)]
        pub struct AVCaptureWhiteBalanceGains {
            pub blueGain: f32,
            pub greenGain: f32,
            pub redGain: f32,
        }

        pub type CVBufferRef = *mut __CVBuffer;

        pub type CVImageBufferRef = CVBufferRef;
        pub type CVPixelBufferRef = CVImageBufferRef;
        pub type CVPixelBufferLockFlags = u64;
        pub type CVReturn = i32;

        pub type OSType = FourCharCode;
        pub type AVVideoCodecType = NSString;

        #[link(name = "AVFoundation", kind = "framework")]
        extern "C" {
            pub static AVVideoCodecKey: NSString;
            pub static AVVideoCodecTypeHEVC: AVVideoCodecType;
            pub static AVVideoCodecTypeH264: AVVideoCodecType;
            pub static AVVideoCodecTypeJPEG: AVVideoCodecType;
            pub static AVVideoCodecTypeAppleProRes4444: AVVideoCodecType;
            pub static AVVideoCodecTypeAppleProRes422: AVVideoCodecType;
            pub static AVVideoCodecTypeAppleProRes422HQ: AVVideoCodecType;
            pub static AVVideoCodecTypeAppleProRes422LT: AVVideoCodecType;
            pub static AVVideoCodecTypeAppleProRes422Proxy: AVVideoCodecType;
            pub static AVVideoCodecTypeHEVCWithAlpha: AVVideoCodecType;
            pub static AVVideoCodecHEVC: NSString;
            pub static AVVideoCodecH264: NSString;
            pub static AVVideoCodecJPEG: NSString;
            pub static AVVideoCodecAppleProRes4444: NSString;
            pub static AVVideoCodecAppleProRes422: NSString;
            pub static AVVideoWidthKey: NSString;
            pub static AVVideoHeightKey: NSString;
            pub static AVVideoExpectedSourceFrameRateKey: NSString;

            pub static AVMediaTypeVideo: AVMediaType;
            pub static AVMediaTypeAudio: AVMediaType;
            pub static AVMediaTypeText: AVMediaType;
            pub static AVMediaTypeClosedCaption: AVMediaType;
            pub static AVMediaTypeSubtitle: AVMediaType;
            pub static AVMediaTypeTimecode: AVMediaType;
            pub static AVMediaTypeMetadata: AVMediaType;
            pub static AVMediaTypeMuxed: AVMediaType;
            pub static AVMediaTypeMetadataObject: AVMediaType;
            pub static AVMediaTypeDepthData: AVMediaType;

            pub static AVCaptureLensPositionCurrent: f32;
            pub static AVCaptureExposureTargetBiasCurrent: f32;
            pub static AVCaptureExposureDurationCurrent: CMTime;
            pub static AVCaptureISOCurrent: f32;
        }
    }

    use crate::core_media::{
        dispatch_queue_create, AVCaptureExposureDurationCurrent,
        AVCaptureExposureTargetBiasCurrent, AVCaptureISOCurrent, AVCaptureWhiteBalanceGains,
        AVMediaTypeAudio, AVMediaTypeClosedCaption, AVMediaTypeDepthData, AVMediaTypeMetadata,
        AVMediaTypeMetadataObject, AVMediaTypeMuxed, AVMediaTypeSubtitle, AVMediaTypeText,
        AVMediaTypeTimecode, AVMediaTypeVideo, CGPoint, CMSampleBufferGetImageBuffer,
        CMVideoFormatDescriptionGetDimensions, CVImageBufferRef, CVPixelBufferGetBaseAddress,
        CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane,
        CVPixelBufferGetDataSize, CVPixelBufferGetHeightOfPlane, CVPixelBufferGetPlaneCount,
        CVPixelBufferGetWidthOfPlane, CVPixelBufferLockBaseAddress, CVPixelBufferUnlockBaseAddress,
        NSObject, OSType,
    };

    use block::ConcreteBlock;
    use cocoa_foundation::{
        base::Nil,
        foundation::{NSArray, NSDictionary, NSInteger, NSString, NSUInteger},
    };
    use core_media_sys::{
        kCMPixelFormat_24RGB, kCMPixelFormat_422YpCbCr8_yuvs,
        kCMPixelFormat_8IndexedGray_WhiteIsZero, kCMVideoCodecType_422YpCbCr8,
        kCMVideoCodecType_JPEG, kCMVideoCodecType_JPEG_OpenDML, CMFormatDescriptionGetMediaSubType,
        CMFormatDescriptionRef, CMSampleBufferRef, CMTime, CMVideoDimensions,
    };
    use core_video_sys::{
        kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange,
        kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
    };
    use flume::{Receiver, Sender};
    use nokhwa_core::{
        error::NokhwaError,
        types::{
            ApiBackend, CameraControl, CameraFormat, CameraIndex, CameraInfo,
            ControlValueDescription, ControlValueSetter, FrameFormat, KnownCameraControl,
            KnownCameraControlFlag, Resolution,
        },
    };
    use objc::runtime::objc_getClass;
    use objc::{
        declare::ClassDecl,
        runtime::{Class, Object, Protocol, Sel, BOOL, NO, YES},
    };
    use once_cell::sync::Lazy;
    use std::ffi::CString;
    use std::{
        borrow::Cow,
        cmp::Ordering,
        collections::BTreeMap,
        convert::TryFrom,
        error::Error,
        ffi::{c_float, c_void, CStr},
        sync::Arc,
    };

    const UTF8_ENCODING: usize = 4;
    type CGFloat = c_float;

    macro_rules! create_boilerplate_impl {
        {
            $( [$class_vis:vis $class_name:ident : $( {$field_vis:vis $field_name:ident : $field_type:ty} ),*] ),+
        } => {
            $(
                $class_vis struct $class_name {
                    inner: *mut Object,
                    $(
                        $field_vis $field_name : $field_type
                    )*
                }

                impl $class_name {
                    pub fn inner(&self) -> *mut Object {
                        self.inner
                    }
                }
            )+
        };

        {
            $( [$class_vis:vis $class_name:ident ] ),+
        } => {
            $(
                $class_vis struct $class_name {
                    inner: *mut Object,
                }

                impl $class_name {
                    pub fn inner(&self) -> *mut Object {
                        self.inner
                    }
                }

                impl From<*mut Object> for $class_name {
                    fn from(obj: *mut Object) -> Self {
                        $class_name {
                            inner: obj,
                        }
                    }
                }
            )+
        };
    }

    fn str_to_nsstr(string: &str) -> *mut Object {
        let cls = class!(NSString);
        let bytes = string.as_ptr() as *const c_void;
        unsafe {
            let obj: *mut Object = msg_send![cls, alloc];
            let obj: *mut Object = msg_send![
                obj,
                initWithBytes:bytes
                length:string.len()
                encoding:UTF8_ENCODING
            ];
            obj
        }
    }

    fn nsstr_to_str<'a>(nsstr: *mut Object) -> Cow<'a, str> {
        let data = unsafe { CStr::from_ptr(nsstr.UTF8String()) };
        data.to_string_lossy()
    }

    fn vec_to_ns_arr<T: Into<*mut Object>>(data: Vec<T>) -> *mut Object {
        let cstr = CString::new("NSMutableArray").unwrap();
        let ns_arr_cls = unsafe { objc_getClass(cstr.as_ptr()) };
        let mutable_array: *mut Object = unsafe { msg_send![ns_arr_cls, array] };
        data.into_iter().for_each(|item| {
            let item_obj: *mut Object = item.into();
            let _: () = unsafe { msg_send![mutable_array, addObject: item_obj] };
        });
        mutable_array
    }

    fn ns_arr_to_vec<T: From<*mut Object>>(data: *mut Object) -> Vec<T> {
        let length = unsafe { NSArray::count(data) };

        let mut out_vec: Vec<T> = Vec::with_capacity(length as usize);
        for index in 0..length {
            let item = unsafe { NSArray::objectAtIndex(data, index) };
            out_vec.push(T::from(item));
        }
        out_vec
    }

    fn try_ns_arr_to_vec<T, TE>(data: *mut Object) -> Result<Vec<T>, TE>
    where
        TE: Error,
        T: TryFrom<*mut Object, Error = TE>,
    {
        let length = unsafe { NSArray::count(data) };

        let mut out_vec: Vec<T> = Vec::with_capacity(length as usize);
        for index in 0..length {
            let item = unsafe { NSArray::objectAtIndex(data, index) };
            out_vec.push(T::try_from(item)?);
        }
        Ok(out_vec)
    }

    fn compare_ns_string(this: *mut Object, other: core_media::NSString) -> bool {
        unsafe {
            let equal: BOOL = msg_send![this, isEqualToString: other];
            equal == YES
        }
    }

    #[allow(non_upper_case_globals)]
    fn raw_fcc_to_frameformat(raw: OSType) -> Option<FrameFormat> {
        match raw {
            kCMVideoCodecType_422YpCbCr8 | kCMPixelFormat_422YpCbCr8_yuvs => {
                Some(FrameFormat::YUYV)
            }
            kCMVideoCodecType_JPEG | kCMVideoCodecType_JPEG_OpenDML => Some(FrameFormat::MJPEG),
            kCMPixelFormat_8IndexedGray_WhiteIsZero => Some(FrameFormat::GRAY),
            kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange
            | kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
            | 875704438 => Some(FrameFormat::NV12),
            kCMPixelFormat_24RGB => Some(FrameFormat::RAWRGB),
            _ => None,
        }
    }

    pub type CompressionData<'a> = (Cow<'a, [u8]>, FrameFormat);
    pub type DataPipe<'a> = (Sender<CompressionData<'a>>, Receiver<CompressionData<'a>>);

    static CALLBACK_CLASS: Lazy<&'static Class> = Lazy::new(|| {
        {
            let mut decl = ClassDecl::new("MyCaptureCallback", class!(NSObject)).unwrap();

            // frame stack
            // oooh scary provenannce-breaking BULLSHIT AAAAAA I LOVE TYPE ERASURE
            decl.add_ivar::<*const c_void>("_arcmutptr"); // ArkMutex, the not-arknights totally not gacha totally not ripoff new vidya game from l-pleasestop-npengtul

            extern "C" fn my_callback_get_arcmutptr(this: &Object, _: Sel) -> *const c_void {
                unsafe { *this.get_ivar("_arcmutptr") }
            }
            extern "C" fn my_callback_set_arcmutptr(
                this: &mut Object,
                _: Sel,
                new_arcmutptr: *const c_void,
            ) {
                unsafe {
                    this.set_ivar("_arcmutptr", new_arcmutptr);
                }
            }

            // Delegate compliance method
            // SAFETY: This assumes that the buffer byte size is a u8. Any other size will cause unsafety.
            #[allow(non_snake_case)]
            #[allow(non_upper_case_globals)]
            extern "C" fn capture_out_callback(
                this: &mut Object,
                _: Sel,
                _: *mut Object,
                didOutputSampleBuffer: CMSampleBufferRef,
                _: *mut Object,
            ) {
                let image_buffer: CVImageBufferRef =
                    unsafe { CMSampleBufferGetImageBuffer(didOutputSampleBuffer) };
                unsafe {
                    CVPixelBufferLockBaseAddress(image_buffer, 0);
                };

                // Bi-planar buffers (e.g. NV12) have no single base address: copy every plane row by
                // row so that the result is tightly packed (no per-row padding), which is what the
                // NV12 => RGB conversion expects (width * height * 3 / 2 bytes).
                let plane_count = unsafe { CVPixelBufferGetPlaneCount(image_buffer) };
                let buffer_as_vec = if plane_count > 0 {
                    let mut out = Vec::new();
                    for plane in 0..plane_count {
                        let base = unsafe { CVPixelBufferGetBaseAddressOfPlane(image_buffer, plane) }
                            as *const u8;
                        if base.is_null() {
                            continue;
                        }
                        let stride = unsafe { CVPixelBufferGetBytesPerRowOfPlane(image_buffer, plane) };
                        let width = unsafe { CVPixelBufferGetWidthOfPlane(image_buffer, plane) };
                        let height = unsafe { CVPixelBufferGetHeightOfPlane(image_buffer, plane) };
                        // Plane 0 is luma (1 byte per pixel); the chroma plane is interleaved CbCr (2 bytes per sample).
                        let row_bytes = if plane == 0 { width } else { width * 2 }.min(stride);
                        out.reserve(row_bytes * height);
                        for row in 0..height {
                            let line =
                                unsafe { std::slice::from_raw_parts(base.add(row * stride), row_bytes) };
                            out.extend_from_slice(line);
                        }
                    }
                    out
                } else {
                    let buffer_length = unsafe { CVPixelBufferGetDataSize(image_buffer) };
                    let buffer_ptr = unsafe { CVPixelBufferGetBaseAddress(image_buffer) };
                    unsafe {
                        std::slice::from_raw_parts_mut(buffer_ptr as *mut u8, buffer_length as usize)
                            .to_vec()
                    }
                };

                unsafe { CVPixelBufferUnlockBaseAddress(image_buffer, 0) };
                // oooooh scarey unsafe
                // AAAAAAAAAAAAAAAAAAAAAAAAA
                // https://c.tenor.com/0e_zWtFLOzQAAAAC/needy-streamer-overload-needy-girl-overdose.gif
                let bufferlck_cv: *const c_void = unsafe { msg_send![this, bufferPtr] };
                let buffer_sndr = unsafe {
                    let ptr = bufferlck_cv.cast::<Sender<(Vec<u8>, FrameFormat)>>();
                    Arc::from_raw(ptr)
                };
                if let Err(_) = buffer_sndr.send((buffer_as_vec, FrameFormat::GRAY)) {
                    // FIXME: dont, what the fuck???
                    return;
                }
                std::mem::forget(buffer_sndr);
            }

            #[allow(non_snake_case)]
            extern "C" fn capture_drop_callback(
                _: &mut Object,
                _: Sel,
                _: *mut Object,
                _: *mut Object,
                _: *mut Object,
            ) {
            }

            unsafe {
                decl.add_method(
                    sel!(bufferPtr),
                    my_callback_get_arcmutptr as extern "C" fn(&Object, Sel) -> *const c_void,
                );
                decl.add_method(
                    sel!(SetBufferPtr:),
                    my_callback_set_arcmutptr as extern "C" fn(&mut Object, Sel, *const c_void),
                );
                decl.add_method(
                    sel!(captureOutput:didOutputSampleBuffer:fromConnection:),
                    capture_out_callback
                        as extern "C" fn(
                            &mut Object,
                            Sel,
                            *mut Object,
                            CMSampleBufferRef,
                            *mut Object,
                        ),
                );
                decl.add_method(
                    sel!(captureOutput:didDropSampleBuffer:fromConnection:),
                    capture_drop_callback
                        as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, *mut Object),
                );

                decl.add_protocol(
                    Protocol::get("AVCaptureVideoDataOutputSampleBufferDelegate").unwrap(),
                );
            }

            decl.register()
        }
    });

    pub fn request_permission_with_callback(callback: impl Fn(bool) + Send + Sync + 'static) {
        let cls = class!(AVCaptureDevice);

        let wrapper = move |bool: BOOL| {
            callback(bool == YES);
        };

        let objc_fn_block: ConcreteBlock<(BOOL,), (), _> = ConcreteBlock::new(wrapper);
        let objc_fn_pass = objc_fn_block.copy();

        unsafe {
            let _: () = msg_send![cls, requestAccessForMediaType:(AVMediaTypeVideo.clone()) completionHandler:objc_fn_pass];
        }
    }

    pub fn current_authorization_status() -> AVAuthorizationStatus {
        let cls = class!(AVCaptureDevice);
        let status: AVAuthorizationStatus = unsafe {
            msg_send![cls, authorizationStatusForMediaType:AVMediaType::Video.into_ns_str()]
        };
        status
    }

    // fuck it, use deprecated APIs
    pub fn query_avfoundation() -> Result<Vec<CameraInfo>, NokhwaError> {
        Ok(AVCaptureDeviceDiscoverySession::new(vec![
            AVCaptureDeviceType::UltraWide,
            AVCaptureDeviceType::WideAngle,
            AVCaptureDeviceType::Telephoto,
            AVCaptureDeviceType::TrueDepth,
            AVCaptureDeviceType::ExternalUnknown,
        ])?
        .devices())
    }

    pub fn get_raw_device_info(index: CameraIndex, device: *mut Object) -> CameraInfo {
        let name = nsstr_to_str(unsafe { msg_send![device, localizedName] });
        let manufacturer = nsstr_to_str(unsafe { msg_send![device, manufacturer] });
        let position: AVCaptureDevicePosition = unsafe { msg_send![device, position] };
        let lens_aperture: f64 = unsafe { msg_send![device, lensAperture] };
        let device_type = nsstr_to_str(unsafe { msg_send![device, deviceType] });
        let model_id = nsstr_to_str(unsafe { msg_send![device, modelID] });
        let description = format!(
            "{}: {} - {}, {:?} f{}",
            manufacturer, model_id, device_type, position, lens_aperture
        );
        let misc = nsstr_to_str(unsafe { msg_send![device, uniqueID] });

        CameraInfo::new(name.as_ref(), &description, misc.as_ref(), index)
    }

    #[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
    pub enum AVCaptureDeviceType {
        Dual,
        DualWide,
        Triple,
        WideAngle,
        UltraWide,
        Telephoto,
        TrueDepth,
        ExternalUnknown,
    }

    impl From<AVCaptureDeviceType> for *mut Object {
        fn from(device_type: AVCaptureDeviceType) -> Self {
            match device_type {
                AVCaptureDeviceType::Dual => str_to_nsstr("AVCaptureDeviceTypeBuiltInDualCamera"),
                AVCaptureDeviceType::DualWide => {
                    str_to_nsstr("AVCaptureDeviceTypeBuiltInDualWideCamera")
                }
                AVCaptureDeviceType::Triple => {
                    str_to_nsstr("AVCaptureDeviceTypeBuiltInTripleCamera")
                }
                AVCaptureDeviceType::WideAngle => {
                    str_to_nsstr("AVCaptureDeviceTypeBuiltInWideAngleCamera")
                }
                AVCaptureDeviceType::UltraWide => {
                    str_to_nsstr("AVCaptureDeviceTypeBuiltInUltraWideCamera")
                }
                AVCaptureDeviceType::Telephoto => {
                    str_to_nsstr("AVCaptureDeviceTypeBuiltInTelephotoCamera")
                }
                AVCaptureDeviceType::TrueDepth => {
                    str_to_nsstr("AVCaptureDeviceTypeBuiltInTrueDepthCamera")
                }
                AVCaptureDeviceType::ExternalUnknown => {
                    str_to_nsstr("AVCaptureDeviceTypeExternalUnknown")
                }
            }
        }
    }

    impl AVCaptureDeviceType {
        pub fn into_ns_str(self) -> *mut Object {
            <*mut Object>::from(self)
        }
    }

    #[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
    pub enum AVMediaType {
        Audio,
        ClosedCaption,
        DepthData,
        Metadata,
        MetadataObject,
        Muxed,
        Subtitle,
        Text,
        Timecode,
        Video,
    }

    impl From<AVMediaType> for *mut Object {
        fn from(media_type: AVMediaType) -> Self {
            match media_type {
                AVMediaType::Audio => unsafe { AVMediaTypeAudio.0 },
                AVMediaType::ClosedCaption => unsafe { AVMediaTypeClosedCaption.0 },
                AVMediaType::DepthData => unsafe { AVMediaTypeDepthData.0 },
                AVMediaType::Metadata => unsafe { AVMediaTypeMetadata.0 },
                AVMediaType::MetadataObject => unsafe { AVMediaTypeMetadataObject.0 },
                AVMediaType::Muxed => unsafe { AVMediaTypeMuxed.0 },
                AVMediaType::Subtitle => unsafe { AVMediaTypeSubtitle.0 },
                AVMediaType::Text => unsafe { AVMediaTypeText.0 },
                AVMediaType::Timecode => unsafe { AVMediaTypeTimecode.0 },
                AVMediaType::Video => unsafe { AVMediaTypeVideo.0 },
            }
        }
    }

    impl TryFrom<*mut Object> for AVMediaType {
        type Error = NokhwaError;

        fn try_from(value: *mut Object) -> Result<Self, Self::Error> {
            unsafe {
                if compare_ns_string(value, (AVMediaTypeAudio).clone()) {
                    Ok(AVMediaType::Audio)
                } else if compare_ns_string(value, (AVMediaTypeClosedCaption).clone()) {
                    Ok(AVMediaType::ClosedCaption)
                } else if compare_ns_string(value, (AVMediaTypeDepthData).clone()) {
                    Ok(AVMediaType::DepthData)
                } else if compare_ns_string(value, (AVMediaTypeMetadata).clone()) {
                    Ok(AVMediaType::Metadata)
                } else if compare_ns_string(value, (AVMediaTypeMetadataObject).clone()) {
                    Ok(AVMediaType::MetadataObject)
                } else if compare_ns_string(value, (AVMediaTypeMuxed).clone()) {
                    Ok(AVMediaType::Muxed)
                } else if compare_ns_string(value, (AVMediaTypeSubtitle).clone()) {
                    Ok(AVMediaType::Subtitle)
                } else if compare_ns_string(value, (AVMediaTypeText).clone()) {
                    Ok(AVMediaType::Text)
                } else if compare_ns_string(value, (AVMediaTypeTimecode).clone()) {
                    Ok(AVMediaType::Timecode)
                } else if compare_ns_string(value, (AVMediaTypeVideo).clone()) {
                    Ok(AVMediaType::Video)
                } else {
                    let name = nsstr_to_str(value);
                    Err(NokhwaError::GetPropertyError {
                        property: "AVMediaType".to_string(),
                        error: format!("Invalid AVMediaType {name}"),
                    })
                }
            }
        }
    }

    impl AVMediaType {
        pub fn into_ns_str(self) -> *mut Object {
            <*mut Object>::from(self)
        }
    }

    #[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
    #[repr(isize)]
    pub enum AVCaptureDevicePosition {
        Unspecified = 0,
        Back = 1,
        Front = 2,
    }

    #[derive(Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq)]
    #[repr(isize)]
    pub enum AVAuthorizationStatus {
        NotDetermined = 0,
        Restricted = 1,
        Denied = 2,
        Authorized = 3,
    }

    pub struct AVCaptureVideoCallback {
        delegate: *mut Object,
        queue: NSObject,
    }

    impl AVCaptureVideoCallback {
        pub fn new(
            device_spec: &CStr,
            buffer: &Arc<Sender<(Vec<u8>, FrameFormat)>>,
        ) -> Result<Self, NokhwaError> {
            let cls = &CALLBACK_CLASS as &Class;
            let delegate: *mut Object = unsafe { msg_send![cls, alloc] };
            let delegate: *mut Object = unsafe { msg_send![delegate, init] };
            let buffer_as_ptr = {
                let arc_raw = Arc::as_ptr(buffer);
                arc_raw.cast::<c_void>()
            };
            unsafe {
                let _: () = msg_send![delegate, SetBufferPtr: buffer_as_ptr];
            }

            let queue = unsafe {
                dispatch_queue_create(device_spec.as_ptr(), NSObject(std::ptr::null_mut()))
            };

            Ok(AVCaptureVideoCallback { delegate, queue })
        }

        pub fn data_len(&self) -> usize {
            unsafe { msg_send![self.delegate, dataLength] }
        }

        pub fn inner(&self) -> *mut Object {
            self.delegate
        }

        pub fn queue(&self) -> &NSObject {
            &self.queue
        }
    }

    create_boilerplate_impl! {
        [pub AVFrameRateRange],
        [pub AVCaptureDeviceDiscoverySession],
        [pub AVCaptureDeviceInput],
        [pub AVCaptureSession]
    }

    impl AVFrameRateRange {
        pub fn max(&self) -> f64 {
            unsafe { msg_send![self.inner, maxFrameRate] }
        }

        pub fn min(&self) -> f64 {
            unsafe { msg_send![self.inner, minFrameRate] }
        }
    }

    #[derive(Debug)]
    pub struct AVCaptureDeviceFormat {
        pub(crate) internal: *mut Object,
        pub resolution: CMVideoDimensions,
        pub fps_list: Vec<f64>,
        pub fourcc: FrameFormat,
    }

    impl TryFrom<*mut Object> for AVCaptureDeviceFormat {
        type Error = NokhwaError;

        fn try_from(value: *mut Object) -> Result<Self, Self::Error> {
            let media_type_raw: *mut Object = unsafe { msg_send![value, mediaType] };
            let media_type = AVMediaType::try_from(media_type_raw)?;
            if media_type != AVMediaType::Video {
                return Err(NokhwaError::StructureError {
                    structure: "AVMediaType".to_string(),
                    error: "Not Video".to_string(),
                });
            }
            let mut fps_list = ns_arr_to_vec::<AVFrameRateRange>(unsafe {
                msg_send![value, videoSupportedFrameRateRanges]
            })
            .into_iter()
            .flat_map(|v| {
                if v.min() != 0_f64 && v.min() != 1_f64 {
                    vec![v.min(), v.max()]
                } else {
                    vec![v.max()] // this gets deduped!
                }
            })
            .collect::<Vec<f64>>();
            fps_list.sort_by(|n, m| n.partial_cmp(m).unwrap_or(Ordering::Equal));
            fps_list.dedup();
            let description_obj: *mut Object = unsafe { msg_send![value, formatDescription] };
            let resolution =
                unsafe { CMVideoFormatDescriptionGetDimensions(description_obj as *mut c_void) };
            let fcc_raw =
                unsafe { CMFormatDescriptionGetMediaSubType(description_obj as *mut c_void) };
            #[allow(non_upper_case_globals)]
            let fourcc = match raw_fcc_to_frameformat(fcc_raw) {
                Some(fcc) => fcc,
                None => {
                    return Err(NokhwaError::StructureError {
                        structure: "FourCharCode".to_string(),
                        error: format!("Unknown FourCharCode {fcc_raw:?}"),
                    })
                }
            };

            Ok(AVCaptureDeviceFormat {
                internal: value,
                resolution,
                fps_list,
                fourcc,
            })
        }
    }

    impl AVCaptureDeviceDiscoverySession {
        pub fn new(device_types: Vec<AVCaptureDeviceType>) -> Result<Self, NokhwaError> {
            let device_types = vec_to_ns_arr(device_types);
            let position = 0 as NSInteger;

            let media_type_video = unsafe { AVMediaTypeVideo.clone() }.0;

            let discovery_session_cls = class!(AVCaptureDeviceDiscoverySession);
            let discovery_session: *mut Object = unsafe {
                msg_send![discovery_session_cls, discoverySessionWithDeviceTypes:device_types mediaType:media_type_video position:position]
            };

            Ok(AVCaptureDeviceDiscoverySession {
                inner: discovery_session,
            })
        }

        pub fn default() -> Result<Self, NokhwaError> {
            AVCaptureDeviceDiscoverySession::new(vec![
                AVCaptureDeviceType::UltraWide,
                AVCaptureDeviceType::Telephoto,
                AVCaptureDeviceType::ExternalUnknown,
                AVCaptureDeviceType::Dual,
                AVCaptureDeviceType::DualWide,
                AVCaptureDeviceType::Triple,
            ])
        }

        pub fn devices(&self) -> Vec<CameraInfo> {
            let device_ns_array: *mut Object = unsafe { msg_send![self.inner, devices] };
            let objects_len: NSUInteger = unsafe { NSArray::count(device_ns_array) };
            let mut devices = Vec::with_capacity(objects_len as usize);
            for index in 0..objects_len {
                let device = unsafe { device_ns_array.objectAtIndex(index) };
                devices.push(get_raw_device_info(
                    CameraIndex::Index(index as u32),
                    device,
                ));
            }

            devices
        }
    }

    pub struct AVCaptureDevice {
        inner: *mut Object,
        device: CameraInfo,
        locked: bool,
    }

    impl AVCaptureDevice {
        pub fn inner(&self) -> *mut Object {
            self.inner
        }
    }

    impl AVCaptureDevice {
        pub fn new(index: &CameraIndex) -> Result<Self, NokhwaError> {
            match &index {
                CameraIndex::Index(idx) => {
                    let devices = query_avfoundation()?;

                    match devices.get(*idx as usize) {
                        Some(device) => Ok(AVCaptureDevice::from_id(
                            &device.misc(),
                            Some(index.clone()),
                        )?),
                        None => Err(NokhwaError::OpenDeviceError(
                            idx.to_string(),
                            "Not Found".to_string(),
                        )),
                    }
                }
                CameraIndex::String(id) => Ok(AVCaptureDevice::from_id(id, None)?),
            }
        }

        pub fn from_id(id: &str, index_hint: Option<CameraIndex>) -> Result<Self, NokhwaError> {
            let nsstr_id = str_to_nsstr(id);
            let avfoundation_capture_cls = class!(AVCaptureDevice);
            let capture: *mut Object =
                unsafe { msg_send![avfoundation_capture_cls, deviceWithUniqueID: nsstr_id] };
            if capture.is_null() {
                return Err(NokhwaError::OpenDeviceError(
                    id.to_string(),
                    "Device is null".to_string(),
                ));
            }
            let camera_info = get_raw_device_info(
                index_hint.unwrap_or_else(|| CameraIndex::String(id.to_string())),
                capture,
            );

            Ok(AVCaptureDevice {
                inner: capture,
                device: camera_info,
                locked: false,
            })
        }

        pub fn info(&self) -> &CameraInfo {
            &self.device
        }

        pub fn supported_formats_raw(&self) -> Result<Vec<AVCaptureDeviceFormat>, NokhwaError> {
            try_ns_arr_to_vec::<AVCaptureDeviceFormat, NokhwaError>(unsafe {
                msg_send![self.inner, formats]
            })
        }

        pub fn supported_formats(&self) -> Result<Vec<CameraFormat>, NokhwaError> {
            Ok(self
                .supported_formats_raw()?
                .iter()
                .flat_map(|av_fmt| {
                    let resolution = av_fmt.resolution;
                    av_fmt.fps_list.iter().map(move |fps_f64| {
                        let fps = *fps_f64 as u32;

                        let resolution =
                            Resolution::new(resolution.width as u32, resolution.height as u32); // FIXME: what the fuck?
                        CameraFormat::new(resolution, av_fmt.fourcc, fps)
                    })
                })
                .filter(|x| x.frame_rate() != 0)
                .collect())
        }

        pub fn already_in_use(&self) -> bool {
            unsafe {
                let result: BOOL = msg_send![self.inner(), isInUseByAnotherApplication];
                result == YES
            }
        }

        pub fn is_suspended(&self) -> bool {
            unsafe {
                let result: BOOL = msg_send![self.inner, isSuspended];
                result == YES
            }
        }

        pub fn lock(&self) -> Result<(), NokhwaError> {
            if self.locked {
                return Ok(());
            }
            if self.already_in_use() {
                return Err(NokhwaError::InitializeError {
                    backend: ApiBackend::AVFoundation,
                    error: "Already in use".to_string(),
                });
            }
            let err_ptr: *mut c_void = std::ptr::null_mut();
            let accepted: BOOL = unsafe { msg_send![self.inner, lockForConfiguration: err_ptr] };
            if !err_ptr.is_null() {
                return Err(NokhwaError::SetPropertyError {
                    property: "lockForConfiguration".to_string(),
                    value: "Locked".to_string(),
                    error: "Cannot lock for configuration".to_string(),
                });
            }
            // Space these out for debug purposes
            if !accepted == YES {
                return Err(NokhwaError::SetPropertyError {
                    property: "lockForConfiguration".to_string(),
                    value: "Locked".to_string(),
                    error: "Lock Rejected".to_string(),
                });
            }
            Ok(())
        }

        pub fn unlock(&mut self) {
            if self.locked {
                self.locked = false;
                unsafe { msg_send![self.inner, unlockForConfiguration] }
            }
        }

        // thank you ffmpeg
        pub fn set_all(&mut self, descriptor: CameraFormat) -> Result<(), NokhwaError> {
            self.lock()?;
            let format_list = try_ns_arr_to_vec::<AVCaptureDeviceFormat, NokhwaError>(unsafe {
                msg_send![self.inner, formats]
            })?;
            let format_description_sel = sel!(formatDescription);

            let mut selected_format: *mut Object = std::ptr::null_mut();
            let mut selected_range: *mut Object = std::ptr::null_mut();

            for format in format_list {
                let format_desc_ref: CMFormatDescriptionRef =
                    unsafe { msg_send![format.internal, performSelector: format_description_sel] };
                let dimensions = unsafe { CMVideoFormatDescriptionGetDimensions(format_desc_ref) };

                if dimensions.height == descriptor.resolution().height() as i32
                    && dimensions.width == descriptor.resolution().width() as i32
                {
                    selected_format = format.internal;

                    for range in ns_arr_to_vec::<AVFrameRateRange>(unsafe {
                        msg_send![format.internal, videoSupportedFrameRateRanges]
                    }) {
                        let max_fps: f64 = unsafe { msg_send![range.inner, maxFrameRate] };
                        // Older Apple cameras (i.e. iMac 2013) return 29.97000002997 as FPS.
                        if (f64::from(descriptor.frame_rate()) - max_fps).abs() < 0.999 {
                            selected_range = range.inner;
                            break;
                        }
                    }
                }
            }
            if selected_range.is_null() || selected_format.is_null() {
                return Err(NokhwaError::SetPropertyError {
                    property: "CameraFormat".to_string(),
                    value: descriptor.to_string(),
                    error: "Not Found/Rejected/Unsupported".to_string(),
                });
            }

            let activefmtkey = str_to_nsstr("activeFormat");
            let min_frame_duration = str_to_nsstr("minFrameDuration");
            let active_video_min_frame_duration = str_to_nsstr("activeVideoMinFrameDuration");
            let active_video_max_frame_duration = str_to_nsstr("activeVideoMaxFrameDuration");
            let _: () =
                unsafe { msg_send![self.inner, setValue:selected_format forKey:activefmtkey] };
            let min_frame_duration: *mut Object =
                unsafe { msg_send![selected_range, valueForKey: min_frame_duration] };
            let _: () = unsafe {
                msg_send![self.inner, setValue:min_frame_duration forKey:active_video_min_frame_duration]
            };
            let _: () = unsafe {
                msg_send![self.inner, setValue:min_frame_duration forKey:active_video_max_frame_duration]
            };
            self.unlock();
            Ok(())
        }

        // 0 => Focus POI
        // 1 => Focus Manual Setting
        // 2 => Exposure POI
        // 3 => Exposure Face Driven
        // 4 => Exposure Target Bias
        // 5 => Exposure ISO
        // 6 => Exposure Duration
        pub fn get_controls(&self) -> Result<Vec<CameraControl>, NokhwaError> {
            let active_format: *mut Object = unsafe { msg_send![self.inner, activeFormat] };

            let mut controls = vec![];
            // get focus modes

            let focus_current: NSInteger = unsafe { msg_send![self.inner, focusMode] };
            let focus_locked: BOOL =
                unsafe { msg_send![self.inner, isFocusModeSupported:NSInteger::from(0)] };
            let focus_auto: BOOL =
                unsafe { msg_send![self.inner, isFocusModeSupported:NSInteger::from(1)] };
            let focus_continuous: BOOL =
                unsafe { msg_send![self.inner, isFocusModeSupported:NSInteger::from(2)] };

            {
                let mut supported_focus_values = vec![];

                if focus_locked == YES {
                    supported_focus_values.push(0)
                }
                if focus_auto == YES {
                    supported_focus_values.push(1)
                }
                if focus_continuous == YES {
                    supported_focus_values.push(2)
                }

                controls.push(CameraControl::new(
                    KnownCameraControl::Focus,
                    "FocusMode".to_string(),
                    ControlValueDescription::Enum {
                        value: focus_current,
                        possible: supported_focus_values,
                        default: focus_current,
                    },
                    vec![],
                    true,
                ));
            }

            let focus_poi_supported: BOOL =
                unsafe { msg_send![self.inner, isFocusPointOfInterestSupported] };
            let focus_poi: CGPoint = unsafe { msg_send![self.inner, focusPointOfInterest] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(0),
                "FocusPointOfInterest".to_string(),
                ControlValueDescription::Point {
                    value: (focus_poi.x as f64, focus_poi.y as f64),
                    default: (0.5, 0.5),
                },
                if focus_poi_supported == NO {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                } else {
                    vec![]
                },
                focus_auto == YES || focus_continuous == YES,
            ));

            let focus_manual: BOOL =
                unsafe { msg_send![self.inner, isLockingFocusWithCustomLensPositionSupported] };
            let focus_lenspos: f32 = unsafe { msg_send![self.inner, lensPosition] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(1),
                "FocusManualLensPosition".to_string(),
                ControlValueDescription::FloatRange {
                    min: 0.0,
                    max: 1.0,
                    value: focus_lenspos as f64,
                    step: f64::MIN_POSITIVE,
                    default: 1.0,
                },
                if focus_manual == YES {
                    vec![]
                } else {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                },
                focus_manual == YES,
            ));

            // get exposures
            let exposure_current: NSInteger = unsafe { msg_send![self.inner, exposureMode] };
            let exposure_locked: BOOL =
                unsafe { msg_send![self.inner, isExposureModeSupported:NSInteger::from(0)] };
            let exposure_auto: BOOL =
                unsafe { msg_send![self.inner, isExposureModeSupported:NSInteger::from(1)] };
            let exposure_continuous: BOOL =
                unsafe { msg_send![self.inner, isExposureModeSupported:NSInteger::from(2)] };
            let exposure_custom: BOOL =
                unsafe { msg_send![self.inner, isExposureModeSupported:NSInteger::from(3)] };

            {
                let mut supported_exposure_values = vec![];

                if exposure_locked == YES {
                    supported_exposure_values.push(0);
                }
                if exposure_auto == YES {
                    supported_exposure_values.push(1);
                }
                if exposure_continuous == YES {
                    supported_exposure_values.push(2);
                }
                if exposure_custom == YES {
                    supported_exposure_values.push(3);
                }

                controls.push(CameraControl::new(
                    KnownCameraControl::Exposure,
                    "ExposureMode".to_string(),
                    ControlValueDescription::Enum {
                        value: exposure_current,
                        possible: supported_exposure_values,
                        default: exposure_current,
                    },
                    vec![],
                    true,
                ));
            }

            let exposure_poi_supported: BOOL =
                unsafe { msg_send![self.inner, isExposurePointOfInterestSupported] };
            let exposure_poi: CGPoint = unsafe { msg_send![self.inner, exposurePointOfInterest] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(2),
                "ExposurePointOfInterest".to_string(),
                ControlValueDescription::Point {
                    value: (exposure_poi.x as f64, exposure_poi.y as f64),
                    default: (0.5, 0.5),
                },
                if exposure_poi_supported == NO {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                } else {
                    vec![]
                },
                focus_auto == YES || focus_continuous == YES,
            ));

            let expposure_face_driven_supported: BOOL =
                unsafe { msg_send![self.inner, isFaceDrivenAutoExposureEnabled] };
            let exposure_face_driven: BOOL = unsafe {
                msg_send![
                    self.inner,
                    automaticallyAdjustsFaceDrivenAutoExposureEnabled
                ]
            };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(3),
                "ExposureFaceDriven".to_string(),
                ControlValueDescription::Boolean {
                    value: exposure_face_driven == YES,
                    default: false,
                },
                if expposure_face_driven_supported == NO {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                } else {
                    vec![]
                },
                exposure_poi_supported == YES,
            ));

            let exposure_bias: f32 = unsafe { msg_send![self.inner, exposureTargetBias] };
            let exposure_bias_min: f32 = unsafe { msg_send![self.inner, minExposureTargetBias] };
            let exposure_bias_max: f32 = unsafe { msg_send![self.inner, maxExposureTargetBias] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(4),
                "ExposureBiasTarget".to_string(),
                ControlValueDescription::FloatRange {
                    min: exposure_bias_min as f64,
                    max: exposure_bias_max as f64,
                    value: exposure_bias as f64,
                    step: f32::MIN_POSITIVE as f64,
                    default: unsafe { AVCaptureExposureTargetBiasCurrent } as f64,
                },
                vec![],
                true,
            ));

            let exposure_duration: CMTime = unsafe { msg_send![self.inner, exposureDuration] };
            let exposure_duration_min: CMTime =
                unsafe { msg_send![active_format, minExposureDuration] };
            let exposure_duration_max: CMTime =
                unsafe { msg_send![active_format, maxExposureDuration] };

            controls.push(CameraControl::new(
                KnownCameraControl::Gamma,
                "ExposureDuration".to_string(),
                ControlValueDescription::IntegerRange {
                    min: exposure_duration_min.value,
                    max: exposure_duration_max.value,
                    value: exposure_duration.value,
                    step: 1,
                    default: unsafe { AVCaptureExposureDurationCurrent.value },
                },
                if exposure_custom == YES {
                    vec![
                        KnownCameraControlFlag::ReadOnly,
                        KnownCameraControlFlag::Volatile,
                    ]
                } else {
                    vec![KnownCameraControlFlag::Volatile]
                },
                exposure_custom == YES,
            ));

            let exposure_iso: f32 = unsafe { msg_send![self.inner, ISO] };
            let exposure_iso_min: f32 = unsafe { msg_send![active_format, minISO] };
            let exposure_iso_max: f32 = unsafe { msg_send![active_format, maxISO] };

            controls.push(CameraControl::new(
                KnownCameraControl::Brightness,
                "ExposureISO".to_string(),
                ControlValueDescription::FloatRange {
                    min: exposure_iso_min as f64,
                    max: exposure_iso_max as f64,
                    value: exposure_iso as f64,
                    step: f32::MIN_POSITIVE as f64,
                    default: unsafe { AVCaptureISOCurrent } as f64,
                },
                if exposure_custom == YES {
                    vec![
                        KnownCameraControlFlag::ReadOnly,
                        KnownCameraControlFlag::Volatile,
                    ]
                } else {
                    vec![KnownCameraControlFlag::Volatile]
                },
                exposure_custom == YES,
            ));

            let lens_aperture: f32 = unsafe { msg_send![self.inner, lensAperture] };

            controls.push(CameraControl::new(
                KnownCameraControl::Iris,
                "LensAperture".to_string(),
                ControlValueDescription::Float {
                    value: lens_aperture as f64,
                    default: lens_aperture as f64,
                    step: lens_aperture as f64,
                },
                vec![KnownCameraControlFlag::ReadOnly],
                false,
            ));

            // get whiteblaance
            let white_balance_current: NSInteger =
                unsafe { msg_send![self.inner, whiteBalanceMode] };
            let white_balance_manual: BOOL =
                unsafe { msg_send![self.inner, isWhiteBalanceModeSupported:NSInteger::from(0)] };
            let white_balance_auto: BOOL =
                unsafe { msg_send![self.inner, isWhiteBalanceModeSupported:NSInteger::from(1)] };
            let white_balance_continuous: BOOL =
                unsafe { msg_send![self.inner, isWhiteBalanceModeSupported:NSInteger::from(2)] };

            {
                let mut possible = vec![];

                if white_balance_manual == YES {
                    possible.push(0);
                }
                if white_balance_auto == YES {
                    possible.push(1);
                }
                if white_balance_continuous == YES {
                    possible.push(2);
                }

                controls.push(CameraControl::new(
                    KnownCameraControl::WhiteBalance,
                    "WhiteBalanceMode".to_string(),
                    ControlValueDescription::Enum {
                        value: white_balance_current as i64,
                        possible,
                        default: 0,
                    },
                    vec![],
                    true,
                ));
            }

            let white_balance_gains: AVCaptureWhiteBalanceGains =
                unsafe { msg_send![self.inner, deviceWhiteBalanceGains] };
            let white_balance_default: AVCaptureWhiteBalanceGains =
                unsafe { msg_send![self.inner, grayWorldDeviceWhiteBalanceGains] };
            let white_balancne_max: AVCaptureWhiteBalanceGains =
                unsafe { msg_send![self.inner, maxWhiteBalanceGain] };
            let white_balance_gain_supported: BOOL = unsafe {
                msg_send![
                    self.inner,
                    isLockingWhiteBalanceWithCustomDeviceGainsSupported
                ]
            };

            controls.push(CameraControl::new(
                KnownCameraControl::Gain,
                "WhiteBalanceGain".to_string(),
                ControlValueDescription::RGB {
                    value: (
                        white_balance_gains.redGain as f64,
                        white_balance_gains.greenGain as f64,
                        white_balance_gains.blueGain as f64,
                    ),
                    max: (
                        white_balancne_max.redGain as f64,
                        white_balancne_max.greenGain as f64,
                        white_balancne_max.blueGain as f64,
                    ),
                    default: (
                        white_balance_default.redGain as f64,
                        white_balance_default.greenGain as f64,
                        white_balance_default.blueGain as f64,
                    ),
                },
                if white_balance_gain_supported == YES {
                    vec![
                        KnownCameraControlFlag::Disabled,
                        KnownCameraControlFlag::ReadOnly,
                    ]
                } else {
                    vec![]
                },
                white_balance_gain_supported == YES,
            ));

            // get flash
            let has_torch: BOOL = unsafe { msg_send![self.inner, isTorchAvailable] };
            let torch_active: BOOL = unsafe { msg_send![self.inner, isTorchActive] };
            let torch_off: BOOL =
                unsafe { msg_send![self.inner, isTorchModeSupported:NSInteger::from(0)] };
            let torch_on: BOOL =
                unsafe { msg_send![self.inner, isTorchModeSupported:NSInteger::from(1)] };
            let torch_auto: BOOL =
                unsafe { msg_send![self.inner, isTorchModeSupported:NSInteger::from(2)] };

            {
                let mut possible = vec![];

                if torch_off == YES {
                    possible.push(0);
                }
                if torch_on == YES {
                    possible.push(1);
                }
                if torch_auto == YES {
                    possible.push(2);
                }

                controls.push(CameraControl::new(
                    KnownCameraControl::Other(5),
                    "TorchMode".to_string(),
                    ControlValueDescription::Enum {
                        value: (torch_active == YES) as i64,
                        possible,
                        default: 0,
                    },
                    if has_torch == YES {
                        vec![
                            KnownCameraControlFlag::Disabled,
                            KnownCameraControlFlag::ReadOnly,
                        ]
                    } else {
                        vec![]
                    },
                    has_torch == YES,
                ));
            }

            // get low light boost
            let has_llb: BOOL = unsafe { msg_send![self.inner, isLowLightBoostSupported] };
            let llb_enabled: BOOL = unsafe { msg_send![self.inner, isLowLightBoostEnabled] };

            {
                controls.push(CameraControl::new(
                    KnownCameraControl::BacklightComp,
                    "LowLightCompensation".to_string(),
                    ControlValueDescription::Boolean {
                        value: llb_enabled == YES,
                        default: false,
                    },
                    if has_llb == NO {
                        vec![
                            KnownCameraControlFlag::Disabled,
                            KnownCameraControlFlag::ReadOnly,
                        ]
                    } else {
                        vec![]
                    },
                    has_llb == YES,
                ));
            }

            // get zoom factor
            let zoom_current: CGFloat = unsafe { msg_send![self.inner, videoZoomFactor] };
            let zoom_min: CGFloat = unsafe { msg_send![self.inner, minAvailableVideoZoomFactor] };
            let zoom_max: CGFloat = unsafe { msg_send![self.inner, maxAvailableVideoZoomFactor] };

            controls.push(CameraControl::new(
                KnownCameraControl::Zoom,
                "Zoom".to_string(),
                ControlValueDescription::FloatRange {
                    min: zoom_min as f64,
                    max: zoom_max as f64,
                    value: zoom_current as f64,
                    step: f32::MIN_POSITIVE as f64,
                    default: 1.0,
                },
                vec![],
                true,
            ));

            // zoom distortion correction
            let distortion_correction_supported: BOOL =
                unsafe { msg_send![self.inner, isGeometricDistortionCorrectionSupported] };
            let distortion_correction_current_value: BOOL =
                unsafe { msg_send![self.inner, isGeometricDistortionCorrectionEnabled] };

            controls.push(CameraControl::new(
                KnownCameraControl::Other(6),
                "DistortionCorrection".to_string(),
                ControlValueDescription::Boolean {
                    value: distortion_correction_current_value == YES,
                    default: false,
                },
                if distortion_correction_supported == YES {
                    vec![
                        KnownCameraControlFlag::ReadOnly,
                        KnownCameraControlFlag::Disabled,
                    ]
                } else {
                    vec![]
                },
                distortion_correction_supported == YES,
            ));

            Ok(controls)
        }

        pub fn set_control(
            &mut self,
            id: KnownCameraControl,
            value: ControlValueSetter,
        ) -> Result<(), NokhwaError> {
            let rc = self.get_controls()?;
            let controls = rc
                .iter()
                .map(|cc| (cc.control(), cc))
                .collect::<BTreeMap<_, _>>();

            match id {
                KnownCameraControl::Brightness => {
                    let isoctrl = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if isoctrl.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error:
                                "Exposure is in improper state to set ISO (Please set to `custom`!)"
                                    .to_string(),
                        });
                    }

                    if isoctrl.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let current_duration = unsafe { AVCaptureExposureDurationCurrent };
                    let new_iso = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Expected float".to_string(),
                    })? as f32;

                    if !isoctrl.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe {
                        msg_send![self.inner, setExposureModeCustomWithDuration:current_duration ISO:new_iso completionHandler:Nil]
                    };

                    Ok(())
                }
                KnownCameraControl::Gamma => {
                    let duration_ctrl = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if duration_ctrl
                        .flag()
                        .contains(&KnownCameraControlFlag::ReadOnly)
                    {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Exposure is in improper state to set Duration (Please set to `custom`!)"
                                .to_string(),
                        });
                    }

                    if duration_ctrl
                        .flag()
                        .contains(&KnownCameraControlFlag::Disabled)
                    {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }
                    let current_duration: CMTime =
                        unsafe { msg_send![self.inner, exposureDuration] };

                    let current_iso = unsafe { AVCaptureISOCurrent };
                    let new_duration = CMTime {
                        value: *value.as_integer().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected i64".to_string(),
                        })?,
                        timescale: current_duration.timescale,
                        flags: current_duration.flags,
                        epoch: current_duration.epoch,
                    };

                    if !duration_ctrl.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe {
                        msg_send![self.inner, setExposureModeCustomWithDuration:new_duration ISO:current_iso completionHandler:Nil]
                    };

                    Ok(())
                }
                KnownCameraControl::WhiteBalance => {
                    let wb_enum_value = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if wb_enum_value
                        .flag()
                        .contains(&KnownCameraControlFlag::ReadOnly)
                    {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if wb_enum_value
                        .flag()
                        .contains(&KnownCameraControlFlag::Disabled)
                    {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }
                    let setter =
                        NSInteger::from(*value.as_enum().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Enum".to_string(),
                        })? as i32);

                    if !wb_enum_value.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { msg_send![self.inner, whiteBalanceMode: setter] };

                    Ok(())
                }
                KnownCameraControl::BacklightComp => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter =
                        NSInteger::from(*value.as_enum().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Enum".to_string(),
                        })? as i32);

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { msg_send![self.inner, whiteBalanceMode: setter] };

                    Ok(())
                }
                KnownCameraControl::Gain => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter = NSInteger::from(*value.as_boolean().ok_or(
                        NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Boolean".to_string(),
                        },
                    )? as i32);

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { msg_send![self.inner, whiteBalanceMode: setter] };

                    Ok(())
                }
                KnownCameraControl::Zoom => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Expected float".to_string(),
                    })? as c_float;

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe {
                        msg_send![self.inner, rampToVideoZoomFactor: setter withRate: 1.0_f32]
                    };

                    Ok(())
                }
                KnownCameraControl::Exposure => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter =
                        NSInteger::from(*value.as_enum().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Enum".to_string(),
                        })? as i32);

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { msg_send![self.inner, exposureMode: setter] };

                    Ok(())
                }
                KnownCameraControl::Iris => Err(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Read Only".to_string(),
                }),
                KnownCameraControl::Focus => {
                    let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Control does not exist".to_string(),
                    })?;

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Read Only".to_string(),
                        });
                    }

                    if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Disabled".to_string(),
                        });
                    }

                    let setter =
                        NSInteger::from(*value.as_enum().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Enum".to_string(),
                        })? as i32);

                    if !ctrlvalue.description().verify_setter(&value) {
                        return Err(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Failed to verify value".to_string(),
                        });
                    }

                    let _: () = unsafe { msg_send![self.inner, focusMode: setter] };

                    Ok(())
                }
                KnownCameraControl::Other(i) => match i {
                    0 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter = value
                            .as_point()
                            .ok_or(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Point".to_string(),
                            })
                            .map(|(x, y)| CGPoint {
                                x: *x as f32,
                                y: *y as f32,
                            })?;

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe { msg_send![self.inner, focusPointOfInterest: setter] };

                        Ok(())
                    }
                    1 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected float".to_string(),
                        })? as c_float;

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe {
                            msg_send![self.inner, setFocusModeLockedWithLensPosition: setter handler: Nil]
                        };

                        Ok(())
                    }
                    2 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter = value
                            .as_point()
                            .ok_or(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Point".to_string(),
                            })
                            .map(|(x, y)| CGPoint {
                                x: *x as f32,
                                y: *y as f32,
                            })?;

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () =
                            unsafe { msg_send![self.inner, exposurePointOfInterest: setter] };

                        Ok(())
                    }
                    3 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter =
                            if *value.as_boolean().ok_or(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Boolean".to_string(),
                            })? {
                                YES
                            } else {
                                NO
                            };

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe {
                            msg_send![
                                self.inner,
                                automaticallyAdjustsFaceDrivenAutoExposureEnabled: setter
                            ]
                        };

                        Ok(())
                    }
                    4 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter = *value.as_float().ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Expected Float".to_string(),
                        })? as f32;

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe {
                            msg_send![self.inner, setExposureTargetBias: setter handler: Nil]
                        };

                        Ok(())
                    }
                    5 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter = NSInteger::from(*value.as_enum().ok_or(
                            NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Enum".to_string(),
                            },
                        )? as i32);

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe { msg_send![self.inner, torchMode: setter] };

                        Ok(())
                    }
                    6 => {
                        let ctrlvalue = controls.get(&id).ok_or(NokhwaError::SetPropertyError {
                            property: id.to_string(),
                            value: value.to_string(),
                            error: "Control does not exist".to_string(),
                        })?;

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::ReadOnly) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Read Only".to_string(),
                            });
                        }

                        if ctrlvalue.flag().contains(&KnownCameraControlFlag::Disabled) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Disabled".to_string(),
                            });
                        }

                        let setter =
                            if *value.as_boolean().ok_or(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Expected Boolean".to_string(),
                            })? {
                                YES
                            } else {
                                NO
                            };

                        if !ctrlvalue.description().verify_setter(&value) {
                            return Err(NokhwaError::SetPropertyError {
                                property: id.to_string(),
                                value: value.to_string(),
                                error: "Failed to verify value".to_string(),
                            });
                        }

                        let _: () = unsafe {
                            msg_send![self.inner, geometricDistortionCorrectionEnabled: setter]
                        };

                        Ok(())
                    }
                    _ => Err(NokhwaError::SetPropertyError {
                        property: id.to_string(),
                        value: value.to_string(),
                        error: "Unknown Control".to_string(),
                    }),
                },
                _ => Err(NokhwaError::SetPropertyError {
                    property: id.to_string(),
                    value: value.to_string(),
                    error: "Unknown Control".to_string(),
                }),
            }
        }

        pub fn active_format(&self) -> Result<CameraFormat, NokhwaError> {
            let af: *mut Object = unsafe { msg_send![self.inner, activeFormat] };
            let avf_format = AVCaptureDeviceFormat::try_from(af)?;
            let resolution = avf_format.resolution;
            let fourcc = avf_format.fourcc;
            let mut a = avf_format
                .fps_list
                .into_iter()
                .map(move |fps_f64| {
                    let fps = fps_f64 as u32;

                    let resolution =
                        Resolution::new(resolution.width as u32, resolution.height as u32); // FIXME: what the fuck?
                    CameraFormat::new(resolution, fourcc, fps)
                })
                .collect::<Vec<_>>();
            a.sort_by(|a, b| a.frame_rate().cmp(&b.frame_rate()));

            if a.len() != 0 {
                Ok(a[a.len() - 1])
            } else {
                Err(NokhwaError::GetPropertyError {
                    property: "activeFormat".to_string(),
                    error: "None??".to_string(),
                })
            }
        }
    }

    impl AVCaptureDeviceInput {
        pub fn new(capture_device: &AVCaptureDevice) -> Result<Self, NokhwaError> {
            let cls = class!(AVCaptureDeviceInput);
            let err_ptr: *mut c_void = std::ptr::null_mut();
            let capture_input: *mut Object = unsafe {
                let allocated: *mut Object = msg_send![cls, alloc];
                msg_send![allocated, initWithDevice:capture_device.inner() error:err_ptr]
            };
            if !err_ptr.is_null() {
                return Err(NokhwaError::InitializeError {
                    backend: ApiBackend::AVFoundation,
                    error: "Failed to create input".to_string(),
                });
            }

            Ok(AVCaptureDeviceInput {
                inner: capture_input,
            })
        }
    }

    pub struct AVCaptureVideoDataOutput {
        inner: *mut Object,
    }

    impl AVCaptureVideoDataOutput {
        pub fn new() -> Self {
            AVCaptureVideoDataOutput::default()
        }

        pub fn add_delegate(&self, delegate: &AVCaptureVideoCallback) -> Result<(), NokhwaError> {
            unsafe {
                let _: () = msg_send![
                    self.inner,
                    setSampleBufferDelegate: delegate.delegate
                    queue: delegate.queue().0
                ];
            };
            Ok(())
        }

        pub fn set_frame_format(&self, format: FrameFormat) -> Result<(), NokhwaError> {
            let cmpixelfmt = match format {
                FrameFormat::YUYV => kCMPixelFormat_422YpCbCr8_yuvs,
                FrameFormat::MJPEG => kCMVideoCodecType_JPEG,
                FrameFormat::GRAY => kCMPixelFormat_8IndexedGray_WhiteIsZero,
                FrameFormat::NV12 => kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
                FrameFormat::RAWRGB => kCMPixelFormat_24RGB,
            };
            let obj = CFNumber::from(cmpixelfmt as i32);
            let obj = obj.as_CFTypeRef() as *mut Object;
            let key = unsafe { kCVPixelBufferPixelFormatTypeKey } as *mut Object;
            let dict = unsafe { NSDictionary::dictionaryWithObject_forKey_(nil, obj, key) };
            let _: () = unsafe { msg_send![self.inner, setVideoSettings:dict] };
            Ok(())
        }
    }

    use cocoa_foundation::base::nil;
    use core_foundation::base::TCFType;
    use core_foundation::number::CFNumber;
    use core_video_sys::kCVPixelBufferPixelFormatTypeKey;
    impl Default for AVCaptureVideoDataOutput {
        fn default() -> Self {
            let cls = class!(AVCaptureVideoDataOutput);
            let inner: *mut Object = unsafe { msg_send![cls, new] };

            AVCaptureVideoDataOutput { inner }
        }
    }

    impl AVCaptureSession {
        pub fn new() -> Self {
            AVCaptureSession::default()
        }

        pub fn begin_configuration(&self) {
            unsafe { msg_send![self.inner, beginConfiguration] }
        }

        pub fn commit_configuration(&self) {
            unsafe { msg_send![self.inner, commitConfiguration] }
        }

        pub fn can_add_input(&self, input: &AVCaptureDeviceInput) -> bool {
            let result: BOOL = unsafe { msg_send![self.inner, canAddInput:input.inner] };
            result == YES
        }

        pub fn add_input(&self, input: &AVCaptureDeviceInput) -> Result<(), NokhwaError> {
            if self.can_add_input(input) {
                let _: () = unsafe { msg_send![self.inner, addInput:input.inner] };
                return Ok(());
            }
            Err(NokhwaError::SetPropertyError {
                property: "AVCaptureDeviceInput".to_string(),
                value: "add new input".to_string(),
                error: "Rejected".to_string(),
            })
        }

        pub fn remove_input(&self, input: &AVCaptureDeviceInput) {
            unsafe { msg_send![self.inner, removeInput:input.inner] }
        }

        pub fn can_add_output(&self, output: &AVCaptureVideoDataOutput) -> bool {
            let result: BOOL = unsafe { msg_send![self.inner, canAddOutput:output.inner] };
            result == YES
        }

        pub fn add_output(&self, output: &AVCaptureVideoDataOutput) -> Result<(), NokhwaError> {
            if self.can_add_output(output) {
                let _: () = unsafe { msg_send![self.inner, addOutput:output.inner] };
                return Ok(());
            }
            Err(NokhwaError::SetPropertyError {
                property: "AVCaptureVideoDataOutput".to_string(),
                value: "add new output".to_string(),
                error: "Rejected".to_string(),
            })
        }

        pub fn remove_output(&self, output: &AVCaptureVideoDataOutput) {
            unsafe { msg_send![self.inner, removeOutput:output.inner] }
        }

        pub fn is_running(&self) -> bool {
            let running: BOOL = unsafe { msg_send![self.inner, isRunning] };
            running == YES
        }

        pub fn start(&self) -> Result<(), NokhwaError> {
            let start_stream_fn = || {
                let _: () = unsafe { msg_send![self.inner, startRunning] };
            };

            if std::panic::catch_unwind(start_stream_fn).is_err() {
                return Err(NokhwaError::OpenStreamError(
                    "Cannot run AVCaptureSession".to_string(),
                ));
            }
            Ok(())
        }

        pub fn stop(&self) {
            unsafe { msg_send![self.inner, stopRunning] }
        }

        pub fn is_interrupted(&self) -> bool {
            let interrupted: BOOL = unsafe { msg_send![self.inner, isInterrupted] };
            interrupted == YES
        }
    }

    impl Default for AVCaptureSession {
        fn default() -> Self {
            let cls = class!(AVCaptureSession);
            let session: *mut Object = {
                let alloc: *mut Object = unsafe { msg_send![cls, alloc] };
                unsafe { msg_send![alloc, init] }
            };
            AVCaptureSession { inner: session }
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use crate::internal::*;
