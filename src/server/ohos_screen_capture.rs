use std::{
    ffi::{c_char, c_void},
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

use hbb_common::log;

const AV_SCREEN_CAPTURE_OK: i32 = 0;
const IMAGE_SUCCESS: i32 = 0;
const NATIVE_ERROR_OK: i32 = 0;
const NATIVEBUFFER_PIXEL_FMT_RGBA_8888: i32 = 12;
const NATIVEBUFFER_PIXEL_FMT_BGRA_8888: i32 = 20;
const MAX_FRAME_BYTES: usize = 512 * 1024 * 1024;
const START_TIMEOUT: Duration = Duration::from_secs(120);

#[repr(C)]
struct OH_AVScreenCapture {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_NativeBuffer {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_ImageReceiverOptions {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_ImageReceiverNative {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_ImageNative {
    _private: [u8; 0],
}

#[repr(C)]
struct OHNativeWindow {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Default)]
struct ImageSize {
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OH_AudioCaptureInfo {
    sample_rate: i32,
    channels: i32,
    source: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OH_AudioEncInfo {
    bitrate: i32,
    codec: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OH_AudioInfo {
    mic: OH_AudioCaptureInfo,
    inner: OH_AudioCaptureInfo,
    enc: OH_AudioEncInfo,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OH_VideoCaptureInfo {
    display_id: u64,
    mission_ids: *mut i32,
    mission_ids_len: i32,
    width: i32,
    height: i32,
    source: i32,
}

impl Default for OH_VideoCaptureInfo {
    fn default() -> Self {
        Self {
            display_id: 0,
            mission_ids: ptr::null_mut(),
            mission_ids_len: 0,
            width: 0,
            height: 0,
            source: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OH_VideoEncInfo {
    codec: i32,
    bitrate: i32,
    frame_rate: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OH_VideoInfo {
    capture: OH_VideoCaptureInfo,
    enc: OH_VideoEncInfo,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OH_RecorderInfo {
    url: *mut c_char,
    url_len: u32,
    format: i32,
}

impl Default for OH_RecorderInfo {
    fn default() -> Self {
        Self {
            url: ptr::null_mut(),
            url_len: 0,
            format: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OH_AVScreenCaptureConfig {
    capture_mode: i32,
    data_type: i32,
    audio: OH_AudioInfo,
    video: OH_VideoInfo,
    recorder: OH_RecorderInfo,
}

#[link(name = "native_avscreen_capture")]
unsafe extern "C" {
    fn OH_AVScreenCapture_Create() -> *mut OH_AVScreenCapture;
    fn OH_AVScreenCapture_Init(
        capture: *mut OH_AVScreenCapture,
        config: OH_AVScreenCaptureConfig,
    ) -> i32;
    fn OH_AVScreenCapture_SetStateCallback(
        capture: *mut OH_AVScreenCapture,
        callback: unsafe extern "C" fn(*mut OH_AVScreenCapture, i32, *mut c_void),
        user_data: *mut c_void,
    ) -> i32;
    fn OH_AVScreenCapture_SetErrorCallback(
        capture: *mut OH_AVScreenCapture,
        callback: unsafe extern "C" fn(*mut OH_AVScreenCapture, i32, *mut c_void),
        user_data: *mut c_void,
    ) -> i32;
    fn OH_AVScreenCapture_SetMicrophoneEnabled(
        capture: *mut OH_AVScreenCapture,
        enabled: bool,
    ) -> i32;
    fn OH_AVScreenCapture_StartScreenCaptureWithSurface(
        capture: *mut OH_AVScreenCapture,
        window: *mut OHNativeWindow,
    ) -> i32;
    fn OH_AVScreenCapture_StopScreenCapture(capture: *mut OH_AVScreenCapture) -> i32;
    fn OH_AVScreenCapture_Release(capture: *mut OH_AVScreenCapture) -> i32;
}

#[link(name = "image_receiver")]
unsafe extern "C" {
    fn OH_ImageReceiverOptions_Create(options: *mut *mut OH_ImageReceiverOptions) -> i32;
    fn OH_ImageReceiverOptions_SetSize(
        options: *mut OH_ImageReceiverOptions,
        size: ImageSize,
    ) -> i32;
    fn OH_ImageReceiverOptions_SetCapacity(
        options: *mut OH_ImageReceiverOptions,
        capacity: i32,
    ) -> i32;
    fn OH_ImageReceiverOptions_Release(options: *mut OH_ImageReceiverOptions) -> i32;
    fn OH_ImageReceiverNative_Create(
        options: *mut OH_ImageReceiverOptions,
        receiver: *mut *mut OH_ImageReceiverNative,
    ) -> i32;
    fn OH_ImageReceiverNative_GetReceivingSurfaceId(
        receiver: *mut OH_ImageReceiverNative,
        surface_id: *mut u64,
    ) -> i32;
    fn OH_ImageReceiverNative_ReadLatestImage(
        receiver: *mut OH_ImageReceiverNative,
        image: *mut *mut OH_ImageNative,
    ) -> i32;
    fn OH_ImageReceiverNative_On(
        receiver: *mut OH_ImageReceiverNative,
        callback: unsafe extern "C" fn(*mut OH_ImageReceiverNative),
    ) -> i32;
    fn OH_ImageReceiverNative_Off(receiver: *mut OH_ImageReceiverNative) -> i32;
    fn OH_ImageReceiverNative_Release(receiver: *mut OH_ImageReceiverNative) -> i32;
}

#[link(name = "ohimage")]
unsafe extern "C" {
    fn OH_ImageNative_GetImageSize(image: *mut OH_ImageNative, size: *mut ImageSize) -> i32;
    fn OH_ImageNative_GetComponentTypes(
        image: *mut OH_ImageNative,
        types: *mut *mut u32,
        type_count: *mut usize,
    ) -> i32;
    fn OH_ImageNative_GetByteBuffer(
        image: *mut OH_ImageNative,
        component_type: u32,
        buffer: *mut *mut OH_NativeBuffer,
    ) -> i32;
    fn OH_ImageNative_GetBufferSize(
        image: *mut OH_ImageNative,
        component_type: u32,
        size: *mut usize,
    ) -> i32;
    fn OH_ImageNative_GetRowStride(
        image: *mut OH_ImageNative,
        component_type: u32,
        row_stride: *mut i32,
    ) -> i32;
    fn OH_ImageNative_GetPixelStride(
        image: *mut OH_ImageNative,
        component_type: u32,
        pixel_stride: *mut i32,
    ) -> i32;
    fn OH_ImageNative_GetFormat(image: *mut OH_ImageNative, format: *mut i32) -> i32;
    fn OH_ImageNative_Release(image: *mut OH_ImageNative) -> i32;
}

#[link(name = "native_window")]
unsafe extern "C" {
    fn OH_NativeWindow_CreateNativeWindowFromSurfaceId(
        surface_id: u64,
        window: *mut *mut OHNativeWindow,
    ) -> i32;
    fn OH_NativeWindow_DestroyNativeWindow(window: *mut OHNativeWindow);
}

#[link(name = "native_buffer")]
unsafe extern "C" {
    fn OH_NativeBuffer_Map(buffer: *mut OH_NativeBuffer, address: *mut *mut c_void) -> i32;
    fn OH_NativeBuffer_Unmap(buffer: *mut OH_NativeBuffer) -> i32;
}

static CAPTURE_HANDLE: AtomicU64 = AtomicU64::new(0);
static IMAGE_RECEIVER_HANDLE: AtomicU64 = AtomicU64::new(0);
static NATIVE_WINDOW_HANDLE: AtomicU64 = AtomicU64::new(0);
static CLEANUP_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static CONFIGURED_WIDTH: AtomicUsize = AtomicUsize::new(0);
static CONFIGURED_HEIGHT: AtomicUsize = AtomicUsize::new(0);
static CONFIGURED_DISPLAY_ID: AtomicU64 = AtomicU64::new(0);
static CAPTURE_STARTED: AtomicBool = AtomicBool::new(false);
static CAPTURE_STATE: AtomicI32 = AtomicI32::new(-1);
static CAPTURE_ERROR: AtomicI32 = AtomicI32::new(0);
static CAPTURE_FRAMES: AtomicU64 = AtomicU64::new(0);
static CAPTURE_BYTES: AtomicU64 = AtomicU64::new(0);
static FRAME_ERROR_REPORTED: AtomicBool = AtomicBool::new(false);

fn capture_owner() -> &'static Mutex<Option<OhosScreenCapture>> {
    static OWNER: OnceLock<Mutex<Option<OhosScreenCapture>>> = OnceLock::new();
    OWNER.get_or_init(|| Mutex::new(None))
}

fn is_terminal_state(state: i32) -> bool {
    matches!(state, 1 | 2 | 3 | 4 | 10 | 11 | 13)
}

fn restore_configured_geometry() {
    let width = CONFIGURED_WIDTH.load(Ordering::Acquire);
    let height = CONFIGURED_HEIGHT.load(Ordering::Acquire);
    scrap::ohos::reset_screen_state();
    if width > 0 && height > 0 {
        let _ = crate::platform::ohos::configure_host_screen(width, height);
    }
}

fn report_frame_error(error: &str) {
    if !FRAME_ERROR_REPORTED.swap(true, Ordering::AcqRel) {
        log::error!("OHOS screen capture frame failed: {error}");
    }
}

fn create_surface_receiver(
    width: usize,
    height: usize,
) -> Result<(*mut OH_ImageReceiverNative, *mut OHNativeWindow), String> {
    let width = u32::try_from(width).map_err(|_| "screen width exceeds u32".to_owned())?;
    let height = u32::try_from(height).map_err(|_| "screen height exceeds u32".to_owned())?;
    let mut options = ptr::null_mut();
    let create_options_result = unsafe { OH_ImageReceiverOptions_Create(&mut options) };
    if create_options_result != IMAGE_SUCCESS || options.is_null() {
        return Err(format!(
            "OH_ImageReceiverOptions_Create failed: {create_options_result}"
        ));
    }
    let size_result =
        unsafe { OH_ImageReceiverOptions_SetSize(options, ImageSize { width, height }) };
    let capacity_result = unsafe { OH_ImageReceiverOptions_SetCapacity(options, 3) };
    let mut receiver = ptr::null_mut();
    let create_receiver_result = if size_result == IMAGE_SUCCESS && capacity_result == IMAGE_SUCCESS
    {
        unsafe { OH_ImageReceiverNative_Create(options, &mut receiver) }
    } else {
        -1
    };
    let options_release_result = unsafe { OH_ImageReceiverOptions_Release(options) };
    if size_result != IMAGE_SUCCESS
        || capacity_result != IMAGE_SUCCESS
        || create_receiver_result != IMAGE_SUCCESS
        || options_release_result != IMAGE_SUCCESS
        || receiver.is_null()
    {
        if !receiver.is_null() {
            unsafe {
                OH_ImageReceiverNative_Release(receiver);
            }
        }
        return Err(format!(
            "OHOS ImageReceiver setup failed: size={size_result} capacity={capacity_result} create={create_receiver_result} release_options={options_release_result}"
        ));
    }

    let callback_result = unsafe { OH_ImageReceiverNative_On(receiver, image_receiver_callback) };
    let mut surface_id = 0u64;
    let surface_result =
        unsafe { OH_ImageReceiverNative_GetReceivingSurfaceId(receiver, &mut surface_id) };
    let mut window = ptr::null_mut();
    let window_result =
        if callback_result == IMAGE_SUCCESS && surface_result == IMAGE_SUCCESS && surface_id != 0 {
            unsafe { OH_NativeWindow_CreateNativeWindowFromSurfaceId(surface_id, &mut window) }
        } else {
            -1
        };
    if callback_result != IMAGE_SUCCESS
        || surface_result != IMAGE_SUCCESS
        || surface_id == 0
        || window_result != NATIVE_ERROR_OK
        || window.is_null()
    {
        unsafe {
            OH_ImageReceiverNative_Off(receiver);
            OH_ImageReceiverNative_Release(receiver);
        }
        return Err(format!(
            "OHOS ImageReceiver surface setup failed: callback={callback_result} surface={surface_result} id={surface_id} window={window_result}"
        ));
    }
    Ok((receiver, window))
}

fn release_surface_receiver() {
    let receiver =
        IMAGE_RECEIVER_HANDLE.swap(0, Ordering::AcqRel) as usize as *mut OH_ImageReceiverNative;
    let window = NATIVE_WINDOW_HANDLE.swap(0, Ordering::AcqRel) as usize as *mut OHNativeWindow;
    if !receiver.is_null() {
        let off_result = unsafe { OH_ImageReceiverNative_Off(receiver) };
        if off_result != IMAGE_SUCCESS {
            log::warn!("OH_ImageReceiverNative_Off failed: {off_result}");
        }
    }
    if !window.is_null() {
        unsafe {
            OH_NativeWindow_DestroyNativeWindow(window);
        }
    }
    if !receiver.is_null() {
        let release_result = unsafe { OH_ImageReceiverNative_Release(receiver) };
        if release_result != IMAGE_SUCCESS {
            log::warn!("OH_ImageReceiverNative_Release failed: {release_result}");
        }
    }
}

fn release_capture(handle: u64) -> Option<(i32, i32)> {
    if handle == 0
        || CLEANUP_IN_PROGRESS
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return None;
    }
    if CAPTURE_HANDLE
        .compare_exchange(handle, 0, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        CLEANUP_IN_PROGRESS.store(false, Ordering::Release);
        return None;
    }
    let capture = handle as usize as *mut OH_AVScreenCapture;
    let stop_result = unsafe { OH_AVScreenCapture_StopScreenCapture(capture) };
    let release_result = unsafe { OH_AVScreenCapture_Release(capture) };
    release_surface_receiver();
    CAPTURE_STARTED.store(false, Ordering::Release);
    restore_configured_geometry();
    CLEANUP_IN_PROGRESS.store(false, Ordering::Release);
    Some((stop_result, release_result))
}

fn schedule_release(capture: *mut OH_AVScreenCapture) {
    let handle = capture as usize as u64;
    if handle == 0 || CAPTURE_HANDLE.load(Ordering::Acquire) != handle {
        return;
    }
    std::thread::spawn(move || {
        if let Some((stop_result, release_result)) = release_capture(handle) {
            if stop_result != AV_SCREEN_CAPTURE_OK || release_result != AV_SCREEN_CAPTURE_OK {
                log::error!(
                    "OHOS screen capture cleanup failed: stop={stop_result} release={release_result}"
                );
            }
            crate::platform::ohos::stop_host();
        }
    });
}

unsafe extern "C" fn capture_state_callback(
    capture: *mut OH_AVScreenCapture,
    state: i32,
    _user_data: *mut c_void,
) {
    if CAPTURE_HANDLE.load(Ordering::Acquire) != capture as usize as u64 {
        return;
    }
    CAPTURE_STATE.store(state, Ordering::Release);
    if state == 0 {
        CAPTURE_STARTED.store(true, Ordering::Release);
    } else if is_terminal_state(state) {
        CAPTURE_STARTED.store(false, Ordering::Release);
        schedule_release(capture);
    }
}

unsafe extern "C" fn capture_error_callback(
    capture: *mut OH_AVScreenCapture,
    error: i32,
    _user_data: *mut c_void,
) {
    if CAPTURE_HANDLE.load(Ordering::Acquire) != capture as usize as u64 {
        return;
    }
    CAPTURE_ERROR.store(error, Ordering::Release);
    CAPTURE_STARTED.store(false, Ordering::Release);
    schedule_release(capture);
}

fn copy_image_frame(image: *mut OH_ImageNative) -> Result<Vec<u8>, String> {
    let mut size = ImageSize::default();
    let size_result = unsafe { OH_ImageNative_GetImageSize(image, &mut size) };
    if size_result != IMAGE_SUCCESS {
        return Err(format!("OH_ImageNative_GetImageSize failed: {size_result}"));
    }
    let width = usize::try_from(size.width).map_err(|_| "image width exceeds usize".to_owned())?;
    let height =
        usize::try_from(size.height).map_err(|_| "image height exceeds usize".to_owned())?;
    CONFIGURED_WIDTH.store(width, Ordering::Release);
    CONFIGURED_HEIGHT.store(height, Ordering::Release);

    let mut type_count = 0usize;
    let count_result =
        unsafe { OH_ImageNative_GetComponentTypes(image, ptr::null_mut(), &mut type_count) };
    if count_result != IMAGE_SUCCESS || type_count == 0 || type_count > 8 {
        return Err(format!(
            "OH_ImageNative_GetComponentTypes count failed: result={count_result} count={type_count}"
        ));
    }
    let mut component_types = vec![0u32; type_count];
    let mut component_types_ptr = component_types.as_mut_ptr();
    let types_result = unsafe {
        OH_ImageNative_GetComponentTypes(image, &mut component_types_ptr, &mut type_count)
    };
    if types_result != IMAGE_SUCCESS || type_count == 0 {
        return Err(format!(
            "OH_ImageNative_GetComponentTypes failed: result={types_result} count={type_count}"
        ));
    }

    let component_type = component_types[0];
    let mut native_buffer = ptr::null_mut();
    let mut buffer_size = 0usize;
    let mut row_stride = 0i32;
    let mut pixel_stride = 0i32;
    let mut format = 0i32;
    let buffer_result =
        unsafe { OH_ImageNative_GetByteBuffer(image, component_type, &mut native_buffer) };
    let buffer_size_result =
        unsafe { OH_ImageNative_GetBufferSize(image, component_type, &mut buffer_size) };
    let row_stride_result =
        unsafe { OH_ImageNative_GetRowStride(image, component_type, &mut row_stride) };
    let pixel_stride_result =
        unsafe { OH_ImageNative_GetPixelStride(image, component_type, &mut pixel_stride) };
    let format_result = unsafe { OH_ImageNative_GetFormat(image, &mut format) };
    if buffer_result != IMAGE_SUCCESS
        || buffer_size_result != IMAGE_SUCCESS
        || row_stride_result != IMAGE_SUCCESS
        || pixel_stride_result != IMAGE_SUCCESS
        || format_result != IMAGE_SUCCESS
        || native_buffer.is_null()
    {
        return Err(format!(
            "OH_ImageNative buffer metadata failed: buffer={buffer_result} size={buffer_size_result} row={row_stride_result} pixel={pixel_stride_result} format={format_result}"
        ));
    }

    let row_stride =
        usize::try_from(row_stride).map_err(|_| "negative image row stride".to_owned())?;
    let pixel_stride =
        usize::try_from(pixel_stride).map_err(|_| "negative image pixel stride".to_owned())?;
    let row_bytes = width
        .checked_mul(4)
        .ok_or_else(|| "image row size overflow".to_owned())?;
    let expected_bytes = row_bytes
        .checked_mul(height)
        .ok_or_else(|| "image size overflow".to_owned())?;
    let required_bytes = row_stride
        .checked_mul(height.saturating_sub(1))
        .and_then(|value| value.checked_add(row_bytes))
        .ok_or_else(|| "image buffer size overflow".to_owned())?;
    if (pixel_stride != 1 && pixel_stride != 4)
        || row_stride < row_bytes
        || required_bytes > buffer_size
        || expected_bytes > MAX_FRAME_BYTES
        || !matches!(
            format,
            NATIVEBUFFER_PIXEL_FMT_RGBA_8888 | NATIVEBUFFER_PIXEL_FMT_BGRA_8888
        )
    {
        return Err(format!(
            "unsupported image buffer: size={width}x{height} bytes={buffer_size} row={row_stride} pixel={pixel_stride} format={format}"
        ));
    }

    let mut address = ptr::null_mut();
    let map_result = unsafe { OH_NativeBuffer_Map(native_buffer, &mut address) };
    if map_result != NATIVE_ERROR_OK || address.is_null() {
        return Err(format!("OH_NativeBuffer_Map failed: {map_result}"));
    }
    let source = unsafe { std::slice::from_raw_parts(address.cast::<u8>(), buffer_size) };
    let mut rgba = vec![0u8; expected_bytes];
    for row in 0..height {
        let source_row = &source[row * row_stride..row * row_stride + row_bytes];
        let target_row = &mut rgba[row * row_bytes..(row + 1) * row_bytes];
        if format == NATIVEBUFFER_PIXEL_FMT_RGBA_8888 {
            target_row.copy_from_slice(source_row);
        } else {
            for (source_pixel, target_pixel) in source_row
                .chunks_exact(4)
                .zip(target_row.chunks_exact_mut(4))
            {
                target_pixel.copy_from_slice(&[
                    source_pixel[2],
                    source_pixel[1],
                    source_pixel[0],
                    source_pixel[3],
                ]);
            }
        }
    }
    let unmap_result = unsafe { OH_NativeBuffer_Unmap(native_buffer) };
    if unmap_result != NATIVE_ERROR_OK {
        return Err(format!("OH_NativeBuffer_Unmap failed: {unmap_result}"));
    }
    Ok(rgba)
}

unsafe extern "C" fn image_receiver_callback(receiver: *mut OH_ImageReceiverNative) {
    if IMAGE_RECEIVER_HANDLE.load(Ordering::Acquire) != receiver as usize as u64
        || CAPTURE_HANDLE.load(Ordering::Acquire) == 0
    {
        return;
    }
    let mut image = ptr::null_mut();
    let read_result = unsafe { OH_ImageReceiverNative_ReadLatestImage(receiver, &mut image) };
    if read_result != IMAGE_SUCCESS || image.is_null() {
        report_frame_error(&format!(
            "OH_ImageReceiverNative_ReadLatestImage failed: {read_result}"
        ));
        return;
    }
    let frame_result = copy_image_frame(image);
    let release_result = unsafe { OH_ImageNative_Release(image) };
    if release_result != IMAGE_SUCCESS {
        report_frame_error(&format!("OH_ImageNative_Release failed: {release_result}"));
        return;
    }
    match frame_result {
        Ok(frame) => {
            let width = CONFIGURED_WIDTH.load(Ordering::Acquire);
            let height = CONFIGURED_HEIGHT.load(Ordering::Acquire);
            if crate::platform::ohos::push_host_screen_frame_rgba(&frame, width, height) {
                CAPTURE_FRAMES.fetch_add(1, Ordering::Relaxed);
                CAPTURE_BYTES.fetch_add(frame.len() as u64, Ordering::Relaxed);
            } else {
                report_frame_error("RustDesk rejected the captured RGBA frame");
            }
        }
        Err(error) => report_frame_error(&error),
    }
}

pub(crate) fn configure(width: usize, height: usize, display_id: u64) -> bool {
    let Some(frame_bytes) = width
        .checked_mul(height)
        .and_then(|value| value.checked_mul(4))
    else {
        return false;
    };
    if width == 0
        || height == 0
        || width > i32::MAX as usize
        || height > i32::MAX as usize
        || frame_bytes > MAX_FRAME_BYTES
    {
        return false;
    }
    CONFIGURED_WIDTH.store(width, Ordering::Release);
    CONFIGURED_HEIGHT.store(height, Ordering::Release);
    CONFIGURED_DISPLAY_ID.store(display_id, Ordering::Release);
    crate::platform::ohos::set_host_display_id(display_id);
    CAPTURE_ERROR.store(0, Ordering::Release);
    crate::platform::ohos::configure_host_screen(width, height)
}

pub(crate) struct OhosScreenCapture {
    handle: u64,
}

impl OhosScreenCapture {
    pub(crate) fn start() -> Result<Self, String> {
        if CLEANUP_IN_PROGRESS.load(Ordering::Acquire) {
            return Err("previous OHOS screen capture cleanup is still in progress".to_owned());
        }
        if CAPTURE_HANDLE.load(Ordering::Acquire) != 0 {
            return Err("OHOS screen capture is already active".to_owned());
        }
        let width = CONFIGURED_WIDTH.load(Ordering::Acquire);
        let height = CONFIGURED_HEIGHT.load(Ordering::Acquire);
        let display_id = CONFIGURED_DISPLAY_ID.load(Ordering::Acquire);
        if !configure(width, height, display_id) {
            return Err("OHOS screen capture geometry is unavailable".to_owned());
        }
        restore_configured_geometry();
        CAPTURE_STATE.store(-1, Ordering::Release);
        CAPTURE_ERROR.store(0, Ordering::Release);
        CAPTURE_STARTED.store(false, Ordering::Release);
        CAPTURE_FRAMES.store(0, Ordering::Release);
        CAPTURE_BYTES.store(0, Ordering::Release);
        FRAME_ERROR_REPORTED.store(false, Ordering::Release);

        let capture = unsafe { OH_AVScreenCapture_Create() };
        if capture.is_null() {
            return Err("OH_AVScreenCapture_Create returned null".to_owned());
        }
        let config = OH_AVScreenCaptureConfig {
            capture_mode: 1,
            data_type: 0,
            audio: OH_AudioInfo::default(),
            video: OH_VideoInfo {
                capture: OH_VideoCaptureInfo {
                    display_id,
                    mission_ids: ptr::null_mut(),
                    mission_ids_len: 0,
                    width: width as i32,
                    height: height as i32,
                    source: 2,
                },
                enc: OH_VideoEncInfo {
                    codec: 0,
                    bitrate: 0,
                    frame_rate: 30,
                },
            },
            recorder: OH_RecorderInfo::default(),
        };
        let microphone_result = unsafe { OH_AVScreenCapture_SetMicrophoneEnabled(capture, false) };
        let state_result = unsafe {
            OH_AVScreenCapture_SetStateCallback(capture, capture_state_callback, ptr::null_mut())
        };
        let error_result = unsafe {
            OH_AVScreenCapture_SetErrorCallback(capture, capture_error_callback, ptr::null_mut())
        };
        let init_result = unsafe { OH_AVScreenCapture_Init(capture, config) };
        if microphone_result != AV_SCREEN_CAPTURE_OK
            || state_result != AV_SCREEN_CAPTURE_OK
            || error_result != AV_SCREEN_CAPTURE_OK
            || init_result != AV_SCREEN_CAPTURE_OK
        {
            unsafe {
                OH_AVScreenCapture_Release(capture);
            }
            return Err(format!(
                "OHOS screen capture setup failed: microphone={microphone_result} state={state_result} error={error_result} init={init_result}"
            ));
        }

        let (receiver, window) = match create_surface_receiver(width, height) {
            Ok(value) => value,
            Err(error) => {
                unsafe {
                    OH_AVScreenCapture_Release(capture);
                }
                return Err(error);
            }
        };
        IMAGE_RECEIVER_HANDLE.store(receiver as usize as u64, Ordering::Release);
        NATIVE_WINDOW_HANDLE.store(window as usize as u64, Ordering::Release);
        let handle = capture as usize as u64;
        CAPTURE_HANDLE.store(handle, Ordering::Release);
        let start_result =
            unsafe { OH_AVScreenCapture_StartScreenCaptureWithSurface(capture, window) };
        if start_result != AV_SCREEN_CAPTURE_OK {
            let _ = release_capture(handle);
            return Err(format!(
                "OH_AVScreenCapture_StartScreenCaptureWithSurface failed: {start_result}"
            ));
        }

        let deadline = Instant::now() + START_TIMEOUT;
        loop {
            if CAPTURE_HANDLE.load(Ordering::Acquire) != handle {
                return Err(format!(
                    "OHOS screen capture stopped during startup: state={} error={}",
                    CAPTURE_STATE.load(Ordering::Acquire),
                    CAPTURE_ERROR.load(Ordering::Acquire)
                ));
            }
            let error = CAPTURE_ERROR.load(Ordering::Acquire);
            if error != 0 {
                let _ = release_capture(handle);
                return Err(format!("OHOS screen capture failed: {error}"));
            }
            if CAPTURE_STARTED.load(Ordering::Acquire) && CAPTURE_FRAMES.load(Ordering::Acquire) > 0
            {
                log::info!("OHOS screen capture started");
                return Ok(Self { handle });
            }
            if Instant::now() >= deadline {
                let state = CAPTURE_STATE.load(Ordering::Acquire);
                let frames = CAPTURE_FRAMES.load(Ordering::Acquire);
                let _ = release_capture(handle);
                return Err(format!(
                    "OHOS screen capture confirmation or first frame timed out: state={state} frames={frames}"
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for OhosScreenCapture {
    fn drop(&mut self) {
        if let Some((stop_result, release_result)) = release_capture(self.handle) {
            if stop_result != AV_SCREEN_CAPTURE_OK {
                log::warn!("OH_AVScreenCapture_StopScreenCapture failed: {stop_result}");
            }
            if release_result != AV_SCREEN_CAPTURE_OK {
                log::error!("OH_AVScreenCapture_Release failed: {release_result}");
            }
            log::info!(
                "OHOS screen capture stopped after {} frames ({} bytes)",
                CAPTURE_FRAMES.load(Ordering::Acquire),
                CAPTURE_BYTES.load(Ordering::Acquire)
            );
        }
    }
}

pub(crate) fn start_captured_host() -> Result<(), String> {
    let mut owner = capture_owner()
        .lock()
        .map_err(|_| "OHOS screen capture owner lock is poisoned".to_owned())?;
    if owner.is_some() && CAPTURE_HANDLE.load(Ordering::Acquire) != 0 {
        return Ok(());
    }
    owner.take();
    let input_enabled = hbb_common::config::option2bool(
        hbb_common::config::keys::OPTION_ENABLE_KEYBOARD,
        &hbb_common::config::Config::get_option(hbb_common::config::keys::OPTION_ENABLE_KEYBOARD),
    );
    if crate::platform::ohos::host_input_capable() && input_enabled {
        if let Err(error) = crate::platform::ohos::request_host_input_authorization() {
            log::warn!("HarmonyOS host input will remain disabled: {error}");
        }
    }
    let capture = OhosScreenCapture::start()?;
    if !crate::platform::ohos::start_host() {
        drop(capture);
        return Err("RustDesk OHOS host failed to start".to_owned());
    }
    *owner = Some(capture);
    Ok(())
}

pub(crate) fn stop_captured_host() -> Result<(), String> {
    let capture = capture_owner()
        .lock()
        .map_err(|_| "OHOS screen capture owner lock is poisoned".to_owned())?
        .take();
    drop(capture);
    crate::platform::ohos::stop_host();
    Ok(())
}
