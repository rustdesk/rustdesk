//! Privacy mode for X11 sessions ("RDP-like" console blackout).
//!
//! While active:
//! * every active CRTC gets an all-zero gamma ramp, so the physical monitors show black,
//!   while the X framebuffer (what screen capture reads) is untouched;
//! * every physical input device is actively grabbed by us (XIGrabDevice). A grabbed slave is
//!   detached from its master, so the desktop gets nothing from it, but we still see its events.
//!   If the process dies, the X server drops the grab and the device works again. A device that
//!   cannot be grabbed is disabled through the XInput "Device Enabled" property instead.
//!   XTEST devices, which remote input is injected through, are never touched.
//!
//! Local takeover (like logging on at the console of a Windows machine used over RDP):
//! a key press or click on the physical keyboard/mouse locks the session with the desktop's own
//! screen locker and brings the console back, showing only the lock screen. If the person at the
//! console unlocks it, [`Options::on_takeover`] is called (the host ends the remote session) and
//! privacy mode ends. If nobody at the console types for [`Options::suspend_idle`], or the lock
//! screen is unlocked remotely, the console is blacked out again.
//!
//! A background thread re-applies everything every [`ENFORCE_INTERVAL`], because the desktop
//! (night light, colour profiles, monitor hotplug) may reset gamma and new devices come up.
//!
//! The state needed to undo everything is written to a file, so that if the process dies while
//! privacy mode is on, [`recover`] (called on the next start) brings the console back.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection,
    protocol::{
        randr::{self, ConnectionExt as _},
        xinput::{self, ConnectionExt as _},
        xproto::{self, AtomEnum, ConnectionExt as _},
        Event,
    },
    rust_connection::RustConnection,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub const ENFORCE_INTERVAL: Duration = Duration::from_millis(300);
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// How long to wait for the screen locker to come up after asking it to lock.
const LOCK_TIMEOUT: Duration = Duration::from_secs(4);
/// Minimum time between two takeover attempts that failed to lock the screen.
const TAKEOVER_RETRY: Duration = Duration::from_secs(5);

/// Device names containing one of these are never blocked: they carry remote input.
const KEEP_DEVICE_PATTERNS: &[&str] = &["XTEST", "RustDesk"];
/// Devices that are blocked but whose events do not count as "someone is at the console":
/// they can fire without a person (ACPI video/lid notifications, vendor WMI events).
const NOT_A_PERSON_PATTERNS: &[&str] = &[
    "Video Bus",
    "Power Button",
    "Sleep Button",
    "Lid Switch",
    "hotkeys",
    "WMI",
];

type Gamma = (Vec<u16>, Vec<u16>, Vec<u16>);

#[derive(Default, Debug, Clone, PartialEq)]
struct SavedState {
    /// CRTC id -> original gamma ramp.
    gamma: BTreeMap<u32, Gamma>,
    /// Names of the devices we disabled (ids may change after a restart, names do not).
    /// Grabbed devices are not recorded: the X server releases those by itself.
    devices: BTreeSet<String>,
}

fn connect() -> Result<(RustConnection, xproto::Window)> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    Ok((conn, root))
}

/// True if this is an X11 session with RandR >= 1.2 and XInput >= 2.0.
pub fn is_supported() -> bool {
    if std::env::var("XDG_SESSION_TYPE").map_or(false, |t| t == "wayland") {
        return false;
    }
    let Ok((conn, _)) = connect() else {
        return false;
    };
    let randr_ok = conn
        .randr_query_version(1, 2)
        .ok()
        .and_then(|c| c.reply().ok())
        .map_or(false, |v| v.major_version > 1 || v.minor_version >= 2);
    let xi_ok = conn
        .xinput_xi_query_version(2, 0)
        .ok()
        .and_then(|c| c.reply().ok())
        .map_or(false, |v| v.major_version >= 2);
    randr_ok && xi_ok
}

// ---------------------------------------------------------------------------------------------
// Gamma
// ---------------------------------------------------------------------------------------------

