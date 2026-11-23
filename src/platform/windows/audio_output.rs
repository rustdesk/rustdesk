//! Host-side audio routing. The virtual endpoint deliberately survives capture
//! teardown: disconnecting must restore the physical output at zero volume.

use base::config::keys::OPTION_REMOTE_ONLY_AUDIO_DEVICE;
use cpal::traits::{DeviceTrait, HostTrait};
use hbb_common::{
    anyhow::{bail, Context},
    config::{Config, LocalConfig},
    log, ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::{ffi::c_void, sync::Mutex};
use windows::{
    core::{IUnknown, IUnknown_Vtbl, Interface, GUID, HRESULT, PCWSTR, PWSTR},
    Win32::{
        Devices::FunctionDiscovery::{PKEY_DeviceInterface_FriendlyName, PKEY_Device_FriendlyName},
        Foundation::{PROPERTYKEY, RPC_E_CHANGED_MODE},
        Media::Audio::{
            eCommunications, eConsole, eMultimedia, eRender, ERole,
            Endpoints::IAudioEndpointVolume, IMMDevice, IMMDeviceEnumerator, IMMEndpoint,
            MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
            StructuredStorage::{PropVariantClear, PropVariantToStringAlloc},
            CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
        },
    },
};

const RESTORE_KEY: &str = "audio-output-restore";
const ROLES: [ERole; 3] = [eConsole, eMultimedia, eCommunications];
static ROUTING: Mutex<()> = Mutex::new(());

#[derive(Serialize)]
struct OutputDevice {
    id: String,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct SavedOutput {
    defaults: [String; 3],
    mutes: Vec<(String, bool)>,
    virtual_volume: (String, f32),
}

// Windows has no documented setter for the default render endpoint. Keep the
// PolicyConfig ABI isolated here; failure leaves the saved restore data intact.
#[repr(transparent)]
#[derive(Clone)]
struct PolicyConfig(IUnknown);
// SAFETY: this transparent COM pointer uses the IPolicyConfig IID and vtable
// layout; IUnknown owns the reference count.
unsafe impl Interface for PolicyConfig {
    type Vtable = PolicyConfigVTable;
    const IID: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
}
#[repr(C)]
struct PolicyConfigVTable {
    base: IUnknown_Vtbl,
    unused_methods: [usize; 10],
    set_default: unsafe extern "system" fn(*mut c_void, PCWSTR, ERole) -> HRESULT,
}

struct ComApartment(bool);
impl ComApartment {
    fn new() -> ResultType<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            return Ok(Self(false));
        }
        hr.ok()?;
        Ok(Self(true))
    }
}
impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn enumerator() -> ResultType<IMMDeviceEnumerator> {
    Ok(unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? })
}

fn take_string(value: PWSTR) -> ResultType<String> {
    let result = unsafe { value.to_string() };
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    Ok(result?)
}

fn device_id(device: &IMMDevice) -> ResultType<String> {
    take_string(unsafe { device.GetId()? })
}

fn property(device: &IMMDevice, key: &PROPERTYKEY) -> ResultType<String> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ)? };
    let mut value = unsafe { store.GetValue(key)? };
    let result = unsafe { PropVariantToStringAlloc(&value) }
        .map_err(Into::into)
        .and_then(take_string);
    unsafe { PropVariantClear(&mut value)? };
    result
}

fn outputs(enumerator: &IMMDeviceEnumerator) -> ResultType<Vec<IMMDevice>> {
    let devices = unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)? };
    (0..unsafe { devices.GetCount()? })
        .map(|i| Ok(unsafe { devices.Item(i)? }))
        .collect()
}

fn get_device(enumerator: &IMMDeviceEnumerator, id: &str) -> ResultType<IMMDevice> {
    let id: Vec<_> = id.encode_utf16().chain(Some(0)).collect();
    Ok(unsafe { enumerator.GetDevice(PCWSTR(id.as_ptr()))? })
}

fn volume(device: &IMMDevice) -> ResultType<IAudioEndpointVolume> {
    Ok(unsafe { device.Activate(CLSCTX_ALL, None)? })
}

fn supported_driver(name: &str) -> bool {
    // Check the driver interface name, not the user-editable speaker name.
    matches!(
        name,
        "VB-Audio Virtual Cable"
            | "VB-Audio Point"
            | "VB-Audio Cable A"
            | "VB-Audio Cable B"
            | "VB-Audio Cable C"
            | "VB-Audio Cable D"
    )
}

