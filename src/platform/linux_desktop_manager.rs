use super::{linux::*, ResultType};
use crate::client::{
    LOGIN_MSG_DESKTOP_NO_DESKTOP, LOGIN_MSG_DESKTOP_SESSION_ANOTHER_USER,
    LOGIN_MSG_DESKTOP_SESSION_NOT_READY, LOGIN_MSG_DESKTOP_XORG_NOT_FOUND,
    LOGIN_MSG_DESKTOP_XSESSION_FAILED, LOGIN_MSG_PASSWORD_WRONG,
};
use hbb_common::{
    allow_err, bail, log,
    rand::prelude::*,
    tokio::time,
    users::{get_user_by_name, os::unix::UserExt, User},
};
use pam;
use std::{
    collections::HashMap,
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, Command},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{sync_channel, SyncSender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Seat0Snapshot {
    sequence: usize,
    username: Option<Option<String>>,
}

lazy_static::lazy_static! {
    static ref DESKTOP_RUNNING: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref DESKTOP_MANAGER: Arc<Mutex<Option<DesktopManager>>> = Arc::new(Mutex::new(None));
    /// Last settled "who owns seat0" answer, for the PRE-AUTH path only; see `is_headless`.
    static ref SEAT0_SNAPSHOT: Mutex<Seat0Snapshot> = Mutex::new(Seat0Snapshot::default());
    static ref SEAT0_NEXT_REFRESH: Mutex<Option<Instant>> = Mutex::new(None);
}

static SEAT0_REFRESH_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
const FIRST_SEAT0_QUERY_SEQUENCE: usize = 1;
static SEAT0_QUERY_SEQUENCE: AtomicUsize = AtomicUsize::new(FIRST_SEAT0_QUERY_SEQUENCE);
const SEAT0_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug)]
struct DesktopManager {
    child_username: String,
    child_exit: Arc<AtomicBool>,
    is_child_running: Arc<AtomicBool>,
}

fn check_desktop_manager() {
    let mut desktop_manager = DESKTOP_MANAGER.lock().unwrap();
    if let Some(desktop_manager) = &mut (*desktop_manager) {
        if desktop_manager.is_child_running.load(Ordering::SeqCst) {
            return;
        }
        desktop_manager.child_exit.store(true, Ordering::SeqCst);
    }
}

pub fn start_xdesktop() {
    debug_assert!(crate::is_server());
    std::thread::spawn(|| {
        DesktopManager::recover_orphaned_session();
        *DESKTOP_MANAGER.lock().unwrap() = Some(DesktopManager::new());
        // Seed the pre-auth snapshot now, off the connection path: without this the first
        // connection of every server process would read no snapshot at all.
        kick_seat0_refresh();

        let interval = time::Duration::from_millis(super::SERVICE_INTERVAL);
        DESKTOP_RUNNING.store(true, Ordering::SeqCst);
        while DESKTOP_RUNNING.load(Ordering::SeqCst) {
            check_desktop_manager();
            std::thread::sleep(interval);
        }
        log::info!("xdesktop child thread exit");
    });
}

pub fn stop_xdesktop() {
    DESKTOP_RUNNING.store(false, Ordering::SeqCst);
    *DESKTOP_MANAGER.lock().unwrap() = None;
}

fn detect_headless() -> Option<&'static str> {
    match run_cmds(&format!("which {}", DesktopManager::get_xorg())) {
        Ok(output) => {
            if output.trim().is_empty() {
                return Some(LOGIN_MSG_DESKTOP_XORG_NOT_FOUND);
            }
        }
        _ => {
            return Some(LOGIN_MSG_DESKTOP_XORG_NOT_FOUND);
        }
    }

    match run_cmds("ls /usr/share/xsessions/") {
        Ok(output) => {
            if output.trim().is_empty() {
                return Some(LOGIN_MSG_DESKTOP_NO_DESKTOP);
            }
        }
        _ => {
            return Some(LOGIN_MSG_DESKTOP_NO_DESKTOP);
        }
    }

    None
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum XSessionStartErrorKind {
    Auth,
    Env,
}

const XSESSION_AUTH_FAILURE_DETAIL: &str = "authentication failed";

#[derive(Debug)]
struct XSessionStartError {
    kind: XSessionStartErrorKind,
    detail: String,
}

impl XSessionStartError {
    fn auth(detail: String) -> Self {
        Self {
            kind: XSessionStartErrorKind::Auth,
            detail,
        }
    }

    fn env(detail: String) -> Self {
        Self {
            kind: XSessionStartErrorKind::Env,
            detail,
        }
    }
}

impl std::fmt::Display for XSessionStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.detail)
    }
}

fn map_xsession_start_error_to_login_msg(kind: XSessionStartErrorKind) -> &'static str {
    match kind {
        XSessionStartErrorKind::Auth => LOGIN_MSG_PASSWORD_WRONG,
        XSessionStartErrorKind::Env => LOGIN_MSG_DESKTOP_XSESSION_FAILED,
    }
}

