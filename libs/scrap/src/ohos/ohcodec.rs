// OpenHarmony OH_VideoDecoder (libnative_media_vdec.so) hardware decoder for
// the controller. Buffer mode: H264/H265 in, NV12 out, converted to RGB with
// libyuv. The AV1 stub (aom_stub.c) is unrelated and keeps AV1 negotiation
// disabled.
//
// The codec is driven in its asynchronous callback mode but consumed
// synchronously, mirroring the Android MediaCodecDecoder flow: packets are
// written into callback-supplied input buffers and one output frame is
// awaited per decode() call. Callbacks carry the DecoderInner through userData
// (an Arc raw pointer), so the decoder can be dropped freely; a generation
// counter distinguishes callbacks of a replaced instance.

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use crate::{
    codec::{base_bitrate, EncoderApi, EncoderCfg},
    CodecFormat, EncodeInput, EncodeYuvFormat, I420ToABGR, I420ToARGB, ImageFormat, ImageRgb,
    NV12ToABGR, NV12ToARGB, Pixfmt,
};
use base::message_proto::{EncodedVideoFrame, EncodedVideoFrames, VideoFrame};
use hbb_common::{bail, bytes::Bytes, log, ResultType};
use std::collections::VecDeque;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_void};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const AV_ERR_OK: c_int = 0;
const AV_ERR_INVALID_STATE: c_int = 8;

const AVCODEC_BUFFER_FLAGS_NONE: c_uint = 0;
const AVCODEC_BUFFER_FLAGS_SYNC_FRAME: c_uint = 1 << 1;
const AVCODEC_BUFFER_FLAGS_CODEC_DATA: c_uint = 1 << 3;

const AV_PIXEL_FORMAT_NV12: c_int = 2;

// OH_BitrateMode
const BITRATE_MODE_CBR: c_int = 0;

// The controlled-side encoder mirrors the desktop hw encoder defaults: 30 fps
// rate-control hint, and a bounded GOP (keyframe_interval is in frames) so a
// lossy stretch self-heals instead of waiting for a session refresh.
const DEFAULT_FPS: i32 = 30;
const DEFAULT_GOP_MS: i32 = 10_000;

#[repr(C)]
pub struct OH_AVCodecBufferAttr {
    pub pts: i64,
    pub size: i32,
    pub offset: i32,
    pub flags: c_uint,
}

#[repr(C)]
pub struct OH_AVCodecCallback {
    onError: Option<unsafe extern "C" fn(codec: *mut c_void, errorCode: c_int, userData: *mut c_void)>,
    onStreamChanged: Option<unsafe extern "C" fn(codec: *mut c_void, format: *mut c_void, userData: *mut c_void)>,
    onNeedInputBuffer: Option<
        unsafe extern "C" fn(
            codec: *mut c_void,
            index: c_uint,
            buffer: *mut c_void,
            userData: *mut c_void,
        ),
    >,
    onNewOutputBuffer: Option<
        unsafe extern "C" fn(
            codec: *mut c_void,
            index: c_uint,
            buffer: *mut c_void,
            userData: *mut c_void,
        ),
    >,
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
    fn OH_VideoDecoder_Prepare(codec: *mut c_void) -> c_int;
    fn OH_VideoDecoder_Start(codec: *mut c_void) -> c_int;
    fn OH_VideoDecoder_Stop(codec: *mut c_void) -> c_int;
    fn OH_VideoDecoder_Flush(codec: *mut c_void) -> c_int;
    fn OH_VideoDecoder_PushInputBuffer(codec: *mut c_void, index: c_uint) -> c_int;
    fn OH_VideoDecoder_FreeOutputBuffer(codec: *mut c_void, index: c_uint) -> c_int;
    fn OH_AVFormat_CreateVideoFormat(mimeType: *const c_char, width: i32, height: i32) -> *mut c_void;
    fn OH_AVFormat_Destroy(format: *mut c_void);
    fn OH_AVFormat_SetIntValue(format: *mut c_void, key: *const c_char, value: i32) -> bool;
    fn OH_AVFormat_SetBuffer(format: *mut c_void, key: *const c_char, addr: *const u8, size: usize) -> bool;
    fn OH_AVFormat_GetIntValue(format: *mut c_void, key: *const c_char, out: *mut i32) -> bool;
    fn OH_AVBuffer_GetAddr(buffer: *mut c_void) -> *mut c_uchar;
    fn OH_AVBuffer_GetCapacity(buffer: *mut c_void) -> i32;
    fn OH_AVBuffer_GetBufferAttr(buffer: *mut c_void, attr: *mut OH_AVCodecBufferAttr) -> c_int;
    fn OH_AVBuffer_SetBufferAttr(buffer: *mut c_void, attr: *const OH_AVCodecBufferAttr) -> c_int;

