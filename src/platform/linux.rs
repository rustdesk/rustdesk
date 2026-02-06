use super::{gtk_sudo, CursorData, ResultType};
use desktop::Desktop;
pub use hbb_common::platform::linux::*;
use hbb_common::{
    allow_err,
    anyhow::anyhow,
    bail,
    config::{keys::OPTION_ALLOW_LINUX_HEADLESS, Config},
    libc::{c_char, c_int, c_long, c_uint, c_void},
    log,
    message_proto::{DisplayInfo, Resolution},
    regex::{Captures, Regex},
    users::{get_user_by_name, os::unix::UserExt},
};
use libxdo_sys::{self, xdo_t, Window};
use std::{
    cell::RefCell,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::{Child, Command},
    string::String,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time::{Duration, Instant},
};
use terminfo::{capability as cap, Database};
use wallpaper;

pub const PA_SAMPLE_RATE: u32 = 48000;
static mut UNMODIFIED: bool = true;

const INVALID_TERM_VALUES: [&str; 3] = ["", "unknown", "dumb"];
const SHELL_PROCESSES: [&str; 4] = ["bash", "zsh", "fish", "sh"];

// Terminal type constants
const TERM_XTERM_256COLOR: &str = "xterm-256color";
const TERM_SCREEN_256COLOR: &str = "screen-256color";
const TERM_XTERM: &str = "xterm";

lazy_static::lazy_static! {
    pub static ref IS_X11: bool = hbb_common::platform::linux::is_x11_or_headless();
    // Cache for TERM value - once TERM_XTERM_256COLOR is found, reuse it directly
    static ref CACHED_TERM: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    static ref DATABASE_XTERM_256COLOR: Option<Database> = {
        match Database::from_name(TERM_XTERM_256COLOR) {
            Ok(database) => Some(database),
            Err(err) => {
                log::error!("Failed to initialize {} database: {}", TERM_XTERM_256COLOR, err);
                None
            }
        }
    };
    // https://github.com/rustdesk/rustdesk/issues/13705
    // Check if `sudo -E` actually preserves environment.
    //
    // This flag is only used by `run_as_user()` (root service -> user session). If the current process is not
    // running as `root`, this check is meaningless (and `sudo -n` may fail), so we return `false` directly.
    //
    // On Ubuntu 25.10, `sudo -E` may still succeed but effectively ignores `-E`. Some versions print a warning
    // to stderr (wording may vary by locale), so we verify behavior instead:
    // - Inject a sentinel environment variable into the `sudo` process
    // - Run `sudo -n -E env` and check whether the sentinel is present in stdout
    static ref SUDO_E_PRESERVES_ENV: bool = {
        if !is_root() {
            log::warn!("Not running as root, SUDO_E_PRESERVES_ENV check skipped");
            false
        } else {
            let key = format!("__RUSTDESK_SUDO_E_TEST_{}", std::process::id());
            let val = "1";
            let expected = format!("{key}={val}");
            Command::new("sudo")
                // -n for non-interactive to avoid password prompt
                .env(&key, val)
                .args(["-n", "-E", "env"])
                .output()
                .map(|o| {
                    o.status.success()
                        && String::from_utf8_lossy(&o.stdout).contains(expected.as_str())
                })
                .unwrap_or(false)
        }
    };
}

thread_local! {
    // XDO context - created via libxdo-sys (which uses dynamic loading stub).
    // If libxdo is not available, xdo will be null and xdo-based functions become no-ops.
    static XDO: RefCell<*mut xdo_t> = RefCell::new({
        let xdo = unsafe { libxdo_sys::xdo_new(std::ptr::null()) };
        if xdo.is_null() {
            log::warn!("Failed to create xdo context, xdo functions will be disabled");
        } else {
            log::info!("xdo context created successfully");
        }
        xdo
    });
    static DISPLAY: RefCell<*mut c_void> = RefCell::new(unsafe { XOpenDisplay(std::ptr::null())});
}

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(display_name: *const c_char) -> *mut c_void;
    // fn XCloseDisplay(d: *mut c_void) -> c_int;
}

#[link(name = "Xfixes")]
extern "C" {
    // fn XFixesQueryExtension(dpy: *mut c_void, event: *mut c_int, error: *mut c_int) -> c_int;
    fn XFixesGetCursorImage(dpy: *mut c_void) -> *const xcb_xfixes_get_cursor_image;
    fn XFree(data: *mut c_void);
}

// /usr/include/X11/extensions/Xfixes.h
#[repr(C)]
pub struct xcb_xfixes_get_cursor_image {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub xhot: u16,
    pub yhot: u16,
    pub cursor_serial: c_long,
    pub pixels: *const c_long,
}

#[inline]
pub fn is_headless_allowed() -> bool {
    Config::get_option(OPTION_ALLOW_LINUX_HEADLESS) == "Y"
}

#[inline]
pub fn is_login_screen_wayland() -> bool {
    let values = get_values_of_seat0_with_gdm_wayland(&[0, 2]);
    is_gdm_user(&values[1]) && get_display_server_of_session(&values[0]) == DISPLAY_SERVER_WAYLAND
}

#[inline]
fn sleep_millis(millis: u64) {
    std::thread::sleep(Duration::from_millis(millis));
}

pub fn get_cursor_pos() -> Option<(i32, i32)> {
    let mut res = None;
    XDO.with(|xdo| {
        if let Ok(xdo) = xdo.try_borrow() {
            if xdo.is_null() {
                return;
            }
            let mut x: c_int = 0;
            let mut y: c_int = 0;
            unsafe {
                libxdo_sys::xdo_get_mouse_location(
                    *xdo as *const _,
                    &mut x as _,
                    &mut y as _,
                    std::ptr::null_mut(),
                );
            }
            res = Some((x, y));
        }
    });
    res
}

pub fn set_cursor_pos(x: i32, y: i32) -> bool {
    let mut res = false;
    XDO.with(|xdo| {
        match xdo.try_borrow() {
            Ok(xdo) => {
                if xdo.is_null() {
                    log::debug!("set_cursor_pos: xdo is null");
                    return;
                }
                unsafe {
                    let ret = libxdo_sys::xdo_move_mouse(*xdo as *const _, x, y, 0);
                    if ret != 0 {
                        log::debug!(
                            "set_cursor_pos: xdo_move_mouse failed with code {} for coordinates ({}, {})",
                            ret, x, y
                        );
                    }
                    res = ret == 0;
                }
            }
            Err(_) => {
                log::debug!("set_cursor_pos: failed to borrow xdo");
            }
        }
    });
    res
}

/// Clip cursor - Linux implementation is a no-op.
///
/// On X11, there's no direct equivalent to Windows ClipCursor. XGrabPointer
/// can confine the pointer but requires a window handle and has side effects.
///
/// On Wayland, pointer constraints require the zwp_pointer_constraints_v1
/// protocol which is compositor-dependent.
///
/// For relative mouse mode on Linux, the Flutter side uses pointer warping
/// (set_cursor_pos) to re-center the cursor after each movement, which achieves
/// a similar effect without requiring cursor clipping.
///
/// Returns true (always succeeds as no-op).
pub fn clip_cursor(_rect: Option<(i32, i32, i32, i32)>) -> bool {
    // Log only once per process to avoid flooding logs when called frequently.
    static LOGGED: AtomicBool = AtomicBool::new(false);
    if !LOGGED.swap(true, Ordering::Relaxed) {
        log::debug!("clip_cursor called (no-op on Linux, this message is logged only once)");
    }
    true
}

pub fn reset_input_cache() {}

