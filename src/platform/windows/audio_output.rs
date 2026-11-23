//! Host-side audio routing. The virtual endpoint deliberately survives capture
//! teardown: disconnecting restores the physical output exactly as it was.

use base::config::keys::{OPTION_REMOTE_ONLY_AUDIO_DEVICE, OPTION_REMOTE_ONLY_AUDIO_MODE};
use cpal::traits::{DeviceTrait, HostTrait};
use hbb_common::{
    anyhow::{bail, Context},
    config::{Config, LocalConfig},
    log, ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::{
    ffi::c_void,
    path::PathBuf,
    sync::{mpsc, OnceLock},
    thread,
    time::{Duration, Instant},
};
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
const STATE_FILE: &str = "audio-output-state.json";
const AUTO_DEVICE: &str = "__rustdesk_auto_vb_cable__";
const ROLES: [ERole; 3] = [eConsole, eMultimedia, eCommunications];

#[derive(Serialize)]
struct OutputDevice {
    id: String,
    name: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct SavedOutput {
    defaults: [String; 3],
    mutes: Vec<(String, bool)>,
    #[serde(default)]
    volumes: Vec<(String, f32)>,
    virtual_volume: (String, f32),
}

#[derive(Serialize, Deserialize, Default)]
struct AudioOutputState {
    switched: bool,
    original_defaults: [String; 3],
    original_mutes: Vec<(String, bool)>,
    #[serde(default)]
    original_volumes: Vec<(String, f32)>,
    virtual_volume: (String, f32),
}

enum Command {
    Configure(String, mpsc::Sender<Result<(), String>>),
    ConnectionCount(usize),
    Status(mpsc::Sender<Result<String, String>>),
}

struct WorkerState {
    connection_count: usize,
    selected: String,
    last_error: String,
    retry_pending: bool,
    retry_after: Instant,
}

static WORKER: OnceLock<mpsc::Sender<Command>> = OnceLock::new();

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

fn state_path() -> PathBuf {
    Config::path(STATE_FILE)
}

fn state_to_saved(state: AudioOutputState) -> Option<SavedOutput> {
    if !state.switched {
        return None;
    }
    Some(SavedOutput {
        defaults: state.original_defaults,
        mutes: state.original_mutes,
        volumes: state.original_volumes,
        virtual_volume: state.virtual_volume,
    })
}

fn load_state() -> ResultType<Option<SavedOutput>> {
    let path = state_path();
    match std::fs::read_to_string(&path) {
        Ok(value) if !value.trim().is_empty() => Ok(state_to_saved(
            serde_json::from_str(&value).context("Cannot read audio output state")?,
        )),
        Ok(_) => Ok(None),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn save_state(saved: &SavedOutput) -> ResultType<()> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let state = AudioOutputState {
        switched: true,
        original_defaults: saved.defaults.clone(),
        original_mutes: saved.mutes.clone(),
        original_volumes: saved.volumes.clone(),
        virtual_volume: saved.virtual_volume.clone(),
    };
    std::fs::write(&path, serde_json::to_vec_pretty(&state)?)?;
    Ok(())
}

fn clear_state() -> ResultType<()> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&AudioOutputState::default())?,
    )?;
    Ok(())
}

fn saved_output() -> ResultType<Option<SavedOutput>> {
    if let Some(saved) = load_state()? {
        return Ok(Some(saved));
    }
    // Migrate the state written by the first implementation without deleting it.
    let json = LocalConfig::get_option_from_file(RESTORE_KEY);
    if json.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        serde_json::from_str(&json).context("Cannot read saved audio output settings")?,
    ))
}

fn endpoint_priority(name: &str, channels: u16) -> u8 {
    let lower = name.to_ascii_lowercase();
    if (lower.contains("speakers") || name.contains("扬声器")) && channels == 2 {
        0
    } else if lower.contains("cable in 16 ch") || lower.contains("cable input") {
        1
    } else {
        2
    }
}

