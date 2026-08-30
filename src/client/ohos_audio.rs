use std::{
    collections::{HashMap, VecDeque},
    ffi::c_void,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, MutexGuard, OnceLock, TryLockError,
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
static AUDIO_LOCK_POISON_REPORTED: AtomicBool = AtomicBool::new(false);

fn lock_audio_state<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            if !AUDIO_LOCK_POISON_REPORTED.swap(true, Ordering::Relaxed) {
                log::error!("recovering poisoned OHOS audio state mutex");
            }
            poisoned.into_inner()
        }
    }
}

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

    fn fill_available(&mut self, output: &mut [f32]) -> bool {
        for sample in output.iter_mut() {
            *sample = self.samples.pop_front().unwrap_or(0.0);
        }
        true
    }

    fn clear(&mut self) {
        self.samples.clear();
    }
}

struct CallbackContext {
    active: AtomicBool,
    queue: Arc<Mutex<PcmQueue>>,
}

fn callback_contexts() -> &'static Mutex<HashMap<usize, Arc<CallbackContext>>> {
    static CONTEXTS: OnceLock<Mutex<HashMap<usize, Arc<CallbackContext>>>> = OnceLock::new();
    CONTEXTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_callback_context(queue: Arc<Mutex<PcmQueue>>) -> usize {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
    let context = Arc::new(CallbackContext {
        active: AtomicBool::new(true),
        queue,
    });
    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id == 0 {
            continue;
        }
        let mut contexts = lock_audio_state(callback_contexts());
        if let std::collections::hash_map::Entry::Vacant(entry) = contexts.entry(id) {
            entry.insert(context);
            return id;
        }
    }
}

fn get_callback_context(id: usize) -> Option<Arc<CallbackContext>> {
    lock_audio_state(callback_contexts()).get(&id).cloned()
}

fn unregister_callback_context(id: usize) {
    if id != 0 {
        lock_audio_state(callback_contexts()).remove(&id);
    }
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
    let Some(context) = get_callback_context(user_data as usize) else {
        return AUDIO_DATA_CALLBACK_RESULT_INVALID;
    };
    if !context.active.load(Ordering::Acquire) {
        return AUDIO_DATA_CALLBACK_RESULT_INVALID;
    }
    let mut queue = match context.queue.try_lock() {
        Ok(queue) => queue,
        Err(TryLockError::Poisoned(poisoned)) => {
            if !AUDIO_LOCK_POISON_REPORTED.swap(true, Ordering::Relaxed) {
                log::error!("recovering poisoned OHOS audio state mutex");
            }
            poisoned.into_inner()
        }
        Err(TryLockError::WouldBlock) => return AUDIO_DATA_CALLBACK_RESULT_INVALID,
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
    queue.fill_available(output);
    AUDIO_DATA_CALLBACK_RESULT_VALID
}

pub struct OhosAudioOutput {
    queue: Arc<Mutex<PcmQueue>>,
    renderer: *mut OH_AudioRenderer,
    callback_context_id: usize,
}

impl OhosAudioOutput {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(PcmQueue::with_capacity(
                48_000 * 2 * PCM_BUFFER_SECONDS,
            ))),
            renderer: ptr::null_mut(),
            callback_context_id: 0,
        }
    }

    pub fn configure(&mut self, sample_rate: u32, channels: u16) -> Result<(), String> {
        if sample_rate == 0 || channels == 0 || channels > 2 {
            return Err("Unsupported remote audio format".to_string());
        }
        self.release_renderer()?;
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

        let callback_context_id = register_callback_context(self.queue.clone());
        let callback_result = unsafe {
            OH_AudioStreamBuilder_SetRendererWriteDataCallback(
                builder,
                Some(write_pcm),
                callback_context_id as *mut c_void,
            )
        };
        if callback_result != AUDIOSTREAM_SUCCESS {
            unregister_callback_context(callback_context_id);
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
            return self.abort_setup(
                renderer,
                callback_context_id,
                "Unable to create OHAudio renderer",
            );
        }
        let start_result = unsafe { OH_AudioRenderer_Start(renderer) };
        if start_result != AUDIOSTREAM_SUCCESS {
            return self.abort_setup(
                renderer,
                callback_context_id,
                "Unable to start OHAudio renderer",
            );
        }
        self.renderer = renderer;
        self.callback_context_id = callback_context_id;
        Ok(())
    }

    pub fn enqueue(&self, pcm: &[f32]) {
        if self.renderer.is_null() || pcm.is_empty() {
            return;
        }
        lock_audio_state(&self.queue).push(pcm);
    }

    pub fn fail(&mut self, message: &str) -> Result<(), String> {
        let error = match self.release_renderer() {
            Ok(()) => message.to_string(),
            Err(release_error) => format!("{}; {}", message, release_error),
        };
        Err(error)
    }

    fn deactivate_callback_context(callback_context_id: usize) {
        if let Some(context) = get_callback_context(callback_context_id) {
            context.active.store(false, Ordering::Release);
        }
    }

    fn abort_setup(
        &mut self,
        renderer: *mut OH_AudioRenderer,
        callback_context_id: usize,
        message: &str,
    ) -> Result<(), String> {
        Self::deactivate_callback_context(callback_context_id);
        lock_audio_state(&self.queue).clear();
        let release_result = Self::release_native_renderer(renderer);
        if release_result != AUDIOSTREAM_SUCCESS {
            let error = format!(
                "{}; OH_AudioRenderer_Release failed: {}",
                message, release_result
            );
            self.renderer = renderer;
            self.callback_context_id = callback_context_id;
            return Err(error);
        }
        unregister_callback_context(callback_context_id);
        self.fail(message)
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
        let release_result = unsafe { OH_AudioRenderer_Release(renderer) };
        if release_result != AUDIOSTREAM_SUCCESS {
            log::error!("OH_AudioRenderer_Release failed: {}", release_result);
        }
        release_result
    }

    fn release_renderer(&mut self) -> Result<(), String> {
        let was_active = !self.renderer.is_null() || self.callback_context_id != 0;
        if !was_active {
            return Ok(());
        }
        let callback_context_id = self.callback_context_id;
        Self::deactivate_callback_context(callback_context_id);
        lock_audio_state(&self.queue).clear();
        let release_result = Self::release_native_renderer(self.renderer);
        if release_result != AUDIOSTREAM_SUCCESS {
            return Err(format!(
                "OH_AudioRenderer_Release failed: {}",
                release_result
            ));
        }
        self.renderer = ptr::null_mut();
        unregister_callback_context(callback_context_id);
        self.callback_context_id = 0;
        Ok(())
    }
}

impl Drop for OhosAudioOutput {
    fn drop(&mut self) {
        let _ = self.release_renderer();
    }
}