pub fn try_start_desktop(_username: &str, _passsword: &str) -> String {
    debug_assert!(crate::is_server());
    if _username.is_empty() {
        let username = get_username();
        if username.is_empty() {
            if let Some(msg) = detect_headless() {
                msg
            } else {
                LOGIN_MSG_DESKTOP_SESSION_NOT_READY
            }
        } else {
            ""
        }
        .to_owned()
    } else {
        let username = get_username();
        log::debug!("try_start_desktop, username: {}, _username: {}", &username, &_username);
        if username == _username {
            // No need to verify password here.
            return "".to_owned();
        }
        if !username.is_empty() {
            // Another user is logged in. No need to start a new xsession.
            return "".to_owned();
        }

        if let Some(msg) = detect_headless() {
            return msg.to_owned();
        }

        match try_start_x_session(_username, _passsword) {
            Ok((username, x11_ready)) => {
                if x11_ready {
                    if _username != username {
                        LOGIN_MSG_DESKTOP_SESSION_ANOTHER_USER.to_owned()
                    } else {
                        "".to_owned()
                    }
                } else {
                    LOGIN_MSG_DESKTOP_SESSION_NOT_READY.to_owned()
                }
            }
            Err(e) => {
                match e.kind {
                    XSessionStartErrorKind::Auth => {
                        log::warn!("Failed to authenticate xsession user {}", e);
                    }
                    XSessionStartErrorKind::Env => {
                        log::error!("Failed to start xsession {}", e);
                    }
                }
                map_xsession_start_error_to_login_msg(e.kind).to_owned()
            }
        }
    }
}

fn try_start_x_session(username: &str, password: &str) -> Result<(String, bool), XSessionStartError> {
    // Seat0 is read BEFORE the manager lock: the lookup runs loginctl, and at a greeter the DRM
    // probe, and holding DESKTOP_MANAGER across those waits serializes every other caller.
    let seat0_username = refresh_seat0_snapshot();
    let mut desktop_manager = DESKTOP_MANAGER.lock().unwrap();
    if let Some(desktop_manager) = &mut (*desktop_manager) {
        if let Some(seat0_username) = seat0_username {
            return Ok((seat0_username, true));
        }

        let _ = desktop_manager.try_start_x_session(username, password)?;
        log::debug!(
            "try_start_x_session, username: {}, {:?}",
            &username,
            &desktop_manager
        );
        Ok((
            desktop_manager.child_username.clone(),
            desktop_manager.is_running(),
        ))
    } else {
        Err(XSessionStartError::env(
            crate::client::LOGIN_MSG_DESKTOP_NOT_INITED.to_owned(),
        ))
    }
}

#[inline]
/// The PRE-AUTH form: connection setup asks this before the peer has authenticated, so it must
/// not run loginctl or wait on the DRM probe (an unauthenticated client would occupy a worker,
/// and every connection would serialize behind the same lookup). It answers from the last
/// settled snapshot and refreshes it off-thread; the decisions that ENFORCE — `get_username`,
/// `try_start_x_session` — stay fresh.
pub fn is_headless() -> bool {
    if DESKTOP_MANAGER.lock().unwrap().is_none() {
        return false;
    }
    let cached = SEAT0_SNAPSHOT.lock().unwrap().username.clone();
    kick_seat0_refresh();
    // No snapshot yet answers NOT headless: guessing in the headless direction would show the
    // OS-login flow over a live Wayland greeter, which reads as an empty seat0 too. A false
    // only delays the headless flow until the first refresh lands, and the snapshot is seeded
    // from `start_xdesktop`, so the empty window is server start, not every connection.
    cached.map_or(false, |answer| answer.is_none())
}

/// A free function on purpose: it runs loginctl (and at a greeter the DRM probe), so no caller
/// may reach it while holding `DESKTOP_MANAGER` — that mutex held across subprocess or IPC waits
/// serializes every connection behind one slow lookup.
fn supported_display_seat0_username() -> Option<String> {
    // Read seat0 fresh on every query: the values cached in `DesktopManager::new()` go stale
    // across a logout or fast-user-switch, which would skip the greeter probe below and hand
    // back the previous session owner. Queried here and not in `new()` also because the read
    // there hides greeters.
    let seat0_values = get_values_of_seat0(&[0, 2]);
    let seat0_username = seat0_values[1].clone();
    #[cfg(feature = "drm")]
    if seat0_username.is_empty() || is_gdm_user(&seat0_username) {
        if let Some(username) = drm_login_screen_seat0_username() {
            return Some(username);
        }
    }
    if seat0_username.is_empty() {
        None
    } else if is_gdm_user(&seat0_username)
        && get_display_server_of_session(&seat0_values[0]) == DISPLAY_SERVER_WAYLAND
    {
        None
    } else {
        Some(seat0_username)
    }
}

fn select_newer_seat0_snapshot(current: Seat0Snapshot, candidate: Seat0Snapshot) -> Seat0Snapshot {
    if candidate.sequence > current.sequence {
        candidate
    } else {
        current
    }
}

fn refresh_seat0_snapshot() -> Option<String> {
    let sequence = SEAT0_QUERY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let fresh = supported_display_seat0_username();
    let candidate = Seat0Snapshot {
        sequence,
        username: Some(fresh.clone()),
    };
    let mut snapshot = SEAT0_SNAPSHOT.lock().unwrap();
    let current = std::mem::take(&mut *snapshot);
    *snapshot = select_newer_seat0_snapshot(current, candidate);
    fresh
}

/// Clears the single-flight flag on every exit, including a panic in the refresh thread; without
/// it a panic would freeze `is_headless` on a stale snapshot for the process lifetime.
struct Seat0RefreshGuard;
impl Drop for Seat0RefreshGuard {
    fn drop(&mut self) {
        SEAT0_REFRESH_IN_FLIGHT.store(false, Ordering::Release);
    }
}

/// Refresh the snapshot off-thread with a process-wide rate limit and single-flight.
fn kick_seat0_refresh() {
    let now = Instant::now();
    {
        let mut next_refresh = SEAT0_NEXT_REFRESH.lock().unwrap();
        let (next, should_refresh) = schedule_seat0_refresh(*next_refresh, now);
        *next_refresh = next;
        if !should_refresh {
            return;
        }
    }
    if SEAT0_REFRESH_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let guard = Seat0RefreshGuard;
    if let Err(err) = std::thread::Builder::new()
        .name("seat0-snapshot".into())
        .spawn(move || {
            let _guard = guard;
            let _ = refresh_seat0_snapshot();
        })
    {
        log::warn!("Could not spawn the seat0 snapshot refresh thread: {err}");
    }
}