    fn OH_VideoEncoder_CreateByMime(mime: *const c_char) -> *mut c_void;
    fn OH_VideoEncoder_Destroy(codec: *mut c_void) -> c_int;
    fn OH_VideoEncoder_RegisterCallback(
        codec: *mut c_void,
        callback: OH_AVCodecCallback,
        userData: *mut c_void,
    ) -> c_int;
    fn OH_VideoEncoder_Configure(codec: *mut c_void, format: *mut c_void) -> c_int;
    fn OH_VideoEncoder_Prepare(codec: *mut c_void) -> c_int;
    fn OH_VideoEncoder_Start(codec: *mut c_void) -> c_int;
    fn OH_VideoEncoder_Stop(codec: *mut c_void) -> c_int;
    fn OH_VideoEncoder_SetParameter(codec: *mut c_void, format: *mut c_void) -> c_int;
    fn OH_VideoEncoder_PushInputBuffer(codec: *mut c_void, index: c_uint) -> c_int;
    fn OH_VideoEncoder_FreeOutputBuffer(codec: *mut c_void, index: c_uint) -> c_int;
    fn OH_AVFormat_Create() -> *mut c_void;
    fn OH_AVFormat_SetLongValue(format: *mut c_void, key: *const c_char, value: i64) -> bool;
}

// Exported constants in libnative_media_core.so; linked by name instead of
// hardcoding the literals.
extern "C" {
    static OH_AVCODEC_MIMETYPE_VIDEO_AVC: c_char;
    static OH_AVCODEC_MIMETYPE_VIDEO_HEVC: c_char;
    static OH_MD_KEY_WIDTH: c_char;
    static OH_MD_KEY_HEIGHT: c_char;
    static OH_MD_KEY_PIXEL_FORMAT: c_char;
    static OH_MD_KEY_CODEC_CONFIG: c_char;
    static OH_MD_KEY_BITRATE: c_char;
    static OH_MD_KEY_FRAME_RATE: c_char;
    static OH_MD_KEY_I_FRAME_INTERVAL: c_char;
    static OH_MD_KEY_VIDEO_ENCODE_BITRATE_MODE: c_char;
}

fn cstr(ptr: *const c_char) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    unsafe { std::ffi::CStr::from_ptr(ptr) }.to_str().unwrap_or("")
}

fn key(ptr: *const c_char) -> *const c_char {
    ptr as _
}

#[derive(Clone, Copy)]
struct OutputBuffer {
    index: u32,
    addr: *mut u8,
    size: usize,
}

unsafe impl Send for OutputBuffer {}

#[derive(Default)]
struct Shared {
    // Input buffer indices the codec handed back: (index, OH_AVBuffer*,
    // data addr, capacity).
    free_inputs: VecDeque<(u32, *mut c_void, *mut u8, usize)>,
    // Finished output frames waiting to be consumed.
    outputs: VecDeque<OutputBuffer>,
    error: Option<c_int>,
    // Reported geometry from the stream-changed event.
    width: i32,
    height: i32,
}

struct DecoderInner {
    codec: *mut c_void,
    shared: Mutex<Shared>,
    cond: Condvar,
}

unsafe impl Send for DecoderInner {}
unsafe impl Sync for DecoderInner {}

pub struct OhCodecDecoder {
    mime: &'static str,
    inner: Arc<DecoderInner>,
    // Pending SPS/PPS delivered with the codec_data flag; prepended to the
    // next frame packet.
    codec_config: Vec<u8>,
    width: i32,
    height: i32,
}

impl OhCodecDecoder {
    pub fn new(format: CodecFormat) -> Option<Self> {
        let mime = match format {
            CodecFormat::H264 => "video/avc\0,",
            CodecFormat::H265 => "video/hevc\0,",
            _ => {
                log::error!("Unsupported codec format for ohcodec: {format:?}");
                return None;
            }
        };
        Self::with_mime(mime)
    }