fn auto_selected_device(enumerator: &IMMDeviceEnumerator) -> ResultType<Option<(String, String)>> {
    let mut candidates = Vec::new();
    for device in outputs(enumerator)? {
        if !virtual_output(&device)? {
            continue;
        }
        let id = device_id(&device)?;
        let name = property(&device, &PKEY_Device_FriendlyName)?;
        let capture = match capture_device_for(&device) {
            Ok(capture) => capture,
            Err(err) => {
                hbb_common::throttled_log!(
                    Duration::from_secs(10),
                    warn,
                    "Skipping unusable VB-CABLE endpoint {name}: {err}"
                );
                continue;
            }
        };
        let channels = capture.default_output_config()?.channels();
        candidates.push((endpoint_priority(&name, channels), id, name));
    }
    candidates.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.2.cmp(&right.2)));
    if let Some((_, id, name)) = candidates.into_iter().next() {
        return Ok(Some((id, name)));
    }
    Ok(None)
}

fn selected_device() -> String {
    let mode = configured_mode();
    if mode != "remote" {
        return String::new();
    }
    let configured = option_value(OPTION_REMOTE_ONLY_AUDIO_DEVICE);
    let result = (|| {
        let _com = ComApartment::new()?;
        let device_enumerator = enumerator()?;
        auto_selected_device(&device_enumerator)
    })();
    match result {
        Ok(Some((id, name))) => {
            log::trace!("Selected VB-CABLE endpoint automatically: {name} ({id})");
            id
        }
        Ok(None) => configured,
        Err(err) => {
            hbb_common::throttled_log!(
                Duration::from_secs(10),
                warn,
                "Cannot enumerate VB-CABLE endpoints: {err}"
            );
            configured
        }
    }
}

fn configured_mode() -> String {
    let mode = option_value(OPTION_REMOTE_ONLY_AUDIO_MODE);
    if !mode.is_empty() {
        return mode;
    }
    // Older builds used a non-empty endpoint option as the mode flag.
    if !option_value(OPTION_REMOTE_ONLY_AUDIO_DEVICE).is_empty() {
        "remote".to_owned()
    } else {
        "normal".to_owned()
    }
}

fn option_value(key: &str) -> String {
    if crate::common::is_server() {
        let value = Config::get_option(key);
        if !value.is_empty() {
            return value;
        }
    }
    let value = crate::ui_interface::get_option(key);
    if value.is_empty() {
        Config::get_option(key)
    } else {
        value
    }
}

fn worker_sender() -> &'static mpsc::Sender<Command> {
    WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let spawn_result = thread::Builder::new()
            .name("rustdesk-audio-routing".to_owned())
            .spawn(move || worker_loop(rx));
        if let Err(err) = spawn_result {
            log::error!("Cannot start audio routing worker: {err}");
        }
        tx
    })
}

fn send_sync<T>(build: impl FnOnce(mpsc::Sender<Result<T, String>>) -> Command) -> ResultType<T> {
    let (reply_tx, reply_rx) = mpsc::channel();
    worker_sender()
        .send(build(reply_tx))
        .map_err(|_| hbb_common::anyhow::anyhow!("Audio routing worker is unavailable"))?;
    match reply_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(hbb_common::anyhow::anyhow!(err)),
        Err(err) => Err(hbb_common::anyhow::anyhow!(
            "Audio routing worker timed out: {err}"
        )),
    }
}

