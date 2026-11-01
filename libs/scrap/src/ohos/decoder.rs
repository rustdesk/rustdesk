#![allow(non_camel_case_types, non_snake_case, dead_code)]

use crate::{
    ohos::direct_render, CodecFormat, I420ToABGR, I420ToARGB, ImageFormat, ImageRgb,
};
use hbb_common::{bail, log, ResultType};
use std::{
    collections::VecDeque,
    convert::TryFrom,
    os::raw::{c_char, c_int, c_uchar, c_uint, c_void},
    ptr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Condvar, Mutex, MutexGuard,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const AV_ERR_OK: c_int = 0;
const AVCODEC_BUFFER_FLAGS_NONE: c_uint = 0;
const AVCODEC_BUFFER_FLAGS_SYNC_FRAME: c_uint = 1 << 1;
const AV_PIXEL_FORMAT_I420: c_int = 1;
const AV_PIXEL_FORMAT_NV12: c_int = 2;
const AV_PIXEL_FORMAT_NV21: c_int = 3;
const BUFFER_WAIT: Duration = Duration::from_millis(100);
const SURFACE_STALL: Duration = Duration::from_secs(3);

#[repr(C)]
struct OH_AVCodecBufferAttr {
    pts: i64,
    size: i32,
    offset: i32,
    flags: c_uint,
}

#[repr(C)]
struct OH_AVCodecCallback {
    onError: Option<unsafe extern "C" fn(*mut c_void, c_int, *mut c_void)>,
    onStreamChanged: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void)>,
    onNeedInputBuffer:
        Option<unsafe extern "C" fn(*mut c_void, c_uint, *mut c_void, *mut c_void)>,
    onNewOutputBuffer:
        Option<unsafe extern "C" fn(*mut c_void, c_uint, *mut c_void, *mut c_void)>,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NativeBufferPlane {
    offset: u64,
    row_stride: u32,
    column_stride: u32,
}

#[repr(C)]
#[derive(Default)]
struct NativeBufferPlanes {
    plane_count: u32,
    planes: [NativeBufferPlane; 4],
}

extern "C" {
    fn OH_VideoDecoder_CreateByMime(mime: *const c_char) -> *mut c_void;
    fn OH_VideoDecoder_Destroy(codec: *mut c_void) -> c_int;
    fn OH_VideoDecoder_RegisterCallback(
        codec: *mut c_void,
        callback: OH_AVCodecCallback,
        userData: *mut c_void,
    ) -> c_int;
    fn OH_VideoDecoder_Configure(codec: *mut c_void, format: *mut c_void) -> c_int;
    fn OH_VideoDecoder_SetSurface(codec: *mut c_void, window: *mut c_void) -> c_int;
    fn OH_VideoDecoder_Prepare(codec: *mut c_void) -> c_int;
    fn OH_VideoDecoder_Start(codec: *mut c_void) -> c_int;
    fn OH_VideoDecoder_Stop(codec: *mut c_void) -> c_int;
    fn OH_VideoDecoder_PushInputBuffer(codec: *mut c_void, index: c_uint) -> c_int;
    fn OH_VideoDecoder_FreeOutputBuffer(codec: *mut c_void, index: c_uint) -> c_int;
    fn OH_VideoDecoder_RenderOutputBuffer(codec: *mut c_void, index: c_uint) -> c_int;
    fn OH_AVFormat_CreateVideoFormat(mime: *const c_char, width: i32, height: i32) -> *mut c_void;
    fn OH_AVFormat_Destroy(format: *mut c_void);
    fn OH_AVFormat_SetIntValue(format: *mut c_void, key: *const c_char, value: i32) -> bool;
    fn OH_AVFormat_GetIntValue(format: *mut c_void, key: *const c_char, out: *mut i32) -> bool;
    fn OH_AVBuffer_GetAddr(buffer: *mut c_void) -> *mut c_uchar;
    fn OH_AVBuffer_GetCapacity(buffer: *mut c_void) -> c_int;
    fn OH_AVBuffer_GetBufferAttr(buffer: *mut c_void, attr: *mut OH_AVCodecBufferAttr) -> c_int;
    fn OH_AVBuffer_SetBufferAttr(buffer: *mut c_void, attr: *const OH_AVCodecBufferAttr) -> c_int;
    fn OH_AVBuffer_GetNativeBuffer(buffer: *mut c_void) -> *mut c_void;
    fn OH_NativeBuffer_MapPlanes(
        buffer: *mut c_void,
        address: *mut *mut c_void,
        planes: *mut NativeBufferPlanes,
    ) -> c_int;
    fn OH_NativeBuffer_Unmap(buffer: *mut c_void) -> c_int;
    fn OH_NativeBuffer_Unreference(buffer: *mut c_void) -> c_int;
    fn OH_NativeWindow_CreateNativeWindowFromSurfaceId(
        surface_id: u64,
        window: *mut *mut c_void,
    ) -> c_int;
    fn OH_NativeWindow_DestroyNativeWindow(window: *mut c_void);
    fn OH_NativeWindow_NativeWindowSetScalingModeV2(window: *mut c_void, mode: c_int) -> c_int;

    static OH_MD_KEY_WIDTH: c_char;
    static OH_MD_KEY_HEIGHT: c_char;
    static OH_MD_KEY_PIXEL_FORMAT: c_char;
    static OH_MD_KEY_VIDEO_STRIDE: c_char;
    static OH_MD_KEY_VIDEO_SLICE_HEIGHT: c_char;
}

#[derive(Clone, Copy)]
struct InputBuffer {
    index: u32,
    buffer: *mut c_void,
    addr: *mut u8,
    capacity: usize,
}

#[derive(Clone, Copy)]
struct OutputBuffer {
    index: u32,
    buffer: *mut c_void,
    format: OutputFormat,
}

#[derive(Clone, Copy, Default)]
struct OutputFormat {
    width: usize,
    height: usize,
    stride: usize,
    slice_height: usize,
    pixel_format: c_int,
}

#[derive(Default)]
struct Shared {
    free_inputs: VecDeque<InputBuffer>,
    outputs: VecDeque<OutputBuffer>,
    format: OutputFormat,
    error: Option<String>,
    running: bool,
}

struct DecoderInner {
    codec: *mut c_void,
    shared: Mutex<Shared>,
    input_ready: Condvar,
    output_ready: Condvar,
    rendered: AtomicU32,
}

unsafe impl Send for DecoderInner {}
unsafe impl Sync for DecoderInner {}

impl DecoderInner {
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn fail(&self, error: String) {
        let mut shared = self.lock();
        if shared.error.is_none() {
            shared.error = Some(error);
        }
        shared.running = false;
        self.input_ready.notify_all();
        self.output_ready.notify_all();
    }
}

struct Cleanup {
    codec: *mut c_void,
    window: *mut c_void,
    callback: *const DecoderInner,
    inner: Arc<DecoderInner>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        stop_callbacks(&self.inner);
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                log::error!("ohcodec output worker panicked");
            }
        }
        unsafe {
            if !self.codec.is_null() {
                let rc = OH_VideoDecoder_Stop(self.codec);
                if rc != AV_ERR_OK {
                    log::error!("OH_VideoDecoder_Stop failed during cleanup: {rc}");
                }
                let rc = OH_VideoDecoder_Destroy(self.codec);
                if rc != AV_ERR_OK {
                    log::error!("OH_VideoDecoder_Destroy failed during cleanup: {rc}");
                    return;
                }
            }
            if !self.callback.is_null() {
                drop(Arc::from_raw(self.callback));
            }
            if !self.window.is_null() {
                OH_NativeWindow_DestroyNativeWindow(self.window);
            }
        }
    }
}