    fn with_mime(mime: &'static str) -> Option<Self> {
        let codec = unsafe { OH_VideoDecoder_CreateByMime(mime.as_ptr() as _) };
        if codec.is_null() {
            log::error!("OH_VideoDecoder_CreateByMime({}) failed", mime.trim_end_matches('\0'));
            return None;
        }
        let inner = Arc::new(DecoderInner {
            codec,
            shared: Mutex::new(Shared::default()),
            cond: Condvar::new(),
        });
        let cb = OH_AVCodecCallback {
            onError: Some(on_error),
            onStreamChanged: Some(on_stream_changed),
            onNeedInputBuffer: Some(on_need_input_buffer),
            onNewOutputBuffer: Some(on_new_output_buffer),
        };
        let rc = unsafe {
            OH_VideoDecoder_RegisterCallback(codec, cb, Arc::as_ptr(&inner) as *mut c_void)
        };
        if rc != AV_ERR_OK {
            log::error!("OH_VideoDecoder_RegisterCallback failed: {rc}");
            unsafe {
                OH_VideoDecoder_Destroy(codec);
            }
            return None;
        }
        // Buffer mode, NV12 output: conversion stays on the CPU like the
        // other ram decoders. Width/height 0 lets the codec report geometry.
        let fmt = unsafe { OH_AVFormat_CreateVideoFormat(mime.as_ptr() as _, 0, 0) };
        if fmt.is_null() {
            unsafe {
                OH_VideoDecoder_Destroy(codec);
            }
            return None;
        }
        let ok = unsafe {
            OH_AVFormat_SetIntValue(fmt, key(&OH_MD_KEY_PIXEL_FORMAT), AV_PIXEL_FORMAT_NV12)
        };
        if !ok {
            unsafe {
                OH_AVFormat_Destroy(fmt);
                OH_VideoDecoder_Destroy(codec);
            }
            return None;
        }
        let rc = unsafe { OH_VideoDecoder_Configure(codec, fmt) };
        unsafe {
            OH_AVFormat_Destroy(fmt);
        }
        if rc != AV_ERR_OK {
            log::error!("OH_VideoDecoder_Configure failed: {rc}");
            unsafe {
                OH_VideoDecoder_Destroy(codec);
            }
            return None;
        }
        if unsafe { OH_VideoDecoder_Prepare(codec) } != AV_ERR_OK
            || unsafe { OH_VideoDecoder_Start(codec) } != AV_ERR_OK
        {
            log::error!("OH_VideoDecoder Prepare/Start failed");
            unsafe {
                OH_VideoDecoder_Destroy(codec);
            }
            return None;
        }
        log::info!("ohcodec decoder created: {}", mime.trim_end_matches('\0'));
        Some(Self {
            mime,
            inner,
            codec_config: Vec::new(),
            width: 0,
            height: 0,
        })
    }

