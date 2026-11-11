//! Per-peer, per-device software volume of a session's audio.
//!
//! Peer options, each a JSON object `{"<device>": <percent>}` (0..=`MAX_PERCENT`, absent = 100):
//! - `OPTION_VOLUME_OUTPUT`: keyed by the output device that plays (the resolved device, also when the selection is the
//!   Windows default).
//! - `OPTION_VOLUME_REMOTE_INPUT`: keyed by the value of the peer option `audio-input-device` (the controlled side's
//!   capture selection).
//! - `OPTION_VOLUME_HOST_INPUT`: keyed by this side's input device streamed into the controlled side's virtual
//!   microphone.
//!
//! Playback multiplies decoded samples by `playback_gain()` = output volume × remote input volume; the client audio
//! service multiplies captured samples by `capture_gain()` before encoding.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, Ordering},
        RwLock,
    },
};

pub const OPTION_VOLUME_OUTPUT: &str = "audio-volume-output";
pub const OPTION_VOLUME_REMOTE_INPUT: &str = "audio-volume-remote-input";
pub const OPTION_VOLUME_HOST_INPUT: &str = "audio-volume-host-input";
/// Peer option `{"<input device>": <dB>}`: the hardware Microphone Boost level applied when the device is streamed.
pub const OPTION_BOOST_HOST_INPUT: &str = "audio-boost-host-input";
pub const MAX_PERCENT: u32 = 200;
/// Peer option `{"<recording device of the peer>": <percent 0..=100>}`: endpoint volume of the peer's recording devices,
/// sent after every login.
pub const OPTION_VOLUME_REMOTE_RECORDING: &str = "audio-volume-remote-recording";
/// Peer option `{"<recording device of the peer>": <dB>}`: Microphone Boost of the peer's recording devices, sent after
/// every login.
pub const OPTION_BOOST_REMOTE_RECORDING: &str = "audio-boost-remote-recording";

#[derive(Default)]
struct PeerVolumes {
    output: HashMap<String, u32>,
    remote_input: HashMap<String, u32>,
    remote_input_selection: String,
    playing_output: String,
}

lazy_static::lazy_static! {
    static ref VOLUMES: RwLock<HashMap<String, PeerVolumes>> = Default::default();
}

/// `capture_gain()` as `f32` bits; 1.0 unless a controller microphone stream set it.
static CAPTURE_GAIN: AtomicU32 = AtomicU32::new(0x3f80_0000);

pub fn parse_percent_map(json: &str) -> HashMap<String, u32> {
    serde_json::from_str::<HashMap<String, u32>>(json).unwrap_or_default()
}

pub fn parse_db_map(json: &str) -> HashMap<String, f32> {
    serde_json::from_str::<HashMap<String, f32>>(json).unwrap_or_default()
}

/// `json` with `device` set to `value`.
pub fn with_entry<V: serde::Serialize + serde::de::DeserializeOwned>(json: &str, device: &str, value: V) -> String {
    let mut map = serde_json::from_str::<HashMap<String, V>>(json).unwrap_or_default();
    map.insert(device.to_owned(), value);
    serde_json::to_string(&map).unwrap_or_default()
}

/// Loads the stored volumes of `peer_id` (`get_option` reads a peer option).
pub fn load(peer_id: &str, get_option: impl Fn(&str) -> String) {
    let mut lock = VOLUMES.write().unwrap();
    let v = lock.entry(peer_id.to_owned()).or_default();
    v.output = parse_percent_map(&get_option(OPTION_VOLUME_OUTPUT));
    v.remote_input = parse_percent_map(&get_option(OPTION_VOLUME_REMOTE_INPUT));
    v.remote_input_selection = get_option(super::audio_devices::OPTION_AUDIO_INPUT_DEVICE);
}

pub fn set_playing_output(peer_id: &str, device: &str) {
    VOLUMES
        .write()
        .unwrap()
        .entry(peer_id.to_owned())
        .or_default()
        .playing_output = device.to_owned();
}

pub fn playing_output(peer_id: &str) -> String {
    VOLUMES
        .read()
        .unwrap()
        .get(peer_id)
        .map(|v| v.playing_output.clone())
        .unwrap_or_default()
}

pub fn set_remote_input_selection(peer_id: &str, selection: &str) {
    VOLUMES
        .write()
        .unwrap()
        .entry(peer_id.to_owned())
        .or_default()
        .remote_input_selection = selection.to_owned();
}

pub fn set_output_volume(peer_id: &str, device: &str, percent: u32) {
    VOLUMES
        .write()
        .unwrap()
        .entry(peer_id.to_owned())
        .or_default()
        .output
        .insert(device.to_owned(), percent.min(MAX_PERCENT));
}

pub fn set_remote_input_volume(peer_id: &str, selection: &str, percent: u32) {
    VOLUMES
        .write()
        .unwrap()
        .entry(peer_id.to_owned())
        .or_default()
        .remote_input
        .insert(selection.to_owned(), percent.min(MAX_PERCENT));
}

/// Output volume of the playing device × remote input volume of the current capture selection.
pub fn playback_gain(peer_id: &str) -> f32 {
    let lock = VOLUMES.read().unwrap();
    let Some(v) = lock.get(peer_id) else {
        return 1.0;
    };
    let output = v.output.get(&v.playing_output).copied().unwrap_or(100);
    let input = v
        .remote_input
        .get(&v.remote_input_selection)
        .copied()
        .unwrap_or(100);
    (output as f32 / 100.0) * (input as f32 / 100.0)
}

pub fn set_capture_gain(percent: u32) {
    let gain = percent.min(MAX_PERCENT) as f32 / 100.0;
    CAPTURE_GAIN.store(gain.to_bits(), Ordering::Relaxed);
}

pub fn capture_gain() -> f32 {
    f32::from_bits(CAPTURE_GAIN.load(Ordering::Relaxed))
}

/// Multiplies `samples` by `gain`, clamped to [-1, 1]; a gain of 1 leaves them untouched.
pub fn apply_gain(samples: &mut [f32], gain: f32) {
    if (gain - 1.0).abs() < 0.001 {
        return;
    }
    for s in samples {
        *s = (*s * gain).clamp(-1.0, 1.0);
    }
}