pub fn get_focused_display(displays: Vec<DisplayInfo>) -> Option<usize> {
    let mut res = None;
    XDO.with(|xdo| {
        if let Ok(xdo) = xdo.try_borrow() {
            if xdo.is_null() {
                return;
            }
            let mut x: c_int = 0;
            let mut y: c_int = 0;
            let mut width: c_uint = 0;
            let mut height: c_uint = 0;
            let mut window: Window = 0;

            unsafe {
                if libxdo_sys::xdo_get_active_window(*xdo as *const _, &mut window) != 0 {
                    return;
                }
                if libxdo_sys::xdo_get_window_location(
                    *xdo as *const _,
                    window,
                    &mut x as _,
                    &mut y as _,
                    std::ptr::null_mut(),
                ) != 0
                {
                    return;
                }
                if libxdo_sys::xdo_get_window_size(
                    *xdo as *const _,
                    window,
                    &mut width,
                    &mut height,
                ) != 0
                {
                    return;
                }
                let center_x = x + (width / 2) as c_int;
                let center_y = y + (height / 2) as c_int;
                res = displays.iter().position(|d| {
                    center_x >= d.x
                        && center_x < d.x + d.width
                        && center_y >= d.y
                        && center_y < d.y + d.height
                });
            }
        }
    });
    res
}

pub fn get_cursor() -> ResultType<Option<u64>> {
    let mut res = None;
    DISPLAY.with(|conn| {
        if let Ok(d) = conn.try_borrow_mut() {
            if !d.is_null() {
                unsafe {
                    let img = XFixesGetCursorImage(*d);
                    if !img.is_null() {
                        res = Some((*img).cursor_serial as u64);
                        XFree(img as _);
                    }
                }
            }
        }
    });
    Ok(res)
}

pub fn get_cursor_data(hcursor: u64) -> ResultType<CursorData> {
    let mut res = None;
    DISPLAY.with(|conn| {
        if let Ok(ref mut d) = conn.try_borrow_mut() {
            if !d.is_null() {
                unsafe {
                    let img = XFixesGetCursorImage(**d);
                    if !img.is_null() && hcursor == (*img).cursor_serial as u64 {
                        let mut cd: CursorData = Default::default();
                        cd.hotx = (*img).xhot as _;
                        cd.hoty = (*img).yhot as _;
                        cd.width = (*img).width as _;
                        cd.height = (*img).height as _;
                        // to-do: how about if it is 0
                        cd.id = (*img).cursor_serial as _;
                        let pixels =
                            std::slice::from_raw_parts((*img).pixels, (cd.width * cd.height) as _);
                        // cd.colors.resize(pixels.len() * 4, 0);
                        let mut cd_colors = vec![0_u8; pixels.len() * 4];
                        for y in 0..cd.height {
                            for x in 0..cd.width {
                                let pos = (y * cd.width + x) as usize;
                                let p = pixels[pos];
                                let a = (p >> 24) & 0xff;
                                let r = (p >> 16) & 0xff;
                                let g = (p >> 8) & 0xff;
                                let b = (p >> 0) & 0xff;
                                if a == 0 {
                                    continue;
                                }
                                let pos = pos * 4;
                                cd_colors[pos] = r as _;
                                cd_colors[pos + 1] = g as _;
                                cd_colors[pos + 2] = b as _;
                                cd_colors[pos + 3] = a as _;
                            }
                        }
                        cd.colors = cd_colors.into();
                        res = Some(cd);
                    }
                    if !img.is_null() {
                        XFree(img as _);
                    }
                }
            }
        }
    });
    match res {
        Some(x) => Ok(x),
        _ => bail!("Failed to get cursor image of {}", hcursor),
    }
}

fn start_uinput_service() {
    use crate::server::uinput::service;
    std::thread::spawn(|| {
        service::start_service_control();
    });
    std::thread::spawn(|| {
        service::start_service_keyboard();
    });
    std::thread::spawn(|| {
        service::start_service_mouse();
    });
}

/// Suggests the best terminal type based on the environment.
///
/// The function prioritizes terminal types in the following order:
/// 1. `screen-256color`: Preferred when running inside `tmux` or `screen` sessions,
///    as these multiplexers often support advanced terminal features.
/// 2. `xterm-256color`: Selected if the terminal supports 256 colors, which is
///    suitable for modern terminal applications.
/// 3. `xterm`: Used as a fallback for basic terminal compatibility.
///
/// Terminals like `linux` and `vt100` are excluded because they lack support for
/// modern features required by many applications.
fn suggest_best_term() -> String {
    if is_running_in_tmux() || is_running_in_screen() {
        return TERM_SCREEN_256COLOR.to_string();
    }
    if term_supports_256_colors(TERM_XTERM_256COLOR) {
        return TERM_XTERM_256COLOR.to_string();
    }
    TERM_XTERM.to_string()
}

fn is_running_in_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

fn is_running_in_screen() -> bool {
    std::env::var("STY").is_ok()
}

fn supports_256_colors(db: &Database) -> bool {
    db.get::<cap::MaxColors>().map_or(false, |n| n.0 >= 256)
}

fn term_supports_256_colors(term: &str) -> bool {
    match term {
        TERM_XTERM_256COLOR => DATABASE_XTERM_256COLOR
            .as_ref()
            .map_or(false, |db| supports_256_colors(db)),
        _ => Database::from_name(term).map_or(false, |db| supports_256_colors(&db)),
    }
}

fn get_cur_term(uid: &str) -> Option<String> {
    // Check cache first - if TERM_XTERM_256COLOR was found before, reuse it
    if let Ok(cache) = CACHED_TERM.lock() {
        if let Some(ref cached) = *cache {
            if cached == TERM_XTERM_256COLOR {
                return Some(cached.clone());
            }
        }
    }

    if uid.is_empty() {
        return None;
    }

    // Check current process environment
    if let Ok(term) = std::env::var("TERM") {
        if term == TERM_XTERM_256COLOR {
            if let Ok(mut cache) = CACHED_TERM.lock() {
                *cache = Some(term.clone());
            }
            return Some(term);
        }
    }

    // Collect all TERM values from shell processes, looking for TERM_XTERM_256COLOR
    let terms = get_all_term_values(uid);

    // Prefer TERM_XTERM_256COLOR
    if terms.iter().any(|t| t == TERM_XTERM_256COLOR) {
        if let Ok(mut cache) = CACHED_TERM.lock() {
            *cache = Some(TERM_XTERM_256COLOR.to_string());
        }
        return Some(TERM_XTERM_256COLOR.to_string());
    }

    // Return first valid TERM if no TERM_XTERM_256COLOR found
    let fallback = terms.into_iter().next();
    if let Some(ref term) = fallback {
        log::debug!(
            "TERM_XTERM_256COLOR not found, using fallback TERM: {}",
            term
        );
    }
    fallback
}

/// Get all TERM values from shell processes (bash, zsh, fish, sh).
/// Returns a Vec of unique, valid TERM values.
fn get_all_term_values(uid: &str) -> Vec<String> {
    let Ok(uid_num) = uid.parse::<u32>() else {
        return Vec::new();
    };

    // Build regex pattern to match shell processes using only argv[0] (the executable path)
    // Pattern: match process name at start or after '/', followed by space or end
    // e.g., "bash", "/bin/bash", "/usr/bin/zsh"
    let shell_pattern = SHELL_PROCESSES
        .iter()
        .map(|p| format!(r"(^|/){p}(\s|$)"))
        .collect::<Vec<_>>()
        .join("|");
    let Ok(re) = Regex::new(&shell_pattern) else {
        return Vec::new();
    };

    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };

    let mut terms = Vec::new();

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(pid_str) = file_name.to_str() else {
            continue;
        };
        if !pid_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let proc_path = entry.path();

        // Check if process belongs to the specified uid
        if let Ok(meta) = std::fs::metadata(&proc_path) {
            use std::os::unix::fs::MetadataExt;
            if meta.uid() != uid_num {
                continue;
            }
        } else {
            continue;
        }

        // Check cmdline matches process pattern
        // /proc/<pid>/cmdline is a sequence of null-terminated strings; the first
        // one (argv[0]) is the executable path. Match the regex only against that
        // to avoid false positives from arguments (e.g., "python /path/to/bash-script.py").
        let cmdline_path = proc_path.join("cmdline");
        let Ok(cmdline) = std::fs::read(&cmdline_path) else {
            continue;
        };
        let exe_end = cmdline
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(cmdline.len());
        let exe_str = String::from_utf8_lossy(&cmdline[..exe_end]);
        if !re.is_match(&exe_str) {
            continue;
        }

        // Read environ and extract TERM
        let environ_path = proc_path.join("environ");
        let Ok(environ) = std::fs::read(&environ_path) else {
            continue;
        };

        for part in environ.split(|&b| b == 0) {
            if part.is_empty() {
                continue;
            }
            if let Some(eq) = part.iter().position(|&b| b == b'=') {
                let key_bytes = &part[..eq];
                if key_bytes == b"TERM" {
                    let val_bytes = &part[eq + 1..];
                    let term = String::from_utf8_lossy(val_bytes).into_owned();
                    if !INVALID_TERM_VALUES.contains(&term.as_str()) && !terms.contains(&term) {
                        // Early return if we found the preferred term
                        if term == TERM_XTERM_256COLOR {
                            return vec![term];
                        }
                        terms.push(term);
                    }
                    break;
                }
            }
        }
    }

    terms
}