    pub fn name(&self) -> String {
        self.mime.to_owned()
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    pub fn decode(&mut self, data: &[u8], key: bool, rgb: &mut ImageRgb) -> ResultType<bool> {
        if data.is_empty() {
            return Ok(false);
        }
        // Prepend buffered codec-specific data so decoding can start on any
        // stream whose config packet arrived separately.
        let mut packet = std::mem::take(&mut self.codec_config);
        packet.extend_from_slice(data);
        self.queue_input(&packet, key)?;
        self.wait_output(rgb)
    }

    fn queue_input(&mut self, packet: &[u8], key: bool) -> ResultType<()> {
        let deadline = Instant::now() + Duration::from_millis(100);
        let (index, attr_buffer, addr, capacity) = {
            let mut shared = self.inner.shared.lock().unwrap();
            loop {
                if let Some(err) = shared.error {
                    bail!("ohcodec error: {err}");
                }
                if let Some(entry) = shared.free_inputs.pop_front() {
                    break entry;
                }
                let now = Instant::now();
                if now >= deadline {
                    bail!("ohcodec input queue timeout");
                }
                let (guard, _timeout) = self
                    .inner
                    .cond
                    .wait_timeout(shared, deadline - now)
                    .unwrap();
                shared = guard;
            }
        };
        if packet.len() > capacity {
            // Return the slot and fail; the caller's fail counter will reset
            // the decoder at a saner resolution.
            unsafe {
                OH_VideoDecoder_PushInputBuffer(self.inner.codec, index);
            }
            bail!(
                "ohcodec packet {} bytes exceeds input buffer {} bytes",
                packet.len(),
                capacity
            );
        }
        unsafe {
            std::ptr::copy_nonoverlapping(packet.as_ptr(), addr, packet.len());
        }
        // Buffer attributes ride the OH_AVBuffer the callback supplied; set
        // them right before handing the slot back to the codec.
        let attr = OH_AVCodecBufferAttr {
            pts: 0,
            size: packet.len() as i32,
            offset: 0,
            flags: if key {
                AVCODEC_BUFFER_FLAGS_SYNC_FRAME
            } else {
                AVCODEC_BUFFER_FLAGS_NONE
            },
        };
        unsafe {
            if !attr_buffer.is_null() {
                OH_AVBuffer_SetBufferAttr(attr_buffer as *mut c_void, &attr);
            }
        }
        let rc = unsafe { OH_VideoDecoder_PushInputBuffer(self.inner.codec, index) };
        if rc != AV_ERR_OK {
            bail!("OH_VideoDecoder_PushInputBuffer failed: {rc}");
        }
        Ok(())
    }

    fn wait_output(&mut self, rgb: &mut ImageRgb) -> ResultType<bool> {
        let deadline = Instant::now() + Duration::from_millis(100);
        let out = {
            let mut shared = self.inner.shared.lock().unwrap();
            loop {
                if let Some(err) = shared.error {
                    bail!("ohcodec error: {err}");
                }
                if shared.width > 0 && shared.height > 0 {
                    self.width = shared.width;
                    self.height = shared.height;
                }
                if let Some(out) = shared.outputs.pop_front() {
                    break out;
                }
                let now = Instant::now();
                if now >= deadline {
                    // No frame ready yet (B-frames/reorder can delay one);
                    // not an error, just no new RGB this round.
                    return Ok(false);
                }
                let (guard, _timeout) = self
                    .inner
                    .cond
                    .wait_timeout(shared, deadline - now)
                    .unwrap();
                shared = guard;
            }
        };
        // Convert before freeing: the codec recycles the buffer as soon as it
        // is returned, reading it afterwards races with the next frame.
        let converted = convert_nv12(out.addr, out.size, rgb, &mut self.width, &mut self.height);
        unsafe {
            OH_VideoDecoder_FreeOutputBuffer(self.inner.codec, out.index);
        }
        converted
    }
}

fn convert_nv12(
    src: *mut u8,
    len: usize,
    rgb: &mut ImageRgb,
    width: &mut i32,
    height: &mut i32,
) -> ResultType<bool> {
    if *width <= 0 || *height <= 0 {
        bail!("ohcodec output geometry unknown");
    }
    let w = *width as usize;
    let h = *height as usize;
    if len < w * h * 3 / 2 {
        bail!("ohcodec output buffer too small: {len} < {}", w * h * 3 / 2);
    }
    rgb.w = w;
    rgb.h = h;
    let dst_align = rgb.align();
    let bytes_per_row = (rgb.w * 4 + dst_align - 1) & !(dst_align - 1);
    rgb.raw.resize(rgb.h * bytes_per_row, 0);
    let y_ptr = src;
    let uv_ptr = unsafe { src.add(w * h) };
    let y_stride = w as i32;
    let uv_stride = w as i32;
    unsafe {
        let f = match rgb.fmt() {
            ImageFormat::ARGB => NV12ToARGB,
            ImageFormat::ABGR => NV12ToABGR,
            _ => bail!("unsupported image format {:?}", rgb.fmt()),
        };
        f(
            y_ptr,
            y_stride,
            uv_ptr,
            uv_stride,
            rgb.raw.as_mut_ptr(),
            bytes_per_row as i32,
            w as i32,
            h as i32,
        );
    }
    let _ = (I420ToARGB, I420ToABGR);
    Ok(true)
}

impl Drop for OhCodecDecoder {
    fn drop(&mut self) {
        unsafe {
            OH_VideoDecoder_Stop(self.inner.codec);
            OH_VideoDecoder_Destroy(self.inner.codec);
        }
    }
}

unsafe extern "C" fn on_stream_changed(
    _codec: *mut c_void,
    format: *mut c_void,
    userData: *mut c_void,
) {
    let inner = &*(userData as *const DecoderInner);
    let mut shared = inner.shared.lock().unwrap();
    if !format.is_null() {
        let mut w = 0;
        let mut h = 0;
        // Read back the geometry the codec negotiated; keys are the same
        // OH_MD_KEY_* constants used at configure time.
        OH_AVFormat_GetIntValue(format, key(&OH_MD_KEY_WIDTH), &mut w);
        OH_AVFormat_GetIntValue(format, key(&OH_MD_KEY_HEIGHT), &mut h);
        shared.width = w;
        shared.height = h;
    }
    inner.cond.notify_all();
}

unsafe extern "C" fn on_error(_codec: *mut c_void, errorCode: c_int, userData: *mut c_void) {
    let inner = &*(userData as *const DecoderInner);
    let mut shared = inner.shared.lock().unwrap();
    if shared.error.is_none() {
        shared.error = Some(errorCode);
    }
    inner.cond.notify_all();
}

unsafe extern "C" fn on_need_input_buffer(
    _codec: *mut c_void,
    index: c_uint,
    buffer: *mut c_void,
    userData: *mut c_void,
) {
    let inner = &*(userData as *const DecoderInner);
    let addr = if buffer.is_null() {
        std::ptr::null_mut()
    } else {
        OH_AVBuffer_GetAddr(buffer)
    };
    let capacity = if buffer.is_null() {
        0
    } else {
        OH_AVBuffer_GetCapacity(buffer)
    };
    let mut shared = inner.shared.lock().unwrap();
    shared
        .free_inputs
        .push_back((index, buffer, addr, capacity as usize));
    inner.cond.notify_all();
}

unsafe extern "C" fn on_new_output_buffer(
    codec: *mut c_void,
    index: c_uint,
    buffer: *mut c_void,
    userData: *mut c_void,
) {
    let inner = &*(userData as *const DecoderInner);
    let mut out = None;
    if !buffer.is_null() {
        let mut attr = OH_AVCodecBufferAttr {
            pts: 0,
            size: 0,
            offset: 0,
            flags: 0,
        };
        if OH_AVBuffer_GetBufferAttr(buffer, &mut attr) == AV_ERR_OK {
            let addr = OH_AVBuffer_GetAddr(buffer);
            let capacity = OH_AVBuffer_GetCapacity(buffer);
            if !addr.is_null() && attr.size > 0 && attr.size as usize <= capacity as usize {
                out = Some(OutputBuffer {
                    index,
                    addr: addr.add(attr.offset as usize),
                    size: attr.size as usize,
                });
            }
        }
    }
    let mut shared = inner.shared.lock().unwrap();
    match out {
        Some(out) => shared.outputs.push_back(out),
        None => unsafe {
            OH_VideoDecoder_FreeOutputBuffer(codec, index);
        },
    }
    inner.cond.notify_all();
}

// ---------------------------------------------------------------------------
// Controlled-side hardware encoder (OH_VideoEncoder, buffer mode): NV12 in,
// Annex-B H264/H265 out. Mirrors the desktop HwRamEncoder contract so
// video_service treats it like any other hardware encoder.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OhEncoderConfig {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub quality: f32,
    pub keyframe_interval: Option<usize>,
    pub format: CodecFormat,
}