fn active_crtcs(conn: &RustConnection, root: xproto::Window) -> Result<Vec<randr::Crtc>> {
    let res = conn.randr_get_screen_resources_current(root)?.reply()?;
    let mut out = Vec::new();
    for crtc in res.crtcs {
        let info = conn
            .randr_get_crtc_info(crtc, res.config_timestamp)?
            .reply()?;
        if info.mode != 0 && !info.outputs.is_empty() {
            out.push(crtc);
        }
    }
    Ok(out)
}

fn is_black((r, g, b): &Gamma) -> bool {
    r.iter().chain(g).chain(b).all(|v| *v == 0)
}

fn linear_ramp(size: usize) -> Vec<u16> {
    if size <= 1 {
        return vec![u16::MAX; size];
    }
    (0..size)
        .map(|i| ((i as u64 * u16::MAX as u64) / (size as u64 - 1)) as u16)
        .collect()
}

fn get_gamma(conn: &RustConnection, crtc: randr::Crtc) -> Result<Gamma> {
    let g = conn.randr_get_crtc_gamma(crtc)?.reply()?;
    Ok((g.red, g.green, g.blue))
}

fn set_gamma(conn: &RustConnection, crtc: randr::Crtc, (r, g, b): &Gamma) -> Result<()> {
    conn.randr_set_crtc_gamma(crtc, r, g, b)?.check()?;
    Ok(())
}

/// Blacks out every active CRTC; records the original ramp of each one it changes.
/// Returns true if anything had to be (re)applied.
fn blackout_all(
    conn: &RustConnection,
    root: xproto::Window,
    saved: &mut SavedState,
) -> Result<bool> {
    let mut changed = false;
    for crtc in active_crtcs(conn, root)? {
        let current = get_gamma(conn, crtc)?;
        if is_black(&current) {
            continue;
        }
        // First time we see it, or the desktop reset it (night light, profile, hotplug):
        // the ramp it has now is what should come back on turn-off.
        saved.gamma.insert(crtc, current.clone());
        let zero = vec![0u16; current.0.len()];
        set_gamma(conn, crtc, &(zero.clone(), zero.clone(), zero))?;
        changed = true;
    }
    Ok(changed)
}

fn restore_gamma(conn: &RustConnection, root: xproto::Window, saved: &SavedState) -> Result<()> {
    let mut first_err = None;
    for crtc in active_crtcs(conn, root)? {
        let current = match get_gamma(conn, crtc) {
            Ok(g) => g,
            Err(e) => {
                first_err.get_or_insert(e);
                continue;
            }
        };
        if !is_black(&current) {
            continue;
        }
        let target = match saved.gamma.get(&crtc) {
            Some(g) if g.0.len() == current.0.len() && !is_black(g) => g.clone(),
            // Unknown CRTC, size changed, or a black ramp got saved: fall back to identity.
            _ => {
                let l = linear_ramp(current.0.len());
                (l.clone(), l.clone(), l)
            }
        };
        if let Err(e) = set_gamma(conn, crtc, &target) {
            first_err.get_or_insert(e);
        }
    }
    first_err.map_or(Ok(()), Err)
}

// ---------------------------------------------------------------------------------------------
// Input devices
// ---------------------------------------------------------------------------------------------

struct InputAtoms {
    device_enabled: xproto::Atom,
}

fn input_atoms(conn: &RustConnection) -> Result<InputAtoms> {
    Ok(InputAtoms {
        device_enabled: conn.intern_atom(false, b"Device Enabled")?.reply()?.atom,
    })
}

fn name_of(info: &xinput::XIDeviceInfo) -> String {
    String::from_utf8_lossy(&info.name).into_owned()
}

fn is_physical(info: &xinput::XIDeviceInfo) -> bool {
    let is_slave = info.type_ == xinput::DeviceType::SLAVE_POINTER
        || info.type_ == xinput::DeviceType::SLAVE_KEYBOARD
        || info.type_ == xinput::DeviceType::FLOATING_SLAVE;
    let name = name_of(info);
    is_slave && !KEEP_DEVICE_PATTERNS.iter().any(|p| name.contains(p))
}