#[inline]
fn try_start_server_(desktop: Option<&Desktop>) -> ResultType<Option<Child>> {
    match desktop {
        Some(desktop) => {
            let mut envs = vec![];
            if !desktop.display.is_empty() {
                envs.push(("DISPLAY", desktop.display.clone()));
            }
            if !desktop.xauth.is_empty() {
                envs.push(("XAUTHORITY", desktop.xauth.clone()));
            }
            if !desktop.wl_display.is_empty() {
                envs.push(("WAYLAND_DISPLAY", desktop.wl_display.clone()));
            }
            if !desktop.home.is_empty() {
                envs.push(("HOME", desktop.home.clone()));
            }
            if !desktop.dbus.is_empty() {
                envs.push(("DBUS_SESSION_BUS_ADDRESS", desktop.dbus.clone()));
            }
            envs.push((
                "TERM",
                get_cur_term(&desktop.uid).unwrap_or_else(|| suggest_best_term()),
            ));
            run_as_user(
                vec!["--server"],
                Some((desktop.uid.clone(), desktop.username.clone())),
                envs,
            )
        }
        None => Ok(Some(crate::run_me(vec!["--server"])?)),
    }
}

#[inline]
fn start_server(desktop: Option<&Desktop>, server: &mut Option<Child>) {
    match try_start_server_(desktop) {
        Ok(ps) => *server = ps,
        Err(err) => {
            log::error!("Failed to start server: {}", err);
        }
    }
}

fn stop_server(server: &mut Option<Child>) {
    if let Some(mut ps) = server.take() {
        allow_err!(ps.kill());
        sleep_millis(30);
        match ps.try_wait() {
            Ok(Some(_status)) => {}
            Ok(None) => {
                let _res = ps.wait();
            }
            Err(e) => log::error!("error attempting to wait: {e}"),
        }
    }
}

fn set_x11_env(desktop: &Desktop) {
    log::info!("DISPLAY: {}", desktop.display);
    log::info!("XAUTHORITY: {}", desktop.xauth);
    if !desktop.display.is_empty() {
        std::env::set_var("DISPLAY", &desktop.display);
    }
    if !desktop.xauth.is_empty() {
        std::env::set_var("XAUTHORITY", &desktop.xauth);
    }
}

#[inline]
fn stop_rustdesk_servers() {
    let _ = run_cmds(&format!(
        r##"ps -ef | grep -E '{} +--server' | awk '{{print $2}}' | xargs -r kill -9"##,
        crate::get_app_name().to_lowercase(),
    ));
}

#[inline]
fn stop_subprocess() {
    let _ = run_cmds(&format!(
        r##"ps -ef | grep '/etc/{}/xorg.conf' | grep -v grep | awk '{{print $2}}' | xargs -r kill -9"##,
        crate::get_app_name().to_lowercase(),
    ));
    let _ = run_cmds(&format!(
        r##"ps -ef | grep -E '{} +--cm-no-ui' | grep -v grep | awk '{{print $2}}' | xargs -r kill -9"##,
        crate::get_app_name().to_lowercase(),
    ));
}

fn should_start_server(
    try_x11: bool,
    is_display_changed: bool,
    uid: &mut String,
    desktop: &Desktop,
    cm0: &mut bool,
    last_restart: &mut Instant,
    server: &mut Option<Child>,
) -> bool {
    let cm = get_cm();
    let mut start_new = false;
    let mut should_kill = false;

    if desktop.is_headless() {
        if !uid.is_empty() {
            // From having a monitor to not having a monitor.
            *uid = "".to_owned();
            should_kill = true;
        }
    } else if is_display_changed || desktop.uid != *uid && !desktop.uid.is_empty() {
        *uid = desktop.uid.clone();
        if try_x11 {
            set_x11_env(&desktop);
        }
        should_kill = true;
    }

    if !should_kill
        && !cm
        && ((*cm0 && last_restart.elapsed().as_secs() > 60)
            || last_restart.elapsed().as_secs() > 3600)
    {
        let terminal_session_count = crate::ipc::get_terminal_session_count().unwrap_or(0);
        if terminal_session_count > 0 {
            // There are terminal sessions, so we don't restart the server.
            // We also need to keep `cm0` unchanged, so that we can reach this branch the next time.
            return false;
        }
        // restart server if new connections all closed, or every one hour,
        // as a workaround to resolve "SpotUdp" (dns resolve)
        // and x server get displays failure issue
        should_kill = true;
        log::info!("restart server");
    }

    if should_kill {
        if let Some(ps) = server.as_mut() {
            allow_err!(ps.kill());
            sleep_millis(30);
            *last_restart = Instant::now();
        }
    }

    if let Some(ps) = server.as_mut() {
        match ps.try_wait() {
            Ok(Some(_)) => {
                *server = None;
                start_new = true;
            }
            _ => {}
        }
    } else {
        start_new = true;
    }
    *cm0 = cm;
    start_new
}

// to-do: stop_server(&mut user_server); may not stop child correctly
// stop_rustdesk_servers() is just a temp solution here.
fn force_stop_server() {
    stop_rustdesk_servers();
    sleep_millis(super::SERVICE_INTERVAL);
}

pub fn start_os_service() {
    check_if_stop_service();
    stop_rustdesk_servers();
    stop_subprocess();
    start_uinput_service();

    std::thread::spawn(|| {
        allow_err!(crate::ipc::start(crate::POSTFIX_SERVICE));
    });

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let (mut display, mut xauth): (String, String) = ("".to_owned(), "".to_owned());
    let mut desktop = Desktop::default();
    let mut sid = "".to_owned();
    let mut uid = "".to_owned();
    let mut server: Option<Child> = None;
    let mut user_server: Option<Child> = None;
    if let Err(err) = ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }) {
        println!("Failed to set Ctrl-C handler: {}", err);
    }

    let mut cm0 = false;
    let mut last_restart = Instant::now();
    while running.load(Ordering::SeqCst) {
        desktop.refresh();

        // Duplicate logic here with should_start_server
        // Login wayland will try to start a headless --server.
        if desktop.username == "root" || desktop.is_login_wayland() {
            // try kill subprocess "--server"
            stop_server(&mut user_server);
            // try start subprocess "--server"
            // No need to check is_display_changed here.
            if should_start_server(
                true,
                false,
                &mut uid,
                &desktop,
                &mut cm0,
                &mut last_restart,
                &mut server,
            ) {
                stop_subprocess();
                force_stop_server();
                start_server(None, &mut server);
            }
        } else if desktop.username != "" {
            // try kill subprocess "--server"
            stop_server(&mut server);

            let is_display_changed = desktop.display != display || desktop.xauth != xauth;
            display = desktop.display.clone();
            xauth = desktop.xauth.clone();

            // try start subprocess "--server"
            if should_start_server(
                !desktop.is_wayland(),
                is_display_changed,
                &mut uid,
                &desktop,
                &mut cm0,
                &mut last_restart,
                &mut user_server,
            ) {
                stop_subprocess();
                force_stop_server();
                start_server(Some(&desktop), &mut user_server);
            }
        } else {
            force_stop_server();
            stop_server(&mut user_server);
            stop_server(&mut server);
        }

        let keeps_headless = sid.is_empty() && desktop.is_headless();
        let keeps_session = sid == desktop.sid;
        if keeps_headless || keeps_session {
            // for fixing https://github.com/rustdesk/rustdesk/issues/3129 to avoid too much dbus calling,
            sleep_millis(500);
        } else {
            sleep_millis(super::SERVICE_INTERVAL);
        }
        if !desktop.is_headless() {
            sid = desktop.sid.clone();
        }
    }

    if let Some(ps) = user_server.take().as_mut() {
        allow_err!(ps.kill());
    }
    if let Some(ps) = server.take().as_mut() {
        allow_err!(ps.kill());
    }
    log::info!("Exit");
}