pub struct OhCodecDecoder {
    mime: &'static str,
    cleanup: Cleanup,
    codec_config: Vec<u8>,
    surface_generation: Option<u64>,
    last_rendered: u32,
    pending_since: Option<Instant>,
    next_pts: i64,
}

unsafe impl Send for InputBuffer {}
unsafe impl Send for OutputBuffer {}

impl OhCodecDecoder {
    pub fn new(format: CodecFormat) -> Option<Self> {
        let mime = match format {
            CodecFormat::H264 => "video/avc\0",
            CodecFormat::H265 => "video/hevc\0",
            _ => return None,
        };
        let surface = direct_render::current_surface();
        match Self::create(mime, surface) {
            Ok(decoder) => Some(decoder),
            Err(error) if surface.is_some() => {
                log::warn!("ohcodec surface init failed, retrying buffer mode: {error}");
                Self::create(mime, None).map(|mut decoder| {
                    decoder.surface_generation = surface.map(|surface| surface.1);
                    decoder
                }).ok()
            }
            Err(error) => {
                log::error!("ohcodec decoder init failed: {error}");
                None
            }
        }
    }

    fn create(mime: &'static str, surface: Option<(u64, u64)>) -> ResultType<Self> {
        let codec = unsafe { OH_VideoDecoder_CreateByMime(mime.as_ptr() as _) };
        if codec.is_null() {
            bail!("OH_VideoDecoder_CreateByMime failed");
        }
        let inner = Arc::new(DecoderInner {
            codec,
            shared: Mutex::new(Shared {
                running: true,
                ..Default::default()
            }),
            input_ready: Condvar::new(),
            output_ready: Condvar::new(),
            rendered: AtomicU32::new(0),
        });
        let mut cleanup = Cleanup {
            codec,
            window: ptr::null_mut(),
            callback: ptr::null(),
            inner: inner.clone(),
            worker: None,
        };
        let callback = Arc::into_raw(inner.clone());
        cleanup.callback = callback;
        let callbacks = OH_AVCodecCallback {
            onError: Some(on_error),
            onStreamChanged: Some(on_stream_changed),
            onNeedInputBuffer: Some(on_need_input_buffer),
            onNewOutputBuffer: Some(on_new_output_buffer),
        };
        ensure_ok(
            unsafe { OH_VideoDecoder_RegisterCallback(codec, callbacks, callback as *mut c_void) },
            "RegisterCallback",
        )?;
        if let Some((surface_id, _)) = surface {
            ensure_ok(
                unsafe {
                    OH_NativeWindow_CreateNativeWindowFromSurfaceId(surface_id, &mut cleanup.window)
                },
                "CreateNativeWindowFromSurfaceId",
            )?;
            if cleanup.window.is_null() {
                bail!("CreateNativeWindowFromSurfaceId returned null");
            }
            ensure_ok(
                unsafe { OH_NativeWindow_NativeWindowSetScalingModeV2(cleanup.window, 4) },
                "NativeWindowSetScalingModeV2",
            )?;
        }
        let (width, height) = direct_render::current_size();
        let format = unsafe { OH_AVFormat_CreateVideoFormat(mime.as_ptr() as _, width, height) };
        if format.is_null() {
            bail!("OH_AVFormat_CreateVideoFormat failed");
        }
        let configured = unsafe {
            if !OH_AVFormat_SetIntValue(format, &OH_MD_KEY_PIXEL_FORMAT, AV_PIXEL_FORMAT_NV12) {
                None
            } else {
                Some(OH_VideoDecoder_Configure(codec, format))
            }
        };
        unsafe { OH_AVFormat_Destroy(format) };
        if !matches!(configured, Some(AV_ERR_OK)) {
            bail!("OH_VideoDecoder_Configure failed: {configured:?}");
        }
        if surface.is_some() {
            ensure_ok(
                unsafe { OH_VideoDecoder_SetSurface(codec, cleanup.window) },
                "SetSurface",
            )?;
        }
        ensure_ok(unsafe { OH_VideoDecoder_Prepare(codec) }, "Prepare")?;
        ensure_ok(unsafe { OH_VideoDecoder_Start(codec) }, "Start")?;
        if surface.is_some() {
            let worker_inner = inner.clone();
            cleanup.worker = Some(
                std::thread::Builder::new()
                    .name("ohos-video-output".to_owned())
                    .spawn(move || render_worker(worker_inner))?,
            );
        }
        log::info!(
            "ohcodec decoder created: {}, output: {}",
            mime.trim_end_matches('\0'),
            if surface.is_some() { "surface" } else { "buffer" }
        );
        Ok(Self {
            mime,
            cleanup,
            codec_config: Vec::new(),
            surface_generation: surface.map(|surface| surface.1),
            last_rendered: 0,
            pending_since: None,
            next_pts: 0,
        })
    }

