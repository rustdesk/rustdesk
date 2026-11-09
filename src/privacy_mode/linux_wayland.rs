//! Privacy mode for Linux Wayland sessions on GNOME (Mutter).
//!
//! Like the macOS implementation: the monitors are blacked out through the CRTC gamma ramp,
//! which Mutter programs into the KMS gamma LUT after composition, so PipeWire and DRM capture
//! still see the desktop. Local input is blocked by the root service, which takes an exclusive
//! `EVIOCGRAB` on every physical input device; RustDesk's own uinput devices are left alone, so
//! remote input keeps working. The grab lives as long as the `--server`'s IPC connection to the
//! service, so the kernel and the service release it if either process dies.

use crate::{
    ipc::{self, Data},
    privacy_mode::{PrivacyMode, PrivacyModeState, INVALID_PRIVACY_MODE_CONN_ID},
};
use hbb_common::{anyhow::anyhow, bail, config::Config, log, tokio, ResultType};
use std::{collections::HashMap, sync::mpsc, thread::JoinHandle, time::Duration};

pub const PRIVACY_MODE_IMPL: &str = "privacy_mode_impl_wayland";

/// Starts with `_uinput_`, so it gets the root-service IPC directory and peer checks.
pub(crate) const IPC_POSTFIX_INPUT_GRAB: &str = "_uinput_privacy";
const DISPLAY_CONFIG: &str = "org.gnome.Mutter.DisplayConfig";
const DISPLAY_CONFIG_PATH: &str = "/org/gnome/Mutter/DisplayConfig";
const DBUS_TIMEOUT: Duration = Duration::from_secs(2);
/// Both workers start in turn within the 7.5 s `turn_on_privacy_async` waits for.
const START_TIMEOUT: Duration = Duration::from_secs(3);
const IPC_REPLY_TIMEOUT_MS: u64 = 2000;
/// Mutter rewrites the gamma ramp on night light, colour profile and monitor changes.
const GAMMA_REAPPLY_INTERVAL: Duration = Duration::from_millis(500);
const LOG_INTERVAL: Duration = Duration::from_secs(60);

type Ramps = (Vec<u16>, Vec<u16>, Vec<u16>);

fn state_file() -> std::path::PathBuf {
    Config::path("privacy_mode_wayland.state")
}

pub fn is_supported() -> bool {
    !crate::platform::linux::is_x11() && crate::platform::is_installed() && has_display_config()
}

fn has_display_config() -> bool {
    let check = || -> ResultType<bool> {
        let conn = dbus::blocking::Connection::new_session()?;
        let proxy = conn.with_proxy(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            DBUS_TIMEOUT,
        );
        let (has_owner,): (bool,) =
            proxy.method_call("org.freedesktop.DBus", "NameHasOwner", (DISPLAY_CONFIG,))?;
        Ok(has_owner)
    };
    check().unwrap_or_else(|e| {
        log::info!("Mutter DisplayConfig is not available: {}", e);
        false
    })
}

/// Restores the gamma ramps left black by a `--server` that died in privacy mode.
pub fn recover() {
    let path = state_file();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    let res = serde_json::from_str::<HashMap<u32, Ramps>>(&content)
        .map_err(|e| anyhow!(e))
        .and_then(|originals| {
            let conn = dbus::blocking::Connection::new_session()?;
            Gamma { conn, originals }.restore()
        });
    match res {
        Ok(()) => log::info!("Restored the monitors left in privacy mode by a previous run"),
        Err(e) => log::error!("Failed to restore the monitors from privacy mode: {}", e),
    }
}

pub struct PrivacyModeImpl {
    impl_key: String,
    conn_id: i32,
    input_grab: Option<Worker>,
    blackout: Option<Worker>,
}

impl PrivacyModeImpl {
    pub fn new(impl_key: &str) -> Self {
        Self {
            impl_key: impl_key.to_owned(),
            conn_id: INVALID_PRIVACY_MODE_CONN_ID,
            input_grab: None,
            blackout: None,
        }
    }

    fn release(&mut self) {
        // Show the screen again only once local input is back.
        if let Some(worker) = self.input_grab.take() {
            worker.stop();
        }
        if let Some(worker) = self.blackout.take() {
            worker.stop();
        }
        self.conn_id = INVALID_PRIVACY_MODE_CONN_ID;
    }
}

impl PrivacyMode for PrivacyModeImpl {
    fn is_async_privacy_mode(&self) -> bool {
        true
    }

    fn init(&self) -> ResultType<()> {
        Ok(())
    }