/// The Wayland greeter on seat0, if the DRM backend can capture and inject into it.
#[cfg(feature = "drm")]
fn drm_login_screen_seat0_username() -> Option<String> {
    // An operator-forced X11 wins over greeter adoption: adopting would rebuild exactly the
    // inconsistency the forced gate exists to prevent — a session admitted for DRM serving
    // while capture and input route down the X11 path.
    if crate::platform::linux::display_server_forced() && crate::platform::linux::is_x11() {
        return None;
    }
    let values = get_values_of_seat0_with_gdm_wayland(&[0, 2]);
    if !is_gdm_user(&values[1])
        || get_display_server_of_session(&values[0]) != DISPLAY_SERVER_WAYLAND
    {
        return None;
    }
    // The cached tri-state, never the probing form: this runs on the unauthenticated login path,
    // so it must not wait out a probe deadline. Only a definitive unavailable hands the seat to
    // X11; an unsettled result keeps the maybe-live greeter (settling happens off-thread).
    if crate::server::drm_capturer::availability_cached()
        == crate::server::drm_capturer::Availability::Unavailable
    {
        return None;
    }
    Some(values[1].clone())
}

fn cached_username_from_state(
    seat0_username: Option<String>,
    managed_session: Option<(&str, bool)>,
) -> String {
    if let Some(username) = seat0_username {
        return username;
    }
    match managed_session {
        Some((username, true)) => username.to_owned(),
        _ => String::new(),
    }
}

fn schedule_seat0_refresh(next_refresh: Option<Instant>, now: Instant) -> (Option<Instant>, bool) {
    if next_refresh.is_some_and(|deadline| now < deadline) {
        return (next_refresh, false);
    }
    (Some(now + SEAT0_REFRESH_INTERVAL), true)
}

/// Returns the last settled username without running external commands.
pub fn get_cached_username() -> String {
    let seat0_username = SEAT0_SNAPSHOT.lock().unwrap().username.clone().flatten();
    let username = {
        let manager = DESKTOP_MANAGER.lock().unwrap();
        let Some(manager) = manager.as_ref() else {
            return String::new();
        };
        cached_username_from_state(
            seat0_username,
            Some((&manager.child_username, manager.is_running())),
        )
    };
    if username.is_empty() {
        kick_seat0_refresh();
    }
    username
}

pub fn get_username() -> String {
    if DESKTOP_MANAGER.lock().unwrap().is_none() {
        return "".to_owned();
    }
    // Computed with the manager lock RELEASED: the lookup runs loginctl, and at a greeter the
    // DRM probe, and holding DESKTOP_MANAGER across those waits serializes every caller behind
    // one slow probe.
    if let Some(seat0_username) = refresh_seat0_snapshot() {
        return seat0_username;
    }
    match &*DESKTOP_MANAGER.lock().unwrap() {
        Some(manager) => {
            if manager.is_running() && !manager.child_username.is_empty() {
                manager.child_username.clone()
            } else {
                "".to_owned()
            }
        }
        None => "".to_owned(),
    }
}

impl Drop for DesktopManager {
    fn drop(&mut self) {
        self.stop_children();
    }
}

impl DesktopManager {
    fn fatal_exit() {
        std::process::exit(0);
    }

    pub fn new() -> Self {
        Self {
            child_username: "".to_owned(),
            child_exit: Arc::new(AtomicBool::new(true)),
            is_child_running: Arc::new(AtomicBool::new(false)),
        }
    }

    #[inline]
    fn get_xauth() -> String {
        let xauth = get_env_var("XAUTHORITY");
        if xauth.is_empty() {
            "/tmp/.Xauthority".to_owned()
        } else {
            xauth
        }
    }

    #[inline]
    fn is_running(&self) -> bool {
        self.is_child_running.load(Ordering::SeqCst)
    }

    fn try_start_x_session(
        &mut self,
        username: &str,
        password: &str,
    ) -> Result<(), XSessionStartError> {
        match get_user_by_name(username) {
            Some(userinfo) => {
                let mut client = pam::Client::with_password(&pam_get_service_name())
                    .map_err(|e| XSessionStartError::env(format!("failed to init pam client, {}", e)))?;
                client
                    .conversation_mut()
                    .set_credentials(username, password);
                match client.authenticate() {
                    Ok(_) => {
                        if self.is_running() {
                            return Ok(());
                        }

                        match self.start_x_session(&userinfo, username, password) {
                            Ok(_) => {
                                log::info!("Succeeded to start x11");
                                self.child_username = username.to_string();
                                Ok(())
                            }
                            Err(e) => {
                                Err(XSessionStartError::env(format!(
                                    "failed to start x session, {}",
                                    e
                                )))
                            }
                        }
                    }
                    Err(_e) => {
                        Err(XSessionStartError::auth(
                            XSESSION_AUTH_FAILURE_DETAIL.to_owned(),
                        ))
                    }
                }
            }
            None => {
                Err(XSessionStartError::auth(
                    XSESSION_AUTH_FAILURE_DETAIL.to_owned(),
                ))
            }
        }
    }

    // The logic mainly from https://github.com/neutrinolabs/xrdp/blob/34fe9b60ebaea59e8814bbc3ca5383cabaa1b869/sesman/session.c#L334.
    fn get_avail_display() -> ResultType<u32> {
        let display_range = 0..51;
        for i in display_range.clone() {
            if Self::is_x_server_running(i) {
                continue;
            }
            return Ok(i);
        }
        bail!("No available display found in range {:?}", display_range)
    }

