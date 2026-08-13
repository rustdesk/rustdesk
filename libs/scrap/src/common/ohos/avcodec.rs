use super::direct_render::DirectRenderTarget;
use crate::{common::GoogleImage, CodecFormat, ImageRgb};
use hbb_common::message_proto::{Chroma, EncodedVideoFrames, SupportedDecoding};
use hbb_common::{anyhow::anyhow, bail, ResultType};
use std::{
    collections::{HashMap, VecDeque},
    convert::TryFrom,
    ffi::{c_char, c_void},
    ffi::{CStr, CString},
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Condvar, Mutex, MutexGuard,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

lazy_static::lazy_static! {
    static ref LAST_DECODER_INIT_ERROR: Mutex<String> = Mutex::new(String::new());
    static ref DECODER_SUPPORT_CACHE: Mutex<Vec<(CodecFormat, bool)>> = Mutex::new(Vec::new());
}

static MEDIA_LOCK_POISON_REPORTED: AtomicBool = AtomicBool::new(false);
static INPUT_SHUTDOWN_DROP_REPORTED: AtomicBool = AtomicBool::new(false);

const AV_ERR_OK: i32 = 0;
const AV_ERR_INVALID_STATE: i32 = 8;
const AV_ERR_TRY_AGAIN_LATER: i32 = 5410006;
const AV_ERR_STREAM_CHANGED: i32 = 5410005;
const SURFACE_QUERY_INPUT_TIMEOUT_US: i64 = 5_000;
const SURFACE_TRACE_SAMPLE_INTERVAL: u32 = 60;
const MAX_STREAM_CHANGED_RETRIES: u32 = 8;
const AV_PIXEL_FORMAT_YUVI420: i32 = 1;
const AV_PIXEL_FORMAT_NV12: i32 = 2;
const AV_PIXEL_FORMAT_NV21: i32 = 3;
const AVCODEC_BUFFER_FLAGS_SYNC_FRAME: u32 = 1 << 1;
const OH_SCALING_MODE_SCALE_FIT_V2: i32 = 4;
const HILOG_DOMAIN: u32 = 0xFF01;
const HILOG_TAG: &[u8] = b"RustDeskNative\0";
const LOG_APP: i32 = 0;
const LOG_DEBUG: i32 = 3;
const LOG_INFO: i32 = 4;
const LOG_WARN: i32 = 5;
const LOG_ERROR: i32 = 6;

fn lock_media_state<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            if !MEDIA_LOCK_POISON_REPORTED.swap(true, Ordering::Relaxed) {
                hilog_error("recovering poisoned OHOS video state mutex");
            }
            poisoned.into_inner()
        }
    }
}

