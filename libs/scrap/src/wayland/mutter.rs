//! Direct GNOME Mutter ScreenCast/RemoteDesktop integration.
//!
//! Mutter explicitly labels these D-Bus interfaces private, so every use is
//! version-gated and the caller must retain the regular XDG portal fallback.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use dbus::{
    arg::{PropMap, RefArg, Variant},
    blocking::{stdintf::org_freedesktop_dbus::Properties, SyncConnection},
    message::MatchRule,
    Path,
};
use hbb_common::{anyhow, bail, config::Config, ResultType};

use super::{
    display::get_displays,
    pipewire::{PwStreamInfo, RdpInputBackend, RdpSessionInfo},
};

pub const CONFIG_OPTION: &str = "enable-gnome-mutter";

pub const SCREEN_CAST_NAME: &str = "org.gnome.Mutter.ScreenCast";
pub const SCREEN_CAST_PATH: &str = "/org/gnome/Mutter/ScreenCast";
pub const SCREEN_CAST_IFACE: &str = "org.gnome.Mutter.ScreenCast";
pub const SCREEN_CAST_SESSION_IFACE: &str = "org.gnome.Mutter.ScreenCast.Session";
pub const SCREEN_CAST_STREAM_IFACE: &str = "org.gnome.Mutter.ScreenCast.Stream";

pub const REMOTE_DESKTOP_NAME: &str = "org.gnome.Mutter.RemoteDesktop";
pub const REMOTE_DESKTOP_PATH: &str = "/org/gnome/Mutter/RemoteDesktop";
pub const REMOTE_DESKTOP_IFACE: &str = "org.gnome.Mutter.RemoteDesktop";
pub const REMOTE_DESKTOP_SESSION_IFACE: &str = "org.gnome.Mutter.RemoteDesktop.Session";

const MIN_SCREEN_CAST_VERSION: i32 = 4;
const MIN_REMOTE_DESKTOP_VERSION: i32 = 1;
const REQUIRED_DEVICE_TYPES: u32 = 1 | 2; // keyboard | pointer
const DBUS_TIMEOUT: Duration = Duration::from_secs(3);
const STREAM_TIMEOUT: Duration = Duration::from_secs(8);

#[inline]
pub fn option_enabled() -> bool {
    Config::get_option(CONFIG_OPTION) == "Y"
}

fn pair_i32(value: &dyn RefArg) -> Option<(i32, i32)> {
    let values = value
        .as_iter()?
        .filter_map(|item| item.as_i64())
        .collect::<Vec<_>>();
    if values.len() == 2 {
        Some((values[0] as i32, values[1] as i32))
    } else {
        None
    }
}

fn check_api(conn: &SyncConnection) -> ResultType<()> {
    let screen_cast = conn.with_proxy(SCREEN_CAST_NAME, SCREEN_CAST_PATH, DBUS_TIMEOUT);
    let screen_cast_version: i32 = screen_cast.get(SCREEN_CAST_IFACE, "Version")?;
    if screen_cast_version < MIN_SCREEN_CAST_VERSION {
        bail!(
            "Mutter ScreenCast API {} is older than required {}",
            screen_cast_version,
            MIN_SCREEN_CAST_VERSION
        );
    }

    let remote_desktop = conn.with_proxy(REMOTE_DESKTOP_NAME, REMOTE_DESKTOP_PATH, DBUS_TIMEOUT);
    let remote_desktop_version: i32 = remote_desktop.get(REMOTE_DESKTOP_IFACE, "Version")?;
    if remote_desktop_version < MIN_REMOTE_DESKTOP_VERSION {
        bail!(
            "Mutter RemoteDesktop API {} is older than required {}",
            remote_desktop_version,
            MIN_REMOTE_DESKTOP_VERSION
        );
    }
    let device_types: u32 = remote_desktop.get(REMOTE_DESKTOP_IFACE, "SupportedDeviceTypes")?;
    if device_types & REQUIRED_DEVICE_TYPES != REQUIRED_DEVICE_TYPES {
        bail!("Mutter RemoteDesktop lacks keyboard/pointer support ({device_types:#x})");
    }
    Ok(())
}

