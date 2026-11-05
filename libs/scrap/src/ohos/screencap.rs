// OpenHarmony AVScreenCapture (libnative_avscreen_capture.so) C API bindings
// and the controlled-side capture lifecycle, hand-written like
// `src/ohos_audio.rs` because only a small slice of the surface is needed.
//
// The capture runs in OH_ORIGINAL_STREAM mode: StartScreenCapture pops the
// system picker (user consent). Buffers are collected with the API-12
// `SetDataCallback` (audio works through it on the emulator; on real devices
// it also delivers video). As a fallback for images where the callback never
// produces video (the emulator's screen capture service cannot make video
// buffers at all), a pump thread polls the legacy `AcquireVideoBuffer` while
// RUNNING and no callback video has been seen. The pump thread also
// owns StopScreenCapture/Release so the handle is never used after teardown.
// Legacy model notes kept: frames are collected with the API-10
// model — `OH_AVScreenCapture_SetCallback` ready-flags plus a pump thread that
// calls `AcquireVideoBuffer` / `AcquireAudioBuffer`. The new-style
// `SetDataCallback` is deliberately NOT used: it forbids the Acquire* calls and
// the emulator never delivers video buffers through it. Video buffers arrive
// as RGBA8888 OH_NativeBuffer (row stride from the buffer config, de-strided
// before pushing); audio buffers are PCM s16le at the configured rate/channels.
//
// Signatures mirror the NDK headers
// (multimedia/player_framework/native_avscreen_capture{,_base}.h,
// native_buffer/native_buffer.h).

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use super::ffi;
use lazy_static::lazy_static;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub type OH_AVScreenCapture = c_void;
pub type OH_AudioCaptureSourceType = c_int;
pub type OH_NativeBuffer = c_void;

// OH_CaptureMode
pub const OH_CAPTURE_HOME_SCREEN: c_int = 0;
// OH_DataType
pub const OH_ORIGINAL_STREAM: c_int = 0;
// OH_AudioCaptureSourceType
pub const OH_MIC: c_int = 1;
pub const OH_ALL_PLAYBACK: c_int = 2;
// OH_VideoSourceType
pub const OH_VIDEO_SOURCE_SURFACE_RGBA: c_int = 2;