    fn clear(&mut self) {
        self.release();
    }

    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool> {
        if self.check_on_conn_id(conn_id)? {
            return Ok(true);
        }
        let input_grab = Worker::start("privacy-input-grab", |ready, stop| {
            hold_input_grab(ready, stop)
        })?;
        let blackout = match Worker::start("privacy-blackout", run_blackout) {
            Ok(worker) => worker,
            Err(e) => {
                input_grab.stop();
                return Err(e);
            }
        };
        self.input_grab = Some(input_grab);
        self.blackout = Some(blackout);
        self.conn_id = conn_id;
        Ok(true)
    }

    fn turn_off_privacy(
        &mut self,
        conn_id: i32,
        _state: Option<PrivacyModeState>,
    ) -> ResultType<()> {
        self.check_off_conn_id(conn_id)?;
        self.release();
        Ok(())
    }

    fn pre_conn_id(&self) -> i32 {
        self.conn_id
    }

    fn get_impl_key(&self) -> &str {
        &self.impl_key
    }
}

impl Drop for PrivacyModeImpl {
    fn drop(&mut self) {
        self.clear();
    }
}

/// A thread that reports once whether it started, then runs until `stop` is dropped.
/// Its channel sends can only fail once `start` has stopped waiting, so they are not checked.
struct Worker {
    stop: mpsc::Sender<()>,
    thread: JoinHandle<()>,
}