#[repr(C)]
struct OH_AVCodec {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_AVBuffer {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_AVFormat {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_AVCapability {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OH_AVRange {
    min_val: i32,
    max_val: i32,
}

#[repr(C)]
struct OH_NativeBuffer {
    _private: [u8; 0],
}

#[repr(C)]
struct OHNativeWindow {
    _private: [u8; 0],
}

type OH_AVCodecOnError =
    unsafe extern "C" fn(codec: *mut OH_AVCodec, errorCode: i32, userData: *mut c_void);
type OH_AVCodecOnStreamChanged =
    unsafe extern "C" fn(codec: *mut OH_AVCodec, format: *mut OH_AVFormat, userData: *mut c_void);
type OH_AVCodecOnNeedInputBuffer = unsafe extern "C" fn(
    codec: *mut OH_AVCodec,
    index: u32,
    buffer: *mut OH_AVBuffer,
    userData: *mut c_void,
);
type OH_AVCodecOnNewOutputBuffer = unsafe extern "C" fn(
    codec: *mut OH_AVCodec,
    index: u32,
    buffer: *mut OH_AVBuffer,
    userData: *mut c_void,
);

#[repr(C)]
#[derive(Clone, Copy)]
struct OH_AVCodecCallback {
    onError: OH_AVCodecOnError,
    onStreamChanged: OH_AVCodecOnStreamChanged,
    onNeedInputBuffer: OH_AVCodecOnNeedInputBuffer,
    onNewOutputBuffer: OH_AVCodecOnNewOutputBuffer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OH_AVCodecBufferAttr {
    pts: i64,
    size: i32,
    offset: i32,
    flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OH_NativeBuffer_Plane {
    offset: u64,
    row_stride: u32,
    column_stride: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OH_NativeBuffer_Planes {
    plane_count: u32,
    planes: [OH_NativeBuffer_Plane; 4],
}

impl Default for OH_NativeBuffer_Planes {
    fn default() -> Self {
        Self {
            plane_count: 0,
            planes: [OH_NativeBuffer_Plane::default(); 4],
        }
    }
}

#[link(name = "native_media_vdec")]
unsafe extern "C" {
    fn OH_VideoDecoder_CreateByMime(mime: *const c_char) -> *mut OH_AVCodec;
    fn OH_VideoDecoder_Destroy(codec: *mut OH_AVCodec) -> i32;
    fn OH_VideoDecoder_RegisterCallback(
        codec: *mut OH_AVCodec,
        callback: OH_AVCodecCallback,
        userData: *mut c_void,
    ) -> i32;
    fn OH_VideoDecoder_Configure(codec: *mut OH_AVCodec, format: *mut OH_AVFormat) -> i32;
    fn OH_VideoDecoder_SetSurface(codec: *mut OH_AVCodec, window: *mut OHNativeWindow) -> i32;
    fn OH_VideoDecoder_Prepare(codec: *mut OH_AVCodec) -> i32;
    fn OH_VideoDecoder_Start(codec: *mut OH_AVCodec) -> i32;
    fn OH_VideoDecoder_Stop(codec: *mut OH_AVCodec) -> i32;
    fn OH_VideoDecoder_PushInputBuffer(codec: *mut OH_AVCodec, index: u32) -> i32;
    fn OH_VideoDecoder_FreeOutputBuffer(codec: *mut OH_AVCodec, index: u32) -> i32;
    fn OH_VideoDecoder_RenderOutputBuffer(codec: *mut OH_AVCodec, index: u32) -> i32;
    fn OH_VideoDecoder_QueryInputBuffer(
        codec: *mut OH_AVCodec,
        index: *mut u32,
        timeout_us: i64,
    ) -> i32;
    fn OH_VideoDecoder_GetInputBuffer(codec: *mut OH_AVCodec, index: u32) -> *mut OH_AVBuffer;
    fn OH_VideoDecoder_QueryOutputBuffer(
        codec: *mut OH_AVCodec,
        index: *mut u32,
        timeout_us: i64,
    ) -> i32;
    fn OH_VideoDecoder_GetOutputBuffer(codec: *mut OH_AVCodec, index: u32) -> *mut OH_AVBuffer;
    fn OH_VideoDecoder_GetOutputDescription(codec: *mut OH_AVCodec) -> *mut OH_AVFormat;
}

#[link(name = "hilog_ndk.z")]
unsafe extern "C" {
    fn OH_LOG_PrintMsg(
        log_type: i32,
        level: i32,
        domain: u32,
        tag: *const c_char,
        message: *const c_char,
    ) -> i32;
}

#[link(name = "native_media_codecbase")]
unsafe extern "C" {
    fn OH_AVCodec_GetCapability(mime: *const c_char, is_encoder: bool) -> *mut OH_AVCapability;
    fn OH_AVCapability_GetVideoWidthRange(
        capability: *mut OH_AVCapability,
        width_range: *mut OH_AVRange,
    ) -> i32;
    fn OH_AVCapability_GetVideoHeightRange(
        capability: *mut OH_AVCapability,
        height_range: *mut OH_AVRange,
    ) -> i32;
    fn OH_AVCapability_IsVideoSizeSupported(
        capability: *mut OH_AVCapability,
        width: i32,
        height: i32,
    ) -> bool;
    static OH_MD_KEY_WIDTH: *const c_char;
    static OH_MD_KEY_HEIGHT: *const c_char;
    static OH_MD_KEY_PIXEL_FORMAT: *const c_char;
    static OH_MD_KEY_RANGE_FLAG: *const c_char;
    static OH_MD_KEY_COLOR_PRIMARIES: *const c_char;
    static OH_MD_KEY_TRANSFER_CHARACTERISTICS: *const c_char;
    static OH_MD_KEY_MATRIX_COEFFICIENTS: *const c_char;
    static OH_MD_KEY_VIDEO_STRIDE: *const c_char;
    static OH_MD_KEY_VIDEO_SLICE_HEIGHT: *const c_char;
    static OH_MD_KEY_VIDEO_PIC_WIDTH: *const c_char;
    static OH_MD_KEY_VIDEO_PIC_HEIGHT: *const c_char;
    static OH_MD_KEY_ENABLE_SYNC_MODE: *const c_char;
    static OH_MD_KEY_VIDEO_ENABLE_LOW_LATENCY: *const c_char;
    static OH_AVCODEC_MIMETYPE_VIDEO_AVC: *const c_char;
    static OH_AVCODEC_MIMETYPE_VIDEO_HEVC: *const c_char;
}

#[link(name = "native_media_core")]
unsafe extern "C" {
    fn OH_AVFormat_Create() -> *mut OH_AVFormat;
    fn OH_AVFormat_Destroy(format: *mut OH_AVFormat);
    fn OH_AVFormat_SetIntValue(format: *mut OH_AVFormat, key: *const c_char, value: i32) -> bool;
    fn OH_AVFormat_GetIntValue(format: *mut OH_AVFormat, key: *const c_char, out: *mut i32)
        -> bool;

    fn OH_AVBuffer_SetBufferAttr(
        buffer: *mut OH_AVBuffer,
        attr: *const OH_AVCodecBufferAttr,
    ) -> i32;
    fn OH_AVBuffer_GetBufferAttr(buffer: *mut OH_AVBuffer, attr: *mut OH_AVCodecBufferAttr) -> i32;
    fn OH_AVBuffer_GetParameter(buffer: *mut OH_AVBuffer) -> *mut OH_AVFormat;
    fn OH_AVBuffer_GetAddr(buffer: *mut OH_AVBuffer) -> *mut u8;
    fn OH_AVBuffer_GetCapacity(buffer: *mut OH_AVBuffer) -> i32;
    fn OH_AVBuffer_GetNativeBuffer(buffer: *mut OH_AVBuffer) -> *mut OH_NativeBuffer;
}

#[link(name = "native_buffer")]
unsafe extern "C" {
    fn OH_NativeBuffer_MapPlanes(
        buffer: *mut OH_NativeBuffer,
        vir_addr: *mut *mut c_void,
        out_planes: *mut OH_NativeBuffer_Planes,
    ) -> i32;
    fn OH_NativeBuffer_Unmap(buffer: *mut OH_NativeBuffer) -> i32;
    fn OH_NativeBuffer_Unreference(buffer: *mut OH_NativeBuffer) -> i32;
}

#[link(name = "native_window")]
unsafe extern "C" {
    fn OH_NativeWindow_CreateNativeWindowFromSurfaceId(
        surface_id: u64,
        window: *mut *mut OHNativeWindow,
    ) -> i32;
    fn OH_NativeWindow_DestroyNativeWindow(window: *mut OHNativeWindow);
    fn OH_NativeWindow_NativeWindowSetScalingModeV2(
        window: *mut OHNativeWindow,
        scaling_mode: i32,
    ) -> i32;
}

#[derive(Clone, Copy)]
struct FormatInfo {
    width: usize,
    height: usize,
    pixel_format: i32,
    stride: usize,
    slice_height: usize,
    range_flag: Option<i32>,
    color_primaries: Option<i32>,
    transfer_characteristics: Option<i32>,
    matrix_coefficients: Option<i32>,
}

struct BufferItem {
    index: u32,
    buffer: *mut OH_AVBuffer,
    queued_at: Instant,
}

unsafe impl Send for BufferItem {}

#[derive(Clone, Copy)]
struct SurfaceInputTrace {
    submitted_at: Instant,
    input_wait: Duration,
}

struct SurfaceOutputTrace {
    pts: i64,
    input_wait: Duration,
    submit_to_worker: Duration,
    output_queue_wait: Duration,
    output_queue_depth: usize,
    pending_inputs: usize,
}

#[derive(Default)]
struct SurfaceQueues {
    input_buffers: VecDeque<BufferItem>,
    output_buffers: VecDeque<BufferItem>,
    running: bool,
    last_error: Option<String>,
}

struct SurfaceCallbackState {
    queues: Mutex<SurfaceQueues>,
    input_ready: Condvar,
    output_ready: Condvar,
    input_pts_at: Mutex<HashMap<i64, SurfaceInputTrace>>,
    last_decode_latency_ms: Mutex<Option<u64>>,
    trace_sequence: AtomicU32,
    input_callback_count: AtomicU32,
    output_callback_count: AtomicU32,
    render_count: AtomicU32,
    session_id: Option<String>,
    display: usize,
}

impl SurfaceCallbackState {
    fn new(session_id: Option<String>, display: usize) -> Self {
        Self {
            queues: Mutex::new(SurfaceQueues {
                input_buffers: VecDeque::new(),
                output_buffers: VecDeque::new(),
                running: true,
                last_error: None,
            }),
            input_ready: Condvar::new(),
            output_ready: Condvar::new(),
            input_pts_at: Mutex::new(HashMap::new()),
            last_decode_latency_ms: Mutex::new(None),
            trace_sequence: AtomicU32::new(0),
            input_callback_count: AtomicU32::new(0),
            output_callback_count: AtomicU32::new(0),
            render_count: AtomicU32::new(0),
            session_id,
            display,
        }
    }
}

fn callback_state_from_user_data(user_data: *mut c_void) -> Option<&'static SurfaceCallbackState> {
    if user_data.is_null() {
        None
    } else {
        Some(unsafe { &*(user_data as *const SurfaceCallbackState) })
    }
}

unsafe extern "C" fn on_decoder_error(
    _codec: *mut OH_AVCodec,
    errorCode: i32,
    userData: *mut c_void,
) {
    if let Some(state) = callback_state_from_user_data(userData) {
        let mut queues = lock_media_state(&state.queues);
        queues.last_error = Some(format!("decoder error {}", errorCode));
        queues.running = false;
        drop(queues);
        state.input_ready.notify_all();
        state.output_ready.notify_all();
    }
    hilog_error(&format!("OHOS decoder callback error: {}", errorCode));
}

unsafe extern "C" fn on_decoder_stream_changed(
    _codec: *mut OH_AVCodec,
    format: *mut OH_AVFormat,
    userData: *mut c_void,
) {
    let Some(state) = callback_state_from_user_data(userData) else {
        return;
    };
    if !format.is_null() {
        if let Ok(info) = output_format_from_avformat(format) {
            hilog_info(&format!(
                "OHOS decoder stream changed width={} height={} pixel_format={} stride={} slice_height={} range={:?} primaries={:?} transfer={:?} matrix={:?}",
                info.width,
                info.height,
                info.pixel_format,
                info.stride,
                info.slice_height,
                info.range_flag,
                info.color_primaries,
                info.transfer_characteristics,
                info.matrix_coefficients
            ));
        }
    }
    let queues = lock_media_state(&state.queues);
    drop(queues);
    state.output_ready.notify_all();
}

unsafe extern "C" fn on_decoder_need_input_buffer(
    _codec: *mut OH_AVCodec,
    index: u32,
    buffer: *mut OH_AVBuffer,
    userData: *mut c_void,
) {
    let Some(state) = callback_state_from_user_data(userData) else {
        return;
    };
    let mut queues = lock_media_state(&state.queues);
    if !queues.running {
        drop(queues);
        if !INPUT_SHUTDOWN_DROP_REPORTED.swap(true, Ordering::Relaxed) {
            hilog_debug("dropping OHOS decoder input buffer during shutdown");
        }
        return;
    }
    queues.input_buffers.push_back(BufferItem {
        index,
        buffer,
        queued_at: Instant::now(),
    });
    drop(queues);
    if state.input_callback_count.fetch_add(1, Ordering::Relaxed) < 3 {
        hilog_info(&format!(
            "OHOS surface input callback index={} buffer={:p}",
            index, buffer
        ));
    }
    state.input_ready.notify_one();
}

unsafe extern "C" fn on_decoder_new_output_buffer(
    _codec: *mut OH_AVCodec,
    index: u32,
    buffer: *mut OH_AVBuffer,
    userData: *mut c_void,
) {
    let Some(state) = callback_state_from_user_data(userData) else {
        return;
    };
    let mut queues = lock_media_state(&state.queues);
    if !queues.running {
        return;
    }
    queues.output_buffers.push_back(BufferItem {
        index,
        buffer,
        queued_at: Instant::now(),
    });
    let output_queue_depth = queues.output_buffers.len();
    drop(queues);
    if state.output_callback_count.fetch_add(1, Ordering::Relaxed) < 3 {
        hilog_info(&format!(
            "OHOS surface output callback index={} buffer={:p}",
            index, buffer
        ));
    }
    let _ = output_queue_depth;
    state.output_ready.notify_one();
}

fn record_surface_output_latency(
    state: &SurfaceCallbackState,
    item: &BufferItem,
    attr: &OH_AVCodecBufferAttr,
    output_queue_depth: usize,
) -> Option<SurfaceOutputTrace> {
    let mut input_pts_at = lock_media_state(&state.input_pts_at);
    let trace = input_pts_at.remove(&attr.pts)?;
    let pending_inputs = input_pts_at.len();
    drop(input_pts_at);
    let submit_to_worker = trace.submitted_at.elapsed();
    Some(SurfaceOutputTrace {
        pts: attr.pts,
        input_wait: trace.input_wait,
        submit_to_worker,
        output_queue_wait: item.queued_at.elapsed(),
        output_queue_depth,
        pending_inputs,
    })
}

fn surface_output_worker(codec: usize, state: Arc<SurfaceCallbackState>) {
    let codec = codec as *mut OH_AVCodec;
    loop {
        let item = {
            let mut queues = lock_media_state(&state.queues);
            while queues.running && queues.last_error.is_none() && queues.output_buffers.is_empty()
            {
                queues = state.output_ready.wait(queues).unwrap_or_else(|poisoned| {
                    if !MEDIA_LOCK_POISON_REPORTED.swap(true, Ordering::Relaxed) {
                        hilog_error("recovering poisoned OHOS video state mutex");
                    }
                    poisoned.into_inner()
                });
            }
            if !queues.running || queues.last_error.is_some() {
                return;
            }
            let output_queue_depth = queues.output_buffers.len();
            queues
                .output_buffers
                .pop_front()
                .map(|item| (item, output_queue_depth))
        };

        let Some((item, output_queue_depth)) = item else {
            continue;
        };

        let output_trace = output_buffer_attr(item.buffer).ok().and_then(|attr| {
            record_surface_output_latency(&state, &item, &attr, output_queue_depth)
        });

        let render_number = state.render_count.load(Ordering::Relaxed) + 1;
        let trace_sample = output_trace.as_ref().is_some_and(|_| {
            (state.trace_sequence.fetch_add(1, Ordering::Relaxed) + 1)
                % SURFACE_TRACE_SAMPLE_INTERVAL
                == 0
        });
        let render_started_at = trace_sample.then(Instant::now);
        if render_number < 3 {
            hilog_info(&format!(
                "OHOS surface render begin number={} index={}",
                render_number, item.index
            ));
        }
        let result = unsafe { OH_VideoDecoder_RenderOutputBuffer(codec, item.index) };
        if let (Some(trace), Some(render_started_at)) = (output_trace.as_ref(), render_started_at) {
            hilog_info(&format!(
                "OHOS surface trace pts={} input_wait_ms={} submit_to_worker_ms={} output_queue_wait_ms={} output_queue_depth={} pending_inputs={} render_call_ms={} render_result={}",
                trace.pts,
                trace.input_wait.as_millis(),
                trace.submit_to_worker.as_millis(),
                trace.output_queue_wait.as_millis(),
                trace.output_queue_depth,
                trace.pending_inputs,
                render_started_at.elapsed().as_millis(),
                result
            ));
        }
        if result == AV_ERR_OK {
            let rendered = state.render_count.fetch_add(1, Ordering::Release) + 1;
            let latency = output_trace
                .as_ref()
                .map(|trace| trace.submit_to_worker.as_millis() as u64);
            *lock_media_state(&state.last_decode_latency_ms) = latency;
            super::direct_render::notify_frame_rendered(
                state.session_id.as_deref(),
                state.display,
                latency,
            );
            if rendered <= 3 {
                hilog_info(&format!(
                    "OHOS surface render complete number={} index={}",
                    rendered, item.index
                ));
            }
        } else {
            hilog_warn(&format!(
                "OHOS surface RenderOutputBuffer failed for {}: {}",
                item.index, result
            ));
            let free_result = unsafe { OH_VideoDecoder_FreeOutputBuffer(codec, item.index) };
            if free_result != AV_ERR_OK {
                hilog_warn(&format!(
                    "OHOS surface FreeOutputBuffer failed for {} after render error: {}",
                    item.index, free_result
                ));
            }
        }
    }
}

enum PushInputState {
    Submitted,
    RetryAfterDrain,
}

impl Default for FormatInfo {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            pixel_format: AV_PIXEL_FORMAT_YUVI420,
            stride: 0,
            slice_height: 0,
            range_flag: None,
            color_primaries: None,
            transfer_characteristics: None,
            matrix_coefficients: None,
        }
    }
}

pub struct OhosVideoDecoder {
    codec: *mut OH_AVCodec,
    window: *mut OHNativeWindow,
    callback_state: Option<Arc<SurfaceCallbackState>>,
    callback_state_raw: *const SurfaceCallbackState,
    surface_output_thread: Option<JoinHandle<()>>,
    pts_origin: Instant,
    last_pts: i64,
    input_pts_at: HashMap<i64, Instant>,
    last_decode_latency_ms: Option<u64>,
    last_observed_render_count: u32,
    frames: Vec<OhosImage>,
}

unsafe impl Send for OhosVideoDecoder {}

impl Drop for OhosVideoDecoder {
    fn drop(&mut self) {
        if let Some(state) = self.callback_state.as_ref() {
            let mut queues = lock_media_state(&state.queues);
            queues.running = false;
            drop(queues);
            state.input_ready.notify_all();
            state.output_ready.notify_all();
        }
        if let Some(thread) = self.surface_output_thread.take() {
            let _ = thread.join();
        }
        if !self.codec.is_null() {
            unsafe {
                let _ = OH_VideoDecoder_Stop(self.codec);
                let _ = OH_VideoDecoder_Destroy(self.codec);
            }
        }
        if !self.callback_state_raw.is_null() {
            unsafe {
                let _ = Arc::from_raw(self.callback_state_raw);
            }
            self.callback_state_raw = ptr::null();
        }
        if !self.window.is_null() {
            unsafe {
                OH_NativeWindow_DestroyNativeWindow(self.window);
            }
        }
    }
}

impl OhosVideoDecoder {
    pub fn new_h264_with_surface(
        width: usize,
        height: usize,
        surface_id: Option<u64>,
    ) -> ResultType<Self> {
        Self::new_with_surface(h264_mime(), width, height, surface_id, None, 0)
    }

