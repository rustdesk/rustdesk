// PipeWire source construction adapted from OpenDroidMic (MIT).
// Copyright (c) 2026 OpenDroidMic Contributors. See remote_microphone_LICENSE.
use hbb_common::anyhow::{Context, Result};
use magnum_opus::{Channels, Decoder};
use parking_lot::Mutex;
use pipewire::{self as pw, properties::properties, spa};
use spa::pod::Pod;
use std::{
    collections::VecDeque,
    io::Cursor,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

static PARKED: Mutex<Option<(String, Arc<Receiver>, std::time::Instant)>> = Mutex::new(None);
static SOURCE_OWNED: AtomicBool = AtomicBool::new(false);
struct SourceLease;
impl Drop for SourceLease {
    fn drop(&mut self) {
        SOURCE_OWNED.store(false, Ordering::Release);
    }
}

struct State {
    epoch: u64,
    active: bool,
    pcm: VecDeque<f32>,
}
pub struct Receiver {
    state: Arc<Mutex<State>>,
    decoder: Mutex<Decoder>,
    running: Arc<AtomicBool>,
}
impl Receiver {
    pub fn acquire(peer: &str) -> Result<Arc<Self>> {
        let mut parked = PARKED.lock();
        if parked.as_ref().map(|(id, _, _)| id.as_str()) == Some(peer) {
            if let Some((_, receiver, _)) = parked.take() {
                if receiver.healthy() {
                    return Ok(receiver);
                }
            }
        }
        drop(parked);
        Self::new().map(Arc::new)
    }
    pub fn park(peer: String, receiver: Arc<Self>) {
        receiver.silence();
        let parked_at = std::time::Instant::now();
        *PARKED.lock() = Some((peer, receiver.clone(), parked_at));
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(20));
            let mut parked = PARKED.lock();
            if parked
                .as_ref()
                .map(|(_, current, stamp)| *stamp == parked_at && Arc::ptr_eq(current, &receiver))
                .unwrap_or(false)
            {
                parked.take();
            }
        });
    }
    pub fn new() -> Result<Self> {
        if SOURCE_OWNED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            hbb_common::bail!("Another session owns the remote microphone");
        }
        let lease = SourceLease;
        let decoder = Decoder::new(48000, Channels::Stereo)?;
        let state = Arc::new(Mutex::new(State {
            epoch: 0,
            active: false,
            pcm: VecDeque::with_capacity(4800),
        }));
        let running = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::sync_channel(1);
        let (s, r) = (state.clone(), running.clone());
        std::thread::Builder::new()
            .name("remote-microphone".into())
            .spawn(move || {
                let _lease = lease;
                if let Err(err) = source(s, r.clone(), tx.clone()) {
                    r.store(false, Ordering::Release);
                    let _ = tx.try_send(Err(format!("{err:#}")));
                }
            })?;
        match rx.recv_timeout(Duration::from_secs(4)) {
            Ok(Ok(())) => {}
            other => {
                running.store(false, Ordering::Release);
                hbb_common::bail!("PipeWire source unavailable: {:?}", other);
            }
        }
        Ok(Self {
            state,
            running,
            decoder: Mutex::new(decoder),
        })
    }
    pub fn is_active(&self) -> bool {
        self.state.lock().active
    }
    pub fn healthy(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
    pub fn activate(&self, epoch: u64) -> Result<()> {
        let mut state = self.state.lock();
        state.active = false;
        state.pcm.clear();
        self.decoder.lock().reset_state()?;
        state.epoch = epoch;
        state.active = true;
        Ok(())
    }
    pub fn silence(&self) {
        let mut state = self.state.lock();
        state.active = false;
        state.pcm.clear();
    }
    pub fn push(&self, epoch: u64, data: &[u8]) {
        if data.is_empty() || data.len() > 4096 {
            return;
        }
        let mut state = self.state.lock();
        if !state.active || state.epoch != epoch {
            return;
        }
        let mut pcm = [0f32; 11520];
        match self.decoder.lock().decode_float(data, &mut pcm, false) {
            Ok(frames) => {
                for frame in pcm[..frames * 2].chunks_exact(2) {
                    state.pcm.push_back((frame[0] + frame[1]) * 0.5);
                }
                // Keep at most 100ms; never play a growing backlog of old speech.
                let excess = state.pcm.len().saturating_sub(4800);
                state.pcm.drain(..excess);
            }
            Err(error) => {
                const DECODE_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(60);
                hbb_common::throttled_log!(
                    DECODE_ERROR_LOG_INTERVAL,
                    warn,
                    "Failed to decode remote microphone Opus frame: {}",
                    error
                );
            }
        }
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        self.silence();
        self.running.store(false, Ordering::Release);
    }
}
fn source(
    state: Arc<Mutex<State>>,
    running: Arc<AtomicBool>,
    ready: mpsc::SyncSender<Result<(), String>>,
) -> Result<()> {
    let main_loop =
        pw::main_loop::MainLoopRc::new(None).context("Failed to create PipeWire main loop")?;

    let context = pw::context::ContextRc::new(&main_loop, None)
        .context("Failed to create PipeWire context")?;

    let core = context
        .connect_rc(None)
        .context("Failed to connect to PipeWire")?;

    let stream = pw::stream::StreamRc::new(
        core.clone(),
        "RustDeskRemoteMicrophone",
        properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => "Source",
            *pw::keys::MEDIA_CLASS => "Audio/Source",
            *pw::keys::MEDIA_ROLE => "Communication",
            *pw::keys::NODE_NAME => "RustDeskRemoteMicrophone",
            *pw::keys::NODE_DESCRIPTION => "RustDeskRemoteMicrophone Virtual Microphone",
            *pw::keys::NODE_VIRTUAL => "true",
            *pw::keys::NODE_ALWAYS_PROCESS => "true",
        },
    )
    .context("Failed to create PipeWire stream")?;

    let status = std::rc::Rc::new(std::cell::RefCell::new(None));
    let callback_status = status.clone();
    let callback_running = running.clone();
    let _listener = stream
        .add_local_listener_with_user_data(state)
        .state_changed(move |_, _, _, state| match state {
            pw::stream::StreamState::Paused | pw::stream::StreamState::Streaming => {
                *callback_status.borrow_mut() = Some(Ok(()))
            }
            pw::stream::StreamState::Error(err) => {
                callback_running.store(false, Ordering::Release);
                *callback_status.borrow_mut() = Some(Err(err.to_owned()))
            }
            pw::stream::StreamState::Unconnected => {
                if callback_status.borrow().is_some() {
                    callback_running.store(false, Ordering::Release);
                    *callback_status.borrow_mut() =
                        Some(Err("PipeWire stream became unconnected".to_string()));
                }
            }
            _ => {}
        })
        .process(|stream, state| {
            if let Some(mut buffer) = stream.dequeue_buffer() {
                let requested = buffer.requested() as usize;
                if let Some(data) = buffer.datas_mut().first_mut() {
                    if let Some(bytes) = data.data() {
                        let capacity = bytes.len() / 4;
                        let samples = if requested == 0 {
                            capacity
                        } else {
                            capacity.min(requested)
                        };
                        let len = samples * 4;
                        let mut queue = state.try_lock();
                        for sample in bytes[..len].chunks_exact_mut(4) {
                            let value = queue
                                .as_mut()
                                .and_then(|s| s.pcm.pop_front())
                                .unwrap_or(0.0);
                            sample.copy_from_slice(&value.to_le_bytes());
                        }
                        let chunk = data.chunk_mut();
                        *chunk.offset_mut() = 0;
                        *chunk.stride_mut() = 4;
                        *chunk.size_mut() = len as u32;
                    }
                }
            }
        })
        .register()?;
    let mut audio_info = spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(spa::param::audio::AudioFormat::F32LE);
    audio_info.set_rate(48000);
    audio_info.set_channels(1);
    let mut position = [0u32; spa::param::audio::MAX_CHANNELS];
    position[0] = pipewire::spa::sys::SPA_AUDIO_CHANNEL_MONO;
    audio_info.set_position(position);

    let values: Vec<u8> = spa::pod::serialize::PodSerializer::serialize(
        Cursor::new(Vec::new()),
        &spa::pod::Value::Object(spa::pod::Object {
            type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
            id: spa::param::ParamType::EnumFormat.as_raw(),
            properties: audio_info.into(),
        }),
    )
    .context("Failed to serialize format")?
    .0
    .into_inner();

    let mut params = [Pod::from_bytes(&values).context("Failed to create Pod from bytes")?];

    stream
        .connect(
            spa::utils::Direction::Output,
            None,
            pw::stream::StreamFlags::AUTOCONNECT
                | pw::stream::StreamFlags::MAP_BUFFERS
                | pw::stream::StreamFlags::RT_PROCESS,
            &mut params,
        )
        .context("Failed to connect stream")?;

    let start = std::time::Instant::now();
    let mut announced = false;
    while running.load(Ordering::Acquire) {
        main_loop
            .loop_()
            .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(10)));
        if let Some(result) = status.borrow_mut().take() {
            match result {
                Ok(()) if !announced => {
                    let _ = ready.send(Ok(()));
                    announced = true;
                }
                Err(err) => {
                    hbb_common::bail!("PipeWire: {}", err);
                }
                _ => {}
            }
        }
        if !announced && start.elapsed() > Duration::from_secs(3) {
            hbb_common::bail!("PipeWire initialization timed out");
        }
    }
    Ok(())
}
