use std::{
    collections::{HashMap, VecDeque},
    ffi::c_void,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

use hbb_common::log;

const AUDIOSTREAM_SUCCESS: i32 = 0;
const AUDIOSTREAM_TYPE_RENDERER: i32 = 1;
const AUDIOSTREAM_SAMPLE_F32LE: i32 = 4;
const AUDIOSTREAM_ENCODING_TYPE_RAW: i32 = 0;
const AUDIOSTREAM_USAGE_MOVIE: i32 = 10;
const AUDIO_DATA_CALLBACK_RESULT_INVALID: i32 = -1;
const AUDIO_DATA_CALLBACK_RESULT_VALID: i32 = 0;
const PCM_BUFFER_SECONDS: usize = 1;

#[repr(C)]
pub struct OH_AudioStreamBuilder {
    _private: [u8; 0],
}

#[repr(C)]
pub struct OH_AudioRenderer {
    _private: [u8; 0],
}

type RendererWriteCallback =
    Option<unsafe extern "C" fn(*mut OH_AudioRenderer, *mut c_void, *mut c_void, i32) -> i32>;

#[link(name = "ohaudio")]
unsafe extern "C" {
    fn OH_AudioStreamBuilder_Create(builder: *mut *mut OH_AudioStreamBuilder, kind: i32) -> i32;
    fn OH_AudioStreamBuilder_Destroy(builder: *mut OH_AudioStreamBuilder) -> i32;
    fn OH_AudioStreamBuilder_SetSamplingRate(builder: *mut OH_AudioStreamBuilder, rate: i32)
        -> i32;
    fn OH_AudioStreamBuilder_SetChannelCount(
        builder: *mut OH_AudioStreamBuilder,
        channels: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetSampleFormat(
        builder: *mut OH_AudioStreamBuilder,
        format: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetEncodingType(
        builder: *mut OH_AudioStreamBuilder,
        encoding: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetRendererInfo(
        builder: *mut OH_AudioStreamBuilder,
        usage: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetRendererWriteDataCallback(
        builder: *mut OH_AudioStreamBuilder,
        callback: RendererWriteCallback,
        user_data: *mut c_void,
    ) -> i32;
    fn OH_AudioStreamBuilder_GenerateRenderer(
        builder: *mut OH_AudioStreamBuilder,
        renderer: *mut *mut OH_AudioRenderer,
    ) -> i32;
    fn OH_AudioRenderer_Start(renderer: *mut OH_AudioRenderer) -> i32;
    fn OH_AudioRenderer_Stop(renderer: *mut OH_AudioRenderer) -> i32;
    fn OH_AudioRenderer_Flush(renderer: *mut OH_AudioRenderer) -> i32;
    fn OH_AudioRenderer_Release(renderer: *mut OH_AudioRenderer) -> i32;
}

#[derive(Clone)]
pub struct OhosAudioStatus {
    pub available: bool,
    pub renderer_active: bool,
    pub error_text: String,
}

impl Default for OhosAudioStatus {
    fn default() -> Self {
        Self {
            available: false,
            renderer_active: false,
            error_text: String::new(),
        }
    }
}

#[derive(Default)]
struct PcmQueue {
    samples: VecDeque<f32>,
    capacity: usize,
}

impl PcmQueue {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, pcm: &[f32]) {
        if self.capacity == 0 {
            return;
        }
        let start = pcm.len().saturating_sub(self.capacity);
        let needed = self
            .samples
            .len()
            .saturating_add(pcm.len().saturating_sub(start));
        let overflow = needed.saturating_sub(self.capacity);
        for _ in 0..overflow {
            self.samples.pop_front();
        }
        for sample in &pcm[start..] {
            self.samples.push_back(*sample);
        }
    }

    fn fill_exact(&mut self, output: &mut [f32]) -> bool {
        if self.samples.len() < output.len() {
            return false;
        }
        for sample in output {
            if let Some(value) = self.samples.pop_front() {
                *sample = value;
            }
        }
        true
    }

    fn clear(&mut self) {
        self.samples.clear();
    }
}

fn status_store() -> &'static Mutex<HashMap<String, OhosAudioStatus>> {
    static STORE: OnceLock<Mutex<HashMap<String, OhosAudioStatus>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn status(session_id: &str) -> OhosAudioStatus {
    status_store()
        .lock()
        .unwrap()
        .get(session_id)
        .cloned()
        .unwrap_or_default()
}

pub fn clear_status(session_id: &str) {
    status_store().lock().unwrap().remove(session_id);
}

fn mark_active(session_id: &str) {
    status_store().lock().unwrap().insert(
        session_id.to_string(),
        OhosAudioStatus {
            available: true,
            renderer_active: true,
            error_text: String::new(),
        },
    );
}

fn mark_inactive(session_id: &str) {
    status_store().lock().unwrap().insert(
        session_id.to_string(),
        OhosAudioStatus {
            available: false,
            renderer_active: false,
            error_text: String::new(),
        },
    );
}

fn mark_error(session_id: &str, error_text: String) {
    status_store().lock().unwrap().insert(
        session_id.to_string(),
        OhosAudioStatus {
            available: false,
            renderer_active: false,
            error_text,
        },
    );
}

struct CallbackContext {
    active: AtomicBool,
    queue: Arc<Mutex<PcmQueue>>,
}

// A failed Release cannot prove that native code has stopped using userData.
// Keep only those exceptional contexts alive to avoid a use-after-free.
fn quarantined_callback_contexts() -> &'static Mutex<Vec<Box<CallbackContext>>> {
    static CONTEXTS: OnceLock<Mutex<Vec<Box<CallbackContext>>>> = OnceLock::new();
    CONTEXTS.get_or_init(|| Mutex::new(Vec::new()))
}

unsafe extern "C" fn write_pcm(
    _renderer: *mut OH_AudioRenderer,
    user_data: *mut c_void,
    audio_data: *mut c_void,
    audio_data_size: i32,
) -> i32 {
    if user_data.is_null() || audio_data.is_null() || audio_data_size <= 0 {
        return AUDIO_DATA_CALLBACK_RESULT_INVALID;
    }
    let byte_len = audio_data_size as usize;
    if byte_len % std::mem::size_of::<f32>() != 0 {
        return AUDIO_DATA_CALLBACK_RESULT_INVALID;
    }
    let context = unsafe { &*(user_data as *const CallbackContext) };
    if !context.active.load(Ordering::Acquire) {
        return AUDIO_DATA_CALLBACK_RESULT_INVALID;
    }
    let Ok(mut queue) = context.queue.try_lock() else {
        return AUDIO_DATA_CALLBACK_RESULT_INVALID;
    };
    if !context.active.load(Ordering::Acquire) {
        return AUDIO_DATA_CALLBACK_RESULT_INVALID;
    }
    let output = unsafe {
        std::slice::from_raw_parts_mut(
            audio_data as *mut f32,
            byte_len / std::mem::size_of::<f32>(),
        )
    };
    if queue.fill_exact(output) {
        AUDIO_DATA_CALLBACK_RESULT_VALID
    } else {
        AUDIO_DATA_CALLBACK_RESULT_INVALID
    }
}

pub struct OhosAudioOutput {
    session_id: String,
    queue: Arc<Mutex<PcmQueue>>,
    renderer: *mut OH_AudioRenderer,
    callback_context: *mut CallbackContext,
}

// The renderer handle is only accessed by the audio worker thread. The OHAudio
// callback receives only the separately owned callback context.
unsafe impl Send for OhosAudioOutput {}

impl OhosAudioOutput {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            queue: Arc::new(Mutex::new(PcmQueue::with_capacity(
                48_000 * 2 * PCM_BUFFER_SECONDS,
            ))),
            renderer: ptr::null_mut(),
            callback_context: ptr::null_mut(),
        }
    }

    pub fn configure(&mut self, sample_rate: u32, channels: u16) -> Result<(), String> {
        if sample_rate == 0 || channels == 0 || channels > 2 {
            let message = "Unsupported remote audio format".to_string();
            mark_error(&self.session_id, message.clone());
            return Err(message);
        }
        self.release_renderer();
        let capacity = (sample_rate as usize)
            .saturating_mul(channels as usize)
            .saturating_mul(PCM_BUFFER_SECONDS);
        self.queue = Arc::new(Mutex::new(PcmQueue::with_capacity(capacity)));

        let mut builder: *mut OH_AudioStreamBuilder = ptr::null_mut();
        let mut renderer: *mut OH_AudioRenderer = ptr::null_mut();
        let result =
            unsafe { OH_AudioStreamBuilder_Create(&mut builder, AUDIOSTREAM_TYPE_RENDERER) };
        if result != AUDIOSTREAM_SUCCESS || builder.is_null() {
            return self.fail("Unable to create OHAudio renderer builder");
        }

        let setup_result =
            unsafe { OH_AudioStreamBuilder_SetSamplingRate(builder, sample_rate as i32) };
        if setup_result != AUDIOSTREAM_SUCCESS {
            unsafe {
                OH_AudioStreamBuilder_Destroy(builder);
            }
            return self.fail("Unable to configure OHAudio renderer sample rate");
        }
        let setup_result =
            unsafe { OH_AudioStreamBuilder_SetChannelCount(builder, channels as i32) };
        if setup_result != AUDIOSTREAM_SUCCESS {
            unsafe {
                OH_AudioStreamBuilder_Destroy(builder);
            }
            return self.fail("Unable to configure OHAudio renderer channel count");
        }
        let setup_result =
            unsafe { OH_AudioStreamBuilder_SetSampleFormat(builder, AUDIOSTREAM_SAMPLE_F32LE) };
        if setup_result != AUDIOSTREAM_SUCCESS {
            unsafe {
                OH_AudioStreamBuilder_Destroy(builder);
            }
            return self.fail("Unable to configure OHAudio renderer sample format");
        }
        let setup_result = unsafe {
            OH_AudioStreamBuilder_SetEncodingType(builder, AUDIOSTREAM_ENCODING_TYPE_RAW)
        };
        if setup_result != AUDIOSTREAM_SUCCESS {
            unsafe {
                OH_AudioStreamBuilder_Destroy(builder);
            }
            return self.fail("Unable to configure OHAudio renderer encoding");
        }
        let setup_result =
            unsafe { OH_AudioStreamBuilder_SetRendererInfo(builder, AUDIOSTREAM_USAGE_MOVIE) };
        if setup_result != AUDIOSTREAM_SUCCESS {
            unsafe {
                OH_AudioStreamBuilder_Destroy(builder);
            }
            return self.fail("Unable to configure OHAudio renderer usage");
        }

        let callback_context = Box::into_raw(Box::new(CallbackContext {
            active: AtomicBool::new(true),
            queue: self.queue.clone(),
        }));
        let callback_result = unsafe {
            OH_AudioStreamBuilder_SetRendererWriteDataCallback(
                builder,
                Some(write_pcm),
                callback_context.cast::<c_void>(),
            )
        };
        if callback_result != AUDIOSTREAM_SUCCESS {
            Self::drop_callback_context(callback_context);
            unsafe {
                OH_AudioStreamBuilder_Destroy(builder);
            }
            return self.fail("Unable to register OHAudio callback");
        }
        let renderer_result =
            unsafe { OH_AudioStreamBuilder_GenerateRenderer(builder, &mut renderer) };
        unsafe {
            OH_AudioStreamBuilder_Destroy(builder);
        }
        if renderer_result != AUDIOSTREAM_SUCCESS || renderer.is_null() {
            if renderer.is_null() {
                Self::drop_callback_context(callback_context);
            } else {
                Self::deactivate_callback_context(callback_context);
                let release_result = Self::release_native_renderer(renderer);
                Self::finish_callback_context(callback_context, release_result);
            }
            return self.fail("Unable to create OHAudio renderer");
        }
        let start_result = unsafe { OH_AudioRenderer_Start(renderer) };
        if start_result != AUDIOSTREAM_SUCCESS {
            Self::deactivate_callback_context(callback_context);
            let release_result = Self::release_native_renderer(renderer);
            Self::finish_callback_context(callback_context, release_result);
            return self.fail("Unable to start OHAudio renderer");
        }
        self.renderer = renderer;
        self.callback_context = callback_context;
        mark_active(&self.session_id);
        Ok(())
    }

    pub fn enqueue(&self, pcm: &[f32]) {
        if self.renderer.is_null() || pcm.is_empty() {
            return;
        }
        self.queue.lock().unwrap().push(pcm);
    }

    pub fn fail(&mut self, message: &str) -> Result<(), String> {
        self.release_renderer();
        let error = message.to_string();
        mark_error(&self.session_id, error.clone());
        Err(error)
    }

    fn deactivate_callback_context(callback_context: *mut CallbackContext) {
        if callback_context.is_null() {
            return;
        }
        unsafe {
            (*callback_context).active.store(false, Ordering::Release);
        }
    }

    fn drop_callback_context(callback_context: *mut CallbackContext) {
        if callback_context.is_null() {
            return;
        }
        Self::deactivate_callback_context(callback_context);
        unsafe {
            drop(Box::from_raw(callback_context));
        }
    }

    fn finish_callback_context(callback_context: *mut CallbackContext, release_result: i32) {
        if callback_context.is_null() {
            return;
        }
        if release_result == AUDIOSTREAM_SUCCESS {
            Self::drop_callback_context(callback_context);
        } else {
            log::error!(
                "OH_AudioRenderer_Release failed: {}; retaining callback context",
                release_result
            );
            Self::deactivate_callback_context(callback_context);
            unsafe {
                quarantined_callback_contexts()
                    .lock()
                    .unwrap()
                    .push(Box::from_raw(callback_context));
            }
        }
    }

    fn release_native_renderer(renderer: *mut OH_AudioRenderer) -> i32 {
        if renderer.is_null() {
            return AUDIOSTREAM_SUCCESS;
        }
        let stop_result = unsafe { OH_AudioRenderer_Stop(renderer) };
        if stop_result != AUDIOSTREAM_SUCCESS {
            log::warn!("OH_AudioRenderer_Stop failed: {}", stop_result);
        }
        let flush_result = unsafe { OH_AudioRenderer_Flush(renderer) };
        if flush_result != AUDIOSTREAM_SUCCESS {
            log::warn!("OH_AudioRenderer_Flush failed: {}", flush_result);
        }
        unsafe { OH_AudioRenderer_Release(renderer) }
    }

    fn release_renderer(&mut self) {
        let was_active = !self.renderer.is_null() || !self.callback_context.is_null();
        let callback_context = std::mem::replace(&mut self.callback_context, ptr::null_mut());
        Self::deactivate_callback_context(callback_context);
        self.queue.lock().unwrap().clear();
        let renderer = std::mem::replace(&mut self.renderer, ptr::null_mut());
        if renderer.is_null() {
            Self::drop_callback_context(callback_context);
        } else {
            let release_result = Self::release_native_renderer(renderer);
            Self::finish_callback_context(callback_context, release_result);
        }
        if was_active {
            mark_inactive(&self.session_id);
        }
    }
}

impl Drop for OhosAudioOutput {
    fn drop(&mut self) {
        self.release_renderer();
        clear_status(&self.session_id);
    }
}
