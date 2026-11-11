//! Per-peer audio device selection.
//!
//! - Output (this side's playback): peer option `OPTION_AUDIO_OUTPUT_DEVICE`; "" is the Windows default output
//!   device, followed live. A named device that is not currently present plays on the default output device until it
//!   is present again.
//! - Input (the controlled side's capture): peer option `OPTION_AUDIO_INPUT_DEVICE`; "" leaves the capture device to
//!   the controlled side's own setting, `INPUT_OVERRIDE_PREFIX` + name selects a device for this connection
//!   (`INPUT_OVERRIDE_PREFIX` alone is System Sound / the default input device). The selection is sent to the
//!   controlled side as `Misc.audio_input_selection` after login and on every change.

use base::message_proto::*;
use std::{
    collections::HashMap,
    sync::{Mutex, RwLock},
};

pub const OPTION_AUDIO_OUTPUT_DEVICE: &str = "audio-output-device";
pub const OPTION_AUDIO_INPUT_DEVICE: &str = "audio-input-device";
pub const INPUT_OVERRIDE_PREFIX: &str = "=";
/// Peer option: this side's input device streamed into the controlled side's virtual microphone; "" = none (the
/// controlled side records from its own input as configured there).
pub const OPTION_AUDIO_HOST_INPUT_DEVICE: &str = "audio-host-input-device";
/// Peer option holding the saved mute state of a session: "Y" muted, "N" unmuted; absent or "" starts muted.
pub const OPTION_AUDIO_MUTE_SAVED: &str = "audio-mute-saved";

lazy_static::lazy_static! {
    /// Peer id → output device name ("" = default output device).
    static ref OUTPUT_SELECTIONS: RwLock<HashMap<String, String>> = Default::default();
    /// Peer id → the last `Misc.recording_level` received from that peer.
    static ref REMOTE_RECORDING_LEVELS: Mutex<HashMap<String, f32>> = Default::default();
    /// Peer id → the last `AudioInputs` received from that peer.
    static ref REMOTE_INPUTS: Mutex<HashMap<String, AudioInputs>> = Default::default();
}

thread_local! {
    /// Peer id of the audio playback thread (`start_audio_thread()`).
    static THREAD_PEER_ID: std::cell::RefCell<String> = Default::default();
}

pub fn set_thread_peer_id(peer_id: &str) {
    THREAD_PEER_ID.with(|p| *p.borrow_mut() = peer_id.to_owned());
}

/// Peer id of the current audio playback thread.
pub fn thread_peer_id() -> String {
    THREAD_PEER_ID.with(|p| p.borrow().clone())
}

/// Output device selected for the peer of the current audio playback thread.
pub fn thread_output_selection() -> String {
    THREAD_PEER_ID.with(|p| output_selection(&p.borrow()))
}

pub fn output_selection(peer_id: &str) -> String {
    OUTPUT_SELECTIONS
        .read()
        .unwrap()
        .get(peer_id)
        .cloned()
        .unwrap_or_default()
}

pub fn set_output_selection(peer_id: &str, device: &str) {
    OUTPUT_SELECTIONS
        .write()
        .unwrap()
        .insert(peer_id.to_owned(), device.to_owned());
}

pub fn set_remote_recording_level(peer_id: &str, level: f32) {
    REMOTE_RECORDING_LEVELS
        .lock()
        .unwrap()
        .insert(peer_id.to_owned(), level);
}

pub fn remote_recording_level(peer_id: &str) -> f32 {
    REMOTE_RECORDING_LEVELS
        .lock()
        .unwrap()
        .get(peer_id)
        .copied()
        .unwrap_or_default()
}