fn virtual_output(device: &IMMDevice) -> ResultType<bool> {
    let endpoint: IMMEndpoint = device.cast()?;
    Ok(unsafe { endpoint.GetDataFlow()? } == eRender
        && unsafe { device.GetState()? } == DEVICE_STATE_ACTIVE
        && supported_driver(&property(device, &PKEY_DeviceInterface_FriendlyName)?))
}

fn policy() -> ResultType<PolicyConfig> {
    Ok(unsafe {
        CoCreateInstance(
            &GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9),
            None,
            CLSCTX_ALL,
        )?
    })
}

fn set_default(policy: &PolicyConfig, id: &str, role: ERole) -> ResultType<()> {
    let id: Vec<_> = id.encode_utf16().chain(Some(0)).collect();
    unsafe { (policy.vtable().set_default)(policy.as_raw(), PCWSTR(id.as_ptr()), role).ok()? };
    Ok(())
}

fn saved_output() -> ResultType<Option<SavedOutput>> {
    let json = LocalConfig::get_option_from_file(RESTORE_KEY);
    if json.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        serde_json::from_str(&json).context("Cannot read saved audio output settings")?,
    ))
}

pub(crate) fn devices_json() -> ResultType<String> {
    let _lock = ROUTING.lock().unwrap();
    let _com = ComApartment::new()?;
    let enumerator = enumerator()?;
    let mut devices = Vec::new();
    for device in outputs(&enumerator)? {
        if virtual_output(&device)? {
            devices.push(OutputDevice {
                id: device_id(&device)?,
                name: property(&device, &PKEY_Device_FriendlyName)?,
            });
        }
    }
    Ok(serde_json::json!({
        "devices": devices,
        "selected": crate::ui_interface::get_option(OPTION_REMOTE_ONLY_AUDIO_DEVICE),
        "can_restore": saved_output()?.is_some(),
    })
    .to_string())
}

fn selected_device() -> String {
    crate::ui_interface::get_option(OPTION_REMOTE_ONLY_AUDIO_DEVICE)
}

fn restore_output(
    enumerator: &IMMDeviceEnumerator,
    policy: &PolicyConfig,
    saved: &SavedOutput,
) -> ResultType<()> {
    let mut restored_defaults = Vec::new();
    for (role, endpoint) in ROLES.iter().zip(&saved.defaults) {
        match get_device(enumerator, endpoint) {
            Ok(device) if unsafe { device.GetState()? } == DEVICE_STATE_ACTIVE => {
                set_default(policy, endpoint, *role)?;
                restored_defaults.push(endpoint.clone());
            }
            Ok(_) => log::warn!("Saved audio output is no longer active: {endpoint}"),
            Err(err) => log::warn!("Cannot restore saved audio output {endpoint}: {err}"),
        }
    }

    for (endpoint, muted) in &saved.mutes {
        if let Ok(device) = get_device(enumerator, endpoint) {
            if unsafe { device.GetState()? } == DEVICE_STATE_ACTIVE {
                if let Err(err) = unsafe { volume(&device)?.SetMute(*muted, std::ptr::null()) } {
                    log::warn!("Cannot restore mute state for {endpoint}: {err}");
                }
            }
        }
    }

    if let Ok(device) = get_device(enumerator, &saved.virtual_volume.0) {
        if unsafe { device.GetState()? } == DEVICE_STATE_ACTIVE {
            if let Err(err) = unsafe {
                volume(&device)?.SetMasterVolumeLevelScalar(
                    saved.virtual_volume.1,
                    std::ptr::null(),
                )
            } {
                log::warn!("Cannot restore virtual audio volume: {err}");
            }
        }
    }

    // The restored physical output is deliberately left unmuted at zero
    // volume. Users can raise it normally after the remote session ends.
    let mut physical_output_restored = false;
    for endpoint in restored_defaults {
        if endpoint == saved.virtual_volume.0 || physical_output_restored {
            continue;
        }
        if let Ok(device) = get_device(enumerator, &endpoint) {
            if unsafe { device.GetState()? } == DEVICE_STATE_ACTIVE {
                unsafe {
                    let endpoint_volume = volume(&device)?;
                    endpoint_volume.SetMasterVolumeLevelScalar(0.0, std::ptr::null())?;
                    endpoint_volume.SetMute(false, std::ptr::null())?;
                }
                physical_output_restored = true;
            }
        }
    }
    if !physical_output_restored {
        for device in outputs(enumerator)? {
            if device_id(&device)? != saved.virtual_volume.0 && !virtual_output(&device)? {
                unsafe {
                    let endpoint_volume = volume(&device)?;
                    endpoint_volume.SetMasterVolumeLevelScalar(0.0, std::ptr::null())?;
                    endpoint_volume.SetMute(false, std::ptr::null())?;
                }
                physical_output_restored = true;
                break;
            }
        }
    }
    if !physical_output_restored {
        log::warn!("No physical audio output is available after disconnect");
    }
    Ok(())
}