    #[inline]
    fn is_x_server_running(display: u32) -> bool {
        Path::new(&format!("/tmp/.X11-unix/X{}", display)).exists()
            || Path::new(&format!("/tmp/.X{}-lock", display)).exists()
    }

    fn start_x_session(
        &mut self,
        userinfo: &User,
        username: &str,
        password: &str,
    ) -> ResultType<()> {
        self.stop_children();

        let display_num = Self::get_avail_display()?;
        // "xServer_ip:display_num.screen_num"

        let uid = userinfo.uid();
        let gid = userinfo.primary_group_id();
        let envs = HashMap::from([
            ("SHELL", userinfo.shell().to_string_lossy().to_string()),
            ("PATH", "/sbin:/bin:/usr/bin:/usr/local/bin".to_owned()),
            ("USER", username.to_string()),
            ("UID", userinfo.uid().to_string()),
            ("HOME", userinfo.home_dir().to_string_lossy().to_string()),
            (
                "XDG_RUNTIME_DIR",
                format!("/run/user/{}", userinfo.uid().to_string()),
            ),
            // ("DISPLAY", self.display.clone()),
            // ("XAUTHORITY", self.xauth.clone()),
            // (ENV_DESKTOP_PROTOCOL, XProtocol::X11.to_string()),
        ]);
        self.child_exit.store(false, Ordering::SeqCst);
        let is_child_running = self.is_child_running.clone();

        let (tx_res, rx_res) = sync_channel(1);
        let password = password.to_string();
        let username = username.to_string();
        // start x11
        std::thread::spawn(move || {
            match Self::start_x_session_thread(
                tx_res.clone(),
                is_child_running,
                uid,
                gid,
                display_num,
                username,
                password,
                envs,
            ) {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Failed to start x session thread");
                    allow_err!(tx_res.send(format!("Failed to start x session thread, {}", e)));
                }
            }
        });

