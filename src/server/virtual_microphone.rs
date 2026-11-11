//! The controlled side's recording devices and its virtual microphone.
//!
//! - `recording_state()` lists the active recording endpoints (friendly names), the default recording endpoint
//!   (`eConsole`) with its endpoint volume and Microphone Boost, and the virtual microphone pair.
//! - `set_default_recording(name)` makes a recording endpoint the Windows default for every role (`eConsole`,
//!   `eMultimedia`, `eCommunications`); the change stays after the session ends.
//! - The virtual microphone is a virtual audio cable (`PAIRS`): the controller's microphone stream plays into its
//!   playback endpoint and arrives on its recording endpoint, which applications record from once it is the default.
//!
//! Windows has no public API to change a default endpoint; `IPolicyConfig::SetDefaultEndpoint` (the interface the
//! Sound control panel uses) is called through its vtable. All calls run on `crate::audio_device_thread`.

use hbb_common::log;
use std::ffi::c_void;
use windows::{
    core::{IUnknown, Interface, GUID, HRESULT, PCWSTR},
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Media::Audio::{
            eCapture, eCommunications, eConsole, eMultimedia, eRender, EDataFlow,
            Endpoints::IAudioEndpointVolume, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
            DEVICE_STATEMASK_ALL, DEVICE_STATE_ACTIVE, DEVICE_STATE_DISABLED, DEVICE_STATE_NOTPRESENT,
            DEVICE_STATE_UNPLUGGED,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
            STGM_READ,
        },
    },
};

/// `(driver, playback endpoint name pattern, recording endpoint name pattern)`; a pattern matches a friendly name that
/// contains it. VB-CABLE: "CABLE Input (VB-Audio Virtual Cable)" → "CABLE Output (VB-Audio Virtual Cable)".
/// Virtual Audio Driver (VirtualDrivers): "Virtual Audio Driver" speaker → "Virtual Mic Driver" microphone.
const PAIRS: &[(&str, &str, &str)] = &[
    ("VB-CABLE", "CABLE Input", "CABLE Output"),
    ("Virtual Audio Driver", "Virtual Audio Driver", "Virtual Mic Driver"),
];

const CLSID_POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const IID_IPOLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
/// `IPolicyConfig::SetDefaultEndpoint`: IUnknown (3) + GetMixFormat, GetDeviceFormat, ResetDeviceFormat,
/// SetDeviceFormat, GetProcessingPeriod, SetProcessingPeriod, GetShareMode, SetShareMode, GetPropertyValue,
/// SetPropertyValue (10).
const SET_DEFAULT_ENDPOINT_SLOT: usize = 13;

#[derive(Clone, Debug)]
pub struct Pair {
    pub driver: String,
    pub playback_name: String,
    pub recording_name: String,
}

/// Recording side of this PC as `recording_state()` reads it.
#[derive(Clone, Debug, Default)]
pub struct RecordingState {
    pub devices: Vec<String>,
    pub default: String,
    /// Endpoint volume of `default`, percent; -1 when unknown.
    pub default_volume: i32,
    /// Microphone Boost of `default`: `(min, max, step, current)` dB.
    pub default_boost: Option<(f32, f32, f32, f32)>,
    pub virtual_microphone: Option<Pair>,
    /// Recording endpoints that are not active: `(name, "disabled" | "unplugged" | "not present")`, one per name.
    pub inactive: Vec<(String, String)>,
}

struct Endpoint {
    id: String,
    name: String,
    device: IMMDevice,
}

unsafe fn enumerator() -> windows::core::Result<IMMDeviceEnumerator> {
    // The audio-device-query thread lives as long as the process; COM stays initialized on it. An STA set up there
    // by CPAL earlier answers RPC_E_CHANGED_MODE, which is fine.
    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
}

unsafe fn device_id(device: &IMMDevice) -> windows::core::Result<String> {
    let id = device.GetId()?;
    let s = id.to_string().unwrap_or_default();
    CoTaskMemFree(Some(id.0 as *const c_void));
    Ok(s)
}

unsafe fn friendly_name(device: &IMMDevice) -> windows::core::Result<String> {
    Ok(device
        .OpenPropertyStore(STGM_READ)?
        .GetValue(&PKEY_Device_FriendlyName)?
        .to_string())
}

unsafe fn endpoints(flow: EDataFlow) -> windows::core::Result<Vec<Endpoint>> {
    let collection = enumerator()?.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
    let mut out = Vec::new();
    for i in 0..collection.GetCount()? {
        let device = collection.Item(i)?;
        out.push(Endpoint {
            id: device_id(&device)?,
            name: friendly_name(&device)?,
            device,
        });
    }
    Ok(out)
}

