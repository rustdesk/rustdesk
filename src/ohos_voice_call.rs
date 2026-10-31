// OpenHarmony microphone capture for voice calls: an OHAudio PCM capturer
// (AUDIOSTREAM_SOURCE_TYPE_VOICE_COMMUNICATION, f32) feeding either an opus
// encoder (controller side, `OhosVoiceCallCapturer`) or a raw PCM pool the
// audio service drains (controlled side, `OhosMicSource`); encoded/raw audio
// reaches the peer as AudioFrame messages.
//
// The captured stream is 48 kHz f32. Opus accepts 48 kHz natively; the
// controller sends the same AudioFrame messages a controlled peer produces.
// The controlled side uses this capturer instead of AVScreenCapture's mic
// stream because the latter requires an active screen share and delivers the
// mic at the device's own rate (a Pura 70 gave 2x the configured 48 kHz),
// which the audio service would encode as 48 kHz and the peer would play at
// double speed.

use crate::ohos_audio::*;
use hbb_common::{anyhow::anyhow, bail, log, ResultType};
use magnum_opus::{Application, Channels, Encoder as OpusEncoder};
use std::{ptr, sync::mpsc::Sender};

pub const VOICE_CALL_SAMPLE_RATE: u32 = 48000;

struct CapturerCtx {
    // Residual samples not yet a whole opus packet.
    pending: std::sync::Mutex<Vec<f32>>,
    encoder: std::sync::Mutex<OpusEncoder>,
    sender: Sender<base::message_proto::Message>,
    channels: u16,
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

// The capturer callback touches only the mutexed fields; the OHAudio raw
// pointer is never dereferenced on the Rust side.
unsafe impl Send for CapturerCtx {}
unsafe impl Sync for CapturerCtx {}

// 20 ms of 48 kHz audio per opus packet, matching the controlled-side packet
// cadence (AUDIO_PACKETS_PER_SECOND = 50).
const PACKET_MS: usize = 20;
const PACKET_SAMPLES: usize = VOICE_CALL_SAMPLE_RATE as usize * PACKET_MS / 1000;

// Under the default audio focus policy the VOICE_COMMUNICATION capturer pauses
// the app's own playback renderer, leaving the remote audio inaudible for the
// whole call. An activated session with MIX_WITH_OTHERS hands focus management
// of the app's streams back to the app, so capture and playback coexist.
fn activate_mix_session() {
    unsafe {
        let mut manager: *mut OH_AudioSessionManager = ptr::null_mut();
        if OH_AudioManager_GetAudioSessionManager(&mut manager) != AUDIOSTREAM_SUCCESS
            || manager.is_null()
        {
            log::warn!("failed to get audio session manager");
            return;
        }
        let strategy = OH_AudioSession_Strategy {
            concurrencyMode: CONCURRENCY_MIX_WITH_OTHERS,
        };
        if OH_AudioSessionManager_ActivateAudioSession(manager, &strategy) != AUDIOSTREAM_SUCCESS
        {
            log::warn!("failed to activate audio session");
        }
    }
}

fn deactivate_mix_session() {
    unsafe {
        let mut manager: *mut OH_AudioSessionManager = ptr::null_mut();
        if OH_AudioManager_GetAudioSessionManager(&mut manager) != AUDIOSTREAM_SUCCESS
            || manager.is_null()
        {
            return;
        }
        let _ = OH_AudioSessionManager_DeactivateAudioSession(manager);
    }
}

unsafe extern "C" fn on_read_data(
    _capturer: *mut OH_AudioCapturer,
    userData: *mut std::os::raw::c_void,
    buffer: *mut std::os::raw::c_void,
    length: i32,
) -> i32 {
    let ctx = match (userData as *const CapturerCtx).as_ref() {
        Some(ctx) => ctx,
        None => return 0,
    };
    if ctx.stopped.load(std::sync::atomic::Ordering::Acquire) {
        return 0;
    }
    if buffer.is_null() || length <= 0 {
        return 0;
    }
    let src = std::slice::from_raw_parts(buffer as *const f32, length as usize / 4);
    let mut pending = ctx.pending.lock().unwrap();
    pending.extend_from_slice(src);
    while pending.len() >= PACKET_SAMPLES * ctx.channels as usize {
        let packet_samples = PACKET_SAMPLES * ctx.channels as usize;
        let packet: Vec<f32> = pending.drain(..packet_samples).collect();
        let mut encoder = ctx.encoder.lock().unwrap();
        match encoder.encode_vec_float(&packet, packet.len() * 6) {
            Ok(data) if !data.is_empty() => {
                let mut msg = base::message_proto::Message::new();
                msg.set_audio_frame(base::message_proto::AudioFrame {
                    data: data.into(),
                    ..Default::default()
                });
                let _ = ctx.sender.send(msg);
            }
            Ok(_) => {}
            Err(error) => {
                hbb_common::throttled_log!(std::time::Duration::from_secs(1), warn, "Failed to encode voice call audio frame: {error:?}");
            }
        }
    }
    0
}

pub struct OhosVoiceCallCapturer {
    capturer: *mut OH_AudioCapturer,
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

// The raw capturer handle is only passed back into the OHAudio C API
// (stop/release); the capture callback only touches the Sync CapturerCtx.
unsafe impl Send for OhosVoiceCallCapturer {}

impl OhosVoiceCallCapturer {
    // Frames of captured + opus-encoded AudioFrame messages arrive on `tx`.
    pub fn new(tx: Sender<base::message_proto::Message>) -> ResultType<Self> {
        let channels: i32 = 1;
        activate_mix_session();
        unsafe {
            let mut builder: *mut OH_AudioStreamBuilder = ptr::null_mut();
            let rc = OH_AudioStreamBuilder_Create(&mut builder, AUDIOSTREAM_TYPE_CAPTURER);
            if rc != AUDIOSTREAM_SUCCESS || builder.is_null() {
                deactivate_mix_session();
                bail!("OH_AudioStreamBuilder_Create(capturer) failed: {}", rc);
            }
            let result = Self::build(builder, channels, tx);
            if result.is_err() {
                let _ = OH_AudioStreamBuilder_Destroy(builder);
                deactivate_mix_session();
            }
            result
        }
    }