pub struct OhEncodedFrame {
    pub data: Vec<u8>,
    pub key: bool,
    pub pts: i64,
}

#[derive(Clone, Copy)]
struct EncoderOutputBuffer {
    index: u32,
    addr: *mut u8,
    size: usize,
    flags: c_uint,
    pts: i64,
}

unsafe impl Send for EncoderOutputBuffer {}

#[derive(Default)]
struct EncoderShared {
    free_inputs: VecDeque<(u32, *mut c_void, *mut u8, usize)>,
    outputs: VecDeque<EncoderOutputBuffer>,
    error: Option<c_int>,
}

struct EncoderInner {
    codec: *mut c_void,
    shared: Mutex<EncoderShared>,
    cond: Condvar,
}

unsafe impl Send for EncoderInner {}
unsafe impl Sync for EncoderInner {}

pub struct OhCodecEncoder {
    mime: &'static str,
    format: CodecFormat,
    inner: Arc<EncoderInner>,
    config: OhEncoderConfig,
    bitrate: u32, // kbps
    // CSD (SPS/PPS/VPS) delivered with the codec-data flag, prepended to the
    // next sync frame so decoders can start mid-stream.
    pending_config: Vec<u8>,
    warmed_up: bool,
}

// bit0 h264, bit1 h265; None = not probed yet. DISABLED makes try_get return
// None for the rest of the process: it mirrors HwCodecConfig::clear, whose
// emptied config list keeps desktop fallbacks sticky instead of re-probing a
// codec that just failed at runtime.
static ENCODE_SUPPORT: Mutex<Option<u8>> = Mutex::new(None);
static ENCODE_DISABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn probe_encoder(mime: &str) -> bool {
    let codec = unsafe { OH_VideoEncoder_CreateByMime(mime.as_ptr() as _) };
    if codec.is_null() {
        return false;
    }
    let ok = (|| {
        let fmt = unsafe { OH_AVFormat_CreateVideoFormat(mime.as_ptr() as _, 1280, 720) };
        if fmt.is_null() {
            return false;
        }
        let ok = unsafe {
            OH_AVFormat_SetIntValue(fmt, key(&OH_MD_KEY_PIXEL_FORMAT), AV_PIXEL_FORMAT_NV12)
                && OH_AVFormat_SetIntValue(
                    fmt,
                    key(&OH_MD_KEY_VIDEO_ENCODE_BITRATE_MODE),
                    BITRATE_MODE_CBR,
                )
                && OH_AVFormat_SetLongValue(fmt, key(&OH_MD_KEY_BITRATE), 2_000_000)
                && OH_AVFormat_SetIntValue(fmt, key(&OH_MD_KEY_FRAME_RATE), DEFAULT_FPS)
                && OH_AVFormat_SetIntValue(fmt, key(&OH_MD_KEY_I_FRAME_INTERVAL), DEFAULT_GOP_MS)
        } && unsafe { OH_VideoEncoder_Configure(codec, fmt) } == AV_ERR_OK
            && unsafe { OH_VideoEncoder_Prepare(codec) } == AV_ERR_OK;
        unsafe { OH_AVFormat_Destroy(fmt) };
        ok
    })();
    unsafe {
        OH_VideoEncoder_Destroy(codec);
    }
    ok
}

fn encode_support_bits() -> u8 {
    use std::sync::atomic::Ordering;
    if ENCODE_DISABLED.load(Ordering::SeqCst) {
        return 0;
    }
    let mut support = ENCODE_SUPPORT.lock().unwrap();
    if support.is_none() {
        let mut bits = 0u8;
        if probe_encoder("video/avc\0") {
            bits |= 0b01;
        }
        if probe_encoder("video/hevc\0") {
            bits |= 0b10;
        }
        log::info!(
            "ohcodec encoder probe: h264={}, h265={}",
            bits & 0b01 != 0,
            bits & 0b10 != 0
        );
        *support = Some(bits);
    }
    support.unwrap()
}

impl OhCodecEncoder {
    pub fn try_get(format: CodecFormat) -> Option<String> {
        let bits = encode_support_bits();
        let mime = mime_for(format)?;
        let supported = match format {
            CodecFormat::H264 => bits & 0b01 != 0,
            CodecFormat::H265 => bits & 0b10 != 0,
            _ => false,
        };
        supported.then(|| {
            format!(
                "OH_VideoEncoder({})",
                mime.trim_start_matches("video/").trim_end_matches('\0')
            )
        })
    }