fn worker_loop(rx: mpsc::Receiver<Command>) {
    let mut state = WorkerState {
        connection_count: 0,
        selected: selected_device(),
        last_error: String::new(),
        retry_pending: false,
        retry_after: Instant::now(),
    };
    if let Err(err) = handle_initialize(&mut state) {
        record_worker_error(&mut state, err);
    }
    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(command) => handle_command(command, &mut state),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if Instant::now() >= state.retry_after {
                    if let Err(err) = reconcile(&mut state) {
                        record_worker_error(&mut state, err);
                    } else {
                        state.last_error.clear();
                        state.retry_pending = false;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn record_worker_error(state: &mut WorkerState, err: impl std::fmt::Display) {
    state.last_error = err.to_string();
    state.retry_pending = true;
    state.retry_after = Instant::now() + Duration::from_secs(1);
    log::error!("Audio routing failed: {}", state.last_error);
}

fn handle_command(command: Command, state: &mut WorkerState) {
    match command {
        Command::Configure(id, reply) => {
            let result = configure_output(state, &id);
            if let Err(err) = &result {
                record_worker_error(state, err);
            } else {
                state.last_error.clear();
                state.retry_pending = false;
            }
            let _ = reply.send(result.map_err(|err| err.to_string()));
        }
        Command::ConnectionCount(count) => {
            if let Err(err) = update_connection_count(state, count) {
                record_worker_error(state, err);
            } else {
                state.last_error.clear();
                state.retry_pending = false;
            }
        }
        Command::Status(reply) => {
            let result = status_json(state).map_err(|err| err.to_string());
            let _ = reply.send(result);
        }
    }
}

fn restore_output(
    enumerator: &IMMDeviceEnumerator,
    policy: &PolicyConfig,
    saved: &SavedOutput,
) -> ResultType<()> {
    let mut all_roles_restored = true;
    for (role, endpoint) in ROLES.iter().zip(&saved.defaults) {
        match get_device(enumerator, endpoint) {
            Ok(device) => match unsafe { device.GetState() } {
                Ok(state) if state == DEVICE_STATE_ACTIVE => {
                    if let Err(err) = set_default(policy, endpoint, *role) {
                        all_roles_restored = false;
                        log::warn!("Cannot restore saved audio output {endpoint}: {err}");
                    }
                }
                Ok(_) => {
                    all_roles_restored = false;
                    log::warn!("Saved audio output is no longer active: {endpoint}");
                }
                Err(err) => {
                    all_roles_restored = false;
                    log::warn!("Cannot inspect saved audio output {endpoint}: {err}");
                }
            },
            Err(err) => {
                all_roles_restored = false;
                log::warn!("Cannot restore saved audio output {endpoint}: {err}");
            }
        }
    }

    // If any saved role disappeared, keep all roles on one active physical
    // endpoint instead of leaving a missing role pointed at the virtual cable.
    let mut fallback_restored = false;
    if !all_roles_restored {
        for device in outputs(enumerator)? {
            if !virtual_output(&device)? {
                let fallback_id = device_id(&device)?;
                let mut fallback_ok = true;
                for role in ROLES {
                    if let Err(err) = set_default(policy, &fallback_id, role) {
                        fallback_ok = false;
                        log::warn!("Cannot set fallback audio output {fallback_id}: {err}");
                    }
                }
                if fallback_ok {
                    log::warn!("Using fallback physical audio output after the saved endpoint disappeared: {fallback_id}");
                    fallback_restored = true;
                }
                break;
            }
        }
    }

    for (endpoint, level) in &saved.volumes {
        if let Ok(device) = get_device(enumerator, endpoint) {
            if matches!(unsafe { device.GetState() }, Ok(state) if state == DEVICE_STATE_ACTIVE) {
                let result = unsafe {
                    volume(&device)?.SetMasterVolumeLevelScalar(*level, std::ptr::null())
                };
                if let Err(err) = result {
                    log::warn!("Cannot restore volume state for {endpoint}: {err}");
                    return Err(err.into());
                }
            }
        }
    }

    for (endpoint, muted) in &saved.mutes {
        if let Ok(device) = get_device(enumerator, endpoint) {
            if matches!(unsafe { device.GetState() }, Ok(state) if state == DEVICE_STATE_ACTIVE) {
                let result = unsafe { volume(&device)?.SetMute(*muted, std::ptr::null()) };
                if let Err(err) = result {
                    log::warn!("Cannot restore mute state for {endpoint}: {err}");
                    return Err(err.into());
                }
            }
        }
    }

    if let Ok(device) = get_device(enumerator, &saved.virtual_volume.0) {
        if matches!(unsafe { device.GetState() }, Ok(state) if state == DEVICE_STATE_ACTIVE) {
            let result = unsafe {
                volume(&device)?
                    .SetMasterVolumeLevelScalar(saved.virtual_volume.1, std::ptr::null())
            };
            if let Err(err) = result {
                log::warn!("Cannot restore virtual audio volume: {err}");
                return Err(err.into());
            }
        }
    }

    if !all_roles_restored && !fallback_restored {
        bail!("No physical audio output is available after disconnect");
    }
    Ok(())
}

fn activate_output(
    enumerator: &IMMDeviceEnumerator,
    policy: &PolicyConfig,
    id: &str,
    target: &IMMDevice,
) -> ResultType<()> {
    let devices = outputs(enumerator)?;
    let mut saved = if let Some(saved) = saved_output()? {
        saved
    } else {
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
        let volumes = devices
            .iter()
            .map(|device| {
                Ok((device_id(device)?, unsafe {
                    volume(device)?.GetMasterVolumeLevelScalar()?
                }))
            })
            .collect::<ResultType<_>>()?;
        let virtual_volume = (id.to_owned(), unsafe {
            volume(target)?.GetMasterVolumeLevelScalar()?
        });
        SavedOutput {
            defaults,
            mutes,
            volumes,
            virtual_volume,
        }
    };

    for device in &devices {
        let endpoint = device_id(device)?;
        if endpoint == id {
            continue;
        }
        if !saved
            .mutes
            .iter()
            .any(|(saved_id, _)| saved_id == &endpoint)
        {
            saved.mutes.push((
                endpoint.clone(),
                unsafe { volume(device)?.GetMute()? }.as_bool(),
            ));
        }
        if !saved
            .volumes
            .iter()
            .any(|(saved_id, _)| saved_id == &endpoint)
        {
            saved.volumes.push((endpoint, unsafe {
                volume(device)?.GetMasterVolumeLevelScalar()?
            }));
        }
    }
    save_state(&saved)?;

    let result = (|| {
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
        Ok::<(), hbb_common::anyhow::Error>(())
    })();
    if let Err(err) = result {
        if let Err(restore_err) = restore_state() {
            bail!("Audio output switch failed: {err}; recovery failed: {restore_err}");
        }
        return Err(err);
    }
    Ok(())
}

fn restore_state() -> ResultType<()> {
    let _com = ComApartment::new()?;
    let enumerator = enumerator()?;
    if let Some(saved) = saved_output()? {
        restore_output(&enumerator, &policy()?, &saved)?;
        clear_state()?;
        LocalConfig::set_option(RESTORE_KEY.to_owned(), String::new());
    }
    Ok(())
}

fn handle_initialize(state: &mut WorkerState) -> ResultType<()> {
    state.selected = selected_device();
    if state.connection_count == 0 {
        restore_state()?;
    }
    Ok(())
}

fn reconcile(state: &mut WorkerState) -> ResultType<()> {
    let mode = configured_mode();
    let configured = selected_device();
    let changed = configured != state.selected;
    state.selected = configured;
    if mode != "remote" && saved_output()?.is_some() {
        restore_state()?;
    } else if mode == "remote" && state.connection_count == 0 && saved_output()?.is_some() {
        restore_state()?;
    } else if mode == "remote"
        && state.connection_count > 0
        && !state.selected.is_empty()
        && (changed || state.retry_pending || saved_output()?.is_none())
    {
        switch_to(&state.selected)?;
        crate::audio_service::restart();
    } else if mode == "remote" && state.connection_count > 0 && state.selected.is_empty() {
        bail!("No usable VB-CABLE playback endpoint is available");
    }
    Ok(())
}

fn validate_target(enumerator: &IMMDeviceEnumerator, id: &str) -> ResultType<IMMDevice> {
    if id.is_empty() {
        bail!("Select a VB-CABLE playback device before enabling Remote only");
    }
    let target = get_device(enumerator, id).context("Remote audio device is unavailable")?;
    if !virtual_output(&target)? {
        bail!("Remote audio device is unavailable; select a VB-CABLE playback device");
    }
    capture_device_for(&target)?;
    Ok(target)
}

fn switch_to(id: &str) -> ResultType<()> {
    let _com = ComApartment::new()?;
    let enumerator = enumerator()?;
    let target = validate_target(&enumerator, id)?;
    let policy = policy()?;
    activate_output(&enumerator, &policy, id, &target)
}

fn configure_output(state: &mut WorkerState, id: &str) -> ResultType<()> {
    if id.is_empty() {
        restore_state()?;
        if state.connection_count > 0 {
            crate::audio_service::restart();
        }
        state.selected.clear();
        log::info!("Audio output mode changed to Normal");
        return Ok(());
    }

    let _com = ComApartment::new()?;
    let enumerator = enumerator()?;
    let effective_id = if id == AUTO_DEVICE {
        auto_selected_device(&enumerator)?
            .map(|(id, _)| id)
            .context("No usable VB-CABLE playback endpoint is available")?
    } else {
        id.to_owned()
    };
    let target = validate_target(&enumerator, &effective_id)?;
    if state.connection_count > 0 {
        let policy = policy()?;
        activate_output(&enumerator, &policy, &effective_id, &target)?;
        crate::audio_service::restart();
    }
    state.selected = effective_id.clone();
    log::info!("Audio output mode changed to Remote only: {effective_id}");
    Ok(())
}

fn update_connection_count(state: &mut WorkerState, count: usize) -> ResultType<()> {
    let (started, ended) = connection_transition(state.connection_count, count);
    state.connection_count = count;
    if started {
        if configured_mode() == "remote" {
            if state.selected.is_empty() {
                bail!("No usable VB-CABLE playback endpoint is available");
            }
            switch_to(&state.selected)?;
            crate::audio_service::restart();
        }
    } else if ended {
        restore_state()?;
    }
    Ok(())
}

fn connection_transition(previous: usize, next: usize) -> (bool, bool) {
    (previous == 0 && next > 0, previous > 0 && next == 0)
}

fn status_json(state: &WorkerState) -> ResultType<String> {
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
        "available": auto_selected_device(&enumerator)?.is_some(),
        "selected": state.selected,
        "mode": configured_mode(),
        "connected_count": state.connection_count,
        "can_restore": saved_output()?.is_some(),
        "error": state.last_error,
    })
    .to_string())
}