fn is_person(name: &str) -> bool {
    !NOT_A_PERSON_PATTERNS.iter().any(|p| name.contains(p))
}

fn set_device_enabled(
    conn: &RustConnection,
    atoms: &InputAtoms,
    id: xinput::DeviceId,
    enabled: bool,
) -> Result<()> {
    conn.xinput_xi_change_property(
        id,
        xproto::PropMode::REPLACE,
        atoms.device_enabled,
        AtomEnum::INTEGER.into(),
        1,
        &xinput::XIChangePropertyAux::Data8(vec![enabled as u8]),
    )?
    .check()?;
    Ok(())
}

fn grab_device(conn: &RustConnection, root: xproto::Window, id: xinput::DeviceId) -> Result<bool> {
    let mask = u32::from(xinput::XIEventMask::KEY_PRESS | xinput::XIEventMask::BUTTON_PRESS);
    let reply = conn
        .xinput_xi_grab_device(
            root,
            x11rb::CURRENT_TIME,
            x11rb::NONE,
            id,
            xproto::GrabMode::ASYNC,
            xproto::GrabMode::ASYNC,
            xinput::GrabOwner::NO_OWNER,
            &[mask],
        )?
        .reply()?;
    Ok(reply.status == xproto::GrabStatus::SUCCESS)
}

/// Re-enables the devices that `disabled` lists by name.
fn enable_inputs(
    conn: &RustConnection,
    atoms: &InputAtoms,
    disabled: &BTreeSet<String>,
) -> Result<()> {
    let mut first_err = None;
    for info in conn
        .xinput_xi_query_device(xinput::Device::ALL)?
        .reply()?
        .infos
    {
        let name = name_of(&info);
        if info.enabled || !is_physical(&info) || !disabled.contains(&name) {
            continue;
        }
        if let Err(e) = set_device_enabled(conn, atoms, info.deviceid, true) {
            log::warn!("x11 privacy: failed to re-enable {}: {}", name, e);
            first_err.get_or_insert(e);
        }
    }
    first_err.map_or(Ok(()), Err)
}

// ---------------------------------------------------------------------------------------------
// Screen locker (session D-Bus)
// ---------------------------------------------------------------------------------------------

const LOCKERS: &[(&str, &str, &[&str])] = &[
    // Cinnamon's Lock takes the away message as an argument.
    (
        "org.cinnamon.ScreenSaver",
        "/org/cinnamon/ScreenSaver",
        &["string:"],
    ),
    ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver", &[]),
    ("org.mate.ScreenSaver", "/org/mate/ScreenSaver", &[]),
    (
        "org.freedesktop.ScreenSaver",
        "/org/freedesktop/ScreenSaver",
        &[],
    ),
];

fn session_bus() -> String {
    std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_else(|_| {
        // SAFETY: getuid never fails.
        format!("unix:path=/run/user/{}/bus", unsafe { libc::getuid() })
    })
}