/// `Misc.request_recording_level`.
pub fn request_recording_level_message() -> Message {
    let mut misc = Misc::new();
    misc.set_request_recording_level(true);
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

pub fn set_remote_inputs(peer_id: &str, inputs: AudioInputs) {
    REMOTE_INPUTS
        .lock()
        .unwrap()
        .insert(peer_id.to_owned(), inputs);
}

/// JSON `{"devices": [...], "own_setting": "...", "system_sound": bool}` of the last `AudioInputs` from `peer_id`, or
/// "" when none was received.
pub fn remote_inputs_json(peer_id: &str) -> String {
    match REMOTE_INPUTS.lock().unwrap().get(peer_id) {
        Some(inputs) => serde_json::json!({
            "devices": inputs.devices,
            "own_setting": inputs.own_setting,
            "system_sound": inputs.system_sound,
            "virtual_microphone": inputs.virtual_microphone,
            "virtual_microphone_install": inputs.virtual_microphone_install,
            "recording_devices": inputs.recording_devices,
            "inactive_recording_devices": inputs.inactive_recording_devices,
            "default_recording": inputs.default_recording,
            "default_recording_volume": inputs.default_recording_volume,
            "default_recording_boost": inputs.default_recording_boost,
        })
        .to_string(),
        None => "".to_owned(),
    }
}

pub fn clear_remote_inputs(peer_id: &str) {
    REMOTE_INPUTS.lock().unwrap().remove(peer_id);
}

pub fn request_inputs_message() -> Message {
    let mut misc = Misc::new();
    misc.set_request_audio_inputs(true);
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

/// `Misc.recording_device_control`: on the controlled side, makes `device` the default recording device
/// (`set_default`), sets its endpoint volume (`volume` percent, -1 leaves it) and its Microphone Boost (`boost_db`).
pub fn recording_device_control_message(
    device: &str,
    set_default: bool,
    volume: i32,
    boost_db: Option<f32>,
) -> Message {
    let mut c = RecordingDeviceControl::new();
    c.device = device.to_owned();
    c.set_default = set_default;
    c.volume = volume;
    c.set_boost = boost_db.is_some();
    c.boost_db = boost_db.unwrap_or_default();
    let mut misc = Misc::new();
    misc.set_recording_device_control(c);
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

/// `Misc.install_virtual_microphone` for driver `driver` ("vb-cable").
pub fn install_virtual_microphone_message(driver: &str) -> Message {
    let mut misc = Misc::new();
    misc.set_install_virtual_microphone(driver.to_owned());
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

/// `Misc.host_microphone`: the stream of this side's microphone into the virtual microphone starts or stops.
pub fn host_microphone_message(active: bool) -> Message {
    let mut host = HostMicrophone::new();
    host.active = active;
    let mut misc = Misc::new();
    misc.set_host_microphone(host);
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

/// `Misc.audio_input_selection` for peer option value `option_value` of `OPTION_AUDIO_INPUT_DEVICE`.
pub fn input_selection_message(option_value: &str) -> Message {
    let mut selection = AudioInputSelection::new();
    if let Some(device) = option_value.strip_prefix(INPUT_OVERRIDE_PREFIX) {
        selection.is_override = true;
        selection.device = device.to_owned();
    }
    let mut misc = Misc::new();
    misc.set_audio_input_selection(selection);
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

/// Output device names of this side (devices with an output configuration). Runs on
/// `crate::audio_device_thread`, which keeps CPAL's process-global device enumerator valid.
#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "ios")))]
pub fn get_output_devices() -> Vec<String> {
    crate::audio_device_thread::run(|| {
        use cpal::traits::{DeviceTrait, HostTrait};
        let mut out = Vec::new();
        if let Ok(devices) = cpal::default_host().output_devices() {
            for device in devices {
                if device.default_output_config().is_err() {
                    continue;
                }
                if let Ok(name) = device.name() {
                    out.push(name);
                }
            }
        }
        out
    })
    .unwrap_or_default()
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "ios"))]
pub fn get_output_devices() -> Vec<String> {
    vec![]
}

/// Input device names of this side (devices with an input configuration). Runs on `crate::audio_device_thread`.
#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "ios")))]
pub fn get_input_devices() -> Vec<String> {
    crate::audio_device_thread::run(|| {
        use cpal::traits::{DeviceTrait, HostTrait};
        let mut out = Vec::new();
        if let Ok(devices) = cpal::default_host().input_devices() {
            for device in devices {
                if device.default_input_config().is_err() {
                    continue;
                }
                if let Ok(name) = device.name() {
                    out.push(name);
                }
            }
        }
        out
    })
    .unwrap_or_default()
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "ios"))]
pub fn get_input_devices() -> Vec<String> {
    vec![]
}