    pub fn clear_support_cache() {
        use std::sync::atomic::Ordering;
        ENCODE_DISABLED.store(true, Ordering::SeqCst);
        *ENCODE_SUPPORT.lock().unwrap() = None;
    }

    pub fn bitrate(width: usize, height: usize, ratio: f32) -> u32 {
        // Same formula as HwRamEncoder::calc_bitrate (h264 branch); hardware
        // encoders need headroom above the vpx base bitrate.
        let base = base_bitrate(width as _, height as _) as f32 * ratio;
        let threshold = 2000.0;
        let decay_rate = 0.001; // 1000 * 0.001 = 1
        let factor: f32 = if base > threshold {
            1.0 + 1.0 / (1.0 + (base - threshold) * decay_rate)
        } else {
            2.0
        };
        (base * factor) as u32
    }

    pub fn encode(&mut self, yuv: &[u8], ms: i64) -> ResultType<Vec<OhEncodedFrame>> {
        let need = self.config.width * self.config.height * 3 / 2;
        if yuv.len() < need {
            bail!(
                "ohcodec encoder input too small: {} < {need}",
                yuv.len()
            );
        }
        self.queue_input(&yuv[..need], ms)?;
        self.wait_output()
    }

    fn queue_input(&mut self, data: &[u8], pts: i64) -> ResultType<()> {
        let deadline = Instant::now() + Duration::from_millis(300);
        let (index, attr_buffer, addr, capacity) = {
            let mut shared = self.inner.shared.lock().unwrap();
            loop {
                if let Some(err) = shared.error {
                    bail!("ohcodec encoder error: {err}");
                }
                if let Some(entry) = shared.free_inputs.pop_front() {
                    break entry;
                }
                let now = Instant::now();
                if now >= deadline {
                    bail!("ohcodec encoder input queue timeout");
                }
                let (guard, _timeout) = self
                    .inner
                    .cond
                    .wait_timeout(shared, deadline - now)
                    .unwrap();
                shared = guard;
            }
        };
        if data.len() > capacity {
            unsafe {
                OH_VideoEncoder_PushInputBuffer(self.inner.codec, index);
            }
            bail!(
                "ohcodec encoder frame {} bytes exceeds input buffer {} bytes",
                data.len(),
                capacity
            );
        }
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), addr, data.len());
        }
        let attr = OH_AVCodecBufferAttr {
            pts,
            size: data.len() as i32,
            offset: 0,
            flags: AVCODEC_BUFFER_FLAGS_NONE,
        };
        unsafe {
            if !attr_buffer.is_null() {
                OH_AVBuffer_SetBufferAttr(attr_buffer as *mut c_void, &attr);
            }
        }
        let rc = unsafe { OH_VideoEncoder_PushInputBuffer(self.inner.codec, index) };
        if rc != AV_ERR_OK {
            bail!("OH_VideoEncoder_PushInputBuffer failed: {rc}");
        }
        Ok(())
    }

    fn wait_output(&mut self) -> ResultType<Vec<OhEncodedFrame>> {
        let deadline = Instant::now() + Duration::from_millis(if self.warmed_up { 300 } else { 1000 });
        let mut frames = Vec::new();
        let mut shared = self.inner.shared.lock().unwrap();
        loop {
            if let Some(err) = shared.error {
                self.warmed_up = true;
                bail!("ohcodec encoder error: {err}");
            }
            match shared.outputs.pop_front() {
                Some(out) => {
                    // Copy before freeing: the codec may recycle the buffer
                    // immediately once it is returned.
                    let mut data = vec![0u8; out.size];
                    unsafe {
                        std::ptr::copy_nonoverlapping(out.addr, data.as_mut_ptr(), out.size);
                        OH_VideoEncoder_FreeOutputBuffer(self.inner.codec, out.index);
                    }
                    if out.flags & AVCODEC_BUFFER_FLAGS_CODEC_DATA != 0 {
                        self.pending_config = data;
                        continue;
                    }
                    if !data.is_empty() {
                        if !self.pending_config.is_empty() {
                            let mut with_config = std::mem::take(&mut self.pending_config);
                            with_config.extend_from_slice(&data);
                            data = with_config;
                        }
                        frames.push(OhEncodedFrame {
                            key: out.flags & AVCODEC_BUFFER_FLAGS_SYNC_FRAME != 0,
                            data,
                            pts: out.pts,
                        });
                    }
                }
                None => {
                    let now = Instant::now();
                    if !frames.is_empty() || now >= deadline {
                        break;
                    }
                    let (guard, _timeout) = self
                        .inner
                        .cond
                        .wait_timeout(shared, deadline - now)
                        .unwrap();
                    shared = guard;
                }
            }
        }
        if !frames.is_empty() {
            self.warmed_up = true;
        }
        Ok(frames)
    }
}

fn mime_for(format: CodecFormat) -> Option<&'static str> {
    match format {
        CodecFormat::H264 => Some("video/avc\0"),
        CodecFormat::H265 => Some("video/hevc\0"),
        _ => None,
    }
}