/// Queue an endpoint change. COM and PolicyConfig calls run only on the worker.
pub(crate) fn set_device(id: &str) -> ResultType<()> {
    let mode = if id.is_empty() { "normal" } else { "remote" };
    if server_worker_available() {
        crate::ui_interface::set_audio_output_mode(mode)?;
        return Ok(());
    }
    send_sync(|reply| Command::Configure(id.to_owned(), reply))?;
    crate::ui_interface::set_audio_output_mode(mode)?;
    Ok(())
}

pub(crate) fn set_mode(mode: &str) -> ResultType<()> {
    match mode {
        "normal" | "remote" => {}
        _ => bail!("Unknown remote-only audio mode: {mode}"),
    }
    if server_worker_available() {
        crate::ui_interface::set_audio_output_mode(mode)?;
        return Ok(());
    }
    let id = if mode == "remote" { AUTO_DEVICE } else { "" };
    send_sync(|reply| Command::Configure(id.to_owned(), reply))?;
    crate::ui_interface::set_audio_output_mode(mode)?;
    Ok(())
}

pub(crate) fn apply_configured_mode() -> ResultType<()> {
    let id = selected_device();
    send_sync(|reply| Command::Configure(id, reply))?;
    Ok(())
}

fn server_worker_available() -> bool {
    !crate::common::is_server() && crate::common::check_process("--server", false)
}