fn restore_orphaned_virtual_output(enumerator: &IMMDeviceEnumerator) -> ResultType<bool> {
    let current = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole)? };
    if !virtual_output(&current)? {
        return Ok(false);
    }

    let mut physical = None;
    for device in outputs(enumerator)? {
        if !virtual_output(&device)? {
            physical = Some((device_id(&device)?, device));
            break;
        }
    }
    let Some((id, device)) = physical else {
        log::warn!("The default output is virtual, but no physical output is available");
        return Ok(false);
    };

    let policy = policy()?;
    for role in ROLES {
        set_default(&policy, &id, role)?;
    }

    unsafe {
        let endpoint_volume = volume(&device)?;
        endpoint_volume.SetMasterVolumeLevelScalar(0.0, std::ptr::null())?;
        endpoint_volume.SetMute(false, std::ptr::null())?;
    }
    log::info!("Recovered orphaned virtual audio output to physical endpoint {id}");
    Ok(true)
}

fn unmute_current_physical_output(enumerator: &IMMDeviceEnumerator) -> ResultType<bool> {
    let current = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole)? };
    if virtual_output(&current)? {
        return restore_orphaned_virtual_output(enumerator);
    }
    unsafe { volume(&current)?.SetMute(false, std::ptr::null())? };
    Ok(true)
}

fn activate_output(
    enumerator: &IMMDeviceEnumerator,
    policy: &PolicyConfig,
    id: &str,
    target: &IMMDevice,
) -> ResultType<()> {
    let devices = outputs(enumerator)?;
    if saved_output()?.is_none() {
        let mut defaults = [String::new(), String::new(), String::new()];
        for (i, role) in ROLES.iter().enumerate() {
            defaults[i] =
                device_id(&unsafe { enumerator.GetDefaultAudioEndpoint(eRender, *role)? })?;
        }
        let mutes = devices
            .iter()
            .map(|device| {
                Ok((
                    device_id(device)?,
                    unsafe { volume(device)?.GetMute()? }.as_bool(),
                ))
            })
            .collect::<ResultType<_>>()?;
        let virtual_volume = (id.to_owned(), unsafe {
            volume(target)?.GetMasterVolumeLevelScalar()?
        });
        let saved = serde_json::to_string(&SavedOutput {
            defaults,
            mutes,
            virtual_volume,
        })?;
        LocalConfig::set_option(RESTORE_KEY.to_owned(), saved.clone());
        if LocalConfig::get_option_from_file(RESTORE_KEY) != saved {
            bail!("Cannot save audio output settings; routing was not changed");
        }
    }

    // Mute physical endpoints only while a remote session is active.
    for device in &devices {
        if device_id(device)? != id {
            unsafe { volume(device)?.SetMute(true, std::ptr::null())? };
        }
    }
    for role in ROLES {
        set_default(policy, id, role)?;
    }
    unsafe {
        let endpoint_volume = volume(target)?;
        endpoint_volume.SetMasterVolumeLevelScalar(1.0, std::ptr::null())?;
        endpoint_volume.SetMute(false, std::ptr::null())?;
    }
    Ok(())
}

/// Executed by the local settings UI so default-endpoint policy is changed for
/// the interactive user, not the Windows service's LocalSystem account.
pub(crate) fn set_device(id: &str) -> ResultType<()> {
    let _lock = ROUTING.lock().unwrap();
    let _com = ComApartment::new()?;
    let enumerator = enumerator()?;
    if id.is_empty() {
        if let Some(saved) = saved_output()? {
            restore_output(&enumerator, &policy()?, &saved)?;
            LocalConfig::set_option(RESTORE_KEY.to_owned(), String::new());
        } else {
            unmute_current_physical_output(&enumerator)?;
        }
        persist_device("")?;
    } else {
        let target = get_device(&enumerator, id).context("Virtual audio device is unavailable")?;
        if !virtual_output(&target)? {
            bail!("Select a supported VB-CABLE playback device");
        }
        // Reject ambiguous friendly names before touching system audio. CPAL's
        // pinned version exposes names but not endpoint IDs.
        capture_device_for(&target)?;
        if let Some(saved) = saved_output()? {
            restore_output(&enumerator, &policy()?, &saved)?;
            LocalConfig::set_option(RESTORE_KEY.to_owned(), String::new());
        }
        persist_device(id)?;
    }
    log::info!(
        "Audio output mode changed to {}",
        if id.is_empty() {
            "Normal"
        } else {
            "Remote only"
        }
    );
    Ok(())
}