    pub fn new_h265_with_surface(
        width: usize,
        height: usize,
        surface_id: Option<u64>,
    ) -> ResultType<Self> {
        Self::new_with_surface(h265_mime(), width, height, surface_id, None, 0)
    }

    fn new_with_surface(
        mime: *const c_char,
        width: usize,
        height: usize,
        surface_id: Option<u64>,
        session_id: Option<String>,
        display: usize,
    ) -> ResultType<Self> {
        *lock_media_state(&LAST_DECODER_INIT_ERROR) = String::new();
        let width_i32 = i32::try_from(width)
            .map_err(|_| anyhow!("OHOS decoder width is out of range: {}", width))?;
        let height_i32 = i32::try_from(height)
            .map_err(|_| anyhow!("OHOS decoder height is out of range: {}", height))?;
        let mime_name = unsafe { CStr::from_ptr(mime).to_string_lossy().into_owned() };
        hilog_info(&format!(
            "OHOS decoder init mime={} width={} height={} surface_id={:?}",
            mime_name, width, height, surface_id
        ));
        // Capability size checks are diagnostics only; some decoders underreport support.
        unsafe { log_video_size_support(mime, width_i32, height_i32) };
        let codec = unsafe { create_decoder(mime) };
        if codec.is_null() {
            *lock_media_state(&LAST_DECODER_INIT_ERROR) =
                format!("CreateByMime returned null for {}", unsafe {
                    CStr::from_ptr(mime).to_string_lossy()
                });
            bail!("failed to create OHOS decoder")
        }
        let window = match surface_id {
            Some(surface_id) => match create_native_window(surface_id, width, height) {
                Ok(window) => window,
                Err(err) => {
                    unsafe {
                        let _ = OH_VideoDecoder_Destroy(codec);
                    }
                    *lock_media_state(&LAST_DECODER_INIT_ERROR) = err.to_string();
                    return Err(err);
                }
            },
            None => ptr::null_mut(),
        };
        let callback_state = if !window.is_null() {
            Some(Arc::new(SurfaceCallbackState::new(session_id, display)))
        } else {
            None
        };
        let mut callback_state_raw = ptr::null();
        let format = unsafe { OH_AVFormat_Create() };
        if format.is_null() {
            unsafe {
                let _ = OH_VideoDecoder_Destroy(codec);
                if !window.is_null() {
                    OH_NativeWindow_DestroyNativeWindow(window);
                }
            }
            *lock_media_state(&LAST_DECODER_INIT_ERROR) =
                "OH_AVFormat_Create returned null".to_owned();
            bail!("failed to create decoder format")
        }
        let mut low_latency_key_available = false;
        let mut low_latency_requested = false;
        unsafe {
            OH_AVFormat_SetIntValue(format, OH_MD_KEY_WIDTH, width_i32);
            OH_AVFormat_SetIntValue(format, OH_MD_KEY_HEIGHT, height_i32);
            OH_AVFormat_SetIntValue(format, OH_MD_KEY_PIXEL_FORMAT, AV_PIXEL_FORMAT_NV12);
            if window.is_null() {
                OH_AVFormat_SetIntValue(format, OH_MD_KEY_ENABLE_SYNC_MODE, 1);
            }
            low_latency_key_available = !OH_MD_KEY_VIDEO_ENABLE_LOW_LATENCY.is_null();
            if low_latency_key_available {
                low_latency_requested =
                    OH_AVFormat_SetIntValue(format, OH_MD_KEY_VIDEO_ENABLE_LOW_LATENCY, 1);
            }
        }
        hilog_info(&format!(
            "OHOS decoder config mime={} width={} height={} low_latency_key_available={} low_latency_set={}",
            mime_name,
            width,
            height,
            low_latency_key_available,
            low_latency_requested
        ));
        if let Some(state) = callback_state.as_ref() {
            callback_state_raw = Arc::into_raw(state.clone());
            let callback = OH_AVCodecCallback {
                onError: on_decoder_error,
                onStreamChanged: on_decoder_stream_changed,
                onNeedInputBuffer: on_decoder_need_input_buffer,
                onNewOutputBuffer: on_decoder_new_output_buffer,
            };
            if let Err(err) = ensure_ok(
                unsafe {
                    OH_VideoDecoder_RegisterCallback(
                        codec,
                        callback,
                        callback_state_raw as *mut c_void,
                    )
                },
                "RegisterCallback",
            ) {
                unsafe {
                    let _ = Arc::from_raw(callback_state_raw);
                    let _ = OH_VideoDecoder_Destroy(codec);
                    OH_NativeWindow_DestroyNativeWindow(window);
                }
                *lock_media_state(&LAST_DECODER_INIT_ERROR) = err.to_string();
                return Err(err);
            }
        }
        let configure = ensure_ok(
            unsafe { OH_VideoDecoder_Configure(codec, format) },
            "Configure",
        );
        unsafe { OH_AVFormat_Destroy(format) };
        if let Err(err) = configure {
            unsafe {
                let _ = OH_VideoDecoder_Destroy(codec);
                if !callback_state_raw.is_null() {
                    let _ = Arc::from_raw(callback_state_raw);
                }
                if !window.is_null() {
                    OH_NativeWindow_DestroyNativeWindow(window);
                }
            }
            *lock_media_state(&LAST_DECODER_INIT_ERROR) = err.to_string();
            return Err(err);
        }
        if !window.is_null() {
            if let Err(err) = ensure_ok(
                unsafe { OH_VideoDecoder_SetSurface(codec, window) },
                "SetSurface",
            ) {
                unsafe {
                    let _ = OH_VideoDecoder_Destroy(codec);
                    if !callback_state_raw.is_null() {
                        let _ = Arc::from_raw(callback_state_raw);
                    }
                    OH_NativeWindow_DestroyNativeWindow(window);
                }
                *lock_media_state(&LAST_DECODER_INIT_ERROR) = err.to_string();
                return Err(err);
            }
        }
        if let Err(err) = ensure_ok(unsafe { OH_VideoDecoder_Prepare(codec) }, "Prepare") {
            unsafe {
                let _ = OH_VideoDecoder_Destroy(codec);
                if !callback_state_raw.is_null() {
                    let _ = Arc::from_raw(callback_state_raw);
                }
                if !window.is_null() {
                    OH_NativeWindow_DestroyNativeWindow(window);
                }
            }
            *lock_media_state(&LAST_DECODER_INIT_ERROR) = err.to_string();
            return Err(err);
        }
        if let Err(err) = ensure_ok(unsafe { OH_VideoDecoder_Start(codec) }, "Start") {
            unsafe {
                let _ = OH_VideoDecoder_Destroy(codec);
                if !callback_state_raw.is_null() {
                    let _ = Arc::from_raw(callback_state_raw);
                }
                if !window.is_null() {
                    OH_NativeWindow_DestroyNativeWindow(window);
                }
            }
            *lock_media_state(&LAST_DECODER_INIT_ERROR) = err.to_string();
            return Err(err);
        }
        let surface_output_thread = if let Some(state) = callback_state.as_ref() {
            let state = state.clone();
            let codec_address = codec as usize;
            match thread::Builder::new()
                .name("ohos-video-output".to_owned())
                .spawn(move || surface_output_worker(codec_address, state))
            {
                Ok(thread) => Some(thread),
                Err(err) => {
                    let message = format!("failed to start OHOS surface output worker: {err}");
                    unsafe {
                        let _ = OH_VideoDecoder_Stop(codec);
                        let _ = OH_VideoDecoder_Destroy(codec);
                        if !callback_state_raw.is_null() {
                            let _ = Arc::from_raw(callback_state_raw);
                        }
                        if !window.is_null() {
                            OH_NativeWindow_DestroyNativeWindow(window);
                        }
                    }
                    *lock_media_state(&LAST_DECODER_INIT_ERROR) = message.clone();
                    return Err(anyhow!(message));
                }
            }
        } else {
            None
        };

        Ok(Self {
            codec,
            window,
            callback_state,
            callback_state_raw,
            surface_output_thread,
            pts_origin: Instant::now(),
            last_pts: -1,
            input_pts_at: HashMap::new(),
            last_decode_latency_ms: None,
            last_observed_render_count: 0,
            frames: Vec::new(),
        })
    }