#[inline]
pub fn get_active_user_id_name() -> (String, String) {
    let vec_id_name = get_values_of_seat0(&[1, 2]);
    (vec_id_name[0].clone(), vec_id_name[1].clone())
}

#[inline]
pub fn get_active_userid() -> String {
    get_values_of_seat0(&[1])[0].clone()
}

fn get_cm() -> bool {
    // We use `CMD_PS` instead of `ps` to suppress some audit messages on some systems.
    if let Ok(output) = Command::new(CMD_PS.as_str()).args(vec!["aux"]).output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.contains(&format!(
                "{} --cm",
                std::env::current_exe()
                    .unwrap_or("".into())
                    .to_string_lossy()
            )) {
                return true;
            }
        }
    }
    false
}

pub fn is_login_wayland() -> bool {
    let files = ["/etc/gdm3/custom.conf", "/etc/gdm/custom.conf"];
    match (
        Regex::new(r"# *WaylandEnable *= *false"),
        Regex::new(r"WaylandEnable *= *true"),
    ) {
        (Ok(pat1), Ok(pat2)) => {
            for file in files {
                if let Ok(contents) = std::fs::read_to_string(file) {
                    return pat1.is_match(&contents) || pat2.is_match(&contents);
                }
            }
        }
        _ => {}
    }
    false
}

#[inline]
pub fn current_is_wayland() -> bool {
    return is_desktop_wayland() && unsafe { UNMODIFIED };
}

// to-do: test the other display manager
fn _get_display_manager() -> String {
    if let Ok(x) = std::fs::read_to_string("/etc/X11/default-display-manager") {
        if let Some(x) = x.split("/").last() {
            return x.to_owned();
        }
    }
    "gdm3".to_owned()
}

#[inline]
pub fn get_active_username() -> String {
    get_values_of_seat0(&[2])[0].clone()
}

pub fn get_user_home_by_name(username: &str) -> Option<PathBuf> {
    return match get_user_by_name(username) {
        None => None,
        Some(user) => {
            let home = user.home_dir();
            if Path::is_dir(home) {
                Some(PathBuf::from(home))
            } else {
                None
            }
        }
    };
}

pub fn get_active_user_home() -> Option<PathBuf> {
    let username = get_active_username();
    if !username.is_empty() {
        match get_user_home_by_name(&username) {
            None => {
                // fallback to most common default pattern
                let home = PathBuf::from(format!("/home/{}", username));
                if home.exists() {
                    return Some(home);
                }
            }
            Some(home) => {
                return Some(home);
            }
        }
    }
    None
}

pub fn get_env_var(k: &str) -> String {
    match std::env::var(k) {
        Ok(v) => v,
        Err(_e) => "".to_owned(),
    }
}

fn is_flatpak() -> bool {
    std::path::PathBuf::from("/.flatpak-info").exists()
}

// Headless is enabled, always return true.
pub fn is_prelogin() -> bool {
    if is_flatpak() {
        return false;
    }
    let name = get_active_username();
    if let Ok(res) = run_cmds(&format!("getent passwd {}", name)) {
        return res.contains("/bin/false") || res.contains("/usr/sbin/nologin");
    }
    false
}

// Check "Lock".
// "Switch user" can't be checked, because `get_values_of_seat0(&[0])` does not return the session.
// The logged in session is "online" not "active".
// And the "Switch user" screen is usually Wayland login session, which we do not support.
pub fn is_locked() -> bool {
    if is_prelogin() {
        return false;
    }

    let values = get_values_of_seat0(&[0]);
    // Though the values can't be empty, we still add check here for safety.
    // Because we cannot guarantee whether the internal implementation will change in the future.
    // https://github.com/rustdesk/hbb_common/blob/ebb4d4a48cf7ed6ca62e93f8ed124065c6408536/src/platform/linux.rs#L119
    if values.is_empty() {
        log::debug!("Failed to check is locked, values vector is empty.");
        return false;
    }
    let session = &values[0];
    if session.is_empty() {
        log::debug!("Failed to check is locked, session is empty.");
        return false;
    }
    is_session_locked(session)
}

pub fn is_root() -> bool {
    crate::username() == "root"
}

fn is_opensuse() -> bool {
    if let Ok(res) = run_cmds("cat /etc/os-release | grep opensuse") {
        if !res.is_empty() {
            return true;
        }
    }
    false
}

pub fn run_as_user<I, K, V>(
    arg: Vec<&str>,
    user: Option<(String, String)>,
    envs: I,
) -> ResultType<Option<Child>>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    let (uid, username) = match user {
        Some(id_name) => id_name,
        None => get_active_user_id_name(),
    };
    let cmd = std::env::current_exe()?;
    if uid.is_empty() {
        bail!("No valid uid");
    }

    let xdg = &format!("XDG_RUNTIME_DIR=/run/user/{uid}");
    if *SUDO_E_PRESERVES_ENV {
        // Original logic: use sudo -E to preserve environment
        let mut args = vec![xdg, "-u", &username, cmd.to_str().unwrap_or("")];
        args.append(&mut arg.clone());
        // -E is required to preserve env
        args.insert(0, "-E");
        let task = Command::new("sudo").envs(envs).args(args).spawn()?;
        Ok(Some(task))
    } else {
        // Fallback: sudo -u username env VAR=VALUE ... cmd args
        // For systems where sudo -E is not supported (e.g., Ubuntu 25.10+)
        //
        // SECURITY: No shell is involved here (we use execve-style argv).
        // Environment is passed via `env` arguments,
        // so there is no shell injection vector.
        //
        // Only accept portable env var names (POSIX portable character set for shells).
        // Most legitimate env vars follow [A-Za-z_][A-Za-z0-9_]* convention.
        // Variables with dots (e.g., "java.home") are Java system properties, not env vars.
        // Being restrictive here is intentional for security in this sudo context.
        fn is_valid_env_key(key: &str) -> bool {
            let mut it = key.chars();
            match it.next() {
                Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
                _ => return false,
            }
            it.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }

        let mut sudo = Command::new("sudo");
        sudo.arg("-u").arg(&username).arg("--").arg("env").arg(xdg);

        for (k, v) in envs {
            let key = k.as_ref().to_string_lossy();
            if !is_valid_env_key(&key) {
                log::warn!("Skipping environment variable with invalid key: '{}'. Only [A-Za-z_][A-Za-z0-9_]* are allowed in sudo context.", key);
                continue;
            }
            // IMPORTANT: do NOT add shell quotes here; `Command` does not invoke a shell.
            // Passing KEY=VALUE as a single argv element is safe and preserves spaces.
            let mut arg = OsString::from(&*key);
            arg.push("=");
            arg.push(v.as_ref());
            sudo.arg(arg);
        }

        sudo.arg(cmd).args(arg);
        let task = sudo.spawn()?;
        Ok(Some(task))
    }
}

pub fn get_pa_monitor() -> String {
    get_pa_sources()
        .drain(..)
        .map(|x| x.0)
        .filter(|x| x.contains("monitor"))
        .next()
        .unwrap_or("".to_owned())
}

pub fn get_pa_source_name(desc: &str) -> String {
    get_pa_sources()
        .drain(..)
        .filter(|x| x.1 == desc)
        .map(|x| x.0)
        .next()
        .unwrap_or("".to_owned())
}