fn dbus_call(dest: &str, path: &str, method: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("dbus-send")
        .env("DBUS_SESSION_BUS_ADDRESS", session_bus())
        .args([
            "--session",
            "--print-reply=literal",
            "--reply-timeout=1000",
            &format!("--dest={dest}"),
            path,
            &format!("{dest}.{method}"),
        ])
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Asks the screen locker whether it is showing. `None` if no known locker answered.
pub fn is_screen_locked() -> Option<bool> {
    LOCKERS
        .iter()
        .find_map(|(dest, path, _)| dbus_call(dest, path, "GetActive", &[]))
        .map(|reply| reply.contains("true"))
}

/// Asks the desktop's screen locker to lock. Returns false if no locker took the request.
pub fn lock_screen() -> bool {
    for (dest, path, args) in LOCKERS {
        if dbus_call(dest, path, "Lock", args).is_some() {
            return true;
        }
    }
    Command::new("loginctl")
        .arg("lock-session")
        .status()
        .map_or(false, |s| s.success())
}

/// Waits (up to `timeout`) until the lock screen is up. Returns false on timeout or if no locker
/// answers. Used before the console is switched back on, so it never shows an unlocked desktop.
pub fn wait_for_screen_locked(timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match is_screen_locked() {
            Some(true) => {
                // Give the locker a moment to finish painting / fading in.
                std::thread::sleep(Duration::from_millis(500));
                return true;
            }
            Some(false) => std::thread::sleep(Duration::from_millis(100)),
            None => return false,
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------
// State file (crash recovery)
// ---------------------------------------------------------------------------------------------

fn serialize(state: &SavedState) -> String {
    let mut s = String::from("# rustdesk x11 privacy mode state; delete to forget\n");
    for (crtc, (r, g, b)) in &state.gamma {
        let join = |v: &Vec<u16>| {
            v.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        s += &format!("gamma {} {} {} {}\n", crtc, join(r), join(g), join(b));
    }
    for d in &state.devices {
        s += &format!("device {}\n", d);
    }
    s
}

fn deserialize(text: &str) -> SavedState {
    let mut state = SavedState::default();
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("device ") {
            state.devices.insert(name.to_owned());
        } else if let Some(rest) = line.strip_prefix("gamma ") {
            let parts: Vec<&str> = rest.split(' ').collect();
            let parse =
                |s: &str| -> Option<Vec<u16>> { s.split(',').map(|x| x.parse().ok()).collect() };
            if let [crtc, r, g, b] = parts[..] {
                if let (Ok(crtc), Some(r), Some(g), Some(b)) =
                    (crtc.parse(), parse(r), parse(g), parse(b))
                {
                    state.gamma.insert(crtc, (r, g, b));
                }
            }
        }
    }
    state
}

fn write_state(path: &Path, state: &SavedState) {
    let res = (|| -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        let mut f = fs::File::create(&tmp)?;
        f.write_all(serialize(state).as_bytes())?;
        f.sync_all()?;
        fs::rename(tmp, path)
    })();
    if let Err(e) = res {
        log::error!("x11 privacy: failed to write state file {:?}: {}", path, e);
    }
}

/// Undo a privacy mode left behind by a crashed process, if `state_file` exists.
/// Returns true if something was restored.
pub fn recover(state_file: &Path) -> Result<bool> {
    let Ok(text) = fs::read_to_string(state_file) else {
        return Ok(false);
    };
    let state = deserialize(&text);
    let (conn, root) = connect()?;
    let atoms = input_atoms(&conn)?;
    let r1 = enable_inputs(&conn, &atoms, &state.devices);
    let r2 = restore_gamma(&conn, root, &state);
    conn.flush()?;
    r1?;
    r2?;
    let _ = fs::remove_file(state_file);
    log::info!("x11 privacy: recovered console from {:?}", state_file);
    Ok(true)
}

// ---------------------------------------------------------------------------------------------
// Local takeover decision (pure, unit tested)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum SuspendedVerdict {
    /// Lock screen stays up, console stays on.
    Stay,
    /// Black out again (nobody at the console, or the remote side unlocked).
    Reactivate,
    /// The person at the console unlocked: hand the session over to them.
    Takeover,
}

/// Decides what to do while the console shows the lock screen after a local key press.
fn suspended_verdict(
    locked: Option<bool>,
    now: Instant,
    suspended_at: Instant,
    last_local: Option<Instant>,
    last_remote: Option<Instant>,
    idle: Duration,
) -> SuspendedVerdict {
    if locked == Some(false) {
        // Unlocked: whoever typed last did it.
        return match (last_local, last_remote) {
            (Some(l), Some(r)) if l > r => SuspendedVerdict::Takeover,
            (Some(_), None) => SuspendedVerdict::Takeover,
            _ => SuspendedVerdict::Reactivate,
        };
    }
    // Still locked (or the locker did not answer this time).
    let last = last_local.map_or(suspended_at, |l| l.max(suspended_at));
    if now.duration_since(last) >= idle {
        SuspendedVerdict::Reactivate
    } else {
        SuspendedVerdict::Stay
    }
}

// ---------------------------------------------------------------------------------------------
// Public handle
// ---------------------------------------------------------------------------------------------

pub type TakeoverCallback = Box<dyn Fn() + Send + 'static>;

pub struct Options {
    pub blackout: bool,
    pub block_input: bool,
    /// Let a person at the console get the session back by unlocking the lock screen.
    pub local_takeover: bool,
    /// While the console shows the lock screen, black it out again after this long without
    /// local input.
    pub suspend_idle: Duration,
    /// Called (from the background thread) when the person at the console unlocked the session.
    /// Privacy mode is already off by then; the host should end the remote session.
    pub on_takeover: Option<TakeoverCallback>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            blackout: true,
            block_input: true,
            local_takeover: true,
            suspend_idle: Duration::from_secs(30),
            on_takeover: None,
        }
    }
}