pub const AV_SCREEN_CAPTURE_ERR_OK: c_int = 0;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OH_AudioCaptureInfo {
    pub audioSampleRate: i32,
    pub audioChannels: i32,
    pub audioSource: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OH_AudioEncInfo {
    pub audioBitrate: i32,
    pub audioCodecformat: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OH_AudioInfo {
    pub micCapInfo: OH_AudioCaptureInfo,
    pub innerCapInfo: OH_AudioCaptureInfo,
    pub audioEncInfo: OH_AudioEncInfo,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OH_VideoCaptureInfo {
    pub displayId: u64,
    pub missionIDs: *mut c_int,
    pub missionIDsLen: i32,
    pub videoFrameWidth: i32,
    pub videoFrameHeight: i32,
    pub videoSource: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OH_VideoEncInfo {
    pub videoCodec: c_int,
    pub videoBitrate: i32,
    pub videoFrameRate: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OH_VideoInfo {
    pub videoCapInfo: OH_VideoCaptureInfo,
    pub videoEncInfo: OH_VideoEncInfo,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OH_RecorderInfo {
    pub url: *mut c_char,
    pub urlLen: u32,
    pub fileFormat: c_int,
}

impl Default for OH_RecorderInfo {
    fn default() -> Self {
        OH_RecorderInfo {
            url: std::ptr::null_mut(),
            urlLen: 0,
            fileFormat: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OH_AVScreenCaptureConfig {
    pub captureMode: c_int,
    pub dataType: c_int,
    pub audioInfo: OH_AudioInfo,
    pub videoInfo: OH_VideoInfo,
    pub recorderInfo: OH_RecorderInfo,
}

impl Default for OH_AVScreenCaptureConfig {
    fn default() -> Self {
        OH_AVScreenCaptureConfig {
            captureMode: 0,
            dataType: 0,
            audioInfo: Default::default(),
            videoInfo: Default::default(),
            recorderInfo: Default::default(),
        }
    }
}

pub type OH_AVScreenCaptureOnError =
    unsafe extern "C" fn(capture: *mut OH_AVScreenCapture, errorCode: c_int);
pub type OH_AVScreenCaptureOnAudioBufferAvailable = unsafe extern "C" fn(
    capture: *mut OH_AVScreenCapture,
    isReady: bool,
    type_: OH_AudioCaptureSourceType,
);
pub type OH_AVScreenCaptureOnVideoBufferAvailable =
    unsafe extern "C" fn(capture: *mut OH_AVScreenCapture, isReady: bool);

#[repr(C)]
pub struct OH_AVScreenCaptureCallback {
    pub onError: OH_AVScreenCaptureOnError,
    pub onAudioBufferAvailable: OH_AVScreenCaptureOnAudioBufferAvailable,
    pub onVideoBufferAvailable: OH_AVScreenCaptureOnVideoBufferAvailable,
}

pub type OH_AVScreenCapture_OnStateChange =
    unsafe extern "C" fn(capture: *mut OH_AVScreenCapture, stateCode: c_int, userData: *mut c_void);
pub type OH_AVScreenCapture_OnError =
    unsafe extern "C" fn(capture: *mut OH_AVScreenCapture, errorCode: c_int, userData: *mut c_void);

// OH_AVScreenCaptureStateCode (subset we react to)
pub const OH_SCREEN_CAPTURE_STATE_STARTED: c_int = 0;
pub const OH_SCREEN_CAPTURE_STATE_CANCELED: c_int = 1;
pub const OH_SCREEN_CAPTURE_STATE_STOPPED_BY_USER: c_int = 2;
pub const OH_SCREEN_CAPTURE_STATE_INTERRUPTED_BY_OTHER: c_int = 3;
pub const OH_SCREEN_CAPTURE_STATE_STOPPED_BY_CALL: c_int = 4;
pub const OH_SCREEN_CAPTURE_STATE_STOPPED_BY_USER_SWITCHES: c_int = 10;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OH_Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OH_AudioBuffer {
    pub buf: *mut u8,
    pub size: i32,
    pub timestamp: i64,
    pub type_: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OH_NativeBuffer_Config {
    pub width: i32,
    pub height: i32,
    pub format: i32,
    pub usage: i32,
    pub stride: i32,
}

#[link(name = "native_avscreen_capture")]
extern "C" {
    fn OH_AVScreenCapture_Create() -> *mut OH_AVScreenCapture;
    fn OH_AVScreenCapture_Init(
        capture: *mut OH_AVScreenCapture,
        config: OH_AVScreenCaptureConfig,
    ) -> c_int;
    fn OH_AVScreenCapture_StartScreenCapture(capture: *mut OH_AVScreenCapture) -> c_int;
    fn OH_AVScreenCapture_StopScreenCapture(capture: *mut OH_AVScreenCapture) -> c_int;
    fn OH_AVScreenCapture_SetCallback(
        capture: *mut OH_AVScreenCapture,
        callback: OH_AVScreenCaptureCallback,
    ) -> c_int;
    fn OH_AVScreenCapture_SetStateCallback(
        capture: *mut OH_AVScreenCapture,
        callback: OH_AVScreenCapture_OnStateChange,
        userData: *mut c_void,
    ) -> c_int;
    fn OH_AVScreenCapture_SetErrorCallback(
        capture: *mut OH_AVScreenCapture,
        callback: OH_AVScreenCapture_OnError,
        userData: *mut c_void,
    ) -> c_int;
    fn OH_AVScreenCapture_SetMicrophoneEnabled(
        capture: *mut OH_AVScreenCapture,
        enable: bool,
    ) -> c_int;
    fn OH_AVScreenCapture_AcquireAudioBuffer(
        capture: *mut OH_AVScreenCapture,
        audiobuffer: *mut *mut OH_AudioBuffer,
        type_: OH_AudioCaptureSourceType,
    ) -> c_int;
    fn OH_AVScreenCapture_ReleaseAudioBuffer(
        capture: *mut OH_AVScreenCapture,
        type_: OH_AudioCaptureSourceType,
    ) -> c_int;
    fn OH_AVScreenCapture_AcquireVideoBuffer(
        capture: *mut OH_AVScreenCapture,
        fence: *mut c_int,
        timestamp: *mut i64,
        region: *mut OH_Rect,
    ) -> *mut OH_NativeBuffer;
    fn OH_AVScreenCapture_ReleaseVideoBuffer(capture: *mut OH_AVScreenCapture) -> c_int;
    fn OH_AVScreenCapture_Release(capture: *mut OH_AVScreenCapture) -> c_int;
}

// API-12 data callback (used for audio on all devices and video where the
// system delivers it) plus the OH_AVBuffer accessors it hands out.
pub type OH_AVBuffer = c_void;

pub type OH_AVScreenCapture_OnBufferAvailable = unsafe extern "C" fn(
    capture: *mut OH_AVScreenCapture,
    buffer: *mut OH_AVBuffer,
    bufferType: c_int,
    timestamp: i64,
    userData: *mut c_void,
);

pub const OH_SCREEN_CAPTURE_BUFFERTYPE_VIDEO: c_int = 0;
pub const OH_SCREEN_CAPTURE_BUFFERTYPE_AUDIO_INNER: c_int = 1;
pub const OH_SCREEN_CAPTURE_BUFFERTYPE_AUDIO_MIC: c_int = 2;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OH_AVCodecBufferAttr {
    pub pts: i64,
    pub size: i32,
    pub offset: i32,
    // Same four-field layout as the native struct in ohcodec.rs; omitting
    // flags undersized the struct for OH_AVBuffer_GetBufferAttr.
    pub flags: u32,
}

#[link(name = "native_avscreen_capture")]
extern "C" {
    fn OH_AVScreenCapture_SetDataCallback(
        capture: *mut OH_AVScreenCapture,
        callback: OH_AVScreenCapture_OnBufferAvailable,
        userData: *mut c_void,
    ) -> c_int;
}

#[link(name = "native_media_core")]
extern "C" {
    fn OH_AVBuffer_GetAddr(buffer: *mut OH_AVBuffer) -> *mut u8;
    fn OH_AVBuffer_GetCapacity(buffer: *mut OH_AVBuffer) -> i32;
    fn OH_AVBuffer_GetBufferAttr(
        buffer: *mut OH_AVBuffer,
        attr: *mut OH_AVCodecBufferAttr,
    ) -> c_int;
}

#[link(name = "native_buffer")]
extern "C" {
    fn OH_NativeBuffer_Map(buffer: *mut OH_NativeBuffer, virAddr: *mut *mut c_void) -> c_int;
    fn OH_NativeBuffer_Unmap(buffer: *mut OH_NativeBuffer) -> c_int;
    fn OH_NativeBuffer_GetConfig(buffer: *mut OH_NativeBuffer, config: *mut OH_NativeBuffer_Config);
}

struct Capture {
    capture: *mut OH_AVScreenCapture,
    width: i32,
    height: i32,
}

unsafe impl Send for Capture {}

lazy_static! {
    static ref CAPTURE: Mutex<Option<Capture>> = Mutex::new(None);
    static ref ON_STATE_CHANGE: Mutex<Option<Box<dyn Fn(c_int) + Send>>> = Mutex::new(None);
    static ref RUNNING: AtomicBool = AtomicBool::new(false);
    static ref MIC_ENABLED: AtomicBool = AtomicBool::new(false);
    static ref VIDEO_READY: AtomicBool = AtomicBool::new(false);
    static ref AUDIO_INNER_READY: AtomicBool = AtomicBool::new(false);
    static ref AUDIO_MIC_READY: AtomicBool = AtomicBool::new(false);
    static ref VIDEO_SEEN: AtomicBool = AtomicBool::new(false);
}

/// Register a hook invoked with the raw OH_AVScreenCaptureStateCode whenever
/// the capture state changes (including user-cancel / system stop). The FFI
/// layer forwards it to the app UI.
pub fn set_on_state_change(f: Option<Box<dyn Fn(c_int) + Send>>) {
    *ON_STATE_CHANGE.lock().unwrap() = f;
}

fn notify_state(state: c_int) {
    if let Some(f) = ON_STATE_CHANGE.lock().unwrap().as_ref() {
        f(state);
    }
}

/// Start screen capture at the given size (the ArkTS side passes the real
/// display size). The system picker asks the user for consent; the state
/// callback reports whether it was granted.
pub fn start(width: i32, height: i32) -> Result<(), String> {
    let mut guard = CAPTURE.lock().unwrap();
    if guard.is_some() {
        return Ok(());
    }
    unsafe {
        let capture = OH_AVScreenCapture_Create();
        if capture.is_null() {
            return Err("OH_AVScreenCapture_Create failed".to_owned());
        }
        let err = OH_AVScreenCapture_SetCallback(
            capture,
            OH_AVScreenCaptureCallback {
                onError: on_error_legacy,
                onAudioBufferAvailable: on_audio_ready,
                onVideoBufferAvailable: on_video_ready,
            },
        );
        if err != AV_SCREEN_CAPTURE_ERR_OK {
            OH_AVScreenCapture_Release(capture);
            return Err(format!("SetCallback failed: {err}"));
        }
        let err = OH_AVScreenCapture_SetDataCallback(
            capture,
            on_buffer_available,
            std::ptr::null_mut(),
        );
        if err != AV_SCREEN_CAPTURE_ERR_OK {
            OH_AVScreenCapture_Release(capture);
            return Err(format!("SetDataCallback failed: {err}"));
        }
        let err =
            OH_AVScreenCapture_SetStateCallback(capture, on_state_change, std::ptr::null_mut());
        if err != AV_SCREEN_CAPTURE_ERR_OK {
            OH_AVScreenCapture_Release(capture);
            return Err(format!("SetStateCallback failed: {err}"));
        }
        let err = OH_AVScreenCapture_SetErrorCallback(capture, on_error, std::ptr::null_mut());
        if err != AV_SCREEN_CAPTURE_ERR_OK {
            OH_AVScreenCapture_Release(capture);
            return Err(format!("SetErrorCallback failed: {err}"));
        }
        let mut config = OH_AVScreenCaptureConfig::default();
        config.captureMode = OH_CAPTURE_HOME_SCREEN;
        config.dataType = OH_ORIGINAL_STREAM;
        config.audioInfo.innerCapInfo = OH_AudioCaptureInfo {
            audioSampleRate: 48000,
            audioChannels: 2,
            audioSource: OH_ALL_PLAYBACK,
        };
        config.audioInfo.micCapInfo = OH_AudioCaptureInfo {
            audioSampleRate: 48000,
            audioChannels: 2,
            audioSource: OH_MIC,
        };
        config.videoInfo.videoCapInfo = OH_VideoCaptureInfo {
            displayId: 0,
            missionIDs: std::ptr::null_mut(),
            missionIDsLen: 0,
            videoFrameWidth: width,
            videoFrameHeight: height,
            videoSource: OH_VIDEO_SOURCE_SURFACE_RGBA,
        };
        let err = OH_AVScreenCapture_Init(capture, config);
        if err != AV_SCREEN_CAPTURE_ERR_OK {
            OH_AVScreenCapture_Release(capture);
            return Err(format!("Init failed: {err}"));
        }
        *guard = Some(Capture {
            capture,
            width,
            height,
        });
    }
    drop(guard);
    RUNNING.store(true, Ordering::SeqCst);
    MIC_ENABLED.store(false, Ordering::SeqCst);
    VIDEO_READY.store(false, Ordering::SeqCst);
    AUDIO_INNER_READY.store(false, Ordering::SeqCst);
    AUDIO_MIC_READY.store(false, Ordering::SeqCst);
    VIDEO_SEEN.store(false, Ordering::SeqCst);
    ffi::set_video_raw_enable(true);
    ffi::set_audio_raw_enable(true);
    if std::thread::Builder::new()
        .name("avsc-pump".into())
        .spawn(pump_loop)
        .is_err()
    {
        // The pump would never run: release the handle so a later start()
        // can try again instead of finding a stuck capture.
        if let Some(c) = CAPTURE.lock().unwrap().take() {
            unsafe {
                OH_AVScreenCapture_Release(c.capture);
            }
        }
        return Err("failed to spawn the avsc-pump thread".to_string());
    }
    // StartScreenCapture pops the consent picker; the state callback reports
    // the outcome. Errors surface through the state/error callbacks.
    let start_err = unsafe {
        let ptr = CAPTURE.lock().unwrap().as_ref().map(|c| c.capture);
        match ptr {
            Some(ptr) => OH_AVScreenCapture_StartScreenCapture(ptr),
            None => -1,
        }
    };
    if start_err != AV_SCREEN_CAPTURE_ERR_OK {
        log::error!("StartScreenCapture failed: {start_err}");
        stop();
        return Err(format!("StartScreenCapture failed: {start_err}"));
    }
    Ok(())
}

/// Signal the pump thread to stop; it performs StopScreenCapture + Release so
/// the capture handle is never used after teardown.
pub fn stop() {
    if RUNNING.swap(false, Ordering::SeqCst) {
        // The pump tears down; wake it if it is parked on an audio wait.
        VIDEO_READY.store(true, Ordering::SeqCst);
    }
}

pub fn is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

/// Toggle microphone collection on the live capture (voice call in/out).
/// Returns false when no capture is running.
pub fn set_mic_enabled(enable: bool) -> bool {
    MIC_ENABLED.store(enable, Ordering::SeqCst);
    let guard = CAPTURE.lock().unwrap();
    match guard.as_ref() {
        Some(cap) => {
            let ok = unsafe { OH_AVScreenCapture_SetMicrophoneEnabled(cap.capture, enable) }
                == AV_SCREEN_CAPTURE_ERR_OK;
            if ok {
                log::info!("avscreen mic enabled: {enable}");
            } else {
                log::warn!("avscreen SetMicrophoneEnabled({enable}) failed");
            }
            ok
        }
        None => false,
    }
}

pub fn screen_size() -> (i32, i32) {
    match CAPTURE.lock().unwrap().as_ref() {
        Some(cap) => (cap.width, cap.height),
        None => (0, 0),
    }
}

// The pump owns the capture handle: it collects video (blocking Acquire, each
// frame at display rate) and audio (ready-flag driven), and after RUNNING goes
// false it is the only place that calls StopScreenCapture / Release.
fn pump_loop() {
    let capture = match CAPTURE.lock().unwrap().as_ref() {
        Some(c) => c.capture,
        None => return,
    };
    let mut packed: Vec<u8> = Vec::new();
    let mut video_frames: u64 = 0;
    let mut audio_frames: u64 = 0;
    let mut acquire_fails: u32 = 0;
    unsafe {
        while RUNNING.load(Ordering::SeqCst) {
            let mic = MIC_ENABLED.load(Ordering::SeqCst);
            if AUDIO_INNER_READY.swap(false, Ordering::SeqCst) {
                pump_audio(capture, OH_ALL_PLAYBACK, false, &mut audio_frames);
            }
            if mic && AUDIO_MIC_READY.swap(false, Ordering::SeqCst) {
                pump_audio(capture, OH_MIC, true, &mut audio_frames);
            }
            if VIDEO_SEEN.load(Ordering::SeqCst) {
                // The data callback delivers video; park briefly and keep the
                // loop responsive to RUNNING.
                std::thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }
            let mut fence: c_int = -1;
            let mut timestamp: i64 = 0;
            let mut region = OH_Rect::default();
            let nb =
                OH_AVScreenCapture_AcquireVideoBuffer(capture, &mut fence, &mut timestamp, &mut region);
            if nb.is_null() {
                // On images where the capture service cannot produce video
                // (the emulator), each failing Acquire costs ~1s of IPC; back
                // off instead of hammering it.
                acquire_fails += 1;
                let wait = if acquire_fails > 10 { 5000 } else { 500 };
                std::thread::sleep(std::time::Duration::from_millis(wait));
                continue;
            }
            acquire_fails = 0;
            let mut cfg = OH_NativeBuffer_Config::default();
            OH_NativeBuffer_GetConfig(nb, &mut cfg);
            let mut vir: *mut c_void = std::ptr::null_mut();
            if OH_NativeBuffer_Map(nb, &mut vir) == 0 && !vir.is_null() {
                let (w, h, stride) = (cfg.width, cfg.height, cfg.stride);
                if w > 0 && h > 0 && stride > 0 {
                    video_frames += 1;
                    if video_frames <= 3 || video_frames % 600 == 0 {
                        log::info!(
                            "avscreen video frame #{video_frames}: {w}x{h} stride={stride} region={}x{}",
                            region.width, region.height
                        );
                    }
                    {
                        let mut guard = CAPTURE.lock().unwrap();
                        if let Some(cap) = guard.as_mut() {
                            cap.width = w;
                            cap.height = h;
                        }
                    }
                    let row_bytes = w as usize * 4;
                    let src = vir as *const u8;
                    if stride == row_bytes as i32 {
                        let slice = std::slice::from_raw_parts(src, row_bytes * h as usize);
                        ffi::push_video_frame(slice);
                    } else {
                        // De-stride row by row into a packed buffer.
                        packed.clear();
                        packed.reserve(row_bytes * h as usize);
                        for row in 0..h as usize {
                            let srow = src.add(row * stride as usize);
                            packed.extend_from_slice(std::slice::from_raw_parts(srow, row_bytes));
                        }
                        ffi::push_video_frame(&packed);
                    }
                }
                OH_NativeBuffer_Unmap(nb);
            }
            if fence >= 0 {
                libc_close(fence);
            }
            OH_AVScreenCapture_ReleaseVideoBuffer(capture);
        }
        // Teardown on the pump thread only.
        OH_AVScreenCapture_StopScreenCapture(capture);
        OH_AVScreenCapture_Release(capture);
        *CAPTURE.lock().unwrap() = None;
    }
    ffi::set_video_raw_enable(false);
    ffi::set_audio_raw_enable(false);
    log::info!("avscreen capture pump exited");
    notify_state(-1);
}

unsafe fn pump_audio(
    capture: *mut OH_AVScreenCapture,
    source: OH_AudioCaptureSourceType,
    mic: bool,
    counter: &mut u64,
) {
    let mut ab: *mut OH_AudioBuffer = std::ptr::null_mut();
    let err = OH_AVScreenCapture_AcquireAudioBuffer(capture, &mut ab, source);
    if err != AV_SCREEN_CAPTURE_ERR_OK || ab.is_null() {
        if *counter < 3 {
            log::warn!("avscreen AcquireAudioBuffer({source}) failed: {err}");
        }
        return;
    }
    let buf = (*ab).buf;
    let size = (*ab).size;
    if !buf.is_null() && size > 0 {
        *counter += 1;
        if *counter <= 3 || *counter % 600 == 0 {
            log::info!("avscreen audio frame #{}: source={source} size={size}", *counter);
        }
        ffi::push_audio_frame(std::slice::from_raw_parts(buf, size as usize), mic);
    }
    OH_AVScreenCapture_ReleaseAudioBuffer(capture, source);
}

static VIDEO_BUFS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static AUDIO_BUFS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
// Scratch for de-striding video buffers delivered by the data callback; the
// callback must copy synchronously and is invoked from a single system thread.
static VIDEO_PACKED: std::sync::Mutex<Vec<u8>> = std::sync::Mutex::new(Vec::new());

// API-12 data callback: audio buffers (all images) and video buffers wherever
// the system delivers them. The buffer is valid only inside this callback; the
// ffi channels copy synchronously.
unsafe extern "C" fn on_buffer_available(
    _capture: *mut OH_AVScreenCapture,
    buffer: *mut OH_AVBuffer,
    buffer_type: c_int,
    _timestamp: i64,
    _userData: *mut c_void,
) {
    if buffer.is_null() {
        return;
    }
    let addr = OH_AVBuffer_GetAddr(buffer);
    if addr.is_null() {
        return;
    }
    let mut attr = OH_AVCodecBufferAttr::default();
    let attr_ok = OH_AVBuffer_GetBufferAttr(buffer, &mut attr) == 0 && attr.size > 0;
    let size = if attr_ok {
        attr.size as usize
    } else {
        OH_AVBuffer_GetCapacity(buffer).max(0) as usize
    };
    match buffer_type {
        OH_SCREEN_CAPTURE_BUFFERTYPE_VIDEO => {
            VIDEO_SEEN.store(true, Ordering::SeqCst);
            let (w, h) = {
                let guard = CAPTURE.lock().unwrap();
                match guard.as_ref() {
                    Some(cap) => (cap.width, cap.height),
                    None => (0, 0),
                }
            };
            let expected = w as usize * h as usize * 4;
            let n = VIDEO_BUFS.fetch_add(1, Ordering::SeqCst);
            if n < 3 || n % 600 == 0 {
                log::info!(
                    "avscreen video buf #{n}: size={size} attr_ok={attr_ok} expected={expected}"
                );
            }
            // The surface row stride is aligned up from the logical width
            // (e.g. 1256 -> 1280); derive it from the buffer size and
            // de-stride, else every row shears against the previous one.
            let row_bytes = w as usize * 4;
            let stride = if h > 0 && size % h as usize == 0 {
                size / h as usize
            } else {
                0
            };
            if stride == row_bytes && size >= expected {
                ffi::push_video_frame(std::slice::from_raw_parts(addr, expected));
            } else if stride > row_bytes && stride % 4 == 0 && stride * h as usize <= size {
                let mut packed = VIDEO_PACKED.lock().unwrap();
                packed.clear();
                packed.reserve(expected);
                for row in 0..h as usize {
                    let srow = addr.add(row * stride);
                    packed.extend_from_slice(std::slice::from_raw_parts(srow, row_bytes));
                }
                ffi::push_video_frame(&packed);
            } else if n < 3 {
                log::error!("avscreen video buf dropped: size={size} expected={expected}");
            }
        }
        OH_SCREEN_CAPTURE_BUFFERTYPE_AUDIO_INNER | OH_SCREEN_CAPTURE_BUFFERTYPE_AUDIO_MIC => {
            let n = AUDIO_BUFS.fetch_add(1, Ordering::SeqCst);
            if n < 3 || n % 600 == 0 {
                log::info!(
                    "avscreen audio buf #{n}: type={buffer_type} size={size} attr_ok={attr_ok}"
                );
            }
            if size > 0 {
                ffi::push_audio_frame(
                    std::slice::from_raw_parts(addr, size),
                    buffer_type == OH_SCREEN_CAPTURE_BUFFERTYPE_AUDIO_MIC,
                );
            }
        }
        _ => {}
    }
}

unsafe extern "C" fn on_video_ready(_capture: *mut OH_AVScreenCapture, is_ready: bool) {
    if is_ready {
        VIDEO_READY.store(true, Ordering::SeqCst);
    }
}

unsafe extern "C" fn on_audio_ready(
    _capture: *mut OH_AVScreenCapture,
    is_ready: bool,
    type_: OH_AudioCaptureSourceType,
) {
    if !is_ready {
        return;
    }
    if type_ == OH_MIC {
        AUDIO_MIC_READY.store(true, Ordering::SeqCst);
    } else {
        AUDIO_INNER_READY.store(true, Ordering::SeqCst);
    }
}

unsafe extern "C" fn on_error_legacy(_capture: *mut OH_AVScreenCapture, error_code: c_int) {
    log::error!("avscreen capture legacy error: {error_code}");
}

unsafe extern "C" fn on_state_change(
    _capture: *mut OH_AVScreenCapture,
    state_code: c_int,
    _userData: *mut c_void,
) {
    log::info!("avscreen capture state: {state_code}");
    match state_code {
        OH_SCREEN_CAPTURE_STATE_STARTED => notify_state(state_code),
        OH_SCREEN_CAPTURE_STATE_CANCELED
        | OH_SCREEN_CAPTURE_STATE_STOPPED_BY_USER
        | OH_SCREEN_CAPTURE_STATE_INTERRUPTED_BY_OTHER
        | OH_SCREEN_CAPTURE_STATE_STOPPED_BY_CALL
        | OH_SCREEN_CAPTURE_STATE_STOPPED_BY_USER_SWITCHES => {
            // Not on the callback thread: teardown happens on the pump.
            stop();
            notify_state(state_code);
        }
        // Transient notices leave the capture running: MIC_MUTED_BY_USER fires
        // from our own SetMicrophoneEnabled when the audio service starts (i.e.
        // right after a client connects), and forwarding it made the share UI
        // tear down its state while frames kept flowing.
        _ => {}
    }
}

unsafe extern "C" fn on_error(
    _capture: *mut OH_AVScreenCapture,
    error_code: c_int,
    _userData: *mut c_void,
) {
    log::error!("avscreen capture error: {error_code}");
}

extern "C" {
    #[link_name = "close"]
    fn libc_close(fd: c_int) -> c_int;
}