    pub fn name(&self) -> String {
        self.mime.to_owned()
    }

    pub fn is_surface_mode(&self) -> bool {
        !self.cleanup.window.is_null()
    }

    pub fn target_is_current(&self) -> bool {
        match self.surface_generation {
            Some(generation) => direct_render::current_surface()
                .is_some_and(|(_, current)| current == generation),
            None => direct_render::current_surface().is_none(),
        }
    }

    pub fn decode(&mut self, data: &[u8], key: bool, rgb: &mut ImageRgb) -> ResultType<bool> {
        if data.is_empty() {
            return Ok(false);
        }
        if !self.target_is_current() {
            return Ok(false);
        }
        let mut packet = std::mem::take(&mut self.codec_config);
        packet.extend_from_slice(data);
        self.queue_input(&packet, key)?;
        if self.is_surface_mode() {
            self.take_rendered_frame()
        } else {
            self.take_buffer_frame(rgb)
        }
    }

    fn queue_input(&mut self, packet: &[u8], key: bool) -> ResultType<()> {
        let inner = &self.cleanup.inner;
        let wait = if self.is_surface_mode() { SURFACE_STALL } else { BUFFER_WAIT };
        let deadline = Instant::now() + wait;
        let input = {
            let mut shared = inner.lock();
            loop {
                check_shared(&shared)?;
                if let Some(input) = shared.free_inputs.pop_front() {
                    break input;
                }
                let now = Instant::now();
                if now >= deadline {
                    bail!("ohcodec input queue timeout");
                }
                shared = inner
                    .input_ready
                    .wait_timeout(shared, deadline - now)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
        };
        if packet.len() > input.capacity || input.addr.is_null() {
            unsafe { OH_VideoDecoder_PushInputBuffer(inner.codec, input.index) };
            bail!("ohcodec input packet does not fit its buffer");
        }
        unsafe {
            ptr::copy_nonoverlapping(packet.as_ptr(), input.addr, packet.len());
            let attr = OH_AVCodecBufferAttr {
                pts: self.next_pts,
                size: i32::try_from(packet.len())?,
                offset: 0,
                flags: if key {
                    AVCODEC_BUFFER_FLAGS_SYNC_FRAME
                } else {
                    AVCODEC_BUFFER_FLAGS_NONE
                },
            };
            ensure_ok(
                OH_AVBuffer_SetBufferAttr(input.buffer, &attr),
                "SetBufferAttr",
            ).inspect_err(|_| {
                OH_VideoDecoder_PushInputBuffer(inner.codec, input.index);
            })?;
            ensure_ok(
                OH_VideoDecoder_PushInputBuffer(inner.codec, input.index),
                "PushInputBuffer",
            )?;
        }
        self.next_pts += 1;
        Ok(())
    }

    fn take_rendered_frame(&mut self) -> ResultType<bool> {
        let inner = &self.cleanup.inner;
        {
            let shared = inner.lock();
            check_shared(&shared)?;
        }
        let rendered = inner.rendered.load(Ordering::Acquire);
        if rendered != self.last_rendered {
            self.last_rendered = rendered;
            self.pending_since = None;
            return Ok(true);
        }
        let pending_since = self.pending_since.get_or_insert_with(Instant::now);
        if pending_since.elapsed() >= SURFACE_STALL {
            bail!("ohcodec surface render stalled");
        }
        Ok(false)
    }

    fn take_buffer_frame(&mut self, rgb: &mut ImageRgb) -> ResultType<bool> {
        let inner = &self.cleanup.inner;
        let deadline = Instant::now() + BUFFER_WAIT;
        let output = {
            let mut shared = inner.lock();
            loop {
                check_shared(&shared)?;
                if let Some(output) = shared.outputs.pop_front() {
                    break output;
                }
                let now = Instant::now();
                if now >= deadline {
                    return Ok(false);
                }
                shared = inner
                    .output_ready
                    .wait_timeout(shared, deadline - now)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
        };
        let converted = copy_output(output.buffer, output.format, rgb);
        let rc = unsafe { OH_VideoDecoder_FreeOutputBuffer(inner.codec, output.index) };
        if rc != AV_ERR_OK && converted.is_ok() {
            bail!("OH_VideoDecoder_FreeOutputBuffer failed: {rc}");
        }
        converted
    }
}

fn stop_callbacks(inner: &Arc<DecoderInner>) {
    let mut shared = inner.lock();
    shared.running = false;
    let outputs = std::mem::take(&mut shared.outputs);
    drop(shared);
    inner.input_ready.notify_all();
    inner.output_ready.notify_all();
    for output in outputs {
        unsafe { OH_VideoDecoder_FreeOutputBuffer(inner.codec, output.index) };
    }
}

fn check_shared(shared: &Shared) -> ResultType<()> {
    if let Some(error) = &shared.error {
        bail!(error.clone());
    }
    if !shared.running {
        bail!("ohcodec is stopping");
    }
    Ok(())
}

fn ensure_ok(rc: c_int, operation: &str) -> ResultType<()> {
    if rc != AV_ERR_OK {
        bail!("OH_VideoDecoder_{operation} failed: {rc}");
    }
    Ok(())
}

fn render_worker(inner: Arc<DecoderInner>) {
    let mut shared = inner.lock();
    loop {
        while shared.running && shared.outputs.is_empty() {
            shared = inner.output_ready.wait(shared).unwrap_or_else(|e| e.into_inner());
        }
        if !shared.running {
            break;
        }
        let Some(output) = shared.outputs.pop_front() else {
            continue;
        };
        drop(shared);
        let rc = unsafe { OH_VideoDecoder_RenderOutputBuffer(inner.codec, output.index) };
        if rc == AV_ERR_OK {
            inner.rendered.fetch_add(1, Ordering::Release);
        } else {
            unsafe { OH_VideoDecoder_FreeOutputBuffer(inner.codec, output.index) };
            inner.fail(format!("OH_VideoDecoder_RenderOutputBuffer failed: {rc}"));
            break;
        }
        shared = inner.lock();
    }
}

unsafe extern "C" fn on_error(_codec: *mut c_void, error: c_int, user_data: *mut c_void) {
    if let Some(inner) = (user_data as *const DecoderInner).as_ref() {
        inner.fail(format!("ohcodec error: {error}"));
    }
}

unsafe extern "C" fn on_stream_changed(
    _codec: *mut c_void,
    format: *mut c_void,
    user_data: *mut c_void,
) {
    let Some(inner) = (user_data as *const DecoderInner).as_ref() else {
        return;
    };
    if format.is_null() {
        return;
    }
    let mut width = 0;
    let mut height = 0;
    let mut stride = 0;
    let mut slice_height = 0;
    let mut pixel_format = AV_PIXEL_FORMAT_NV12;
    OH_AVFormat_GetIntValue(format, &OH_MD_KEY_WIDTH, &mut width);
    OH_AVFormat_GetIntValue(format, &OH_MD_KEY_HEIGHT, &mut height);
    OH_AVFormat_GetIntValue(format, &OH_MD_KEY_VIDEO_STRIDE, &mut stride);
    OH_AVFormat_GetIntValue(format, &OH_MD_KEY_VIDEO_SLICE_HEIGHT, &mut slice_height);
    OH_AVFormat_GetIntValue(format, &OH_MD_KEY_PIXEL_FORMAT, &mut pixel_format);
    if width <= 0 || height <= 0 {
        return;
    }
    let mut shared = inner.lock();
    shared.format = OutputFormat {
        width: width as usize,
        height: height as usize,
        stride: stride.max(width) as usize,
        slice_height: slice_height.max(height) as usize,
        pixel_format,
    };
    inner.output_ready.notify_all();
}

unsafe extern "C" fn on_need_input_buffer(
    _codec: *mut c_void,
    index: c_uint,
    buffer: *mut c_void,
    user_data: *mut c_void,
) {
    let Some(inner) = (user_data as *const DecoderInner).as_ref() else {
        return;
    };
    let mut shared = inner.lock();
    if !shared.running || buffer.is_null() {
        return;
    }
    let addr = OH_AVBuffer_GetAddr(buffer);
    let capacity = OH_AVBuffer_GetCapacity(buffer);
    if addr.is_null() || capacity < 0 {
        drop(shared);
        inner.fail("invalid ohcodec input buffer".to_owned());
        return;
    }
    shared.free_inputs.push_back(InputBuffer {
        index,
        buffer,
        addr,
        capacity: capacity as usize,
    });
    inner.input_ready.notify_one();
}

unsafe extern "C" fn on_new_output_buffer(
    _codec: *mut c_void,
    index: c_uint,
    buffer: *mut c_void,
    user_data: *mut c_void,
) {
    let Some(inner) = (user_data as *const DecoderInner).as_ref() else {
        return;
    };
    let mut shared = inner.lock();
    if shared.running && !buffer.is_null() {
        let format = shared.format;
        shared.outputs.push_back(OutputBuffer {
            index,
            buffer,
            format,
        });
        inner.output_ready.notify_one();
    }
}

#[derive(Clone, Copy)]
struct Plane<'a> {
    data: &'a [u8],
    stride: usize,
}

fn checked_plane(data: &[u8], stride: usize, width: usize, height: usize) -> ResultType<Plane<'_>> {
    if width == 0 || height == 0 || stride < width {
        bail!("invalid ohcodec plane layout");
    }
    let len = (height - 1)
        .checked_mul(stride)
        .and_then(|end| end.checked_add(width))
        .ok_or_else(|| hbb_common::anyhow::anyhow!("ohcodec plane size overflow"))?;
    if len > data.len() {
        bail!("ohcodec plane exceeds output buffer: {len} > {}", data.len());
    }
    Ok(Plane { data: &data[..len], stride })
}