/// The count is computed by the server from AUTHED_CONNS and delivered over IPC.
/// Only the per-session worker mutates the interactive user's endpoint policy.
pub(crate) fn set_remote_connection_state(count: usize) {
    if let Err(err) = worker_sender().send(Command::ConnectionCount(count)) {
        log::error!("Cannot queue remote audio connection count {count}: {err}");
    }
}

/// Start the per-session worker and recover a stale switched state even when
/// the Flutter window is not running. The Windows service itself runs in
/// Session 0 and must never call this function.
pub(crate) fn initialize() {
    let _ = worker_sender();
}

pub(crate) fn devices_json() -> ResultType<String> {
    if server_worker_available() {
        return enumerated_status_json();
    }
    send_sync(Command::Status)
}

fn enumerated_status_json() -> ResultType<String> {
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
        "available": auto_selected_device(&enumerator)?.is_some(),
        "selected": selected_device(),
        "mode": configured_mode(),
        "connected_count": 0,
        "can_restore": false,
        "error": "",
    })
    .to_string())
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
    let device = matches
        .pop()
        .context("Virtual audio device is unavailable")?;
    let channels = device.default_output_config()?.channels();
    if channels == 0 || channels > 8 {
        bail!("Select a virtual playback device with 1 to 8 channels, such as CABLE Input");
    }
    Ok(device)
}