    unsafe fn build(
        builder: *mut OH_AudioStreamBuilder,
        channels: i32,
        tx: Sender<base::message_proto::Message>,
    ) -> ResultType<Self> {
        if OH_AudioStreamBuilder_SetSamplingRate(builder, VOICE_CALL_SAMPLE_RATE as _)
            != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetChannelCount(builder, channels) != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetSampleFormat(builder, AUDIOSTREAM_SAMPLE_F32LE)
                != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetEncodingType(builder, AUDIOSTREAM_ENCODING_TYPE_RAW)
                != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetCapturerInfo(
                builder,
                AUDIOSTREAM_SOURCE_TYPE_VOICE_COMMUNICATION,
            ) != AUDIOSTREAM_SUCCESS
        {
            bail!("failed to configure OHAudio capturer");
        }
        let _ = OH_AudioStreamBuilder_SetFrameSizeInCallback(
            builder,
            (VOICE_CALL_SAMPLE_RATE * channels as u32 / 50) as i32,
        );
        let encoder = OpusEncoder::new(VOICE_CALL_SAMPLE_RATE, Channels::Mono, Application::Voip)
            .map_err(|e| anyhow!("failed to create opus encoder: {e:?}"))?;
        let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ctx = Box::into_raw(Box::new(CapturerCtx {
            pending: std::sync::Mutex::new(Vec::new()),
            encoder: std::sync::Mutex::new(encoder),
            sender: tx,
            channels: channels as u16,
            stopped: stopped.clone(),
        }));
        let callbacks = OH_AudioCapturer_Callbacks {
            OnReadData: Some(on_read_data),
            ..Default::default()
        };
        if OH_AudioStreamBuilder_SetCapturerCallback(builder, callbacks, ctx as *mut _)
            != AUDIOSTREAM_SUCCESS
        {
            drop(Box::from_raw(ctx));
            bail!("SetCapturerCallback failed");
        }
        let mut capturer: *mut OH_AudioCapturer = ptr::null_mut();
        if OH_AudioStreamBuilder_GenerateCapturer(builder, &mut capturer) != AUDIOSTREAM_SUCCESS
            || capturer.is_null()
        {
            drop(Box::from_raw(ctx));
            bail!("GenerateCapturer failed");
        }
        if OH_AudioCapturer_Start(capturer) != AUDIOSTREAM_SUCCESS {
            let _ = OH_AudioCapturer_Release(capturer);
            drop(Box::from_raw(ctx));
            bail!("OH_AudioCapturer_Start failed");
        }
        log::info!("ohos voice call capturer started");
        // The builder handle stays alive with the capturer after
        // GenerateCapturer succeeds; it is not freed separately.
        Ok(Self {
            capturer,
            stopped,
        })
    }