unsafe fn pair(playback: &[Endpoint], recording: &[Endpoint]) -> Option<Pair> {
    for (driver, play_pattern, rec_pattern) in PAIRS {
        let p = playback.iter().find(|e| e.name.contains(play_pattern));
        let r = recording.iter().find(|e| e.name.contains(rec_pattern));
        if let (Some(p), Some(r)) = (p, r) {
            return Some(Pair {
                driver: driver.to_string(),
                playback_name: p.name.clone(),
                recording_name: r.name.clone(),
            });
        }
    }
    None
}

/// The first installed virtual microphone of `PAIRS`.
pub fn detect() -> Option<Pair> {
    crate::audio_device_thread::run(|| unsafe {
        match (endpoints(eRender), endpoints(eCapture)) {
            (Ok(p), Ok(r)) => pair(&p, &r),
            (Err(e), _) | (_, Err(e)) => {
                log::error!("Virtual microphone detection failed: {e}");
                None
            }
        }
    })
    .flatten()
}

unsafe fn inactive_recording() -> windows::core::Result<Vec<(String, String)>> {
    let collection = enumerator()?.EnumAudioEndpoints(eCapture, windows::Win32::Media::Audio::DEVICE_STATE(DEVICE_STATEMASK_ALL))?;
    let mut out: Vec<(String, String)> = Vec::new();
    for i in 0..collection.GetCount()? {
        let device = collection.Item(i)?;
        let state = device.GetState()?;
        let label = if state.0 & DEVICE_STATE_ACTIVE.0 != 0 {
            continue;
        } else if state.0 & DEVICE_STATE_DISABLED.0 != 0 {
            "disabled"
        } else if state.0 & DEVICE_STATE_UNPLUGGED.0 != 0 {
            "unplugged"
        } else if state.0 & DEVICE_STATE_NOTPRESENT.0 != 0 {
            "not present"
        } else {
            continue;
        };
        let Ok(name) = friendly_name(&device) else {
            continue;
        };
        if !out.iter().any(|(n, _)| *n == name) {
            out.push((name, label.to_owned()));
        }
    }
    Ok(out)
}

unsafe fn recording_state_inner() -> windows::core::Result<RecordingState> {
    let recording = endpoints(eCapture)?;
    let playback = endpoints(eRender)?;
    let mut state = RecordingState {
        devices: recording.iter().map(|e| e.name.clone()).collect(),
        default_volume: -1,
        virtual_microphone: pair(&playback, &recording),
        inactive: inactive_recording().unwrap_or_default(),
        ..Default::default()
    };
    if let Ok(default) = enumerator()?.GetDefaultAudioEndpoint(eCapture, eConsole) {
        state.default = friendly_name(&default)?;
        if let Ok(volume) = default.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
            if let Ok(level) = volume.GetMasterVolumeLevelScalar() {
                state.default_volume = (level * 100.0).round() as i32;
            }
        }
    }
    Ok(state)
}

/// The recording devices, the default one with its volume and boost, and the virtual microphone.
pub fn recording_state() -> RecordingState {
    let mut state = crate::audio_device_thread::run(|| match unsafe { recording_state_inner() } {
        Ok(s) => s,
        Err(e) => {
            log::error!("Reading the recording devices failed: {e}");
            RecordingState { default_volume: -1, ..Default::default() }
        }
    })
    .unwrap_or_default();
    if !state.default.is_empty() {
        state.default_boost = crate::client::microphone_boost::get(&state.default)
            .map(|b| (b.min, b.max, b.step, b.current));
    }
    log::info!(
        "Recording devices: {:?}, default {:?} ({}%), virtual microphone {:?}",
        state.devices,
        state.default,
        state.default_volume,
        state.virtual_microphone.as_ref().map(|p| &p.recording_name)
    );
    state
}

unsafe fn find_recording(name: &str) -> windows::core::Result<Option<Endpoint>> {
    Ok(endpoints(eCapture)?.into_iter().find(|e| e.name == name))
}

/// Makes endpoint `id` the Windows default of its flow for every role (`IPolicyConfig::SetDefaultEndpoint`).
unsafe fn set_default_endpoint(id: &str) -> windows::core::Result<()> {
    let unknown: IUnknown = CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL)?;
    let mut raw: *mut c_void = std::ptr::null_mut();
    unknown.query(&IID_IPOLICY_CONFIG, &mut raw).ok()?;
    let policy = IUnknown::from_raw(raw);
    type SetDefaultEndpoint = unsafe extern "system" fn(*mut c_void, PCWSTR, i32) -> HRESULT;
    let vtable = *(policy.as_raw() as *const *const usize);
    let set: SetDefaultEndpoint = std::mem::transmute(*vtable.add(SET_DEFAULT_ENDPOINT_SLOT));
    let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
    for role in [eConsole, eMultimedia, eCommunications] {
        set(policy.as_raw(), PCWSTR(wide.as_ptr()), role.0).ok()?;
    }
    Ok(())
}