/// Synchronize Windows endpoint routing with the remote-session count. This is
/// called by the interactive UI process, because the service runs as
/// LocalSystem and cannot safely change the user's default audio policy.
pub(crate) fn set_remote_connection_state(active: bool) -> ResultType<()> {
    let _lock = ROUTING.lock().unwrap();
    let _com = ComApartment::new()?;
    let enumerator = enumerator()?;
    if active {
        let id = selected_device();
        if id.is_empty() || saved_output()?.is_some() {
            return Ok(());
        }
        let target = get_device(&enumerator, &id).context("Virtual audio device is unavailable")?;
        if !virtual_output(&target)? {
            bail!("Remote-only audio device is unavailable");
        }
        capture_device_for(&target)?;
        activate_output(&enumerator, &policy()?, &id, &target)?;
    } else if let Some(saved) = saved_output()? {
        restore_output(&enumerator, &policy()?, &saved)?;
        LocalConfig::set_option(RESTORE_KEY.to_owned(), String::new());
    } else if !selected_device().is_empty() {
        restore_orphaned_virtual_output(&enumerator)?;
    }
    Ok(())
}

fn persist_device(id: &str) -> ResultType<()> {
    crate::ui_interface::set_audio_output_config(id)?;
    crate::audio_service::restart();
    Ok(())
}

fn capture_device_for(endpoint: &IMMDevice) -> ResultType<cpal::Device> {
    let name = property(endpoint, &PKEY_Device_FriendlyName)?;
    let host = cpal::default_host();
    let mut matches = Vec::new();
    for device in host.output_devices()? {
        if device.name()? == name {
            matches.push(device);
        }
    }
    if matches.len() != 1 {
        bail!("Virtual audio device name is missing or ambiguous: {name}");
    }
    let device = matches.pop().context("Virtual audio device is unavailable")?;
    let channels = device.default_output_config()?.channels();
    if channels == 0 || channels > 8 {
        bail!("Select a virtual playback device with 1 to 8 channels, such as CABLE Input");
    }
    Ok(device)
}

pub(crate) fn capture_device() -> ResultType<Option<cpal::Device>> {
    let id = Config::get_option(OPTION_REMOTE_ONLY_AUDIO_DEVICE);
    if id.is_empty() {
        return Ok(None);
    }
    let _com = ComApartment::new()?;
    let endpoint = get_device(&enumerator()?, &id)?;
    if !virtual_output(&endpoint)? {
        bail!("Remote-only audio device is unavailable; refusing speaker fallback");
    }
    Ok(Some(capture_device_for(&endpoint)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_supported_cable_drivers_can_be_selected() {
        assert!(supported_driver("VB-Audio Virtual Cable"));
        assert!(supported_driver("VB-Audio Point"));
        assert!(supported_driver("VB-Audio Cable A"));
        for name in [
            "Speakers",
            "Senary Audio",
            "NVIDIA High Definition Audio",
            "Virtual Cable Speakers",
        ] {
            assert!(!supported_driver(name));
        }
    }

    #[test]
    #[ignore = "requires a running Windows Audio service; read-only device/COM smoke test"]
    fn enumerates_outputs_without_changing_system_audio() {
        let _com = ComApartment::new().unwrap();
        for device in outputs(&enumerator().unwrap()).unwrap() {
            assert!(!device_id(&device).unwrap().is_empty());
            assert!(!property(&device, &PKEY_Device_FriendlyName)
                .unwrap()
                .is_empty());
            virtual_output(&device).unwrap();
            let before = unsafe { volume(&device).unwrap().GetMute().unwrap() }.as_bool();
            if !virtual_output(&device).unwrap() {
                assert!(set_device(&device_id(&device).unwrap()).is_err());
                assert_eq!(unsafe { volume(&device).unwrap().GetMute().unwrap() }.as_bool(), before);
            }
        }
        policy().unwrap();
        assert!(set_device("nonexistent-rustdesk-audio-output").is_err());
    }
}