impl std::fmt::Debug for Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Options")
            .field("blackout", &self.blackout)
            .field("block_input", &self.block_input)
            .field("local_takeover", &self.local_takeover)
            .field("suspend_idle", &self.suspend_idle)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    /// Console black, physical input blocked.
    Active,
    /// Someone touched the console: it shows the lock screen, physical input works.
    Suspended,
    /// The person at the console unlocked; privacy mode is over.
    TakenOver,
}

/// Privacy mode is on for as long as this lives (or until [`PrivacyGuard::turn_off`]).
pub struct PrivacyGuard {
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    state_file: PathBuf,
}

struct Shared {
    saved: SavedState,
    phase: Phase,
}

impl PrivacyGuard {
    pub fn turn_on(opts: Options, state_file: PathBuf) -> Result<Self> {
        // Leftovers from a crash would otherwise be saved as "original" (black) values.
        let _ = recover(&state_file);

        let mut worker = Worker::new(opts, state_file.clone())?;
        // Write the (empty) state first, so a crash in the middle is still recoverable.
        write_state(&state_file, &worker.saved);
        if let Err(e) = worker.activate() {
            worker.restore_all();
            let _ = fs::remove_file(&state_file);
            return Err(e);
        }

        let shared = Arc::new(Mutex::new(Shared {
            saved: worker.saved.clone(),
            phase: Phase::Active,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (shared, stop) = (shared.clone(), stop.clone());
            std::thread::Builder::new()
                .name("x11-privacy".into())
                .spawn(move || worker.run(shared, stop))?
        };
        Ok(Self {
            shared,
            stop,
            thread: Some(thread),
            state_file,
        })
    }

    pub fn phase(&self) -> Phase {
        self.shared.lock().unwrap().phase
    }

    pub fn turn_off(mut self) -> Result<()> {
        self.do_turn_off()
    }

    fn do_turn_off(&mut self) -> Result<()> {
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        self.stop.store(true, Ordering::SeqCst);
        // The worker restores everything itself before it exits.
        let _ = thread.join();
        // Belt and braces, in case the worker could not: restore from the saved state.
        let saved = self.shared.lock().unwrap().saved.clone();
        let (conn, root) = connect()?;
        let atoms = input_atoms(&conn)?;
        let r1 = enable_inputs(&conn, &atoms, &saved.devices);
        let r2 = restore_gamma(&conn, root, &saved);
        conn.flush()?;
        r1?;
        r2?;
        let _ = fs::remove_file(&self.state_file);
        Ok(())
    }
}

impl Drop for PrivacyGuard {
    fn drop(&mut self) {
        if let Err(e) = self.do_turn_off() {
            log::error!("x11 privacy: failed to turn off on drop: {}", e);
        }
    }
}

struct Worker {
    conn: RustConnection,
    root: xproto::Window,
    atoms: InputAtoms,
    opts: Options,
    state_file: PathBuf,
    saved: SavedState,
    phase: Phase,
    /// Devices currently grabbed by us.
    grabbed: BTreeSet<xinput::DeviceId>,
    /// Device id -> (name, physical) for classifying input events.
    devices: HashMap<xinput::DeviceId, (String, bool)>,
    suspended_at: Instant,
    last_local: Option<Instant>,
    last_remote: Option<Instant>,
    last_failed_takeover: Option<Instant>,
}

impl Worker {
    fn new(opts: Options, state_file: PathBuf) -> Result<Self> {
        let (conn, root) = connect()?;
        let atoms = input_atoms(&conn)?;
        // Raw events tell us where input comes from (physical device or XTEST) in every phase.
        let raw = xinput::XIEventMask::RAW_KEY_PRESS | xinput::XIEventMask::RAW_BUTTON_PRESS;
        conn.xinput_xi_select_events(
            root,
            &[xinput::EventMask {
                deviceid: xinput::Device::ALL.into(),
                mask: vec![raw],
            }],
        )?
        .check()?;
        Ok(Self {
            conn,
            root,
            atoms,
            opts,
            state_file,
            saved: SavedState::default(),
            phase: Phase::Active,
            grabbed: BTreeSet::new(),
            devices: HashMap::new(),
            suspended_at: Instant::now(),
            last_local: None,
            last_remote: None,
            last_failed_takeover: None,
        })
    }

    /// Blacks out and blocks input; called on turn-on and periodically while active.
    /// Returns true if anything changed.
    fn activate(&mut self) -> Result<bool> {
        let mut changed = false;
        if self.opts.blackout {
            changed |= blackout_all(&self.conn, self.root, &mut self.saved)?;
        }
        if self.opts.block_input {
            changed |= self.block_inputs()?;
        }
        self.conn.flush()?;
        if changed {
            write_state(&self.state_file, &self.saved);
        }
        Ok(changed)
    }

    fn refresh_devices(&mut self) -> Result<Vec<xinput::XIDeviceInfo>> {
        let infos = self
            .conn
            .xinput_xi_query_device(xinput::Device::ALL)?
            .reply()?
            .infos;
        self.devices = infos
            .iter()
            .map(|i| (i.deviceid, (name_of(i), is_physical(i))))
            .collect();
        // Forget grabs of devices that went away.
        let present: BTreeSet<_> = infos.iter().map(|i| i.deviceid).collect();
        self.grabbed.retain(|id| present.contains(id));
        Ok(infos)
    }

    fn block_inputs(&mut self) -> Result<bool> {
        let mut changed = false;
        let mut locked = None;
        for info in self.refresh_devices()? {
            if !is_physical(&info) || !info.enabled || self.grabbed.contains(&info.deviceid) {
                continue;
            }
            let name = name_of(&info);
            match grab_device(&self.conn, self.root, info.deviceid) {
                Ok(true) => {
                    log::info!(
                        "x11 privacy: grabbed input device {} ({})",
                        info.deviceid,
                        name
                    );
                    self.grabbed.insert(info.deviceid);
                    changed = true;
                    continue;
                }
                Ok(false) => log::warn!("x11 privacy: {} is grabbed by someone else", name),
                Err(e) => log::warn!("x11 privacy: failed to grab {}: {}", name, e),
            }
            // While the lock screen is up, local input cannot reach the desktop, so keep the
            // device usable for a console logon and try grabbing it again on the next round.
            if *locked.get_or_insert_with(is_screen_locked) == Some(true) {
                continue;
            }
            // Could not grab it: disable it instead (no takeover from this device then).
            match set_device_enabled(&self.conn, &self.atoms, info.deviceid, false) {
                Ok(()) => {
                    log::info!(
                        "x11 privacy: disabled input device {} ({})",
                        info.deviceid,
                        name
                    );
                    self.saved.devices.insert(name);
                    changed = true;
                }
                // A device may vanish between query and change; keep going with the others.
                Err(e) => log::warn!("x11 privacy: failed to disable {}: {}", name, e),
            }
        }
        Ok(changed)
    }

    fn unblock_inputs(&mut self) {
        for id in std::mem::take(&mut self.grabbed) {
            if let Ok(c) = self.conn.xinput_xi_ungrab_device(x11rb::CURRENT_TIME, id) {
                let _ = c.check();
            }
        }
        if let Err(e) = enable_inputs(&self.conn, &self.atoms, &self.saved.devices) {
            log::warn!("x11 privacy: failed to re-enable inputs: {}", e);
        }
        self.saved.devices.clear();
    }

    /// Brings the console back (gamma and input). `saved` is kept for a later re-activation.
    fn restore_all(&mut self) {
        self.unblock_inputs();
        if let Err(e) = restore_gamma(&self.conn, self.root, &self.saved) {
            log::error!("x11 privacy: failed to restore gamma: {}", e);
        }
        let _ = self.conn.flush();
        write_state(&self.state_file, &self.saved);
    }

    /// Reads pending X events. Returns true if a person touched a physical device.
    fn drain_events(&mut self) -> bool {
        let mut local = false;
        let now = Instant::now();
        loop {
            let event = match self.conn.poll_for_event() {
                Ok(Some(e)) => e,
                Ok(None) => break,
                Err(e) => {
                    log::warn!("x11 privacy: X connection error: {}", e);
                    break;
                }
            };
            let source = match &event {
                Event::XinputKeyPress(e) => e.sourceid,
                Event::XinputButtonPress(e) => e.sourceid,
                Event::XinputRawKeyPress(e) => e.sourceid,
                Event::XinputRawButtonPress(e) => e.sourceid,
                _ => continue,
            };
            match self.devices.get(&source) {
                Some((name, true)) if is_person(name) => {
                    self.last_local = Some(now);
                    local = true;
                }
                Some((_, false)) => self.last_remote = Some(now),
                _ => {}
            }
        }
        local
    }

    /// Someone touched the console while it is black: lock, then bring the console back.
    fn begin_takeover(&mut self) {
        if self
            .last_failed_takeover
            .map_or(false, |t| t.elapsed() < TAKEOVER_RETRY)
        {
            return;
        }
        log::info!("x11 privacy: local input, locking the screen for a console logon");
        let locked = (is_screen_locked() == Some(true) || lock_screen())
            && wait_for_screen_locked(LOCK_TIMEOUT);
        if !locked {
            // Never show an unlocked desktop: stay black.
            log::warn!("x11 privacy: the screen did not lock, staying in privacy mode");
            self.last_failed_takeover = Some(Instant::now());
            return;
        }
        self.restore_all();
        self.phase = Phase::Suspended;
        self.suspended_at = Instant::now();
        self.last_local = None;
        self.last_remote = None;
    }

    fn step_suspended(&mut self) {
        let verdict = suspended_verdict(
            is_screen_locked(),
            Instant::now(),
            self.suspended_at,
            self.last_local,
            self.last_remote,
            self.opts.suspend_idle,
        );
        match verdict {
            SuspendedVerdict::Stay => {}
            SuspendedVerdict::Reactivate => {
                log::info!("x11 privacy: nobody logged on at the console, blacking out again");
                self.phase = Phase::Active;
                if let Err(e) = self.activate() {
                    log::warn!("x11 privacy: failed to re-activate: {}", e);
                }
            }
            SuspendedVerdict::Takeover => {
                log::info!("x11 privacy: unlocked at the console, ending privacy mode");
                self.phase = Phase::TakenOver;
                let _ = fs::remove_file(&self.state_file);
                if let Some(cb) = &self.opts.on_takeover {
                    cb();
                }
            }
        }
    }

    fn run(mut self, shared: Arc<Mutex<Shared>>, stop: Arc<AtomicBool>) {
        let mut last_enforce = Instant::now();
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(EVENT_POLL_INTERVAL);
            let local = self.drain_events();
            match self.phase {
                Phase::Active if local && self.opts.local_takeover => self.begin_takeover(),
                Phase::Active if last_enforce.elapsed() >= ENFORCE_INTERVAL => {
                    last_enforce = Instant::now();
                    if let Err(e) = self.activate() {
                        log::warn!("x11 privacy: enforce failed: {}", e);
                    }
                }
                Phase::Suspended if last_enforce.elapsed() >= ENFORCE_INTERVAL => {
                    last_enforce = Instant::now();
                    // Keep the device map fresh for classifying events.
                    let _ = self.refresh_devices();
                    self.step_suspended();
                }
                _ => {}
            }
            let mut s = shared.lock().unwrap();
            s.phase = self.phase;
            s.saved = self.saved.clone();
        }
        if self.phase != Phase::TakenOver {
            self.restore_all();
        }
        shared.lock().unwrap().saved = self.saved.clone();
    }
}

/// Human readable snapshot, for diagnostics.
pub fn status() -> Result<String> {
    let (conn, root) = connect()?;
    let mut s = String::new();
    for crtc in active_crtcs(&conn, root)? {
        let g = get_gamma(&conn, crtc)?;
        s += &format!(
            "crtc {}: gamma size {}, black: {}\n",
            crtc,
            g.0.len(),
            is_black(&g)
        );
    }
    for info in conn
        .xinput_xi_query_device(xinput::Device::ALL)?
        .reply()?
        .infos
    {
        let name = name_of(&info);
        s += &format!(
            "device {:>3} {:<45} type {} enabled {} physical {} person {}\n",
            info.deviceid,
            name,
            u16::from(info.type_),
            info.enabled,
            is_physical(&info),
            is_person(&name),
        );
    }
    s += &format!("screen locked: {:?}\n", is_screen_locked());
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip() {
        let mut s = SavedState::default();
        s.gamma
            .insert(63, (vec![0, 100, 65535], vec![1, 2, 3], vec![4, 5, 6]));
        s.devices.insert("SYNA Touchpad".into());
        s.devices.insert("AT Translated Set 2 keyboard".into());
        assert_eq!(deserialize(&serialize(&s)), s);
    }

    #[test]
    fn ramp() {
        assert_eq!(linear_ramp(3), vec![0, 32767, 65535]);
        assert!(is_black(&(vec![0; 4], vec![0; 4], vec![0; 4])));
    }

    #[test]
    fn person_devices() {
        assert!(is_person("AT Translated Set 2 keyboard"));
        assert!(is_person("Logitech Wireless Receiver Mouse"));
        assert!(is_person("ELAN071A:00 04F3:30FD Touchpad"));
        assert!(!is_person("Video Bus"));
        assert!(!is_person("Power Button"));
        assert!(!is_person("HP WMI hotkeys"));
        assert!(!is_person("Wireless hotkeys"));
    }

    #[test]
    fn verdicts() {
        use SuspendedVerdict::*;
        let t0 = Instant::now();
        let s = |n: u64| t0 + Duration::from_secs(n);
        let idle = Duration::from_secs(30);
        // Still locked, recent local typing: stay.
        assert_eq!(
            suspended_verdict(Some(true), s(20), t0, Some(s(15)), None, idle),
            Stay
        );
        // Still locked, nobody typed for 30s: black out again.
        assert_eq!(
            suspended_verdict(Some(true), s(30), t0, None, None, idle),
            Reactivate
        );
        assert_eq!(
            suspended_verdict(Some(true), s(50), t0, Some(s(20)), None, idle),
            Reactivate
        );
        // Locker did not answer: same as locked.
        assert_eq!(suspended_verdict(None, s(10), t0, None, None, idle), Stay);
        // Unlocked after local typing: takeover.
        assert_eq!(
            suspended_verdict(Some(false), s(5), t0, Some(s(4)), None, idle),
            Takeover
        );
        assert_eq!(
            suspended_verdict(Some(false), s(5), t0, Some(s(4)), Some(s(2)), idle),
            Takeover
        );
        // Unlocked by the remote side: black out again.
        assert_eq!(
            suspended_verdict(Some(false), s(5), t0, Some(s(2)), Some(s(4)), idle),
            Reactivate
        );
        assert_eq!(
            suspended_verdict(Some(false), s(5), t0, None, None, idle),
            Reactivate
        );
    }
}