        // wait x11
        match rx_res.recv_timeout(Duration::from_millis(10_000)) {
            Ok(res) => {
                if res == "" {
                    Ok(())
                } else {
                    bail!(res)
                }
            }
            Err(e) => {
                bail!("Failed to recv x11 result {}", e)
            }
        }
    }

    #[inline]
    fn display_from_num(num: u32) -> String {
        format!(":{num}")
    }

    fn start_x_session_thread(
        tx_res: SyncSender<String>,
        is_child_running: Arc<AtomicBool>,
        uid: u32,
        gid: u32,
        display_num: u32,
        username: String,
        password: String,
        envs: HashMap<&str, String>,
    ) -> ResultType<()> {
        let mut client = pam::Client::with_password(&pam_get_service_name())?;
        client
            .conversation_mut()
            .set_credentials(&username, &password);
        client.authenticate()?;

        client.set_item(pam::PamItemType::TTY, &Self::display_from_num(display_num))?;
        client.open_session()?;

        // fixme: FreeBSD kernel needs to login here.
        // see: https://github.com/neutrinolabs/xrdp/blob/a64573b596b5fb07ca3a51590c5308d621f7214e/sesman/session.c#L556

        let (child_xorg, child_wm) = Self::start_x11(uid, gid, username, display_num, &envs)?;
        is_child_running.store(true, Ordering::SeqCst);

        // capture the logind session scope (from a live child) for teardown and crash
        // recovery, see reap_session_scope and recover_orphaned_session.
        let scope_dir = Self::session_scope_dir(child_xorg.id());
        Self::save_orphaned_marker(&scope_dir, display_num);

        log::info!("Start xorg and wm done, notify and wait xtop x11");
        allow_err!(tx_res.send("".to_owned()));

        Self::wait_stop_x11(child_xorg, child_wm, scope_dir, display_num);
        log::info!("Wait x11 stop done");
        Ok(())
    }

    fn wait_xorg_exit(child_xorg: &mut Child) -> ResultType<String> {
        if let Ok(_) = child_xorg.kill() {
            for _ in 0..3 {
                match child_xorg.try_wait() {
                    Ok(Some(status)) => return Ok(format!("Xorg exit with {}", status)),
                    Ok(None) => {}
                    Err(e) => {
                        // fatal error
                        log::error!("Failed to wait xorg process, {}", e);
                        bail!("Failed to wait xorg process, {}", e)
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1_000));
            }
            log::error!("Failed to wait xorg process, not exit");
            bail!("Failed to wait xorg process, not exit")
        } else {
            Ok("Xorg is already exited".to_owned())
        }
    }

    fn add_xauth_cookie(
        file: &str,
        display: &str,
        uid: u32,
        gid: u32,
        envs: &HashMap<&str, String>,
    ) -> ResultType<()> {
        let randstr = (0..16)
            .map(|_| format!("{:02x}", random::<u8>()))
            .collect::<String>();
        let output = Command::new("xauth")
            .uid(uid)
            .gid(gid)
            .envs(envs)
            .args(vec!["-q", "-f", file, "add", display, ".", &randstr])
            .output()?;
        // xauth run success, even the following error occurs.
        // Ok(Output { status: ExitStatus(unix_wait_status(0)), stdout: "", stderr: "xauth:  file .Xauthority does not exist\n" })
        let errmsg = String::from_utf8_lossy(&output.stderr).to_string();
        if !errmsg.is_empty() {
            if !errmsg.contains("does not exist") {
                bail!("Failed to launch xauth, {}", errmsg)
            }
        }
        Ok(())
    }

    fn wait_x_server_running(pid: u32, display_num: u32, max_wait_secs: u64) -> ResultType<()> {
        let wait_begin = Instant::now();
        loop {
            if run_cmds(&format!("ls /proc/{}", pid))?.is_empty() {
                bail!("X server exit");
            }

            if Self::is_x_server_running(display_num) {
                return Ok(());
            }
            if wait_begin.elapsed().as_secs() > max_wait_secs {
                bail!("Failed to wait xserver after {} seconds", max_wait_secs);
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    fn start_x11(
        uid: u32,
        gid: u32,
        username: String,
        display_num: u32,
        envs: &HashMap<&str, String>,
    ) -> ResultType<(Child, Child)> {
        log::debug!("envs of user {}: {:?}", &username, &envs);

        let xauth = Self::get_xauth();
        let display = Self::display_from_num(display_num);

        Self::add_xauth_cookie(&xauth, &display, uid, gid, &envs)?;

        // Start Xorg
        let mut child_xorg = Self::start_x_server(&xauth, &display, uid, gid, &envs)?;

        log::info!("xorg started, wait 10 secs to ensuer x server is running");

        let max_wait_secs = 10;
        // wait x server running
        if let Err(e) = Self::wait_x_server_running(child_xorg.id(), display_num, max_wait_secs) {
            match Self::wait_xorg_exit(&mut child_xorg) {
                Ok(msg) => log::info!("{}", msg),
                Err(e) => {
                    log::error!("{}", e);
                    Self::fatal_exit();
                }
            }
            bail!(e)
        }

        log::info!(
            "xorg is running, start x window manager with DISPLAY: {}, XAUTHORITY: {}",
            &display,
            &xauth
        );

        std::env::set_var("DISPLAY", &display);
        std::env::set_var("XAUTHORITY", &xauth);
        // start window manager (startwm.sh)
        let child_wm = match Self::start_x_window_manager(uid, gid, &envs) {
            Ok(c) => c,
            Err(e) => {
                match Self::wait_xorg_exit(&mut child_xorg) {
                    Ok(msg) => log::info!("{}", msg),
                    Err(e) => {
                        log::error!("{}", e);
                        Self::fatal_exit();
                    }
                }
                bail!(e)
            }
        };
        log::info!("x window manager is started");

        Ok((child_xorg, child_wm))
    }

    fn try_wait_x11_child_exit(child_xorg: &mut Child, child_wm: &mut Child) -> bool {
        match child_xorg.try_wait() {
            Ok(Some(status)) => {
                log::info!("Xorg exit with {}", status);
                return true;
            }
            Ok(None) => {}
            Err(e) => log::error!("Failed to wait xorg process, {}", e),
        }

        match child_wm.try_wait() {
            Ok(Some(status)) => {
                // Logout may result "wm exit with signal: 11 (SIGSEGV) (core dumped)"
                log::info!("wm exit with {}", status);
                return true;
            }
            Ok(None) => {}
            Err(e) => log::error!("Failed to wait xorg process, {}", e),
        }
        false
    }

    fn wait_x11_children_exit(child_xorg: &mut Child, child_wm: &mut Child) {
        log::debug!("Try kill child process xorg");
        if let Ok(_) = child_xorg.kill() {
            let mut exited = false;
            for _ in 0..2 {
                match child_xorg.try_wait() {
                    Ok(Some(status)) => {
                        log::info!("Xorg exit with {}", status);
                        exited = true;
                        break;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        log::error!("Failed to wait xorg process, {}", e);
                        Self::fatal_exit();
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1_000));
            }
            if !exited {
                log::error!("Failed to wait child xorg, after kill()");
                // try kill -9?
            }
        }
        log::debug!("Try kill child process wm");
        if let Ok(_) = child_wm.kill() {
            let mut exited = false;
            for _ in 0..2 {
                match child_wm.try_wait() {
                    Ok(Some(status)) => {
                        // Logout may result "wm exit with signal: 11 (SIGSEGV) (core dumped)"
                        log::info!("wm exit with {}", status);
                        exited = true;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        log::error!("Failed to wait wm process, {}", e);
                        Self::fatal_exit();
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1_000));
            }
            if !exited {
                log::error!("Failed to wait child xorg, after kill()");
                // try kill -9?
            }
        }
    }

    // resolve the "session-<id>.scope" directory pam_systemd put the x session in, read
    // from a live child pid. cgroup v2 mounts every cgroup under /sys/fs/cgroup, v1/hybrid
    // keeps the scope under the systemd controller mount; pick by the controller field and
    // confirm the cgroup is real. empty if there is no such scope (e.g. no logind).
    fn session_scope_dir(pid: u32) -> String {
        let path = format!("/proc/{}/cgroup", pid);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to read {} to find session scope: {}", path, e);
                return "".to_owned();
            }
        };
        for line in content.lines() {
            // "<hierarchy>:<controllers>:<path>"; v2 unified is "0::<path>", the v1
            // systemd hierarchy is "<n>:name=systemd:<path>".
            let mut fields = line.splitn(3, ':');
            let (controllers, cgroup) = match (fields.next(), fields.next(), fields.next()) {
                (Some(_), Some(c), Some(p)) => (c, p),
                _ => continue,
            };
            let scope = match Self::session_scope(cgroup) {
                Some(s) => s,
                None => continue,
            };
            let mount = if controllers.is_empty() {
                "/sys/fs/cgroup"
            } else if controllers.split(',').any(|c| c == "name=systemd") {
                "/sys/fs/cgroup/systemd"
            } else {
                continue;
            };
            let dir = format!("{}{}", mount, scope);
            if Path::new(&format!("{}/cgroup.procs", dir)).exists() {
                return dir;
            }
        }
        "".to_owned()
    }

    // the "/.../session-<id>.scope" prefix of a cgroup path, dropping any nested child
    // cgroup below it so a descendant scope does not get mistaken for the session.
    fn session_scope(cgroup: &str) -> Option<String> {
        let mut scope = String::new();
        for comp in cgroup.split('/').filter(|c| !c.is_empty()) {
            scope.push('/');
            scope.push_str(comp);
            if comp.starts_with("session-") && comp.ends_with(".scope") {
                return Some(scope);
            }
        }
        None
    }

    // on teardown reap the whole session scope subtree, not just the xorg + wm pids:
    // the per-session pipewire and other desktop children otherwise outlive them and
    // hold the logind session in "closing", leaking sockets + displays on reconnect
    // (rustdesk/rustdesk#15183). SIGTERM first so pipewire unlinks its sockets, then
    // SIGKILL stragglers; skip our own pid (pam put the service in the scope too).
    fn reap_session_scope(scope_dir: &str) {
        if scope_dir.is_empty() {
            return;
        }
        let me = std::process::id();
        // spare the --server's own children and any descendants of them sharing this scope
        // (see pid_is_spared); only the desktop session's leftovers are reaped.
        let spared: Vec<u32> = crate::server::CHILD_PROCESS
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.id())
            .collect();
        for sig in [hbb_common::libc::SIGTERM, hbb_common::libc::SIGKILL] {
            let mut pids = Vec::new();
            Self::collect_scope_pids(Path::new(scope_dir), &mut pids);
            let mut any = false;
            for pid in pids {
                if pid == me || Self::pid_is_spared(pid, &spared, me) {
                    continue;
                }
                any = true;
                log::info!("Reaping leftover session process {} (signal {})", pid, sig);
                unsafe {
                    if hbb_common::libc::kill(pid as hbb_common::libc::pid_t, sig) != 0 {
                        let err = std::io::Error::last_os_error();
                        // ESRCH = it already exited (or did between snapshot and now).
                        if err.raw_os_error() != Some(hbb_common::libc::ESRCH) {
                            log::warn!("Failed to signal session process {}: {}", pid, err);
                        }
                    }
                }
            }
            if !any {
                break;
            }
            if sig == hbb_common::libc::SIGTERM {
                std::thread::sleep(Duration::from_millis(300));
            }
        }
    }

    // a tracked --server child (the sudo wrapper run_as_user spawns) or any descendant of
    // one: with use_pty sudo runs --cm-no-ui under a monitor with its own pid, so walk the
    // parent chain (stopping at the --server) to spare the worker, not just the wrapper.
    fn pid_is_spared(pid: u32, spared: &[u32], me: u32) -> bool {
        let mut cur = pid;
        for _ in 0..32 {
            if spared.contains(&cur) {
                return true;
            }
            if cur <= 1 || cur == me {
                return false;
            }
            match Self::parent_pid(cur) {
                Some(ppid) => cur = ppid,
                None => return false,
            }
        }
        false
    }

    fn parent_pid(pid: u32) -> Option<u32> {
        // /proc/<pid>/stat is "pid (comm) state ppid ..."; comm can contain spaces and ')',
        // so read the fields after the last ')'.
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
        stat.rsplit_once(')')?
            .1
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    }

    // collect every pid in the cgroup subtree rooted at dir. "cgroup.procs" lists only
    // the procs directly in a cgroup, so recurse into child cgroup directories to catch
    // processes the desktop session moved into descendant scopes.
    fn collect_scope_pids(dir: &Path, out: &mut Vec<u32>) {
        let procs = dir.join("cgroup.procs");
        match std::fs::read_to_string(&procs) {
            Ok(content) => {
                out.extend(content.lines().filter_map(|l| l.trim().parse::<u32>().ok()));
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                log::warn!("Failed to read {}: {}", procs.display(), e);
            }
            Err(_) => {}
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                log::warn!("Failed to list cgroup dir {}: {}", dir.display(), e);
                return;
            }
            Err(_) => return,
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    log::warn!("Failed to read entry under {}: {}", dir.display(), e);
                    continue;
                }
            };
            match entry.file_type() {
                Ok(t) if t.is_dir() => Self::collect_scope_pids(&entry.path(), out),
                Ok(_) => {}
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    log::warn!("Failed to stat {}: {}", entry.path().display(), e);
                }
                Err(_) => {}
            }
        }
    }

    // a SIGKILL'd Xorg (how wait_x11_children_exit ends it) leaves "/tmp/.X<n>-lock" and
    // "/tmp/.X11-unix/X<n>" behind, and get_avail_display() treats either file as "display
    // in use", so the number is never reused and climbs until none are free
    // (rustdesk/rustdesk#15183). a clean exit would remove them; do the same on teardown,
    // but skip it if a live process still holds the lock: another server could have taken
    // the number in the gap, and removing its files would break that display.
    fn cleanup_x_display_files(display_num: u32) {
        let lock = format!("/tmp/.X{}-lock", display_num);
        if let Ok(content) = std::fs::read_to_string(&lock) {
            if let Ok(pid) = content.trim().parse::<i32>() {
                if Self::pid_alive(pid) {
                    log::info!("X display {} still held by pid {}, leaving its files", display_num, pid);
                    return;
                }
            }
        }
        for path in [lock, format!("/tmp/.X11-unix/X{}", display_num)] {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Failed to remove stale X file {}: {}", path, e);
                }
            }
        }
    }

    // signal-0 probe: the pid exists if kill succeeds or fails with EPERM (alive but not
    // ours); only ESRCH means it is gone.
    fn pid_alive(pid: i32) -> bool {
        unsafe {
            if hbb_common::libc::kill(pid as hbb_common::libc::pid_t, 0) == 0 {
                return true;
            }
        }
        std::io::Error::last_os_error().raw_os_error() == Some(hbb_common::libc::EPERM)
    }

    const ORPHANED_SESSION_KEY: &'static str = "headless-orphaned-session";

    fn save_orphaned_marker(scope_dir: &str, display_num: u32) {
        // tag the marker with this boot's id: a logind session id is only unique within a
        // boot (the counter lives in /run and resets), so recovery must not reap a recorded
        // scope path after a reboot, when it may name a different live session.
        let boot_id = Self::current_boot_id().unwrap_or_default();
        hbb_common::config::LocalConfig::set_option(
            Self::ORPHANED_SESSION_KEY.to_owned(),
            format!("{};{};{}", scope_dir, display_num, boot_id),
        );
    }

    fn current_boot_id() -> Option<String> {
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .ok()
            .map(|s| s.trim().to_owned())
    }

    fn clear_orphaned_marker() {
        hbb_common::config::LocalConfig::set_option(
            Self::ORPHANED_SESSION_KEY.to_owned(),
            String::new(),
        );
    }

    fn parse_orphaned_marker(marker: &str) -> Option<(&str, u32, &str)> {
        let (rest, boot_id) = marker.rsplit_once(';')?;
        let (scope_dir, display) = rest.rsplit_once(';')?;
        Some((scope_dir, display.trim().parse::<u32>().ok()?, boot_id))
    }

    // a run that dies before wait_stop_x11 (service or --server crash) leaks the headless
    // session scope + X lock files, the same as a missed teardown (rustdesk/rustdesk#15183).
    // reap exactly what the dead run recorded - never a scan, so unrelated sessions are safe.
    fn recover_orphaned_session() {
        let marker = hbb_common::config::LocalConfig::get_option(Self::ORPHANED_SESSION_KEY);
        if marker.is_empty() {
            return;
        }
        if let Some((scope_dir, display_num, boot_id)) = Self::parse_orphaned_marker(&marker) {
            // only reap the recorded scope when the marker is from this same boot: a leaked
            // cgroup cannot outlive a reboot, so cross-boot there is nothing legitimate to
            // reap, and the recorded "session-N.scope" may by then name a different live
            // session. the X lock cleanup is pid-guarded, so run it either way.
            let same_boot = Self::current_boot_id().map_or(false, |b| b == boot_id);
            log::info!(
                "Recovering leaked headless session from a previous run: scope {}, display {} (same boot: {})",
                scope_dir,
                display_num,
                same_boot
            );
            if same_boot {
                Self::reap_session_scope(scope_dir);
            }
            Self::cleanup_x_display_files(display_num);
        }
        Self::clear_orphaned_marker();
    }

    fn try_wait_stop_x11(
        child_xorg: &mut Child,
        child_wm: &mut Child,
        scope_dir: &str,
        display_num: u32,
    ) -> bool {
        let mut desktop_manager = DESKTOP_MANAGER.lock().unwrap();
        let mut exited = true;
        if let Some(desktop_manager) = &mut (*desktop_manager) {
            if desktop_manager.child_exit.load(Ordering::SeqCst) {
                exited = true;
            } else {
                exited = Self::try_wait_x11_child_exit(child_xorg, child_wm);
            }
            if exited {
                log::debug!("Wait x11 children exiting");
                Self::wait_x11_children_exit(child_xorg, child_wm);
                Self::reap_session_scope(scope_dir);
                Self::cleanup_x_display_files(display_num);
                Self::clear_orphaned_marker();
                desktop_manager
                    .is_child_running
                    .store(false, Ordering::SeqCst);
                desktop_manager.child_exit.store(true, Ordering::SeqCst);
            }
        }
        exited
    }

    fn wait_stop_x11(
        mut child_xorg: Child,
        mut child_wm: Child,
        scope_dir: String,
        display_num: u32,
    ) {
        loop {
            if Self::try_wait_stop_x11(&mut child_xorg, &mut child_wm, &scope_dir, display_num) {
                break;
            }
            std::thread::sleep(Duration::from_millis(super::SERVICE_INTERVAL));
        }
    }

    fn get_xorg() -> &'static str {
        // Fedora 26 or later
        let xorg = "/usr/libexec/Xorg";
        if Path::new(xorg).is_file() {
            return xorg;
        }
        // Debian 9 or later
        let xorg = "/usr/lib/xorg/Xorg";
        if Path::new(xorg).is_file() {
            return xorg;
        }
        // Ubuntu 16.04 or later
        let xorg = "/usr/lib/xorg/Xorg";
        if Path::new(xorg).is_file() {
            return xorg;
        }
        // Arch Linux
        let xorg = "/usr/lib/xorg-server/Xorg";
        if Path::new(xorg).is_file() {
            return xorg;
        }
        // Arch Linux
        let xorg = "/usr/lib/Xorg";
        if Path::new(xorg).is_file() {
            return xorg;
        }
        // CentOS 7 /usr/bin/Xorg or param=Xorg

        log::warn!("Failed to find xorg, use default Xorg.\n Please add \"allowed_users=anybody\" to \"/etc/X11/Xwrapper.config\".");
        "Xorg"
    }

    fn start_x_server(
        xauth: &str,
        display: &str,
        uid: u32,
        gid: u32,
        envs: &HashMap<&str, String>,
    ) -> ResultType<Child> {
        let xorg = Self::get_xorg();
        log::info!("Use xorg: {}", &xorg);
        let app_name = crate::get_app_name().to_lowercase();
        let conf = format!("/etc/{app_name}/xorg.conf");
        match Command::new(xorg)
            .envs(envs)
            .uid(uid)
            .gid(gid)
            .args(vec![
                "-noreset",
                "+extension",
                "GLX",
                "+extension",
                "RANDR",
                "+extension",
                "RENDER",
                "-config",
                conf.as_ref(),
                "-auth",
                xauth,
                display,
            ])
            .spawn()
        {
            Ok(c) => Ok(c),
            Err(e) => {
                bail!("Failed to start Xorg with display {}, {}", display, e);
            }
        }
    }

    fn start_x_window_manager(
        uid: u32,
        gid: u32,
        envs: &HashMap<&str, String>,
    ) -> ResultType<Child> {
        let app_name = crate::get_app_name().to_lowercase();
        match Command::new(&format!("/etc/{app_name}/startwm.sh"))
            .envs(envs)
            .uid(uid)
            .gid(gid)
            .spawn()
        {
            Ok(c) => Ok(c),
            Err(e) => {
                bail!("Failed to start window manager, {}", e);
            }
        }
    }

    fn stop_children(&mut self) {
        self.child_exit.store(true, Ordering::SeqCst);
        for _i in 1..10 {
            if !self.is_child_running.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(super::SERVICE_INTERVAL));
        }
        if self.is_child_running.load(Ordering::SeqCst) {
            log::warn!("xdesktop child is still running!");
        }
    }
}

