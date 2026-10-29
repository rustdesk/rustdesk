// OpenHarmony OHAudio (libohaudio.so) C API bindings, hand-written because the
// sysroot headers are not bindgen-friendly for the whole surface and only a
// small slice is needed: a PCM renderer (remote audio playback) and a PCM
// capturer (voice call microphone input).
//
// Signatures mirror the NDK headers (ohaudio/native_audiostream{,_builder}.h,
// native_audiorenderer.h, native_audiocapturer.h).

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use std::os::raw::{c_int, c_void};

type c_int32 = i32;

pub type OH_AudioStreamBuilder = c_void;
pub type OH_AudioRenderer = c_void;
pub type OH_AudioCapturer = c_void;
pub type OH_AudioSessionManager = c_void;

pub const AUDIOSTREAM_TYPE_RENDERER: c_int = 1;
pub const AUDIOSTREAM_TYPE_CAPTURER: c_int = 2;

pub const AUDIOSTREAM_SUCCESS: c_int = 0;
pub const AUDIOSTREAM_ERROR_INVALID_PARAM: c_int = 1;
pub const AUDIOSTREAM_ERROR_ILLEGAL_STATE: c_int = 2;

pub const AUDIOSTREAM_SAMPLE_U8: c_int = 0;
pub const AUDIOSTREAM_SAMPLE_S16LE: c_int = 1;
pub const AUDIOSTREAM_SAMPLE_S24LE: c_int = 2;
pub const AUDIOSTREAM_SAMPLE_S32LE: c_int = 3;
pub const AUDIOSTREAM_SAMPLE_F32LE: c_int = 4;

pub const AUDIOSTREAM_ENCODING_TYPE_RAW: c_int = 0;

pub const AUDIOSTREAM_USAGE_MUSIC: c_int = 1;
pub const AUDIOSTREAM_USAGE_VOICE_COMMUNICATION: c_int = 2;
pub const AUDIOSTREAM_USAGE_GAME: c_int = 11;
pub const AUDIOSTREAM_USAGE_MOVIE: c_int = 10;

pub const AUDIOSTREAM_SOURCE_TYPE_MIC: c_int = 0;
pub const AUDIOSTREAM_SOURCE_TYPE_VOICE_COMMUNICATION: c_int = 7;

pub const CH_LAYOUT_MONO: u64 = 0x4;
pub const CH_LAYOUT_STEREO: u64 = 0x3;

// AUDIO_DATA_CALLBACK_RESULT_VALID; the write callback returns this to accept
// the buffer it just filled.
pub const AUDIO_DATA_CALLBACK_RESULT_VALID: c_int = 0;

// OH_AudioSession_ConcurrencyMode; MIX_WITH_OTHERS keeps the app's own
// renderer playing while its voice-communication capturer holds focus.
pub const CONCURRENCY_MIX_WITH_OTHERS: c_int = 1;

#[repr(C)]
pub struct OH_AudioSession_Strategy {
    pub concurrencyMode: c_int,
}

#[repr(C)]
#[derive(Default)]
pub struct OH_AudioRenderer_Callbacks {
    pub OnWriteData: Option<
        unsafe extern "C" fn(
            renderer: *mut OH_AudioRenderer,
            userData: *mut c_void,
            buffer: *mut c_void,
            length: c_int32,
        ) -> c_int32,
    >,
    pub OnStreamEvent: Option<
        unsafe extern "C" fn(
            renderer: *mut OH_AudioRenderer,
            userData: *mut c_void,
            event: c_int,
        ) -> c_int32,
    >,
    pub OnInterruptEvent: Option<
        unsafe extern "C" fn(
            renderer: *mut OH_AudioRenderer,
            userData: *mut c_void,
            forceType: c_int,
            hint: c_int,
        ) -> c_int32,
    >,
    pub OnError: Option<
        unsafe extern "C" fn(
            renderer: *mut OH_AudioRenderer,
            userData: *mut c_void,
            error: c_int,
        ) -> c_int32,
    >,
}

#[repr(C)]
#[derive(Default)]
pub struct OH_AudioCapturer_Callbacks {
    pub OnReadData: Option<
        unsafe extern "C" fn(
            capturer: *mut OH_AudioCapturer,
            userData: *mut c_void,
            buffer: *mut c_void,
            length: c_int32,
        ) -> c_int32,
    >,
    pub OnStreamEvent: Option<
        unsafe extern "C" fn(
            capturer: *mut OH_AudioCapturer,
            userData: *mut c_void,
            event: c_int,
        ) -> c_int32,
    >,
    pub OnInterruptEvent: Option<
        unsafe extern "C" fn(
            capturer: *mut OH_AudioCapturer,
            userData: *mut c_void,
            forceType: c_int,
            hint: c_int,
        ) -> c_int32,
    >,
    pub OnError: Option<
        unsafe extern "C" fn(
            capturer: *mut OH_AudioCapturer,
            userData: *mut c_void,
            error: c_int,
        ) -> c_int32,
    >,
}