fn copy_plane(src: Plane<'_>, dst: &mut [u8], dst_stride: usize, width: usize, height: usize) {
    for row in 0..height {
        let src = &src.data[row * src.stride..][..width];
        dst[row * dst_stride..][..width].copy_from_slice(src);
    }
}

fn copy_output(buffer: *mut c_void, format: OutputFormat, rgb: &mut ImageRgb) -> ResultType<bool> {
    let OutputFormat { width, height, .. } = format;
    if width == 0 || height == 0 {
        bail!("ohcodec output geometry unknown");
    }
    let capacity = unsafe { OH_AVBuffer_GetCapacity(buffer) };
    if capacity < 0 {
        bail!("invalid ohcodec output capacity");
    }
    let capacity = capacity as usize;
    let mut attr = OH_AVCodecBufferAttr {
        pts: 0,
        size: 0,
        offset: 0,
        flags: 0,
    };
    if unsafe { OH_AVBuffer_GetBufferAttr(buffer, &mut attr) } != AV_ERR_OK
        || attr.size < 0
        || attr.offset < 0
        || attr.offset as usize > capacity
    {
        bail!("invalid ohcodec output buffer attributes");
    }
    let mut mapped = ptr::null_mut();
    let addr = unsafe { OH_AVBuffer_GetAddr(buffer) };
    let offset = attr.offset as usize;
    let (base, planes) = if !addr.is_null() {
        (unsafe { addr.add(offset) }, None)
    } else {
        let native = unsafe { OH_AVBuffer_GetNativeBuffer(buffer) };
        if native.is_null() {
            bail!("ohcodec output has no accessible buffer");
        }
        let mut planes = NativeBufferPlanes::default();
        let rc = unsafe { OH_NativeBuffer_MapPlanes(native, &mut mapped, &mut planes) };
        if rc != AV_ERR_OK || mapped.is_null() {
            unsafe { OH_NativeBuffer_Unreference(native) };
            bail!("OH_NativeBuffer_MapPlanes failed: {rc}");
        }
        (mapped as *mut u8, Some((native, planes)))
    };
    // attr.offset only applies to the linear AVBuffer view; native planes
    // carry their own offsets from the mapped base.
    let capacity = if planes.is_none() { capacity - offset } else { capacity };
    let result = unsafe {
        copy_output_memory(base, capacity, planes.as_ref().map(|(_, planes)| planes), format, rgb)
    };
    if let Some((native, _)) = planes {
        unsafe {
            let rc = OH_NativeBuffer_Unmap(native);
            if rc != AV_ERR_OK && result.is_ok() {
                OH_NativeBuffer_Unreference(native);
                bail!("OH_NativeBuffer_Unmap failed: {rc}");
            }
            OH_NativeBuffer_Unreference(native);
        }
    }
    result
}