    pub fn is_surface_mode(&self) -> bool {
        !self.window.is_null()
    }

    pub fn decode_with_key<'a>(
        &'a mut self,
        data: &[u8],
        key: bool,
    ) -> ResultType<OhosDecodeFrames<'a>> {
        // Sync-mode decoders can block new input until pending outputs are released.
        let mut frames = self.drain_outputs(false)?;
        if matches!(self.push_input(data, key)?, PushInputState::RetryAfterDrain) {
            frames.extend(self.drain_outputs(true)?);
            if matches!(self.push_input(data, key)?, PushInputState::RetryAfterDrain) {
                bail!("OHOS decoder QueryInputBuffer remained unavailable after draining outputs")
            }
        }
        frames.extend(self.drain_outputs(true)?);
        self.frames = frames;
        Ok(OhosDecodeFrames {
            inner: self.frames.drain(..),
        })
    }

    fn push_input(&mut self, data: &[u8], key: bool) -> ResultType<PushInputState> {
        let mut index = 0u32;
        let timeout_us = if self.window.is_null() {
            100_000
        } else {
            SURFACE_QUERY_INPUT_TIMEOUT_US
        };
        let ret = unsafe { OH_VideoDecoder_QueryInputBuffer(self.codec, &mut index, timeout_us) };
        match ret {
            AV_ERR_OK => {}
            AV_ERR_INVALID_STATE | AV_ERR_TRY_AGAIN_LATER => {
                return Ok(PushInputState::RetryAfterDrain);
            }
            _ => bail!("OHOS decoder QueryInputBuffer failed: {}", ret),
        }
        let buffer = unsafe { OH_VideoDecoder_GetInputBuffer(self.codec, index) };
        if buffer.is_null() {
            bail!("OHOS decoder input buffer is null")
        }
        let (addr, data_size) = checked_input_addr(buffer, data.len())?;
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), addr, data.len());
        }
        let attr = OH_AVCodecBufferAttr {
            pts: self.next_pts(),
            size: data_size,
            offset: 0,
            flags: if key {
                AVCODEC_BUFFER_FLAGS_SYNC_FRAME
            } else {
                0
            },
        };
        self.input_pts_at.insert(attr.pts, Instant::now());
        ensure_ok(
            unsafe { OH_AVBuffer_SetBufferAttr(buffer, &attr) },
            "SetBufferAttr",
        )?;
        ensure_ok(
            unsafe { OH_VideoDecoder_PushInputBuffer(self.codec, index) },
            "PushInputBuffer",
        )?;
        Ok(PushInputState::Submitted)
    }

    pub fn submit_to_surface_with_key(&mut self, data: &[u8], key: bool) -> ResultType<bool> {
        if self.window.is_null() {
            bail!("OHOS decoder surface output requested without a bound NativeWindow")
        }
        let input_wait_started_at = Instant::now();
        let Some(item) = self.wait_for_input_buffer(Duration::from_millis(30))? else {
            // Surface callbacks can lag input delivery during decoder warmup.
            return Ok(false);
        };
        self.submit_callback_input(item, data, key, input_wait_started_at.elapsed())?;
        Ok(true)
    }

    fn drain_outputs(&mut self, wait_first: bool) -> ResultType<Vec<OhosImage>> {
        let mut images = Vec::new();
        if wait_first {
            if let Some(item) = self.query_output(Duration::from_millis(120))? {
                images.push(self.copy_output(item)?);
            }
        }
        while let Some(item) = self.query_output(Duration::from_millis(1))? {
            images.push(self.copy_output(item)?);
        }
        Ok(images)
    }

    fn copy_output(&mut self, item: BufferItem) -> ResultType<OhosImage> {
        if let Ok(attr) = output_buffer_attr(item.buffer) {
            self.record_output_attr(&attr);
        }
        let result = (|| {
            let format = output_format(self.codec, item.buffer)?;
            copy_output_image(item.buffer, format)
        })();
        let free_result = ensure_ok(
            unsafe { OH_VideoDecoder_FreeOutputBuffer(self.codec, item.index) },
            "FreeOutputBuffer",
        );
        match (result, free_result) {
            (Ok(image), Ok(())) => Ok(image),
            (Err(err), Ok(())) => Err(err),
            (Ok(_), Err(err)) => Err(err),
            (Err(err), Err(free_err)) => {
                hilog_warn(&format!(
                    "OHOS decoder failed to free output buffer after copy error: {free_err}"
                ));
                Err(err)
            }
        }
    }

    fn record_output_attr(&mut self, attr: &OH_AVCodecBufferAttr) {
        if let Some(started_at) = self.input_pts_at.remove(&attr.pts) {
            self.last_decode_latency_ms = Some(started_at.elapsed().as_millis() as u64);
        }
    }

    pub fn last_decode_latency_ms(&self) -> Option<u64> {
        if let Some(state) = self.callback_state.as_ref() {
            *lock_media_state(&state.last_decode_latency_ms)
        } else {
            self.last_decode_latency_ms
        }
    }

    fn take_surface_rendered_frame(&mut self) -> bool {
        let Some(state) = self.callback_state.as_ref() else {
            return false;
        };
        let current = state.render_count.load(Ordering::Acquire);
        let rendered = current != self.last_observed_render_count;
        self.last_observed_render_count = current;
        rendered
    }

    fn wait_for_input_buffer(&self, timeout: Duration) -> ResultType<Option<BufferItem>> {
        let Some(state) = self.callback_state.as_ref() else {
            bail!("OHOS decoder surface callback state missing")
        };
        let mut guard = lock_media_state(&state.queues);
        let timeout_result = state
            .input_ready
            .wait_timeout_while(guard, timeout, |queues| {
                queues.running && queues.last_error.is_none() && queues.input_buffers.is_empty()
            })
            .unwrap_or_else(|poisoned| {
                if !MEDIA_LOCK_POISON_REPORTED.swap(true, Ordering::Relaxed) {
                    hilog_error("recovering poisoned OHOS video state mutex");
                }
                poisoned.into_inner()
            });
        guard = timeout_result.0;
        if let Some(err) = &guard.last_error {
            bail!("OHOS decoder callback error: {}", err)
        }
        if !guard.running {
            bail!("OHOS decoder callback loop stopped")
        }
        Ok(guard.input_buffers.pop_front())
    }

    fn submit_callback_input(
        &mut self,
        item: BufferItem,
        data: &[u8],
        key: bool,
        input_wait: Duration,
    ) -> ResultType<()> {
        let (addr, data_size) = checked_input_addr(item.buffer, data.len())?;
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), addr, data.len());
        }
        let attr = OH_AVCodecBufferAttr {
            pts: self.next_pts(),
            size: data_size,
            offset: 0,
            flags: if key {
                AVCODEC_BUFFER_FLAGS_SYNC_FRAME
            } else {
                0
            },
        };
        if let Some(state) = self.callback_state.as_ref() {
            lock_media_state(&state.input_pts_at).insert(
                attr.pts,
                SurfaceInputTrace {
                    submitted_at: Instant::now(),
                    input_wait,
                },
            );
            if state.input_callback_count.load(Ordering::Relaxed) <= 3 {
                hilog_info(&format!(
                    "OHOS surface input submit index={} pts={} size={}",
                    item.index, attr.pts, attr.size
                ));
            }
        } else {
            self.input_pts_at.insert(attr.pts, Instant::now());
        }
        ensure_ok(
            unsafe { OH_AVBuffer_SetBufferAttr(item.buffer, &attr) },
            "SetBufferAttr",
        )?;
        ensure_ok(
            unsafe { OH_VideoDecoder_PushInputBuffer(self.codec, item.index) },
            "PushInputBuffer",
        )?;
        Ok(())
    }

    fn next_pts(&mut self) -> i64 {
        let elapsed = self.pts_origin.elapsed().as_micros().min(i64::MAX as u128) as i64;
        let pts = elapsed.max(self.last_pts.saturating_add(1));
        self.last_pts = pts;
        pts
    }

    fn query_output(&self, timeout: Duration) -> ResultType<Option<BufferItem>> {
        let mut stream_changed_retries = 0u32;
        loop {
            let mut index = 0u32;
            let ret = unsafe {
                OH_VideoDecoder_QueryOutputBuffer(
                    self.codec,
                    &mut index,
                    timeout.as_micros() as i64,
                )
            };
            match ret {
                AV_ERR_OK => {
                    let buffer = unsafe { OH_VideoDecoder_GetOutputBuffer(self.codec, index) };
                    if buffer.is_null() {
                        bail!("OHOS decoder output buffer is null")
                    }
                    return Ok(Some(BufferItem {
                        index,
                        buffer,
                        queued_at: Instant::now(),
                    }));
                }
                AV_ERR_STREAM_CHANGED => {
                    let desc = unsafe { OH_VideoDecoder_GetOutputDescription(self.codec) };
                    if !desc.is_null() {
                        unsafe { OH_AVFormat_Destroy(desc) };
                    }
                    stream_changed_retries += 1;
                    if stream_changed_retries > MAX_STREAM_CHANGED_RETRIES {
                        return Ok(None);
                    }
                    continue;
                }
                AV_ERR_INVALID_STATE | AV_ERR_TRY_AGAIN_LATER => return Ok(None),
                _ => bail!("OHOS decoder QueryOutputBuffer failed: {}", ret),
            }
        }
    }
}