#[link(name = "ohaudio")]
extern "C" {
    pub fn OH_AudioStreamBuilder_Create(
        builder: *mut *mut OH_AudioStreamBuilder,
        ty: c_int,
    ) -> c_int;
    pub fn OH_AudioStreamBuilder_Destroy(builder: *mut OH_AudioStreamBuilder) -> c_int;
    pub fn OH_AudioStreamBuilder_SetSamplingRate(builder: *mut OH_AudioStreamBuilder, rate: c_int32) -> c_int;
    pub fn OH_AudioStreamBuilder_SetChannelCount(builder: *mut OH_AudioStreamBuilder, count: c_int32) -> c_int;
    pub fn OH_AudioStreamBuilder_SetSampleFormat(builder: *mut OH_AudioStreamBuilder, format: c_int) -> c_int;
    pub fn OH_AudioStreamBuilder_SetEncodingType(builder: *mut OH_AudioStreamBuilder, encodingType: c_int) -> c_int;
    pub fn OH_AudioStreamBuilder_SetRendererInfo(builder: *mut OH_AudioStreamBuilder, usage: c_int) -> c_int;
    pub fn OH_AudioStreamBuilder_SetCapturerInfo(builder: *mut OH_AudioStreamBuilder, sourceType: c_int) -> c_int;
    pub fn OH_AudioStreamBuilder_SetFrameSizeInCallback(builder: *mut OH_AudioStreamBuilder, frameSize: c_int32) -> c_int;
    pub fn OH_AudioStreamBuilder_SetRendererCallback(
        builder: *mut OH_AudioStreamBuilder,
        callbacks: OH_AudioRenderer_Callbacks,
        userData: *mut c_void,
    ) -> c_int;
    pub fn OH_AudioStreamBuilder_SetCapturerCallback(
        builder: *mut OH_AudioStreamBuilder,
        callbacks: OH_AudioCapturer_Callbacks,
        userData: *mut c_void,
    ) -> c_int;
    pub fn OH_AudioStreamBuilder_GenerateRenderer(
        builder: *mut OH_AudioStreamBuilder,
        renderer: *mut *mut OH_AudioRenderer,
    ) -> c_int;
    pub fn OH_AudioStreamBuilder_GenerateCapturer(
        builder: *mut OH_AudioStreamBuilder,
        capturer: *mut *mut OH_AudioCapturer,
    ) -> c_int;

    pub fn OH_AudioRenderer_Start(renderer: *mut OH_AudioRenderer) -> c_int;
    pub fn OH_AudioRenderer_Pause(renderer: *mut OH_AudioRenderer) -> c_int;
    pub fn OH_AudioRenderer_Stop(renderer: *mut OH_AudioRenderer) -> c_int;
    pub fn OH_AudioRenderer_Flush(renderer: *mut OH_AudioRenderer) -> c_int;
    pub fn OH_AudioRenderer_Release(renderer: *mut OH_AudioRenderer) -> c_int;
    pub fn OH_AudioRenderer_GetFrameSizeInCallback(renderer: *mut OH_AudioRenderer, frameSize: *mut c_int32) -> c_int;

    pub fn OH_AudioCapturer_Start(capturer: *mut OH_AudioCapturer) -> c_int;
    pub fn OH_AudioCapturer_Pause(capturer: *mut OH_AudioCapturer) -> c_int;
    pub fn OH_AudioCapturer_Stop(capturer: *mut OH_AudioCapturer) -> c_int;
    pub fn OH_AudioCapturer_Flush(capturer: *mut OH_AudioCapturer) -> c_int;
    pub fn OH_AudioCapturer_Release(capturer: *mut OH_AudioCapturer) -> c_int;
    pub fn OH_AudioCapturer_GetFrameSizeInCallback(capturer: *mut OH_AudioCapturer, frameSize: *mut c_int32) -> c_int;

    pub fn OH_AudioManager_GetAudioSessionManager(
        manager: *mut *mut OH_AudioSessionManager,
    ) -> c_int;
    pub fn OH_AudioSessionManager_ActivateAudioSession(
        manager: *mut OH_AudioSessionManager,
        strategy: *const OH_AudioSession_Strategy,
    ) -> c_int;
    pub fn OH_AudioSessionManager_DeactivateAudioSession(manager: *mut OH_AudioSessionManager) -> c_int;
}