unsafe fn copy_output_memory(
    base: *mut u8,
    capacity: usize,
    planes: Option<&NativeBufferPlanes>,
    format: OutputFormat,
    rgb: &mut ImageRgb,
) -> ResultType<bool> {
    let OutputFormat {
        width,
        height,
        stride,
        slice_height,
        pixel_format,
    } = format;
    let uv_width = width.div_ceil(2);
    let uv_height = height.div_ceil(2);
    let y_plane;
    let u_plane;
    let v_plane;
    let uv_plane;
    if let Some(planes) = planes {
        if planes.plane_count > 4 || planes.plane_count < 2 {
            bail!("invalid ohcodec output plane count");
        }
        let plane = |index: usize, width: usize, height: usize, uv: bool| -> ResultType<Plane<'static>> {
            let plane = planes.planes[index];
            if plane.offset > capacity as u64 {
                bail!("ohcodec plane offset exceeds output buffer");
            }
            let data = std::slice::from_raw_parts(base, capacity);
            let data = &data[plane.offset as usize..];
            let column_stride = plane.column_stride as usize;
            if column_stride != 1 && (!uv || column_stride != 2) {
                bail!("unsupported ohcodec plane column stride: {column_stride}");
            }
            checked_plane(
                data,
                plane.row_stride as usize,
                if uv { width * column_stride } else { width },
                height,
            )
        };
        y_plane = plane(0, width, height, false)?;
        if pixel_format == AV_PIXEL_FORMAT_I420 {
            if planes.plane_count < 3 {
                bail!("ohcodec I420 output has fewer than three planes");
            }
            u_plane = plane(1, uv_width, uv_height, false)?;
            v_plane = plane(2, uv_width, uv_height, false)?;
            uv_plane = y_plane;
        } else {
            uv_plane = plane(1, uv_width, uv_height, true)?;
            u_plane = y_plane;
            v_plane = y_plane;
        }
    } else {
        let data = std::slice::from_raw_parts(base, capacity);
        y_plane = checked_plane(data, stride, width, height)?;
        let uv_offset = stride
            .checked_mul(slice_height)
            .ok_or_else(|| hbb_common::anyhow::anyhow!("ohcodec UV offset overflow"))?;
        if uv_offset > data.len() {
            bail!("ohcodec UV plane offset exceeds output buffer");
        }
        let data = &data[uv_offset..];
        if pixel_format == AV_PIXEL_FORMAT_I420 {
            let uv_stride = stride.div_ceil(2);
            let uv_len = uv_stride
                .checked_mul(slice_height.div_ceil(2))
                .ok_or_else(|| hbb_common::anyhow::anyhow!("ohcodec V offset overflow"))?;
            u_plane = checked_plane(data, uv_stride, uv_width, uv_height)?;
            if uv_len > data.len() {
                bail!("ohcodec V plane offset exceeds output buffer");
            }
            v_plane = checked_plane(&data[uv_len..], uv_stride, uv_width, uv_height)?;
            uv_plane = y_plane;
        } else {
            uv_plane = checked_plane(data, stride, uv_width * 2, uv_height)?;
            u_plane = y_plane;
            v_plane = y_plane;
        }
    }
    let mut packed_u;
    let mut packed_v;
    match pixel_format {
        AV_PIXEL_FORMAT_I420 => {
            packed_u = Vec::new();
            packed_v = Vec::new();
        }
        AV_PIXEL_FORMAT_NV12 | AV_PIXEL_FORMAT_NV21 => {
            packed_u = vec![0; uv_width * uv_height];
            packed_v = vec![0; uv_width * uv_height];
            for row in 0..uv_height {
                for col in 0..uv_width {
                    let pair = &uv_plane.data[row * uv_plane.stride..][col * 2..];
                    let (u, v) = if pixel_format == AV_PIXEL_FORMAT_NV21 {
                        (pair[1], pair[0])
                    } else {
                        (pair[0], pair[1])
                    };
                    packed_u[row * uv_width + col] = u;
                    packed_v[row * uv_width + col] = v;
                }
            }
        }
        _ => bail!("unsupported ohcodec output pixel format: {pixel_format}"),
    }
    rgb.w = width;
    rgb.h = height;
    let dst_stride = (width * 4 + rgb.align() - 1) & !(rgb.align() - 1);
    rgb.raw.resize(height * dst_stride, 0);
    let convert = match rgb.fmt() {
        ImageFormat::ARGB => I420ToARGB,
        ImageFormat::ABGR => I420ToABGR,
        _ => bail!("unsupported image format {:?}", rgb.fmt()),
    };
    let (u, v, uv_stride) = if pixel_format == AV_PIXEL_FORMAT_I420 {
        (u_plane.data.as_ptr(), v_plane.data.as_ptr(), u_plane.stride)
    } else {
        (packed_u.as_ptr(), packed_v.as_ptr(), uv_width)
    };
    convert(
        y_plane.data.as_ptr(),
        y_plane.stride as _,
        u,
        uv_stride as _,
        v,
        uv_stride as _,
        rgb.raw.as_mut_ptr(),
        dst_stride as _,
        width as _,
        height as _,
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_padded_plane_outside_capacity() {
        let data = [0u8; 16];
        assert!(checked_plane(&data, 8, 4, 2).is_ok());
        assert!(checked_plane(&data, 8, 4, 3).is_err());
        assert!(checked_plane(&data, 3, 4, 2).is_err());
    }

    #[test]
    fn copies_only_visible_rows() {
        let data: Vec<u8> = (0..24).collect();
        let src = checked_plane(&data, 8, 4, 3).unwrap();
        let mut dst = [0u8; 12];
        copy_plane(src, &mut dst, 4, 4, 3);
        assert_eq!(dst, [0, 1, 2, 3, 8, 9, 10, 11, 16, 17, 18, 19]);
    }
}