pub fn apply_supported_decodings(decoding: &mut SupportedDecoding) {
    // RustDesk's upstream software-decoder capabilities stay untouched here.
    // AVCodec discovery only contributes the two platform decoder abilities.
    decoding.ability_h264 = if supports_decoder(CodecFormat::H264) {
        1
    } else {
        0
    };
    decoding.ability_h265 = if supports_decoder(CodecFormat::H265) {
        1
    } else {
        0
    };
}

pub fn new_h26x_decoder(
    format: CodecFormat,
    target: DirectRenderTarget,
) -> ResultType<OhosVideoDecoder> {
    let (width, height) = target.decode_size.unwrap_or((64, 64));
    let render_context = target
        .surface_id
        .and_then(super::direct_render::take_render_context);
    let (session_id, display) = render_context
        .map(|context| (Some(context.session_id), context.display))
        .unwrap_or((None, 0));
    match format {
        CodecFormat::H264 => OhosVideoDecoder::new_with_surface(
            h264_mime(),
            width,
            height,
            target.surface_id,
            session_id,
            display,
        ),
        CodecFormat::H265 => OhosVideoDecoder::new_with_surface(
            h265_mime(),
            width,
            height,
            target.surface_id,
            session_id,
            display,
        ),
        _ => bail!("unsupported OHOS H26x decoder format: {format:?}"),
    }
}

pub fn handle_h26x_video_frames(
    decoder: &mut OhosVideoDecoder,
    frames: &EncodedVideoFrames,
    rgb: &mut ImageRgb,
    pixelbuffer: &mut bool,
) -> ResultType<bool> {
    if decoder.is_surface_mode() {
        *pixelbuffer = false;
        for frame in frames.frames.iter() {
            if !decoder.submit_to_surface_with_key(&frame.data, frame.key)? {
                return Ok(false);
            }
        }
        return Ok(decoder.take_surface_rendered_frame());
    }
    let mut last_frame = OhosImage::empty();
    for frame in frames.frames.iter() {
        for image in decoder.decode_with_key(&frame.data, frame.key)? {
            drop(last_frame);
            last_frame = image;
        }
    }
    if last_frame.is_null() {
        Ok(false)
    } else {
        last_frame.to(rgb);
        Ok(true)
    }
}

pub fn supports_decoder(format: CodecFormat) -> bool {
    if let Some((_, available)) = lock_media_state(&DECODER_SUPPORT_CACHE)
        .iter()
        .find(|(cached_format, _)| *cached_format == format)
    {
        return *available;
    }
    let available = probe_decoder_support(format);
    lock_media_state(&DECODER_SUPPORT_CACHE).push((format, available));
    available
}

fn probe_decoder_support(format: CodecFormat) -> bool {
    let mime = match format {
        CodecFormat::H264 => h264_mime(),
        CodecFormat::H265 => h265_mime(),
        _ => return false,
    };
    // OH_AVCodec_GetCapability uses the same recommendation strategy as
    // CreateByMime. Negotiation must query capability only; creating a decoder
    // here would consume a native instance before a session has a Surface.
    let available = unsafe { has_decoder_capability(mime) };
    hilog_info(&format!(
        "OHOS supports_decoder {:?}: capability={}",
        format, available
    ));
    available
}

fn h264_mime() -> *const c_char {
    unsafe { mime_or_fallback(OH_AVCODEC_MIMETYPE_VIDEO_AVC, b"video/avc\0") }
}

fn h265_mime() -> *const c_char {
    unsafe { mime_or_fallback(OH_AVCODEC_MIMETYPE_VIDEO_HEVC, b"video/hevc\0") }
}

unsafe fn mime_or_fallback(value: *const c_char, fallback: &'static [u8]) -> *const c_char {
    if value.is_null() {
        return fallback.as_ptr().cast();
    }
    let cstr = CStr::from_ptr(value);
    if cstr.to_bytes().is_empty() {
        fallback.as_ptr().cast()
    } else {
        value
    }
}

unsafe fn has_decoder_capability(mime: *const c_char) -> bool {
    !OH_AVCodec_GetCapability(mime, false).is_null()
}

unsafe fn log_video_size_support(mime: *const c_char, width: i32, height: i32) {
    let capability = OH_AVCodec_GetCapability(mime, false);
    if capability.is_null() {
        return;
    }
    let size_supported = OH_AVCapability_IsVideoSizeSupported(capability, width, height);
    let mut width_range = OH_AVRange::default();
    let mut height_range = OH_AVRange::default();
    let width_range_ret = OH_AVCapability_GetVideoWidthRange(capability, &mut width_range);
    let height_range_ret = OH_AVCapability_GetVideoHeightRange(capability, &mut height_range);
    let mime_name = CStr::from_ptr(mime).to_string_lossy();
    let level = if size_supported { LOG_INFO } else { LOG_WARN };
    hilog_print(
        level,
        &format!(
            "OHOS decoder capability size mime={} requested={}x{} size_supported={} width_range_ret={} width_range={}..{} height_range_ret={} height_range={}..{} (advisory only, not gating)",
            mime_name,
            width,
            height,
            size_supported,
            width_range_ret,
            width_range.min_val,
            width_range.max_val,
            height_range_ret,
            height_range.min_val,
            height_range.max_val
        ),
    );
}