/// Endpoint id of the Windows default (`eConsole`) of `flow`.
unsafe fn default_endpoint_id(flow: EDataFlow) -> Option<String> {
    let device = enumerator().ok()?.GetDefaultAudioEndpoint(flow, eConsole).ok()?;
    device_id(&device).ok()
}

/// Friendly name of the default recording device; "" when there is none.
pub fn default_recording_name() -> String {
    crate::audio_device_thread::run(|| unsafe {
        enumerator()
            .and_then(|e| e.GetDefaultAudioEndpoint(eCapture, eConsole))
            .and_then(|d| friendly_name(&d))
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

/// Makes recording endpoint `name` the Windows default recording device for every role.
pub fn set_default_recording(name: &str) {
    let name = name.to_owned();
    crate::audio_device_thread::run(move || unsafe {
        let result = (|| -> windows::core::Result<bool> {
            let Some(endpoint) = find_recording(&name)? else {
                return Ok(false);
            };
            set_default_endpoint(&endpoint.id)?;
            Ok(true)
        })();
        match result {
            Ok(true) => log::info!("Default recording device set to \"{name}\""),
            Ok(false) => log::warn!("Recording device \"{name}\" is not present"),
            Err(e) => log::error!("Failed to make \"{name}\" the default recording device: {e}"),
        }
    });
}

/// Sets the endpoint volume of recording endpoint `name` to `percent` (0..=100).
pub fn set_recording_volume(name: &str, percent: u32) {
    let name = name.to_owned();
    crate::audio_device_thread::run(move || unsafe {
        let result = (|| -> windows::core::Result<bool> {
            let Some(endpoint) = find_recording(&name)? else {
                return Ok(false);
            };
            let volume: IAudioEndpointVolume = endpoint.device.Activate(CLSCTX_ALL, None)?;
            volume.SetMasterVolumeLevelScalar(percent.min(100) as f32 / 100.0, std::ptr::null())?;
            Ok(true)
        })();
        match result {
            Ok(true) => log::info!("Volume of recording device \"{name}\" set to {percent}%"),
            Ok(false) => log::warn!("Recording device \"{name}\" is not present"),
            Err(e) => log::error!("Failed to set the volume of \"{name}\": {e}"),
        }
    });
}

/// Download of VB-CABLE (https://vb-audio.com/Cable/): driver pack with `VBCABLE_Setup_x64.exe`.
pub const VB_CABLE_PACK_URL: &str = "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip";

/// Installs virtual microphone driver `driver` ("vb-cable"): downloads the driver pack into the temp directory,
/// extracts it with `Expand-Archive` and runs `VBCABLE_Setup_x64.exe -i -h`. Windows asks the user of this side to
/// allow the signed driver. Blocking; returns once the installer exits.
pub fn install(driver: &str) -> hbb_common::ResultType<()> {
    use hbb_common::anyhow::{anyhow, bail};
    if driver != "vb-cable" {
        bail!("unknown virtual microphone driver {driver:?}");
    }
    let dir = std::env::temp_dir().join("rustdesk-virtual-microphone");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let zip = dir.join("VBCABLE_Driver_Pack.zip");
    log::info!("Downloading {VB_CABLE_PACK_URL}");
    let bytes = crate::hbbs_http::create_http_client_with_url(VB_CABLE_PACK_URL)
        .get(VB_CABLE_PACK_URL)
        .send()?
        .error_for_status()?
        .bytes()?;
    std::fs::write(&zip, &bytes)?;
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "Expand-Archive -Force -LiteralPath '{}' -DestinationPath '{}'",
                zip.display(),
                dir.display()
            ),
        ])
        .status()?;
    if !status.success() {
        bail!("Expand-Archive exited with {status}");
    }
    // The VB-CABLE setup makes its endpoints the default playback and recording devices; the previous defaults are put
    // back afterwards (the recording default is chosen in the controller's Audio dialog).
    let previous = crate::audio_device_thread::run(|| unsafe {
        (default_endpoint_id(eRender), default_endpoint_id(eCapture))
    })
    .unwrap_or_default();
    let setup = dir.join("VBCABLE_Setup_x64.exe");
    log::info!("Running {} -i -h", setup.display());
    let status = std::process::Command::new(&setup).args(["-i", "-h"]).status()?;
    log::info!("VB-CABLE setup exited with {status}");
    crate::audio_device_thread::run(move || unsafe {
        for id in [previous.0, previous.1].into_iter().flatten() {
            match set_default_endpoint(&id) {
                Ok(()) => log::info!("Default endpoint {id} restored after the VB-CABLE setup"),
                Err(e) => log::error!("Failed to restore default endpoint {id}: {e}"),
            }
        }
    });
    if detect().is_none() {
        return Err(anyhow!("the installer exited ({status}) and no virtual microphone is present"));
    }
    Ok(())
}