pub fn get_pa_sources() -> Vec<(String, String)> {
    use pulsectl::controllers::*;
    let mut out = Vec::new();
    match SourceController::create() {
        Ok(mut handler) => {
            if let Ok(devices) = handler.list_devices() {
                for dev in devices.clone() {
                    out.push((
                        dev.name.unwrap_or("".to_owned()),
                        dev.description.unwrap_or("".to_owned()),
                    ));
                }
            }
        }
        Err(err) => {
            log::error!("Failed to get_pa_sources: {:?}", err);
        }
    }
    out
}

pub fn get_default_pa_source() -> Option<(String, String)> {
    use pulsectl::controllers::*;
    match SourceController::create() {
        Ok(mut handler) => {
            if let Ok(dev) = handler.get_default_device() {
                return Some((
                    dev.name.unwrap_or("".to_owned()),
                    dev.description.unwrap_or("".to_owned()),
                ));
            }
        }
        Err(err) => {
            log::error!("Failed to get_pa_source: {:?}", err);
        }
    }
    None
}

pub fn lock_screen() {
    Command::new("xdg-screensaver").arg("lock").spawn().ok();
}

pub fn toggle_blank_screen(_v: bool) {
    // https://unix.stackexchange.com/questions/17170/disable-keyboard-mouse-input-on-unix-under-x
}

pub fn block_input(_v: bool) -> (bool, String) {
    (true, "".to_owned())
}

pub fn is_installed() -> bool {
    if let Ok(p) = std::env::current_exe() {
        p.to_str().unwrap_or_default().starts_with("/usr")
            || p.to_str().unwrap_or_default().starts_with("/nix/store")
    } else {
        false
    }
}

/// Get multiple environment variables from a process matching the given criteria.
/// This version reads /proc directly instead of spawning shell commands.
///
/// # Arguments
/// * `uid` - User ID to filter processes
/// * `process_pat` - Regex pattern to match process cmdline
/// * `names` - Environment variable names to retrieve. **Must be <= 64 elements** due to
///   the internal bitmask used for tie-breaking.
///
/// # Panics (debug builds)
/// Panics if `names.len() > 64`.
///
/// # Implementation notes
/// - Returns values from a *single* best-matching process_pat (for consistency).
/// - Avoids repeated scanning by parsing `environ` once per process.
fn get_envs<'a>(
    uid: &str,
    process_pat: &str,
    names: &[&'a str],
) -> std::collections::HashMap<&'a str, String> {
    // The tie-breaking logic uses a u64 bitmask, limiting us to 64 variables.
    debug_assert!(
        names.len() <= 64,
        "get_envs: names.len() must be <= 64, got {}",
        names.len()
    );

    let empty: std::collections::HashMap<&'a str, String> =
        names.iter().map(|&n| (n, String::new())).collect();

    let Ok(uid_num) = uid.parse::<u32>() else {
        return empty;
    };
    let Ok(re) = Regex::new(process_pat) else {
        return empty;
    };

    // Used for stable tie-breaking when multiple processes match.
    // Higher bits correspond to earlier entries in `names`.
    let name_indices: std::collections::HashMap<&'a str, usize> =
        names.iter().enumerate().map(|(i, &n)| (n, i)).collect();

    let mut best = empty.clone();
    let mut best_count = 0usize;
    let mut best_mask: u64 = 0;

    // Iterate /proc to find matching processes
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return best;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(pid_str) = file_name.to_str() else {
            continue;
        };
        if !pid_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let proc_path = entry.path();

        // Check if process belongs to the specified uid
        if let Ok(meta) = std::fs::metadata(&proc_path) {
            use std::os::unix::fs::MetadataExt;
            if meta.uid() != uid_num {
                continue;
            }
        } else {
            continue;
        }

        // Check cmdline matches process pattern
        let cmdline_path = proc_path.join("cmdline");
        let Ok(cmdline) = std::fs::read(&cmdline_path) else {
            continue;
        };
        let cmdline_str = String::from_utf8_lossy(&cmdline).replace('\0', " ");
        if !re.is_match(&cmdline_str) {
            continue;
        }

        // Read environ and extract matching variables
        let environ_path = proc_path.join("environ");
        let Ok(environ) = std::fs::read(&environ_path) else {
            continue;
        };

        let mut found = empty.clone();
        let mut found_count = 0usize;
        let mut found_mask: u64 = 0;

        for part in environ.split(|&b| b == 0) {
            if part.is_empty() {
                continue;
            }
            let Some(eq) = part.iter().position(|&b| b == b'=') else {
                continue;
            };
            let key_bytes = &part[..eq];
            let val_bytes = &part[eq + 1..];

            let Ok(key) = std::str::from_utf8(key_bytes) else {
                continue;
            };
            if let Some(slot) = found.get_mut(key) {
                if slot.is_empty() {
                    *slot = String::from_utf8_lossy(val_bytes).into_owned();
                    found_count += 1;

                    if let Some(&idx) = name_indices.get(key) {
                        let total = names.len();
                        if total <= 64 {
                            let bit = 1u64 << (total - 1 - idx);
                            found_mask |= bit;
                        }
                    }

                    if found_count == names.len() {
                        return found;
                    }
                }
            }
        }

        if found_count > best_count || (found_count == best_count && found_mask > best_mask) {
            best = found;
            best_count = found_count;
            best_mask = found_mask;
        }
    }

    best
}

/// Deprecated: Use `get_envs` instead.
///
/// https://github.com/rustdesk/rustdesk/discussions/11959
///
/// **Note**: This function is retained for conservative migration. The plan is to gradually
/// transition all callers to `get_envs` after it proves stable and reliable. Once `get_envs`
/// is confirmed to work correctly across all use cases, this function will be removed entirely.
///
/// # Arguments
/// * `name` - Environment variable name to retrieve
/// * `uid` - User ID to filter processes
/// * `process` - Process name pattern to match
///
/// # Returns
/// The environment variable value, or empty string if not found
#[inline]
fn get_env(name: &str, uid: &str, process: &str) -> String {
    let cmd = format!("ps -u {} -f | grep -E '{}' | grep -v 'grep' | tail -1 | awk '{{print $2}}' | xargs -I__ cat /proc/__/environ 2>/dev/null | tr '\\0' '\\n' | grep '^{}=' | tail -1 | sed 's/{}=//g'", uid, process, name, name);
    if let Ok(x) = run_cmds(&cmd) {
        x.trim_end().to_string()
    } else {
        "".to_owned()
    }
}

#[inline]
fn get_env_from_pid(name: &str, pid: &str) -> String {
    let cmd = format!("cat /proc/{}/environ 2>/dev/null | tr '\\0' '\\n' | grep '^{}=' | tail -1 | sed 's/{}=//g'", pid, name, name);
    if let Ok(x) = run_cmds(&cmd) {
        x.trim_end().to_string()
    } else {
        "".to_owned()
    }
}

#[link(name = "gtk-3")]
extern "C" {
    fn gtk_main_quit();
}

pub fn quit_gui() {
    unsafe { gtk_main_quit() };
}

/*
pub fn exec_privileged(args: &[&str]) -> ResultType<Child> {
    Ok(Command::new("pkexec").args(args).spawn()?)
}
*/

pub fn check_super_user_permission() -> ResultType<bool> {
    gtk_sudo::run(vec!["echo"])?;
    Ok(true)
}

/*
pub fn elevate(args: Vec<&str>) -> ResultType<bool> {
    let cmd = std::env::current_exe()?;
    match cmd.to_str() {
        Some(cmd) => {
            let mut args_with_exe = vec![cmd];
            args_with_exe.append(&mut args.clone());
            // -E required for opensuse
            if is_opensuse() {
                args_with_exe.insert(0, "-E");
            }
            let res = match exec_privileged(&args_with_exe)?.wait() {
                Ok(status) => {
                    if status.success() {
                        true
                    } else {
                        log::error!(
                            "Failed to wait install process, process status: {:?}",
                            status
                        );
                        false
                    }
                }
                Err(e) => {
                    log::error!("Failed to wait install process, error: {}", e);
                    false
                }
            };
            Ok(res)
        }
        None => {
            hbb_common::bail!("Failed to get current exe as str");
        }
    }
}
*/