    pub fn stop(&mut self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Release);
        unsafe {
            if !self.capturer.is_null() {
                let _ = OH_AudioCapturer_Stop(self.capturer);
                let _ = OH_AudioCapturer_Release(self.capturer);
                self.capturer = ptr::null_mut();
            }
        }
        deactivate_mix_session();
    }
}

impl Drop for OhosVoiceCallCapturer {
    fn drop(&mut self) {
        self.stop();
    }
}

// One second of mono f32; when the consumer stalls past this, drop the oldest
// half so a resumed consumer gets fresh audio instead of a growing backlog.
const MIC_PENDING_MAX: usize = VOICE_CALL_SAMPLE_RATE as usize;

struct MicCtx {
    pending: std::sync::Arc<std::sync::Mutex<Vec<f32>>>,
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

// The capturer callback touches only the Arc'd fields; the OHAudio raw
// pointer is never dereferenced on the Rust side.
unsafe impl Send for MicCtx {}
unsafe impl Sync for MicCtx {}

unsafe extern "C" fn on_mic_read_data(
    _capturer: *mut OH_AudioCapturer,
    userData: *mut std::os::raw::c_void,
    buffer: *mut std::os::raw::c_void,
    length: i32,
) -> i32 {
    let ctx = match (userData as *const MicCtx).as_ref() {
        Some(ctx) => ctx,
        None => return 0,
    };
    if ctx.stopped.load(std::sync::atomic::Ordering::Acquire) {
        return 0;
    }
    if buffer.is_null() || length <= 0 {
        return 0;
    }
    let src = std::slice::from_raw_parts(buffer as *const f32, length as usize / 4);
    if let Ok(mut pending) = ctx.pending.lock() {
        if pending.len() > MIC_PENDING_MAX {
            let stale = pending.len() - MIC_PENDING_MAX / 2;
            pending.drain(..stale);
        }
        pending.extend_from_slice(src);
    }
    0
}

/// Controlled-side voice call microphone: a 48 kHz mono f32 OHAudio capturer
/// the audio service drains via [`take_pending`](Self::take_pending). Runs
/// independently of AVScreenCapture, so the call mic works without an active
/// screen share; the mix session keeps the app's own playback (the remote
/// voice) audible while the capturer holds voice-communication focus.
pub struct OhosMicSource {
    capturer: *mut OH_AudioCapturer,
    pending: std::sync::Arc<std::sync::Mutex<Vec<f32>>>,
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

// The raw capturer handle is only passed back into the OHAudio C API
// (stop/release); the capture callback only touches the Sync MicCtx fields.
unsafe impl Send for OhosMicSource {}

impl OhosMicSource {
    pub fn new() -> ResultType<Self> {
        activate_mix_session();
        unsafe {
            let mut builder: *mut OH_AudioStreamBuilder = ptr::null_mut();
            let rc = OH_AudioStreamBuilder_Create(&mut builder, AUDIOSTREAM_TYPE_CAPTURER);
            if rc != AUDIOSTREAM_SUCCESS || builder.is_null() {
                deactivate_mix_session();
                bail!("OH_AudioStreamBuilder_Create(mic capturer) failed: {}", rc);
            }
            let result = Self::build(builder);
            if result.is_err() {
                let _ = OH_AudioStreamBuilder_Destroy(builder);
                deactivate_mix_session();
            }
            result
        }
    }