impl Worker {
    fn start<F>(name: &str, f: F) -> ResultType<Self>
    where
        F: FnOnce(mpsc::Sender<ResultType<()>>, mpsc::Receiver<()>) + Send + 'static,
    {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop, stop_rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || f(ready_tx, stop_rx))?;
        let worker = Self { stop, thread };
        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => Ok(worker),
            Ok(Err(e)) => {
                worker.stop();
                Err(e)
            }
            Err(e) => {
                worker.stop();
                bail!("{} did not start: {}", name, e)
            }
        }
    }

    fn stop(self) {
        drop(self.stop);
        if self.thread.join().is_err() {
            log::error!("A privacy mode worker thread panicked");
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn hold_input_grab(ready: mpsc::Sender<ResultType<()>>, stop: mpsc::Receiver<()>) {
    let mut conn = match ipc::connect(1000, IPC_POSTFIX_INPUT_GRAB).await {
        Ok(conn) => conn,
        Err(e) => {
            let _ = ready.send(Err(anyhow!(
                "Failed to connect to the RustDesk service: {}",
                e
            )));
            return;
        }
    };
    match conn.next_timeout(IPC_REPLY_TIMEOUT_MS).await {
        Ok(Some(Data::Empty)) => {}
        res => {
            let _ = ready.send(Err(anyhow!("Failed to block local input: {:?}", res)));
            return;
        }
    }
    let _ = ready.send(Ok(()));
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    std::thread::spawn(move || {
        let _ = stop.recv();
        let _ = stop_tx.send(());
    });
    tokio::select! {
        _ = stop_rx => {}
        res = conn.next() => {
            log::error!("The service ended the privacy mode input grab: {:?}", res);
        }
    }
}

fn run_blackout(ready: mpsc::Sender<ResultType<()>>, stop: mpsc::Receiver<()>) {
    let mut gamma = match Gamma::black_out() {
        Ok(gamma) => gamma,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    while let Err(mpsc::RecvTimeoutError::Timeout) = stop.recv_timeout(GAMMA_REAPPLY_INTERVAL) {
        if let Err(e) = gamma.apply() {
            hbb_common::throttled_log!(
                LOG_INTERVAL,
                warn,
                "Failed to keep the monitors black: {}",
                e
            );
        }
    }
    if let Err(e) = gamma.restore() {
        log::error!("Failed to restore the monitors after privacy mode: {}", e);
    }
}

struct Gamma {
    conn: dbus::blocking::Connection,
    /// Ramps to restore, per CRTC id.
    originals: HashMap<u32, Ramps>,
}

impl Gamma {
    fn black_out() -> ResultType<Self> {
        let mut gamma = Self {
            conn: dbus::blocking::Connection::new_session()?,
            originals: HashMap::new(),
        };
        if let Err(e) = gamma.apply() {
            if let Err(restore_err) = gamma.restore() {
                log::error!("Failed to restore the monitors: {}", restore_err);
            }
            return Err(e);
        }
        if gamma.originals.is_empty() {
            bail!("No active monitor to black out");
        }
        Ok(gamma)
    }

    fn proxy(&self) -> dbus::blocking::Proxy<'_, &dbus::blocking::Connection> {
        self.conn
            .with_proxy(DISPLAY_CONFIG, DISPLAY_CONFIG_PATH, DBUS_TIMEOUT)
    }

    /// Returns the configuration serial and the ids of the CRTCs that drive a monitor.
    fn active_crtcs(&self) -> ResultType<(u32, Vec<u32>)> {
        use dbus::{arg::RefArg, blocking::BlockingSender};
        // Read generically: a typed tuple for the CRTC struct decodes as an empty array.
        let msg = dbus::Message::new_method_call(
            DISPLAY_CONFIG,
            DISPLAY_CONFIG_PATH,
            DISPLAY_CONFIG,
            "GetResources",
        )
        .map_err(|e| anyhow!(e))?;
        let reply = self.conn.send_with_reply_and_block(msg, DBUS_TIMEOUT)?;
        let mut args = reply.iter_init();
        let serial: u32 = args.read()?;
        let crtcs = args
            .get_refarg()
            .ok_or_else(|| anyhow!("GetResources returned no CRTCs"))?;
        let mut active = Vec::new();
        for crtc in crtcs.as_iter().into_iter().flatten() {
            let mut fields = crtc.as_iter().into_iter().flatten();
            let id = fields.next().and_then(|f| f.as_u64());
            // Fields 1-6: winsys id, x, y, width, height, current mode (-1 when off).
            let current_mode = fields.nth(5).and_then(|f| f.as_i64());
            if let (Some(id), Some(mode)) = (id, current_mode) {
                if mode >= 0 {
                    active.push(id as u32);
                }
            }
        }
        Ok((serial, active))
    }

    /// Blacks out every active CRTC whose ramp is not black, keeping the ramp it had as the one
    /// to restore. A ramp found non-black was (re)written by Mutter, so it is the current one.
    fn apply(&mut self) -> ResultType<()> {
        let (serial, crtcs) = self.active_crtcs()?;
        let mut to_black_out = Vec::new();
        for crtc in crtcs {
            let ramps: Ramps =
                self.proxy()
                    .method_call(DISPLAY_CONFIG, "GetCrtcGamma", (serial, crtc))?;
            if is_black(&ramps) {
                continue;
            }
            let size = ramps.0.len();
            if size == 0 {
                bail!("The gamma ramp of CRTC {} cannot be changed", crtc);
            }
            self.originals.insert(crtc, ramps);
            to_black_out.push((crtc, size));
        }
        if to_black_out.is_empty() {
            return Ok(());
        }
        // Saved first, so that `recover()` can undo a blackout this process dies in.
        std::fs::write(state_file(), serde_json::to_string(&self.originals)?)?;
        for (crtc, size) in to_black_out {
            let zero = vec![0u16; size];
            self.proxy().method_call::<(), _, _, _>(
                DISPLAY_CONFIG,
                "SetCrtcGamma",
                (serial, crtc, &zero[..], &zero[..], &zero[..]),
            )?;
        }
        Ok(())
    }

    fn restore(self) -> ResultType<()> {
        let (serial, crtcs) = self.active_crtcs()?;
        for (crtc, (r, g, b)) in &self.originals {
            if crtcs.contains(crtc) {
                self.proxy().method_call::<(), _, _, _>(
                    DISPLAY_CONFIG,
                    "SetCrtcGamma",
                    (serial, *crtc, &r[..], &g[..], &b[..]),
                )?;
            }
        }
        let path = state_file();
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

fn is_black(ramps: &Ramps) -> bool {
    !ramps.0.is_empty()
        && [&ramps.0, &ramps.1, &ramps.2]
            .iter()
            .all(|ramp| ramp.iter().all(|v| *v == 0))
}

/// Root service side: blocks local input while a `--server` holds the IPC connection.
pub mod service {
    use super::*;
    use std::path::PathBuf;

    const RESCAN_INTERVAL: Duration = Duration::from_secs(1);
    /// RustDesk's own uinput devices: "RustDesk UInput Keyboard", "RustDesk DRM display wake",
    /// and the uinput mouse (`src/server/uinput.rs`).
    const OWN_DEVICE_PREFIXES: [&str; 2] = ["RustDesk ", "mouce-library-fake-mouse"];
    /// Power and sleep buttons stay usable.
    const SYSTEM_KEYS: [evdev::Key; 4] = [
        evdev::Key::KEY_POWER,
        evdev::Key::KEY_SLEEP,
        evdev::Key::KEY_WAKEUP,
        evdev::Key::KEY_SUSPEND,
    ];

    pub(crate) fn spawn_handler(mut stream: ipc::Connection) {
        tokio::spawn(async move {
            let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
            let (stop_tx, stop_rx) = mpsc::channel::<()>();
            let grabber = std::thread::spawn(move || grab_input(ready_tx, stop_rx));
            if ready_rx.await.is_ok() {
                hbb_common::allow_err!(stream.send(&Data::Empty).await);
                while let Ok(Some(_)) = stream.next().await {}
            }
            drop(stop_tx);
            if !matches!(
                tokio::task::spawn_blocking(move || grabber.join()).await,
                Ok(Ok(()))
            ) {
                log::error!("The privacy mode input grab thread panicked");
            }
        });
    }

    fn grab_input(ready: tokio::sync::oneshot::Sender<()>, stop: mpsc::Receiver<()>) {
        let mut grabbed: HashMap<PathBuf, evdev::Device> = HashMap::new();
        grab_new_devices(&mut grabbed);
        log::info!("Privacy mode blocks {} local input devices", grabbed.len());
        let _ = ready.send(());
        while let Err(mpsc::RecvTimeoutError::Timeout) = stop.recv_timeout(RESCAN_INTERVAL) {
            grab_new_devices(&mut grabbed);
        }
        for (path, device) in grabbed.iter_mut() {
            if let Err(e) = device.ungrab() {
                log::warn!("Privacy mode failed to release {:?}: {}", path, e);
            }
        }
        log::info!(
            "Privacy mode released {} local input devices",
            grabbed.len()
        );
    }

    fn grab_new_devices(grabbed: &mut HashMap<PathBuf, evdev::Device>) {
        // A removed device fails the ioctl, and its event node may be reused by the next one.
        grabbed.retain(|_, device| device.get_key_state().is_ok());
        let Ok(entries) = std::fs::read_dir("/dev/input") else {
            return;
        };
        for path in entries.flatten().map(|e| e.path()) {
            let is_event_node = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("event"));
            if !is_event_node || grabbed.contains_key(&path) {
                continue;
            }
            let Ok(mut device) = evdev::Device::open(&path) else {
                continue;
            };
            if !should_grab(
                device.name().unwrap_or_default(),
                device.supported_keys().map(|keys| keys.iter().collect()),
                device
                    .supported_relative_axes()
                    .is_some_and(|a| a.iter().next().is_some()),
                device
                    .supported_absolute_axes()
                    .is_some_and(|a| a.iter().next().is_some()),
            ) {
                continue;
            }
            // The desktop would keep a key held at grab time pressed; try again next scan.
            if device
                .get_key_state()
                .map_or(true, |keys| keys.iter().next().is_some())
            {
                continue;
            }
            match device.grab() {
                Ok(()) => {
                    log::debug!("Privacy mode grabbed {:?} {:?}", path, device.name());
                    grabbed.insert(path, device);
                }
                Err(e) => {
                    hbb_common::throttled_log!(
                        LOG_INTERVAL,
                        warn,
                        "Privacy mode failed to grab {:?}: {}",
                        path,
                        e
                    );
                }
            }
        }
    }

    fn should_grab(
        name: &str,
        keys: Option<Vec<evdev::Key>>,
        has_relative: bool,
        has_absolute: bool,
    ) -> bool {
        if OWN_DEVICE_PREFIXES.iter().any(|p| name.starts_with(p)) {
            return false;
        }
        let has_input_keys = keys.is_some_and(|keys| keys.iter().any(|k| !SYSTEM_KEYS.contains(k)));
        has_input_keys || has_relative || has_absolute
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use evdev::Key;

        #[test]
        fn grabs_physical_input_but_not_own_devices_or_power_buttons() {
            assert!(should_grab(
                "USB Keyboard",
                Some(vec![Key::KEY_A]),
                false,
                false
            ));
            assert!(should_grab(
                "USB Mouse",
                Some(vec![Key::BTN_LEFT]),
                true,
                false
            ));
            assert!(should_grab(
                "USB Receiver Consumer Control",
                Some(vec![Key::KEY_VOLUMEUP, Key::KEY_VOLUMEDOWN]),
                true,
                false
            ));
            assert!(!should_grab(
                "Power Button",
                Some(vec![Key::KEY_POWER]),
                false,
                false
            ));
            assert!(!should_grab("Lid Switch", None, false, false));
            assert!(!should_grab(
                "RustDesk UInput Keyboard",
                Some(vec![Key::KEY_A]),
                false,
                false
            ));
            assert!(!should_grab(
                "mouce-library-fake-mouse",
                Some(vec![Key::BTN_LEFT]),
                true,
                false
            ));
        }
    }
}