unsafe fn create_decoder(mime: *const c_char) -> *mut OH_AVCodec {
    let mime_name = CStr::from_ptr(mime).to_string_lossy().into_owned();
    let codec = OH_VideoDecoder_CreateByMime(mime);
    if codec.is_null() {
        hilog_error(&format!(
            "OHOS decoder CreateByMime failed for {}",
            mime_name
        ));
    } else {
        hilog_info(&format!("OHOS decoder created by mime for {}", mime_name));
    }
    codec
}

fn hilog_print(level: i32, message: &str) {
    if let Ok(c_message) = CString::new(message) {
        unsafe {
            let _ = OH_LOG_PrintMsg(
                LOG_APP,
                level,
                HILOG_DOMAIN,
                HILOG_TAG.as_ptr().cast(),
                c_message.as_ptr(),
            );
        }
    }
}

fn hilog_info(message: &str) {
    hilog_print(LOG_INFO, message);
}

fn hilog_debug(message: &str) {
    hilog_print(LOG_DEBUG, message);
}

fn hilog_warn(message: &str) {
    hilog_print(LOG_WARN, message);
}

fn hilog_error(message: &str) {
    hilog_print(LOG_ERROR, message);
}

pub fn last_decoder_init_error() -> String {
    lock_media_state(&LAST_DECODER_INIT_ERROR).clone()
}

pub struct OhosDecodeFrames<'a> {
    inner: std::vec::Drain<'a, OhosImage>,
}

impl<'a> Iterator for OhosDecodeFrames<'a> {
    type Item = OhosImage;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct OhosImage {
    raw: Vec<u8>,
    width: usize,
    height: usize,
    stride: [i32; 3],
    offsets: [usize; 3],
}

impl OhosImage {
    pub fn empty() -> Self {
        Self {
            raw: Vec::new(),
            width: 0,
            height: 0,
            stride: [0, 0, 0],
            offsets: [0, 0, 0],
        }
    }