type GtkSettingsPtr = *mut c_void;
type GObjectPtr = *mut c_void;
#[link(name = "gtk-3")]
extern "C" {
    // fn gtk_init(argc: *mut c_int, argv: *mut *mut c_char);
    fn gtk_settings_get_default() -> GtkSettingsPtr;
}

#[link(name = "gobject-2.0")]
extern "C" {
    fn g_object_get(object: GObjectPtr, first_property_name: *const c_char, ...);
}

pub fn get_double_click_time() -> u32 {
    // GtkSettings *settings = gtk_settings_get_default ();
    // g_object_get (settings, "gtk-double-click-time", &double_click_time, NULL);
    unsafe {
        let mut double_click_time = 0u32;
        let Ok(property) = std::ffi::CString::new("gtk-double-click-time") else {
            return 0;
        };
        let settings = gtk_settings_get_default();
        g_object_get(
            settings,
            property.as_ptr(),
            &mut double_click_time as *mut u32,
            0 as *const c_void,
        );
        double_click_time
    }
}

#[inline]
fn get_width_height_from_captures<'t>(caps: &Captures<'t>) -> Option<(i32, i32)> {
    match (caps.name("width"), caps.name("height")) {
        (Some(width), Some(height)) => {
            match (
                width.as_str().parse::<i32>(),
                height.as_str().parse::<i32>(),
            ) {
                (Ok(width), Ok(height)) => {
                    return Some((width, height));
                }
                _ => {}
            }
        }
        _ => {}
    }
    None
}

#[inline]
fn get_xrandr_conn_pat(name: &str) -> String {
    format!(
        r"{}\s+connected.+?(?P<width>\d+)x(?P<height>\d+)\+(?P<x>\d+)\+(?P<y>\d+).*?\n",
        name
    )
}