impl EncoderApi for OhCodecEncoder {
    fn new(cfg: EncoderCfg, _i444: bool) -> ResultType<Self>
    where
        Self: Sized,
    {
        match cfg {
            EncoderCfg::OHOS_HW(config) => {
                if config.format != CodecFormat::H264 && config.format != CodecFormat::H265 {
                    bail!("unsupported format: {:?}", config.format);
                }
                let mime = match mime_for(config.format) {
                    Some(mime) => mime,
                    None => bail!("unsupported format: {:?}", config.format),
                };
                let bitrate = Self::bitrate(config.width, config.height, config.quality);
                let gop_ms = config.keyframe_interval.map_or(DEFAULT_GOP_MS, |frames| {
                    ((frames as i32) * 1000 / DEFAULT_FPS).max(1000)
                });
                let codec = unsafe { OH_VideoEncoder_CreateByMime(mime.as_ptr() as _) };
                if codec.is_null() {
                    bail!("OH_VideoEncoder_CreateByMime({}) failed", mime.trim_end_matches('\0'));
                }
                let inner = Arc::new(EncoderInner {
                    codec,
                    shared: Mutex::new(EncoderShared::default()),
                    cond: Condvar::new(),
                });
                let cb = OH_AVCodecCallback {
                    onError: Some(enc_on_error),
                    onStreamChanged: Some(enc_on_stream_changed),
                    onNeedInputBuffer: Some(enc_on_need_input_buffer),
                    onNewOutputBuffer: Some(enc_on_new_output_buffer),
                };
                let rc = unsafe {
                    OH_VideoEncoder_RegisterCallback(codec, cb, Arc::as_ptr(&inner) as *mut c_void)
                };
                if rc != AV_ERR_OK {
                    log::error!("OH_VideoEncoder_RegisterCallback failed: {rc}");
                    unsafe {
                        OH_VideoEncoder_Destroy(codec);
                    }
                    bail!("OH_VideoEncoder_RegisterCallback failed: {rc}");
                }
                let fmt = unsafe {
                    OH_AVFormat_CreateVideoFormat(
                        mime.as_ptr() as _,
                        config.width as i32,
                        config.height as i32,
                    )
                };
                if fmt.is_null() {
                    unsafe {
                        OH_VideoEncoder_Destroy(codec);
                    }
                    bail!("OH_AVFormat_CreateVideoFormat failed");
                }
                let ok = unsafe {
                    OH_AVFormat_SetIntValue(fmt, key(&OH_MD_KEY_PIXEL_FORMAT), AV_PIXEL_FORMAT_NV12)
                        && OH_AVFormat_SetIntValue(
                            fmt,
                            key(&OH_MD_KEY_VIDEO_ENCODE_BITRATE_MODE),
                            BITRATE_MODE_CBR,
                        )
                        && OH_AVFormat_SetLongValue(
                            fmt,
                            key(&OH_MD_KEY_BITRATE),
                            bitrate as i64 * 1000,
                        )
                        && OH_AVFormat_SetIntValue(fmt, key(&OH_MD_KEY_FRAME_RATE), DEFAULT_FPS)
                        && OH_AVFormat_SetIntValue(fmt, key(&OH_MD_KEY_I_FRAME_INTERVAL), gop_ms)
                };
                let rc = if ok {
                    unsafe { OH_VideoEncoder_Configure(codec, fmt) }
                } else {
                    AV_ERR_INVALID_STATE
                };
                unsafe {
                    OH_AVFormat_Destroy(fmt);
                }
                if rc != AV_ERR_OK {
                    log::error!("OH_VideoEncoder_Configure failed: {rc}");
                    unsafe {
                        OH_VideoEncoder_Destroy(codec);
                    }
                    bail!("OH_VideoEncoder_Configure failed: {rc}");
                }
                if unsafe { OH_VideoEncoder_Prepare(codec) } != AV_ERR_OK
                    || unsafe { OH_VideoEncoder_Start(codec) } != AV_ERR_OK
                {
                    log::error!("OH_VideoEncoder Prepare/Start failed");
                    unsafe {
                        OH_VideoEncoder_Destroy(codec);
                    }
                    bail!("OH_VideoEncoder Prepare/Start failed");
                }
                log::info!(
                    "ohcodec encoder created: {}, {}x{}, bitrate {bitrate} kbps, gop {gop_ms} ms",
                    mime.trim_end_matches('\0'),
                    config.width,
                    config.height
                );
                Ok(Self {
                    mime,
                    format: config.format,
                    inner,
                    config,
                    bitrate,
                    pending_config: Vec::new(),
                    warmed_up: false,
                })
            }
            _ => bail!("encoder type mismatch"),
        }
    }