pub fn request_session() -> ResultType<RdpSessionInfo> {
    let conn = SyncConnection::new_session()?;
    check_api(&conn)?;

    let displays = get_displays();
    let connectors = displays
        .displays
        .iter()
        .filter(|display| !display.name.is_empty())
        .map(|display| display.name.clone())
        .collect::<Vec<_>>();
    if connectors.is_empty() {
        bail!("Mutter backend found no named active monitor connectors");
    }

    let remote_manager = conn.with_proxy(REMOTE_DESKTOP_NAME, REMOTE_DESKTOP_PATH, DBUS_TIMEOUT);
    let (remote_session_path,): (Path<'static>,) =
        remote_manager.method_call(REMOTE_DESKTOP_IFACE, "CreateSession", ())?;
    let remote_session = conn.with_proxy(
        REMOTE_DESKTOP_NAME,
        remote_session_path.clone(),
        DBUS_TIMEOUT,
    );
    let session_id: String = remote_session.get(REMOTE_DESKTOP_SESSION_IFACE, "SessionId")?;

    let mut session_properties = PropMap::new();
    session_properties.insert(
        "remote-desktop-session-id".into(),
        Variant(Box::new(session_id)),
    );
    let screen_manager = conn.with_proxy(SCREEN_CAST_NAME, SCREEN_CAST_PATH, DBUS_TIMEOUT);
    let (screen_session_path,): (Path<'static>,) =
        screen_manager.method_call(SCREEN_CAST_IFACE, "CreateSession", (session_properties,))?;
    let screen_session = conn.with_proxy(SCREEN_CAST_NAME, screen_session_path, DBUS_TIMEOUT);

    let pending_paths = Arc::new(Mutex::new(HashMap::<String, u32>::new()));
    let mut stream_paths = Vec::<(String, Path<'static>)>::new();
    let mut _match_tokens = Vec::new();

    for connector in connectors {
        let mut properties = PropMap::new();
        properties.insert("cursor-mode".into(), Variant(Box::new(0u32)));
        properties.insert("is-recording".into(), Variant(Box::new(false)));
        let (stream_path,): (Path<'static>,) = screen_session.method_call(
            SCREEN_CAST_SESSION_IFACE,
            "RecordMonitor",
            (connector.clone(), properties),
        )?;

        let path_key = stream_path.to_string();
        let nodes = pending_paths.clone();
        let mut rule = MatchRule::new_signal(SCREEN_CAST_STREAM_IFACE, "PipeWireStreamAdded");
        rule.path = Some(stream_path.clone());
        _match_tokens.push(conn.add_match(rule, move |(node_id,): (u32,), _, _| {
            nodes.lock().unwrap().insert(path_key.clone(), node_id);
            true
        })?);
        stream_paths.push((connector, stream_path));
    }

    let _: () = remote_session.method_call(REMOTE_DESKTOP_SESSION_IFACE, "Start", ())?;

    let deadline = Instant::now() + STREAM_TIMEOUT;
    while pending_paths.lock().unwrap().len() < stream_paths.len() && Instant::now() < deadline {
        conn.process(Duration::from_millis(50))?;
    }
    if pending_paths.lock().unwrap().len() != stream_paths.len() {
        bail!(
            "Mutter announced {}/{} PipeWire streams before timeout",
            pending_paths.lock().unwrap().len(),
            stream_paths.len()
        );
    }

    let mut streams = Vec::with_capacity(stream_paths.len());
    for (connector, object_path) in stream_paths {
        let object_path_key = object_path.to_string();
        let node_id = pending_paths
            .lock()
            .unwrap()
            .get(&object_path_key)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("missing PipeWire node for {connector}"))?;
        let stream_proxy = conn.with_proxy(SCREEN_CAST_NAME, object_path.clone(), DBUS_TIMEOUT);
        let parameters: PropMap = stream_proxy.get(SCREEN_CAST_STREAM_IFACE, "Parameters")?;
        let position = parameters
            .get("position")
            .and_then(|value| pair_i32(value.0.as_ref()))
            .unwrap_or((0, 0));
        let size = parameters
            .get("size")
            .and_then(|value| pair_i32(value.0.as_ref()))
            .filter(|(width, height)| *width > 0 && *height > 0)
            .map(|(width, height)| (width as usize, height as usize))
            .ok_or_else(|| anyhow::anyhow!("Mutter stream {connector} has no size"))?;
        streams.push(PwStreamInfo::new_mutter(
            node_id as u64,
            object_path,
            position,
            size,
        ));
    }

    Ok(RdpSessionInfo {
        conn: Arc::new(conn),
        streams,
        fd: None,
        session: remote_session_path,
        input_backend: RdpInputBackend::Mutter,
        close_when_idle: true,
        is_support_restore_token: false,
        resolution: Arc::new(Mutex::new(None)),
    })
}