pub fn resolutions(name: &str) -> Vec<Resolution> {
    let resolutions_pat = r"(?P<resolutions>(\s*\d+x\d+\s+\d+.*\n)+)";
    let connected_pat = get_xrandr_conn_pat(name);
    let mut v = vec![];
    if let Ok(re) = Regex::new(&format!("{}{}", connected_pat, resolutions_pat)) {
        match run_cmds("xrandr --query | tr -s ' '") {
            Ok(xrandr_output) => {
                // There'are different kinds of xrandr output.
                /*
                1.
                Screen 0: minimum 320 x 175, current 1920 x 1080, maximum 1920 x 1080
                default connected 1920x1080+0+0 0mm x 0mm
                 1920x1080 10.00*
                 1280x720 25.00
                 1680x1050 60.00
                Virtual2 disconnected (normal left inverted right x axis y axis)
                Virtual3 disconnected (normal left inverted right x axis y axis)

                Screen 0: minimum 320 x 200, current 1920 x 1080, maximum 16384 x 16384
                eDP-1 connected primary 1920x1080+0+0 (normal left inverted right x axis y axis) 344mm x 193mm
                1920x1080     60.01*+  60.01    59.97    59.96    59.93
                1680x1050     59.95    59.88
                1600x1024     60.17

                XWAYLAND0 connected primary 1920x984+0+0 (normal left inverted right x axis y axis) 0mm x 0mm
                Virtual1 connected primary 1920x984+0+0 (normal left inverted right x axis y axis) 0mm x 0mm
                HDMI-0 connected (normal left inverted right x axis y axis)

                rdp0 connected primary 1920x1080+0+0 0mm x 0mm
                    */
                if let Some(caps) = re.captures(&xrandr_output) {
                    if let Some(resolutions) = caps.name("resolutions") {
                        let resolution_pat =
                            r"\s*(?P<width>\d+)x(?P<height>\d+)\s+(?P<rates>(\d+\.\d+\D*)+)\s*\n";
                        let Ok(resolution_re) = Regex::new(&format!(r"{}", resolution_pat)) else {
                            log::error!("Regex new failed");
                            return vec![];
                        };
                        for resolution_caps in resolution_re.captures_iter(resolutions.as_str()) {
                            if let Some((width, height)) =
                                get_width_height_from_captures(&resolution_caps)
                            {
                                let resolution = Resolution {
                                    width,
                                    height,
                                    ..Default::default()
                                };
                                if !v.contains(&resolution) {
                                    v.push(resolution);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => log::error!("Failed to run xrandr query, {}", e),
        }
    }

    v
}

pub fn current_resolution(name: &str) -> ResultType<Resolution> {
    let xrandr_output = run_cmds("xrandr --query | tr -s ' '")?;
    let re = Regex::new(&get_xrandr_conn_pat(name))?;
    if let Some(caps) = re.captures(&xrandr_output) {
        if let Some((width, height)) = get_width_height_from_captures(&caps) {
            return Ok(Resolution {
                width,
                height,
                ..Default::default()
            });
        }
    }
    bail!("Failed to find current resolution for {}", name);
}

pub fn change_resolution_directly(name: &str, width: usize, height: usize) -> ResultType<()> {
    Command::new("xrandr")
        .args(vec![
            "--output",
            name,
            "--mode",
            &format!("{}x{}", width, height),
        ])
        .spawn()?;
    Ok(())
}

#[inline]
pub fn is_xwayland_running() -> bool {
    if let Ok(output) = run_cmds("pgrep -a Xwayland") {
        return output.contains("Xwayland");
    }
    false
}

mod desktop {
    use super::*;

    pub const XFCE4_PANEL: &str = "xfce4-panel";
    pub const SDDM_GREETER: &str = "sddm-greeter";

    // xdg-desktop-portal runs on all Wayland desktops (GNOME, KDE, wlroots, etc.)
    const XDG_DESKTOP_PORTAL: &str = "xdg-desktop-portal";
    const XWAYLAND: &str = "Xwayland";
    const IBUS_DAEMON: &str = "ibus-daemon";
    const PLASMA_KDED: &str = "kded[0-9]+";
    const GNOME_GOA_DAEMON: &str = "goa-daemon";

    const ENV_KEY_DISPLAY: &str = "DISPLAY";
    const ENV_KEY_XAUTHORITY: &str = "XAUTHORITY";
    const ENV_KEY_WAYLAND_DISPLAY: &str = "WAYLAND_DISPLAY";
    const ENV_KEY_DBUS_SESSION_BUS_ADDRESS: &str = "DBUS_SESSION_BUS_ADDRESS";

    #[derive(Debug, Clone, Default)]
    pub struct Desktop {
        pub sid: String,
        pub username: String,
        pub uid: String,
        pub protocol: String,
        pub display: String,
        pub xauth: String,
        pub home: String,
        pub dbus: String,
        pub is_rustdesk_subprocess: bool,
        pub wl_display: String,
    }

    impl Desktop {
        #[inline]
        pub fn is_wayland(&self) -> bool {
            self.protocol == DISPLAY_SERVER_WAYLAND
        }

        #[inline]
        pub fn is_login_wayland(&self) -> bool {
            super::is_gdm_user(&self.username) && self.protocol == DISPLAY_SERVER_WAYLAND
        }

        #[inline]
        pub fn is_headless(&self) -> bool {
            self.sid.is_empty() || self.is_rustdesk_subprocess
        }

        fn get_display_xauth_wayland(&mut self) {
            for _ in 1..=10 {
                // Prefer Wayland-related variables first when multiple portal processes match.
                let mut envs = get_envs(
                    &self.uid,
                    XDG_DESKTOP_PORTAL,
                    &[
                        ENV_KEY_WAYLAND_DISPLAY,
                        ENV_KEY_DBUS_SESSION_BUS_ADDRESS,
                        ENV_KEY_DISPLAY,
                        ENV_KEY_XAUTHORITY,
                    ],
                );
                self.display = envs.remove(ENV_KEY_DISPLAY).unwrap_or_default();
                self.xauth = envs.remove(ENV_KEY_XAUTHORITY).unwrap_or_default();
                self.wl_display = envs.remove(ENV_KEY_WAYLAND_DISPLAY).unwrap_or_default();
                self.dbus = envs
                    .remove(ENV_KEY_DBUS_SESSION_BUS_ADDRESS)
                    .unwrap_or_default();
                // For pure Wayland sessions, prefer `WAYLAND_DISPLAY`.
                // NOTE: On some systems (e.g. Ubuntu 25.10), `DISPLAY`/`XAUTHORITY` may exist even when XWayland
                // is not running, so do NOT treat them as a success condition here.
                let has_wayland = !self.wl_display.is_empty();
                let has_dbus = !self.dbus.is_empty();
                if has_wayland && has_dbus {
                    return;
                }
                sleep_millis(300);
            }
        }

        fn get_display_xauth_xwayland(&mut self) {
            let tray = format!("{} +--tray", crate::get_app_name().to_lowercase());
            for _ in 1..=10 {
                let display_proc = vec![
                    XDG_DESKTOP_PORTAL,
                    XWAYLAND,
                    IBUS_DAEMON,
                    GNOME_GOA_DAEMON,
                    PLASMA_KDED,
                    tray.as_str(),
                ];
                for proc in display_proc {
                    self.display = get_env(ENV_KEY_DISPLAY, &self.uid, proc);
                    self.xauth = get_env(ENV_KEY_XAUTHORITY, &self.uid, proc);
                    self.wl_display = get_env(ENV_KEY_WAYLAND_DISPLAY, &self.uid, proc);
                    self.dbus = get_env(ENV_KEY_DBUS_SESSION_BUS_ADDRESS, &self.uid, proc);
                    if !self.display.is_empty() && !self.xauth.is_empty() {
                        return;
                    }
                }
                sleep_millis(300);
            }
        }

        fn get_display_x11(&mut self) {
            for _ in 1..=10 {
                let display_proc = vec![
                    XWAYLAND,
                    IBUS_DAEMON,
                    GNOME_GOA_DAEMON,
                    PLASMA_KDED,
                    XFCE4_PANEL,
                    SDDM_GREETER,
                ];
                for proc in display_proc {
                    self.display = get_env(ENV_KEY_DISPLAY, &self.uid, proc);
                    if !self.display.is_empty() {
                        break;
                    }
                }
                if !self.display.is_empty() {
                    break;
                }
                sleep_millis(300);
            }

            if self.display.is_empty() {
                self.display = Self::get_display_by_user(&self.username);
            }
            if self.display.is_empty() {
                self.display = ":0".to_owned();
            }
            self.display = self
                .display
                .replace(&hbb_common::whoami::hostname(), "")
                .replace("localhost", "");
        }

        fn get_home(&mut self) {
            self.home = "".to_string();

            let cmd = format!(
                "getent passwd '{}' | awk -F':' '{{print $6}}'",
                &self.username
            );
            self.home = run_cmds_trim_newline(&cmd).unwrap_or(format!("/home/{}", &self.username));
        }

        fn get_xauth_from_xorg(&mut self) {
            if let Ok(output) = run_cmds(&format!(
                "ps -u {} -f | grep 'Xorg' | grep -v 'grep'",
                &self.uid
            )) {
                for line in output.lines() {
                    let mut auth_found = false;

                    for v in line.split_whitespace() {
                        if v == "-auth" {
                            auth_found = true;
                        } else if auth_found {
                            if std::path::Path::new(v).is_absolute()
                                && std::path::Path::new(v).exists()
                            {
                                self.xauth = v.to_string();
                            } else {
                                if let Some(pid) = line.split_whitespace().nth(1) {
                                    let mut base_dir: String = String::from("/home"); // default pattern
                                    let home_dir = get_env_from_pid("HOME", pid);
                                    if home_dir.is_empty() {
                                        if let Some(home) = get_user_home_by_name(&self.username) {
                                            base_dir = home.as_path().to_string_lossy().to_string();
                                        };
                                    } else {
                                        base_dir = home_dir;
                                    }
                                    if Path::new(&base_dir).exists() {
                                        self.xauth = format!("{}/{}", base_dir, v);
                                    };
                                } else {
                                    // unreachable!
                                }
                            }
                            return;
                        }
                    }
                }
            }
        }

        fn get_xauth_x11(&mut self) {
            // try by direct access to window manager process by name
            let tray = format!("{} +--tray", crate::get_app_name().to_lowercase());
            for _ in 1..=10 {
                let display_proc = vec![
                    XWAYLAND,
                    IBUS_DAEMON,
                    GNOME_GOA_DAEMON,
                    PLASMA_KDED,
                    XFCE4_PANEL,
                    SDDM_GREETER,
                    tray.as_str(),
                ];
                for proc in display_proc {
                    self.xauth = get_env("XAUTHORITY", &self.uid, proc);
                    if !self.xauth.is_empty() {
                        break;
                    }
                }
                if !self.xauth.is_empty() {
                    break;
                }
                sleep_millis(300);
            }

            // get from Xorg process, parameter and environment
            if self.xauth.is_empty() {
                self.get_xauth_from_xorg();
            }

            // fallback to default file name
            if self.xauth.is_empty() {
                let gdm = format!("/run/user/{}/gdm/Xauthority", self.uid);
                self.xauth = if std::path::Path::new(&gdm).exists() {
                    gdm
                } else {
                    let username = &self.username;
                    match get_user_home_by_name(username) {
                        None => {
                            if username == "root" {
                                format!("/{}/.Xauthority", username)
                            } else {
                                let tmp = format!("/home/{}/.Xauthority", username);
                                if std::path::Path::new(&tmp).exists() {
                                    tmp
                                } else {
                                    format!("/var/lib/{}/.Xauthority", username)
                                }
                            }
                        }
                        Some(home) => {
                            format!(
                                "{}/.Xauthority",
                                home.as_path().to_string_lossy().to_string()
                            )
                        }
                    }
                };
            }
        }

        fn get_display_by_user(user: &str) -> String {
            // log::debug!("w {}", &user);
            if let Ok(output) = std::process::Command::new("w").arg(&user).output() {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let mut iter = line.split_whitespace();
                    let b = iter.nth(2);
                    if let Some(b) = b {
                        if b.starts_with(":") {
                            return b.to_owned();
                        }
                    }
                }
            }
            // above not work for gdm user
            //log::debug!("ls -l /tmp/.X11-unix/");
            let mut last = "".to_owned();
            if let Ok(output) = std::process::Command::new("ls")
                .args(vec!["-l", "/tmp/.X11-unix/"])
                .output()
            {
                for line in String::from_utf8_lossy(&output.stdout).lines() {
                    let mut iter = line.split_whitespace();
                    let user_field = iter.nth(2);
                    if let Some(x) = iter.last() {
                        if x.starts_with("X") {
                            last = x.replace("X", ":").to_owned();
                            if user_field == Some(&user) {
                                return last;
                            }
                        }
                    }
                }
            }
            last
        }

        fn set_is_subprocess(&mut self) {
            self.is_rustdesk_subprocess = false;
            let cmd = format!(
                "ps -ef | grep '{}/xorg.conf' | grep -v grep | wc -l",
                crate::get_app_name().to_lowercase()
            );
            if let Ok(res) = run_cmds(&cmd) {
                if res.trim() != "0" {
                    self.is_rustdesk_subprocess = true;
                }
            }
        }

        pub fn refresh(&mut self) {
            if !self.sid.is_empty() && is_active_and_seat0(&self.sid) {
                // Xwayland display and xauth may not be available in a short time after login.
                if is_xwayland_running() && !self.is_login_wayland() {
                    self.get_display_xauth_xwayland();
                    self.is_rustdesk_subprocess = false;
                } else if self.is_wayland() {
                    self.get_display_xauth_wayland();
                }
                return;
            }

            let seat0_values = get_values_of_seat0_with_gdm_wayland(&[0, 1, 2]);
            if seat0_values[0].is_empty() {
                *self = Self::default();
                self.is_rustdesk_subprocess = false;
                return;
            }

            self.sid = seat0_values[0].clone();
            self.uid = seat0_values[1].clone();
            self.username = seat0_values[2].clone();
            self.protocol = get_display_server_of_session(&self.sid).into();
            if self.is_login_wayland() {
                self.display = "".to_owned();
                self.xauth = "".to_owned();
                self.is_rustdesk_subprocess = false;
                return;
            }

            self.get_home();
            if self.is_wayland() {
                if is_xwayland_running() {
                    self.get_display_xauth_xwayland();
                } else {
                    self.get_display_xauth_wayland();
                }
                self.is_rustdesk_subprocess = false;
            } else {
                self.get_display_x11();
                self.get_xauth_x11();
                self.set_is_subprocess();
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_desktop_env() {
            let mut d = Desktop::default();
            d.refresh();
            if d.username == "root" {
                assert_eq!(d.home, "/root");
            } else {
                if !d.username.is_empty() {
                    let home = super::super::get_env_var("HOME");
                    if !home.is_empty() {
                        assert_eq!(d.home, home);
                    } else {
                        //
                    }
                }
            }
        }
    }
}

pub struct WakeLock(Option<keepawake::AwakeHandle>);

impl WakeLock {
    pub fn new(display: bool, idle: bool, sleep: bool) -> Self {
        WakeLock(
            keepawake::Builder::new()
                .display(display)
                .idle(idle)
                .sleep(sleep)
                .create()
                .ok(),
        )
    }
}

fn has_cmd(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .status()
        .map(|x| x.success())
        .unwrap_or_default()
}

pub fn run_cmds_privileged(cmds: &str) -> bool {
    crate::platform::gtk_sudo::run(vec![cmds]).is_ok()
}

/// Spawn the current executable after a delay.
///
/// # Security
/// The executable path is safely quoted using `shell_quote()` to prevent
/// command injection vulnerabilities. The `secs` parameter is a u32, so it
/// cannot contain malicious input.
///
/// # Arguments
/// * `secs` - Number of seconds to wait before spawning
pub fn run_me_with(secs: u32) {
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            log::error!("Failed to get current exe: {}", e);
            return;
        }
    };

    // SECURITY: Use shell_quote to safely escape the executable path,
    // preventing command injection even if the path contains special characters.
    let exe_quoted = shell_quote(&exe.to_string_lossy());

    // Spawn a background process that sleeps and then executes.
    // The child process is automatically orphaned when parent exits,
    // and will be adopted by init (PID 1).
    Command::new(CMD_SH.as_str())
        .arg("-c")
        .arg(&format!("sleep {secs}; exec {exe_quoted}"))
        .spawn()
        .ok();
}

fn switch_service(stop: bool) -> String {
    // SECURITY: Use trusted home directory lookup via getpwuid instead of $HOME env var
    // to prevent confused-deputy attacks where an attacker manipulates environment variables.
    let home = get_home_dir_trusted()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    Config::set_option("stop-service".into(), if stop { "Y" } else { "" }.into());
    if !home.is_empty() && home != "/root" && !Config::get().is_empty() {
        let app_name_lower = crate::get_app_name().to_lowercase();
        let app_name0 = crate::get_app_name();
        let config_subdir = format!(".config/{}", app_name_lower);

        // SECURITY: Quote all paths to prevent shell injection from paths containing
        // spaces, semicolons, or other special characters.
        let src1 = shell_quote(&format!("{}/{}/{}.toml", home, config_subdir, app_name0));
        let src2 = shell_quote(&format!("{}/{}/{}2.toml", home, config_subdir, app_name0));
        let dst = shell_quote(&format!("/root/{}/", config_subdir));

        format!("cp -f {} {}; cp -f {} {};", src1, dst, src2, dst)
    } else {
        "".to_owned()
    }
}

pub fn uninstall_service(show_new_window: bool, _: bool) -> bool {
    if !has_cmd("systemctl") {
        // Failed when installed + flutter run + started by `show_new_window`.
        return false;
    }
    log::info!("Uninstalling service...");
    let cp = switch_service(true);
    let app_name = crate::get_app_name().to_lowercase();
    // systemctl kill rustdesk --tray, execute cp first
    if !run_cmds_privileged(&format!(
        "{cp} systemctl disable {app_name}; systemctl stop {app_name};"
    )) {
        Config::set_option("stop-service".into(), "".into());
        return true;
    }
    // systemctl stop will kill child processes, below may not be executed.
    if show_new_window {
        run_me_with(2);
    }
    std::process::exit(0);
}

pub fn install_service() -> bool {
    let _installing = crate::platform::InstallingService::new();
    if !has_cmd("systemctl") {
        return false;
    }
    log::info!("Installing service...");
    let cp = switch_service(false);
    let app_name = crate::get_app_name().to_lowercase();
    if !run_cmds_privileged(&format!(
        "{cp} systemctl enable {app_name}; systemctl start {app_name};"
    )) {
        Config::set_option("stop-service".into(), "Y".into());
    }
    true
}

fn check_if_stop_service() {
    if Config::get_option("stop-service".into()) == "Y" {
        let app_name = crate::get_app_name().to_lowercase();
        allow_err!(run_cmds(&format!(
            "systemctl disable {app_name}; systemctl stop {app_name}"
        )));
    }
}

pub fn check_autostart_config() -> ResultType<()> {
    // SECURITY: Use trusted home directory lookup via getpwuid instead of $HOME env var
    // to prevent confused-deputy attacks where an attacker manipulates environment variables.
    let home = match get_home_dir_trusted() {
        Some(p) => p.to_string_lossy().to_string(),
        None => {
            log::warn!("Failed to get trusted home directory for autostart config check");
            return Ok(());
        }
    };
    let app_name = crate::get_app_name().to_lowercase();
    let path = format!("{home}/.config/autostart");
    let file = format!("{path}/{app_name}.desktop");
    // https://github.com/rustdesk/rustdesk/issues/4863
    std::fs::remove_file(&file).ok();
    /*
        std::fs::create_dir_all(&path).ok();
        if !Path::new(&file).exists() {
            // write text to the desktop file
            let mut file = std::fs::File::create(&file)?;
            file.write_all(
                format!(
                    "
    [Desktop Entry]
    Type=Application
    Exec={app_name} --tray
    NoDisplay=false
            "
                )
                .as_bytes(),
            )?;
        }
        */
    Ok(())
}

pub struct WallPaperRemover {
    old_path: String,
    old_path_dark: Option<String>, // ubuntu 22.04 light/dark theme have different uri
}

impl WallPaperRemover {
    pub fn new() -> ResultType<Self> {
        let start = std::time::Instant::now();
        let old_path = wallpaper::get().map_err(|e| anyhow!(e.to_string()))?;
        let old_path_dark = wallpaper::get_dark().ok();
        if old_path.is_empty() && old_path_dark.clone().unwrap_or_default().is_empty() {
            bail!("already solid color");
        }
        wallpaper::set_from_path("").map_err(|e| anyhow!(e.to_string()))?;
        wallpaper::set_dark_from_path("").ok();
        log::info!(
            "created wallpaper remover,  old_path: {:?}, old_path_dark: {:?}, elapsed: {:?}",
            old_path,
            old_path_dark,
            start.elapsed(),
        );
        Ok(Self {
            old_path,
            old_path_dark,
        })
    }

    pub fn support() -> bool {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        if wallpaper::gnome::is_compliant(&desktop) || desktop.as_str() == "XFCE" {
            return wallpaper::get().is_ok();
        }
        false
    }
}

impl Drop for WallPaperRemover {
    fn drop(&mut self) {
        allow_err!(wallpaper::set_from_path(&self.old_path).map_err(|e| anyhow!(e.to_string())));
        if let Some(old_path_dark) = &self.old_path_dark {
            allow_err!(wallpaper::set_dark_from_path(old_path_dark.as_str())
                .map_err(|e| anyhow!(e.to_string())));
        }
    }
}

#[inline]
pub fn is_x11() -> bool {
    *IS_X11
}

#[inline]
pub fn is_selinux_enforcing() -> bool {
    match run_cmds("getenforce") {
        Ok(output) => output.trim() == "Enforcing",
        Err(_) => match run_cmds("sestatus") {
            Ok(output) => {
                for line in output.lines() {
                    if line.contains("Current mode:") {
                        return line.contains("enforcing");
                    }
                }
                false
            }
            Err(_) => false,
        },
    }
}