    fn encode_to_message(&mut self, input: EncodeInput, ms: i64) -> ResultType<VideoFrame> {
        let yuv = input.yuv()?;
        let mut frames = Vec::new();
        for frame in self.encode(yuv, ms)? {
            frames.push(EncodedVideoFrame {
                data: Bytes::from(frame.data),
                pts: frame.pts,
                key: frame.key,
                ..Default::default()
            });
        }
        if frames.is_empty() {
            bail!("no valid frame");
        }
        let mut vf = VideoFrame::new();
        let frames = EncodedVideoFrames {
            frames: frames.into(),
            ..Default::default()
        };
        match self.format {
            CodecFormat::H264 => vf.set_h264s(frames),
            CodecFormat::H265 => vf.set_h265s(frames),
            _ => bail!("unsupported format: {:?}", self.format),
        }
        Ok(vf)
    }

    fn yuvfmt(&self) -> EncodeYuvFormat {
        EncodeYuvFormat {
            pixfmt: Pixfmt::NV12,
            w: self.config.width,
            h: self.config.height,
            stride: vec![self.config.width, self.config.width],
            u: self.config.width * self.config.height,
            v: 0,
        }
    }

    fn set_quality(&mut self, ratio: f32) -> ResultType<()> {
        let bitrate = Self::bitrate(self.config.width, self.config.height, ratio);
        if bitrate > 0 && bitrate != self.bitrate {
            let fmt = unsafe { OH_AVFormat_Create() };
            if fmt.is_null() {
                bail!("OH_AVFormat_Create failed");
            }
            let rc = unsafe {
                OH_AVFormat_SetLongValue(fmt, key(&OH_MD_KEY_BITRATE), bitrate as i64 * 1000);
                OH_VideoEncoder_SetParameter(self.inner.codec, fmt)
            };
            unsafe {
                OH_AVFormat_Destroy(fmt);
            }
            if rc == AV_ERR_OK {
                self.bitrate = bitrate;
            } else {
                log::warn!("OH_VideoEncoder_SetParameter(bitrate) failed: {rc}");
            }
        }
        self.config.quality = ratio;
        Ok(())
    }

    fn bitrate(&self) -> u32 {
        self.bitrate
    }

    fn support_changing_quality(&self) -> bool {
        true
    }

    fn latency_free(&self) -> bool {
        false
    }

    fn is_hardware(&self) -> bool {
        true
    }

    fn disable(&self) {
        Self::clear_support_cache();
        crate::codec::Encoder::update(crate::codec::EncodingUpdate::Check);
    }
}

impl Drop for OhCodecEncoder {
    fn drop(&mut self) {
        unsafe {
            OH_VideoEncoder_Stop(self.inner.codec);
            OH_VideoEncoder_Destroy(self.inner.codec);
        }
    }
}

unsafe extern "C" fn enc_on_stream_changed(
    _codec: *mut c_void,
    _format: *mut c_void,
    _userData: *mut c_void,
) {
}

unsafe extern "C" fn enc_on_error(_codec: *mut c_void, errorCode: c_int, userData: *mut c_void) {
    let inner = &*(userData as *const EncoderInner);
    let mut shared = inner.shared.lock().unwrap();
    if shared.error.is_none() {
        shared.error = Some(errorCode);
    }
    inner.cond.notify_all();
}

unsafe extern "C" fn enc_on_need_input_buffer(
    _codec: *mut c_void,
    index: c_uint,
    buffer: *mut c_void,
    userData: *mut c_void,
) {
    let inner = &*(userData as *const EncoderInner);
    let addr = if buffer.is_null() {
        std::ptr::null_mut()
    } else {
        OH_AVBuffer_GetAddr(buffer)
    };
    let capacity = if buffer.is_null() {
        0
    } else {
        OH_AVBuffer_GetCapacity(buffer)
    };
    let mut shared = inner.shared.lock().unwrap();
    shared
        .free_inputs
        .push_back((index, buffer, addr, capacity as usize));
    inner.cond.notify_all();
}

unsafe extern "C" fn enc_on_new_output_buffer(
    codec: *mut c_void,
    index: c_uint,
    buffer: *mut c_void,
    userData: *mut c_void,
) {
    let inner = &*(userData as *const EncoderInner);
    let mut out = None;
    if !buffer.is_null() {
        let mut attr = OH_AVCodecBufferAttr {
            pts: 0,
            size: 0,
            offset: 0,
            flags: 0,
        };
        if OH_AVBuffer_GetBufferAttr(buffer, &mut attr) == AV_ERR_OK {
            let addr = OH_AVBuffer_GetAddr(buffer);
            let capacity = OH_AVBuffer_GetCapacity(buffer);
            if !addr.is_null() && attr.size > 0 && attr.size as usize <= capacity as usize {
                out = Some(EncoderOutputBuffer {
                    index,
                    addr: addr.add(attr.offset as usize),
                    size: attr.size as usize,
                    flags: attr.flags,
                    pts: attr.pts,
                });
            }
        }
    }
    let mut shared = inner.shared.lock().unwrap();
    match out {
        Some(out) => shared.outputs.push_back(out),
        None => unsafe {
            OH_VideoEncoder_FreeOutputBuffer(codec, index);
        },
    }
    inner.cond.notify_all();
}
