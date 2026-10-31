// OpenHarmony audio playback for the controller: an OHAudio PCM renderer
// (AUDIOSTREAM_SAMPLE_F32LE) pulling from the same AudioBuffer ring the cpal
// backends feed on other platforms. The decoded remote stream is opus -> f32
// interleaved; the renderer callback drains the ring buffer and plays silence
// on underrun.

use super::{AudioBuffer, AudioFormat};
use crate::ohos_audio::*;
use hbb_common::{anyhow, anyhow::Context, log, ResultType};
use std::ptr;

const DEVICE_RATE: u32 = 48000;

struct PlayerCtx {
    // Owns a clone of the Arc'd ring; sees the same PCM the handler pushes.
    buffer: AudioBuffer,
}

// The renderer callback only touches the Arc'd ring buffer inside
// AudioBuffer; the raw OH_AudioRenderer pointer is never dereferenced.
unsafe impl Send for PlayerCtx {}
unsafe impl Sync for PlayerCtx {}

unsafe extern "C" fn on_write_data(
    _renderer: *mut OH_AudioRenderer,
    userData: *mut std::os::raw::c_void,
    buffer: *mut std::os::raw::c_void,
    length: i32,
) -> i32 {
    if buffer.is_null() || length <= 0 {
        return AUDIO_DATA_CALLBACK_RESULT_VALID;
    }
    let ctx = &*(userData as *const PlayerCtx);
    let dst = std::slice::from_raw_parts_mut(buffer as *mut f32, length as usize / 4);
    use ringbuf::{ring_buffer::RbBase, Rb};
    let mut lock = ctx.buffer.0.lock().unwrap();
    let n = lock.occupied_len().min(dst.len());
    lock.pop_slice(&mut dst[..n]);
    dst[n..].fill(0.0);
    AUDIO_DATA_CALLBACK_RESULT_VALID
}

// Focus events land here when the system pauses/ducks this renderer (e.g. a
// capturer taking voice-communication focus); hint 2 = pause, 3 = stop.
unsafe extern "C" fn on_interrupt_event(
    _renderer: *mut OH_AudioRenderer,
    _userData: *mut std::os::raw::c_void,
    force_type: i32,
    hint: i32,
) -> i32 {
    log::warn!("ohos renderer interrupt: force={force_type} hint={hint}");
    AUDIO_DATA_CALLBACK_RESULT_VALID
}

pub struct OhosAudioPlayer {
    _renderer: *mut OH_AudioRenderer,
    // Handle to the ring the renderer callback drains; the handler pushes
    // decoded PCM through it. Field order matters: dropped after `_renderer`.
    buffer: AudioBuffer,
    sample_rate: u32,
    channels: u16,
    started: bool,
    // Native callback context; reclaimed in Drop once the renderer (and
    // with it the native callbacks) is gone.
    ctx: *mut PlayerCtx,
}

// The raw renderer handle is only passed back into the OHAudio C API from
// whichever thread drops the player; the ring shared with the callback is
// itself Sync.
unsafe impl Send for OhosAudioPlayer {}
unsafe impl Sync for OhosAudioPlayer {}

impl OhosAudioPlayer {
    pub fn new(format: &AudioFormat) -> ResultType<Self> {
        let channels: i32 = if format.channels > 1 { 2 } else { 1 };
        unsafe {
            let mut builder: *mut OH_AudioStreamBuilder = ptr::null_mut();
            let rc = OH_AudioStreamBuilder_Create(&mut builder, AUDIOSTREAM_TYPE_RENDERER);
            if rc != AUDIOSTREAM_SUCCESS || builder.is_null() {
                anyhow::bail!("OH_AudioStreamBuilder_Create(renderer) failed: {}", rc);
            }
            let result = Self::build(builder, channels);
            if result.is_err() {
                let _ = OH_AudioStreamBuilder_Destroy(builder);
            }
            result
        }
    }

    unsafe fn build(builder: *mut OH_AudioStreamBuilder, channels: i32) -> ResultType<Self> {
        if OH_AudioStreamBuilder_SetSamplingRate(builder, DEVICE_RATE as _) != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetChannelCount(builder, channels) != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetSampleFormat(builder, AUDIOSTREAM_SAMPLE_F32LE)
                != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetEncodingType(builder, AUDIOSTREAM_ENCODING_TYPE_RAW)
                != AUDIOSTREAM_SUCCESS
            || OH_AudioStreamBuilder_SetRendererInfo(builder, AUDIOSTREAM_USAGE_GAME)
                != AUDIOSTREAM_SUCCESS
        {
            anyhow::bail!("failed to configure OHAudio renderer");
        }
        // 20 ms callback granularity: low latency without starving under
        // scheduling jitter.
        let _ = OH_AudioStreamBuilder_SetFrameSizeInCallback(
            builder,
            (DEVICE_RATE * channels as u32 / 50) as i32,
        );
        // The handler appends into this ring; the callback drains the same
        // Arc'd ring through a shared clone. Ring sized at the device rate.
        let mut buffer = AudioBuffer::default();
        buffer.resize(DEVICE_RATE as usize, channels as usize);
        let ctx = Box::into_raw(Box::new(PlayerCtx {
            buffer: buffer.clone_shared(),
        }));
        let callbacks = OH_AudioRenderer_Callbacks {
            OnWriteData: Some(on_write_data),
            OnInterruptEvent: Some(on_interrupt_event),
            ..Default::default()
        };
        if OH_AudioStreamBuilder_SetRendererCallback(builder, callbacks, ctx as *mut _)
            != AUDIOSTREAM_SUCCESS
        {
            drop(Box::from_raw(ctx));
            anyhow::bail!("SetRendererCallback failed");
        }
        let mut renderer: *mut OH_AudioRenderer = ptr::null_mut();
        if OH_AudioStreamBuilder_GenerateRenderer(builder, &mut renderer) != AUDIOSTREAM_SUCCESS
            || renderer.is_null()
        {
            drop(Box::from_raw(ctx));
            anyhow::bail!("GenerateRenderer failed");
        }
        // The builder handle stays alive with the renderer after
        // GenerateRenderer succeeds on OHOS; it is not freed separately.
        if OH_AudioRenderer_Start(renderer) != AUDIOSTREAM_SUCCESS {
            let _ = OH_AudioRenderer_Release(renderer);
            drop(Box::from_raw(ctx));
            anyhow::bail!("OH_AudioRenderer_Start failed");
        }
        log::info!("ohos audio renderer started: {} Hz, {} ch", DEVICE_RATE, channels);
        Ok(Self {
            _renderer: renderer,
            buffer,
            sample_rate: DEVICE_RATE,
            channels: channels as u16,
            started: true,
            ctx,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn buffer_mut(&mut self) -> &mut AudioBuffer {
        &mut self.buffer
    }

    pub fn started(&self) -> bool {
        self.started
    }
}

impl Drop for OhosAudioPlayer {
    fn drop(&mut self) {
        unsafe {
            if !self._renderer.is_null() {
                let _ = OH_AudioRenderer_Stop(self._renderer);
                let _ = OH_AudioRenderer_Release(self._renderer);
                self._renderer = ptr::null_mut();
            }
            // Release has returned, so no native callback can touch ctx.
            if !self.ctx.is_null() {
                drop(Box::from_raw(self.ctx));
                self.ctx = ptr::null_mut();
            }
        }
    }
}