fn pam_get_service_name() -> String {
    let app_name = crate::get_app_name().to_lowercase();
    if Path::new(&format!("/etc/pam.d/{app_name}")).is_file() {
        app_name
    } else {
        "gdm".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_username_prefers_seat0_and_running_managed_session() {
        assert_eq!(
            cached_username_from_state(Some("seat0".to_owned()), Some(("managed", true))),
            "seat0"
        );
        assert_eq!(
            cached_username_from_state(None, Some(("managed", true))),
            "managed"
        );
        assert_eq!(
            cached_username_from_state(None, Some(("managed", false))),
            ""
        );
        assert_eq!(cached_username_from_state(None, None), "");
    }

    #[test]
    fn seat0_refresh_schedule_limits_process_wide_rate() {
        let started = Instant::now();
        let (next_refresh, should_refresh) = schedule_seat0_refresh(None, started);
        assert!(should_refresh);

        let (unchanged, should_refresh) = schedule_seat0_refresh(next_refresh, started);
        assert!(!should_refresh);
        assert_eq!(unchanged, next_refresh);

        let (_, should_refresh) =
            schedule_seat0_refresh(next_refresh, started + SEAT0_REFRESH_INTERVAL);
        assert!(should_refresh);
    }

    #[test]
    fn session_scope_truncates_at_first_scope() {
        assert_eq!(
            DesktopManager::session_scope("/user.slice/user-1000.slice/session-3.scope").as_deref(),
            Some("/user.slice/user-1000.slice/session-3.scope")
        );
        // a nested child scope must not be mistaken for the session
        assert_eq!(
            DesktopManager::session_scope(
                "/user.slice/user-1000.slice/session-3.scope/app-foo.scope"
            )
            .as_deref(),
            Some("/user.slice/user-1000.slice/session-3.scope")
        );
        assert_eq!(
            DesktopManager::session_scope(
                "/user.slice/user-1000.slice/user@1000.service/app.slice/x.service"
            ),
            None
        );
        assert_eq!(DesktopManager::session_scope("/"), None);
    }

    #[test]
    fn collect_scope_pids_walks_descendant_cgroups() {
        // regression for #15183: pids in descendant cgroups must be collected too
        let base = std::env::temp_dir().join(format!("rustdesk-cgtest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let scope = base.join("session-3.scope");
        let child = scope.join("app-foo.scope");
        let nested = child.join("deeper.scope");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(scope.join("empty.scope")).unwrap();
        std::fs::write(scope.join("cgroup.procs"), "100\n101\n").unwrap();
        std::fs::write(scope.join("cgroup.controllers"), "memory pids\n").unwrap();
        std::fs::write(child.join("cgroup.procs"), "200\n").unwrap();
        std::fs::write(nested.join("cgroup.procs"), "300\n").unwrap();

        let mut pids = Vec::new();
        DesktopManager::collect_scope_pids(&scope, &mut pids);
        pids.sort();
        let _ = std::fs::remove_dir_all(&base);

        assert_eq!(pids, vec![100, 101, 200, 300]);
    }

    #[test]
    fn parses_orphaned_session_marker() {
        assert_eq!(
            DesktopManager::parse_orphaned_marker(
                "/sys/fs/cgroup/user.slice/user-1000.slice/session-3.scope;7;abc-123"
            ),
            Some((
                "/sys/fs/cgroup/user.slice/user-1000.slice/session-3.scope",
                7,
                "abc-123"
            ))
        );
        // an empty scope still carries the display so its stale X lock can be cleaned
        assert_eq!(DesktopManager::parse_orphaned_marker(";5;abc-123"), Some(("", 5, "abc-123")));
        // an empty boot id never matches the live one, so the scope reap is skipped
        assert_eq!(DesktopManager::parse_orphaned_marker("/scope;5;"), Some(("/scope", 5, "")));
        assert_eq!(DesktopManager::parse_orphaned_marker(""), None);
        assert_eq!(DesktopManager::parse_orphaned_marker("garbage"), None);
        // the pre-boot-id two-field format no longer parses, recovery just skips it
        assert_eq!(DesktopManager::parse_orphaned_marker("/scope;7"), None);
        assert_eq!(DesktopManager::parse_orphaned_marker("/scope;notnum;abc"), None);
    }
}