    unsafe fn build(builder: *mut OH_AudioStreamBuilder) -> ResultType<Self> {
        if OH_AudioStreamBuilder_SetSamplingRate(builder, VOICE_CALL_SAMPLE_RATE as _)
            != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetChannelCount(builder, 1) != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetSampleFormat(builder, AUDIOSTREAM_SAMPLE_F32LE)
                != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetEncodingType(builder, AUDIOSTREAM_ENCODING_TYPE_RAW)
                != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetCapturerInfo(
                builder,
                AUDIOSTREAM_SOURCE_TYPE_VOICE_COMMUNICATION,
            ) != AUDIOSTREAM_SUCCESS
        {
            bail!("failed to configure OHAudio mic capturer");
        }
        let _ = OH_AudioStreamBuilder_SetFrameSizeInCallback(
            builder,
            (VOICE_CALL_SAMPLE_RATE / 50) as i32,
        );
        let pending = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let stopped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ctx = Box::into_raw(Box::new(MicCtx {
            pending: pending.clone(),
            stopped: stopped.clone(),
        }));
        let callbacks = OH_AudioCapturer_Callbacks {
            OnReadData: Some(on_mic_read_data),
            ..Default::default()
        };
        if OH_AudioStreamBuilder_SetCapturerCallback(builder, callbacks, ctx as *mut _)
            != AUDIOSTREAM_SUCCESS
        {
            drop(Box::from_raw(ctx));
            bail!("SetCapturerCallback failed");
        }
        let mut capturer: *mut OH_AudioCapturer = ptr::null_mut();
        if OH_AudioStreamBuilder_GenerateCapturer(builder, &mut capturer) != AUDIOSTREAM_SUCCESS
            || capturer.is_null()
        {
            drop(Box::from_raw(ctx));
            bail!("GenerateCapturer failed");
        }
        if OH_AudioCapturer_Start(capturer) != AUDIOSTREAM_SUCCESS {
            let _ = OH_AudioCapturer_Release(capturer);
            drop(Box::from_raw(ctx));
            bail!("OH_AudioCapturer_Start failed");
        }
        // The builder handle stays alive with the capturer after
        // GenerateCapturer succeeds; it is not freed separately. The ctx stays
        // leaked like the controller-side capturer's: the callback may still
        // run while stop() releases the stream, so it cannot be freed on the
        // Rust side.
        log::info!("ohos voice call mic source started");
        Ok(Self {
            capturer,
            pending,
            stopped,
        })
    }

    /// Move out the PCM accumulated since the last call (mono f32, 48 kHz).
    pub fn take_pending(&self, dst: &mut Vec<f32>) {
        if let Ok(mut pending) = self.pending.lock() {
            dst.append(&mut *pending);
        }
    }

    pub fn stop(&mut self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Release);
        unsafe {
            if !self.capturer.is_null() {
                let _ = OH_AudioCapturer_Stop(self.capturer);
                let _ = OH_AudioCapturer_Release(self.capturer);
                self.capturer = ptr::null_mut();
            }
        }
        deactivate_mix_session();
    }
}

impl Drop for OhosMicSource {
    fn drop(&mut self) {
        self.stop();
    }
}
