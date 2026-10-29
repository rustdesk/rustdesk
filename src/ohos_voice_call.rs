// OpenHarmony microphone capture for controller-side voice calls: an OHAudio
// PCM capturer (AUDIOSTREAM_SOURCE_TYPE_MIC, f32 mono/stereo) feeding an opus
// encoder; encoded AudioFrame messages are handed to the session sender.
//
// The captured stream is 48 kHz f32. Opus accepts 48 kHz natively; the
// controller sends the same AudioFrame messages a controlled peer produces.

use crate::ohos_audio::*;
use hbb_common::{anyhow::anyhow, bail, log, ResultType};
use magnum_opus::{Application, Channels, Encoder as OpusEncoder};
use std::ptr;
use std::sync::mpsc::Sender;

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
                log::warn!("Failed to encode voice call audio frame: {error:?}");
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