pub(crate) fn capture_device() -> ResultType<Option<cpal::Device>> {
    let id = selected_device();
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
    fn preferred_vb_cable_endpoint_names_are_ranked_first() {
        assert_eq!(endpoint_priority("扬声器 (VB-Audio Virtual Cable)", 2), 0);
        assert_eq!(endpoint_priority("Speakers (VB-Audio Virtual Cable)", 2), 0);
        assert_eq!(endpoint_priority("Speakers (VB-Audio Virtual Cable)", 6), 2);
        assert_eq!(
            endpoint_priority("CABLE In 16 Ch (VB-Audio Virtual Cable)", 16),
            1
        );
        assert_eq!(
            endpoint_priority("CABLE Input (VB-Audio Virtual Cable)", 2),
            1
        );
        assert_eq!(endpoint_priority("VB-Audio Cable A", 2), 2);
    }

    #[test]
    fn connection_count_switches_only_on_edges() {
        assert_eq!(connection_transition(0, 1), (true, false));
        assert_eq!(connection_transition(0, 3), (true, false));
        assert_eq!(connection_transition(1, 2), (false, false));
        assert_eq!(connection_transition(3, 1), (false, false));
        assert_eq!(connection_transition(1, 0), (false, true));
        assert_eq!(connection_transition(3, 0), (false, true));
        assert_eq!(connection_transition(0, 0), (false, false));
    }

    #[test]
    fn legacy_state_without_volume_snapshot_remains_readable() {
        let state: AudioOutputState = serde_json::from_str(
            r#"{"switched":true,"original_defaults":["a","b","c"],"original_mutes":[],"virtual_volume":["v",1.0]}"#,
        )
        .unwrap();
        let saved = state_to_saved(state).unwrap();
        assert!(saved.volumes.is_empty());
        assert_eq!(saved.virtual_volume.0, "v");
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
                assert!(
                    validate_target(&enumerator().unwrap(), &device_id(&device).unwrap()).is_err()
                );
                assert_eq!(
                    unsafe { volume(&device).unwrap().GetMute().unwrap() }.as_bool(),
                    before
                );
            }
        }
        policy().unwrap();
        assert!(
            validate_target(&enumerator().unwrap(), "nonexistent-rustdesk-audio-output").is_err()
        );
    }
}
