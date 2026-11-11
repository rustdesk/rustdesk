//! Hardware Microphone Boost of this side's input devices (the "Microphone Boost" level of the Sound control panel).
//!
//! The boost is a volume node of the device topology: `IMMDevice` → `IDeviceTopology` → connector 0 → the adapter
//! connector it is connected to → the parts upstream of it (`EnumPartsIncoming`). The first part whose name contains
//! "Boost" and that activates `IAudioVolumeLevel` is the boost; its levels are in dB. A device without such a node has
//! no boost. All calls run on `crate::audio_device_thread`.

use hbb_common::log;
use windows::{
    core::{Interface, Result},
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Media::Audio::{
            eCapture, IAudioVolumeLevel, IConnector, IDeviceTopology, IMMDeviceEnumerator, IPart,
            MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
        },
        System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ},
    },
};

/// Boost range and level of a device, in dB.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Boost {
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub current: f32,
}

unsafe fn boost_level(device_name: &str) -> Result<Option<IAudioVolumeLevel>> {
    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    let collection = enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?;
    for i in 0..collection.GetCount()? {
        let device = collection.Item(i)?;
        let name = device
            .OpenPropertyStore(STGM_READ)?
            .GetValue(&PKEY_Device_FriendlyName)?
            .to_string();
        if name != device_name {
            continue;
        }
        let topology: IDeviceTopology = device.Activate(CLSCTX_ALL, None)?;
        let connector: IConnector = topology.GetConnector(0)?;
        let adapter_connector = connector.GetConnectedTo()?;
        let start: IPart = adapter_connector.cast()?;
        // Breadth-first walk upstream from the adapter connector.
        let mut queue = vec![start];
        let mut visited = 0;
        while let Some(part) = queue.pop() {
            visited += 1;
            if visited > 64 {
                break;
            }
            let part_name = part.GetName().map(|n| n.to_string().unwrap_or_default()).unwrap_or_default();
            if part_name.to_lowercase().contains("boost") {
                let mut raw = std::ptr::null_mut();
                if part.Activate(CLSCTX_ALL.0, &IAudioVolumeLevel::IID, Some(&mut raw)).is_ok() && !raw.is_null() {
                    return Ok(Some(IAudioVolumeLevel::from_raw(raw)));
                }
            }
            if let Ok(incoming) = part.EnumPartsIncoming() {
                for j in 0..incoming.GetCount().unwrap_or(0) {
                    if let Ok(p) = incoming.GetPart(j) {
                        queue.push(p);
                    }
                }
            }
        }
        return Ok(None);
    }
    Ok(None)
}

/// The boost of input device `device_name`; `None` when the device has no boost node.
pub fn get(device_name: &str) -> Option<Boost> {
    let name = device_name.to_owned();
    crate::audio_device_thread::run(move || unsafe {
        let level = boost_level(&name).ok().flatten()?;
        let (mut min, mut max, mut step) = (0f32, 0f32, 0f32);
        level.GetLevelRange(0, &mut min, &mut max, &mut step).ok()?;
        let current = level.GetLevel(0).ok()?;
        Some(Boost { min, max, step, current })
    })
    .flatten()
}

/// Sets the boost of input device `device_name` to `db` on every channel (clamped to the device's range).
pub fn set(device_name: &str, db: f32) {
    let name = device_name.to_owned();
    crate::audio_device_thread::run(move || unsafe {
        let Ok(Some(level)) = boost_level(&name) else {
            return;
        };
        let (mut min, mut max, mut step) = (0f32, 0f32, 0f32);
        if level.GetLevelRange(0, &mut min, &mut max, &mut step).is_err() {
            return;
        }
        let db = db.clamp(min, max);
        let channels = level.GetChannelCount().unwrap_or(1);
        for c in 0..channels {
            if let Err(e) = level.SetLevel(c, db, None) {
                log::warn!("Failed to set the microphone boost of \"{name}\" to {db} dB: {e}");
                return;
            }
        }
        log::info!("Microphone boost of \"{name}\" set to {db} dB");
    });
}