    pub fn is_null(&self) -> bool {
        self.raw.is_empty()
    }
}

impl GoogleImage for OhosImage {
    fn width(&self) -> usize {
        self.width
    }
    fn height(&self) -> usize {
        self.height
    }
    fn stride(&self) -> Vec<i32> {
        self.stride.to_vec()
    }
    fn planes(&self) -> Vec<*mut u8> {
        let ptr = self.raw.as_ptr() as *mut u8;
        vec![
            unsafe { ptr.add(self.offsets[0]) },
            unsafe { ptr.add(self.offsets[1]) },
            unsafe { ptr.add(self.offsets[2]) },
        ]
    }
    fn chroma(&self) -> Chroma {
        Chroma::I420
    }
}

fn ensure_ok(code: i32, label: &str) -> ResultType<()> {
    if code == AV_ERR_OK {
        Ok(())
    } else {
        bail!("OHOS decoder {} failed: {}", label, code)
    }
}

fn output_buffer_attr(buffer: *mut OH_AVBuffer) -> ResultType<OH_AVCodecBufferAttr> {
    let mut attr = OH_AVCodecBufferAttr::default();
    ensure_ok(
        unsafe { OH_AVBuffer_GetBufferAttr(buffer, &mut attr) },
        "GetBufferAttr",
    )?;
    Ok(attr)
}

fn create_native_window(
    surface_id: u64,
    _width: usize,
    _height: usize,
) -> ResultType<*mut OHNativeWindow> {
    let mut window = ptr::null_mut();
    ensure_ok(
        unsafe { OH_NativeWindow_CreateNativeWindowFromSurfaceId(surface_id, &mut window) },
        "CreateNativeWindowFromSurfaceId",
    )?;
    if window.is_null() {
        bail!("OHOS NativeWindow is null for surface {}", surface_id)
    }
    let _ = unsafe {
        OH_NativeWindow_NativeWindowSetScalingModeV2(window, OH_SCALING_MODE_SCALE_FIT_V2)
    };
    Ok(window)
}

fn output_format(codec: *mut OH_AVCodec, buffer: *mut OH_AVBuffer) -> ResultType<FormatInfo> {
    let format = unsafe { OH_VideoDecoder_GetOutputDescription(codec) };
    if format.is_null() {
        let format = unsafe { OH_AVBuffer_GetParameter(buffer) };
        if format.is_null() {
            bail!("OHOS decoder output format is null")
        }
        let result = output_format_from_avformat(format);
        unsafe { OH_AVFormat_Destroy(format) };
        return result;
    }
    let result = output_format_from_avformat(format);
    unsafe { OH_AVFormat_Destroy(format) };
    result
}

fn output_format_from_avformat(format: *mut OH_AVFormat) -> ResultType<FormatInfo> {
    let mut info = FormatInfo::default();
    unsafe {
        info.width = checked_format_dimension(
            "width",
            get_format_i32(format, OH_MD_KEY_VIDEO_PIC_WIDTH)
                .or_else(|| get_format_i32(format, OH_MD_KEY_WIDTH)),
        )?;
        info.height = checked_format_dimension(
            "height",
            get_format_i32(format, OH_MD_KEY_VIDEO_PIC_HEIGHT)
                .or_else(|| get_format_i32(format, OH_MD_KEY_HEIGHT)),
        )?;
        info.pixel_format =
            get_format_i32(format, OH_MD_KEY_PIXEL_FORMAT).unwrap_or(AV_PIXEL_FORMAT_YUVI420);
        info.stride = checked_format_layout_value(
            "stride",
            get_format_i32(format, OH_MD_KEY_VIDEO_STRIDE)
                .or_else(|| get_format_i32(format, OH_MD_KEY_WIDTH)),
        )?;
        info.slice_height = checked_format_layout_value(
            "slice height",
            get_format_i32(format, OH_MD_KEY_VIDEO_SLICE_HEIGHT)
                .or_else(|| get_format_i32(format, OH_MD_KEY_HEIGHT)),
        )?;
        info.range_flag = get_format_i32(format, OH_MD_KEY_RANGE_FLAG);
        info.color_primaries = get_format_i32(format, OH_MD_KEY_COLOR_PRIMARIES);
        info.transfer_characteristics = get_format_i32(format, OH_MD_KEY_TRANSFER_CHARACTERISTICS);
        info.matrix_coefficients = get_format_i32(format, OH_MD_KEY_MATRIX_COEFFICIENTS);
    }
    Ok(info)
}

fn checked_format_dimension(label: &str, value: Option<i32>) -> ResultType<usize> {
    let value = value.unwrap_or_default();
    if value <= 0 {
        bail!("OHOS decoder returned invalid output {} {}", label, value)
    }
    usize::try_from(value)
        .map_err(|_| anyhow!("OHOS decoder output {} is out of range: {}", label, value).into())
}

fn checked_format_layout_value(label: &str, value: Option<i32>) -> ResultType<usize> {
    let value = value.unwrap_or_default();
    if value < 0 {
        bail!("OHOS decoder returned invalid output {} {}", label, value)
    }
    usize::try_from(value)
        .map_err(|_| anyhow!("OHOS decoder output {} is out of range: {}", label, value).into())
}

fn get_format_i32(format: *mut OH_AVFormat, key: *const c_char) -> Option<i32> {
    if key.is_null() {
        return None;
    }
    let mut out = 0i32;
    let ok = unsafe { OH_AVFormat_GetIntValue(format, key, &mut out) };
    ok.then_some(out)
}

fn checked_input_addr(buffer: *mut OH_AVBuffer, data_len: usize) -> ResultType<(*mut u8, i32)> {
    let data_size = i32::try_from(data_len)
        .map_err(|_| anyhow!("OHOS decoder input is too large: {} bytes", data_len))?;
    let capacity = unsafe { OH_AVBuffer_GetCapacity(buffer) };
    if capacity < 0 {
        bail!("failed to query OHOS decoder input buffer capacity")
    }
    if capacity < data_size {
        bail!(
            "OHOS decoder input buffer too small: {} < {}",
            capacity,
            data_len
        )
    }
    let addr = unsafe { OH_AVBuffer_GetAddr(buffer) };
    if addr.is_null() {
        bail!("OHOS decoder input buffer addr is null")
    }
    Ok((addr, data_size))
}

fn copy_output_image(buffer: *mut OH_AVBuffer, format: FormatInfo) -> ResultType<OhosImage> {
    let capacity = unsafe { OH_AVBuffer_GetCapacity(buffer) };
    if capacity < 0 {
        bail!("failed to query OHOS decoder output buffer capacity")
    }
    let capacity = usize::try_from(capacity)
        .map_err(|_| anyhow!("OHOS decoder output capacity is out of range"))?;
    let addr = unsafe { OH_AVBuffer_GetAddr(buffer) };
    if !addr.is_null() {
        let required = linear_buffer_required_size(format)?;
        if required > capacity {
            bail!(
                "OHOS decoder output buffer too small: {} < {}",
                capacity,
                required
            )
        }
        return copy_from_linear_buffer(addr, capacity, format);
    }

    let native_buffer = unsafe { OH_AVBuffer_GetNativeBuffer(buffer) };
    if !native_buffer.is_null() {
        let image = copy_from_native_buffer(native_buffer, capacity, format);
        unsafe {
            let _ = OH_NativeBuffer_Unreference(native_buffer);
        }
        return image;
    }

    bail!("OHOS decoder output has neither linear addr nor native buffer")
}

fn linear_buffer_required_size(format: FormatInfo) -> ResultType<usize> {
    let y_stride = format.stride.max(format.width);
    let slice_height = format.slice_height.max(format.height);
    let y_end = checked_plane_end(0, y_stride, format.width, format.height)?;
    let y_storage = y_stride
        .checked_mul(slice_height)
        .ok_or_else(|| anyhow!("OHOS decoder Y plane size overflow"))?;
    let uv_width = format.width.div_ceil(2);
    let uv_height = format.height.div_ceil(2);

    match format.pixel_format {
        AV_PIXEL_FORMAT_YUVI420 => {
            let uv_stride = y_stride.div_ceil(2);
            let uv_storage_height = slice_height.div_ceil(2);
            let uv_storage = uv_stride
                .checked_mul(uv_storage_height)
                .ok_or_else(|| anyhow!("OHOS decoder UV plane size overflow"))?;
            let u_end = checked_plane_end(y_storage, uv_stride, uv_width, uv_height)?;
            let v_offset = y_storage
                .checked_add(uv_storage)
                .ok_or_else(|| anyhow!("OHOS decoder V plane offset overflow"))?;
            let v_end = checked_plane_end(v_offset, uv_stride, uv_width, uv_height)?;
            Ok(y_end.max(u_end).max(v_end))
        }
        AV_PIXEL_FORMAT_NV12 | AV_PIXEL_FORMAT_NV21 => {
            let uv_row_width = uv_width
                .checked_mul(2)
                .ok_or_else(|| anyhow!("OHOS decoder UV row size overflow"))?;
            let uv_end = checked_plane_end(y_storage, y_stride, uv_row_width, uv_height)?;
            Ok(y_end.max(uv_end))
        }
        _ => bail!(
            "unsupported OHOS linear buffer pixel format {}",
            format.pixel_format
        ),
    }
}

fn checked_plane_end(
    offset: usize,
    stride: usize,
    width: usize,
    height: usize,
) -> ResultType<usize> {
    if height == 0 {
        return Ok(offset);
    }
    let last_row = (height - 1)
        .checked_mul(stride)
        .ok_or_else(|| anyhow!("OHOS decoder plane row offset overflow"))?;
    offset
        .checked_add(last_row)
        .and_then(|value| value.checked_add(width))
        .ok_or_else(|| anyhow!("OHOS decoder plane end overflow").into())
}

fn copy_from_native_buffer(
    buffer: *mut OH_NativeBuffer,
    capacity: usize,
    format: FormatInfo,
) -> ResultType<OhosImage> {
    let mut vir_addr: *mut c_void = ptr::null_mut();
    let mut planes = OH_NativeBuffer_Planes::default();
    ensure_ok(
        unsafe { OH_NativeBuffer_MapPlanes(buffer, &mut vir_addr, &mut planes) },
        "MapPlanes",
    )?;
    let result = if vir_addr.is_null() {
        Err(anyhow!("OHOS NativeBuffer MapPlanes returned a null address").into())
    } else if planes.plane_count as usize > planes.planes.len() {
        Err(anyhow!(
            "OHOS NativeBuffer returned invalid plane count {}",
            planes.plane_count
        )
        .into())
    } else {
        copy_from_planes(vir_addr.cast::<u8>(), capacity, &planes, format)
    };
    let unmap_result = ensure_ok(unsafe { OH_NativeBuffer_Unmap(buffer) }, "Unmap");
    match (result, unmap_result) {
        (Ok(image), Ok(())) => Ok(image),
        (Err(err), Ok(())) => Err(err),
        (Ok(_), Err(err)) => Err(err),
        (Err(err), Err(unmap_err)) => {
            hilog_warn(&format!(
                "OHOS NativeBuffer unmap failed after copy error: {unmap_err}"
            ));
            Err(err)
        }
    }
}

fn copy_from_planes(
    base: *mut u8,
    capacity: usize,
    planes: &OH_NativeBuffer_Planes,
    format: FormatInfo,
) -> ResultType<OhosImage> {
    match format.pixel_format {
        AV_PIXEL_FORMAT_YUVI420 => {
            copy_i420_planes(base, capacity, planes, format.width, format.height)
        }
        AV_PIXEL_FORMAT_NV12 => {
            copy_nv12_planes(base, capacity, planes, format.width, format.height, false)
        }
        AV_PIXEL_FORMAT_NV21 => {
            copy_nv12_planes(base, capacity, planes, format.width, format.height, true)
        }
        _ => bail!(
            "unsupported OHOS native buffer pixel format {}",
            format.pixel_format
        ),
    }
}

fn copy_from_linear_buffer(
    addr: *mut u8,
    capacity: usize,
    format: FormatInfo,
) -> ResultType<OhosImage> {
    match format.pixel_format {
        AV_PIXEL_FORMAT_YUVI420 => copy_i420_linear(addr, capacity, format),
        AV_PIXEL_FORMAT_NV12 => copy_nv12_linear(addr, capacity, format, false),
        AV_PIXEL_FORMAT_NV21 => copy_nv12_linear(addr, capacity, format, true),
        _ => bail!(
            "unsupported OHOS linear buffer pixel format {}",
            format.pixel_format
        ),
    }
}

fn make_i420_image(
    width: usize,
    height: usize,
    y_stride: usize,
    uv_stride: usize,
) -> ResultType<OhosImage> {
    let uv_width = width.div_ceil(2);
    if y_stride < width || uv_stride < uv_width {
        bail!(
            "OHOS decoder destination stride is too small: y={} uv={} width={}",
            y_stride,
            uv_stride,
            width
        )
    }
    let y_len = y_stride
        .checked_mul(height)
        .ok_or_else(|| anyhow!("OHOS decoder destination Y plane size overflow"))?;
    let uv_height = height.div_ceil(2);
    let uv_len = uv_stride
        .checked_mul(uv_height)
        .ok_or_else(|| anyhow!("OHOS decoder destination UV plane size overflow"))?;
    let v_offset = y_len
        .checked_add(uv_len)
        .ok_or_else(|| anyhow!("OHOS decoder destination V plane offset overflow"))?;
    let total_len = uv_len
        .checked_mul(2)
        .and_then(|value| y_len.checked_add(value))
        .ok_or_else(|| anyhow!("OHOS decoder destination image size overflow"))?;
    let y_stride_i32 = i32::try_from(y_stride)
        .map_err(|_| anyhow!("OHOS decoder destination Y stride is out of range"))?;
    let uv_stride_i32 = i32::try_from(uv_stride)
        .map_err(|_| anyhow!("OHOS decoder destination UV stride is out of range"))?;
    Ok(OhosImage {
        raw: vec![0u8; total_len],
        width,
        height,
        stride: [y_stride_i32, uv_stride_i32, uv_stride_i32],
        offsets: [0, y_len, v_offset],
    })
}

fn copy_i420_planes(
    base: *mut u8,
    capacity: usize,
    planes: &OH_NativeBuffer_Planes,
    width: usize,
    height: usize,
) -> ResultType<OhosImage> {
    if planes.plane_count < 3 {
        bail!("OHOS I420 output has only {} planes", planes.plane_count)
    }
    let y_stride = checked_u32_layout("Y row stride", planes.planes[0].row_stride)?;
    let uv_stride = checked_u32_layout("U row stride", planes.planes[1].row_stride)?;
    let mut image = make_i420_image(
        width,
        height,
        y_stride.max(width),
        uv_stride.max(width.div_ceil(2)),
    )?;
    let y_dst_offset = image.offsets[0];
    let y_dst_stride = image.stride[0] as usize;
    copy_plane(
        base,
        capacity,
        checked_u64_layout("Y plane offset", planes.planes[0].offset)?,
        y_stride,
        checked_u32_layout("Y column stride", planes.planes[0].column_stride)?,
        &mut image.raw,
        y_dst_offset,
        y_dst_stride,
        width,
        height,
    )?;
    let uv_width = width.div_ceil(2);
    let uv_height = height.div_ceil(2);
    let u_dst_offset = image.offsets[1];
    let u_dst_stride = image.stride[1] as usize;
    copy_plane(
        base,
        capacity,
        checked_u64_layout("U plane offset", planes.planes[1].offset)?,
        uv_stride,
        checked_u32_layout("U column stride", planes.planes[1].column_stride)?,
        &mut image.raw,
        u_dst_offset,
        u_dst_stride,
        uv_width,
        uv_height,
    )?;
    let v_stride = checked_u32_layout("V row stride", planes.planes[2].row_stride)?;
    let v_dst_offset = image.offsets[2];
    let v_dst_stride = image.stride[2] as usize;
    copy_plane(
        base,
        capacity,
        checked_u64_layout("V plane offset", planes.planes[2].offset)?,
        v_stride,
        checked_u32_layout("V column stride", planes.planes[2].column_stride)?,
        &mut image.raw,
        v_dst_offset,
        v_dst_stride,
        uv_width,
        uv_height,
    )?;
    Ok(image)
}

fn copy_nv12_planes(
    base: *mut u8,
    capacity: usize,
    planes: &OH_NativeBuffer_Planes,
    width: usize,
    height: usize,
    swap_uv: bool,
) -> ResultType<OhosImage> {
    if planes.plane_count < 2 {
        bail!(
            "OHOS NV12/NV21 output has only {} planes",
            planes.plane_count
        )
    }
    let y_stride = checked_u32_layout("Y row stride", planes.planes[0].row_stride)?;
    let uv_stride = width.div_ceil(2);
    let mut image = make_i420_image(width, height, y_stride.max(width), uv_stride)?;
    let y_dst_offset = image.offsets[0];
    let y_dst_stride = image.stride[0] as usize;
    copy_plane(
        base,
        capacity,
        checked_u64_layout("Y plane offset", planes.planes[0].offset)?,
        y_stride,
        checked_u32_layout("Y column stride", planes.planes[0].column_stride)?,
        &mut image.raw,
        y_dst_offset,
        y_dst_stride,
        width,
        height,
    )?;
    let uv_width = width.div_ceil(2);
    let uv_height = height.div_ceil(2);
    let plane = planes.planes[1];
    let plane_offset = checked_u64_layout("UV plane offset", plane.offset)?;
    let plane_stride = checked_u32_layout("UV row stride", plane.row_stride)?;
    let column_stride = checked_u32_layout("UV column stride", plane.column_stride)?;
    if column_stride < 2 {
        bail!(
            "OHOS decoder UV column stride is too small: {}",
            column_stride
        )
    }
    let source_end = checked_strided_plane_end(
        plane_offset,
        plane_stride,
        column_stride,
        uv_width,
        uv_height,
        2,
    )?;
    if source_end > capacity {
        bail!(
            "OHOS UV plane exceeds mapped buffer: {} > {}",
            source_end,
            capacity
        )
    }
    for row in 0..uv_height {
        for col in 0..uv_width {
            let offset = plane_offset + row * plane_stride + col * column_stride;
            unsafe {
                let first = *base.add(offset);
                let second = *base.add(offset + 1);
                let (u, v) = if swap_uv {
                    (second, first)
                } else {
                    (first, second)
                };
                image.raw[image.offsets[1] + row * image.stride[1] as usize + col] = u;
                image.raw[image.offsets[2] + row * image.stride[2] as usize + col] = v;
            }
        }
    }
    Ok(image)
}

fn copy_i420_linear(addr: *mut u8, capacity: usize, format: FormatInfo) -> ResultType<OhosImage> {
    let y_stride = format.stride.max(format.width);
    let slice_height = format.slice_height.max(format.height);
    let uv_stride = y_stride.div_ceil(2);
    let mut image = make_i420_image(format.width, format.height, y_stride, uv_stride)?;
    let y_dst_offset = image.offsets[0];
    let y_dst_stride = image.stride[0] as usize;
    copy_plane(
        addr,
        capacity,
        0,
        y_stride,
        1,
        &mut image.raw,
        y_dst_offset,
        y_dst_stride,
        format.width,
        format.height,
    )?;
    let uv_width = format.width.div_ceil(2);
    let uv_height = format.height.div_ceil(2);
    let u_offset = y_stride
        .checked_mul(slice_height)
        .ok_or_else(|| anyhow!("OHOS decoder U plane offset overflow"))?;
    let v_offset = uv_stride
        .checked_mul(slice_height.div_ceil(2))
        .and_then(|value| u_offset.checked_add(value))
        .ok_or_else(|| anyhow!("OHOS decoder V plane offset overflow"))?;
    let u_dst_offset = image.offsets[1];
    let u_dst_stride = image.stride[1] as usize;
    copy_plane(
        addr,
        capacity,
        u_offset,
        uv_stride,
        1,
        &mut image.raw,
        u_dst_offset,
        u_dst_stride,
        uv_width,
        uv_height,
    )?;
    let v_dst_offset = image.offsets[2];
    let v_dst_stride = image.stride[2] as usize;
    copy_plane(
        addr,
        capacity,
        v_offset,
        uv_stride,
        1,
        &mut image.raw,
        v_dst_offset,
        v_dst_stride,
        uv_width,
        uv_height,
    )?;
    Ok(image)
}

fn copy_nv12_linear(
    addr: *mut u8,
    capacity: usize,
    format: FormatInfo,
    swap_uv: bool,
) -> ResultType<OhosImage> {
    let y_stride = format.stride.max(format.width);
    let slice_height = format.slice_height.max(format.height);
    let uv_stride = format.width.div_ceil(2);
    let mut image = make_i420_image(format.width, format.height, y_stride, uv_stride)?;
    let y_dst_offset = image.offsets[0];
    let y_dst_stride = image.stride[0] as usize;
    copy_plane(
        addr,
        capacity,
        0,
        y_stride,
        1,
        &mut image.raw,
        y_dst_offset,
        y_dst_stride,
        format.width,
        format.height,
    )?;
    let uv_offset = y_stride
        .checked_mul(slice_height)
        .ok_or_else(|| anyhow!("OHOS decoder UV plane offset overflow"))?;
    let uv_width = format.width.div_ceil(2);
    let uv_height = format.height.div_ceil(2);
    let source_end = checked_strided_plane_end(uv_offset, y_stride, 2, uv_width, uv_height, 2)?;
    if source_end > capacity {
        bail!(
            "OHOS linear UV plane exceeds buffer: {} > {}",
            source_end,
            capacity
        )
    }
    for row in 0..uv_height {
        for col in 0..uv_width {
            let offset = uv_offset + row * y_stride + col * 2;
            unsafe {
                let first = *addr.add(offset);
                let second = *addr.add(offset + 1);
                let (u, v) = if swap_uv {
                    (second, first)
                } else {
                    (first, second)
                };
                image.raw[image.offsets[1] + row * image.stride[1] as usize + col] = u;
                image.raw[image.offsets[2] + row * image.stride[2] as usize + col] = v;
            }
        }
    }
    Ok(image)
}

fn checked_u64_layout(label: &str, value: u64) -> ResultType<usize> {
    usize::try_from(value)
        .map_err(|_| anyhow!("OHOS decoder {} is out of range: {}", label, value).into())
}

fn checked_u32_layout(label: &str, value: u32) -> ResultType<usize> {
    let value = usize::try_from(value)
        .map_err(|_| anyhow!("OHOS decoder {} is out of range: {}", label, value))?;
    if value == 0 {
        bail!("OHOS decoder {} must be positive", label)
    }
    Ok(value)
}

fn checked_strided_plane_end(
    offset: usize,
    row_stride: usize,
    column_stride: usize,
    width: usize,
    height: usize,
    sample_size: usize,
) -> ResultType<usize> {
    if width == 0 || height == 0 {
        return Ok(offset);
    }
    let row_width = (width - 1)
        .checked_mul(column_stride)
        .and_then(|value| value.checked_add(sample_size))
        .ok_or_else(|| anyhow!("OHOS decoder plane row size overflow"))?;
    if row_stride < row_width {
        bail!(
            "OHOS decoder plane row stride is too small: {} < {}",
            row_stride,
            row_width
        )
    }
    (height - 1)
        .checked_mul(row_stride)
        .and_then(|value| offset.checked_add(value))
        .and_then(|value| value.checked_add(row_width))
        .ok_or_else(|| anyhow!("OHOS decoder plane end overflow").into())
}

fn copy_plane(
    src_base: *mut u8,
    src_capacity: usize,
    src_offset: usize,
    src_stride: usize,
    src_column_stride: usize,
    dst: &mut [u8],
    dst_offset: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
) -> ResultType<()> {
    if src_base.is_null() {
        bail!("OHOS decoder plane base address is null")
    }
    let src_end =
        checked_strided_plane_end(src_offset, src_stride, src_column_stride, width, height, 1)?;
    if src_end > src_capacity {
        bail!(
            "OHOS decoder plane exceeds source buffer: {} > {}",
            src_end,
            src_capacity
        )
    }
    let dst_end = checked_plane_end(dst_offset, dst_stride, width, height)?;
    if dst_end > dst.len() {
        bail!(
            "OHOS decoder plane exceeds destination buffer: {} > {}",
            dst_end,
            dst.len()
        )
    }
    for row in 0..height {
        let src_row = src_offset + row * src_stride;
        let dst_row = dst_offset + row * dst_stride;
        if src_column_stride == 1 {
            unsafe {
                ptr::copy_nonoverlapping(
                    src_base.add(src_row),
                    dst.as_mut_ptr().add(dst_row),
                    width,
                );
            }
        } else {
            for col in 0..width {
                dst[dst_row + col] = unsafe { *src_base.add(src_row + col * src_column_stride) };
            }
        }
    }
    Ok(())
}
