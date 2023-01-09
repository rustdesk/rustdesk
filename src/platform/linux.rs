use super::{CursorData, ResultType};
pub use hbb_common::platform::linux::*;
use hbb_common::{allow_err, bail, log};
use libc::{c_char, c_int, c_void};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

type Xdo = *const c_void;

pub const PA_SAMPLE_RATE: u32 = 48000;
static mut UNMODIFIED: bool = true;

thread_local! {
    static XDO: RefCell<Xdo> = RefCell::new(unsafe { xdo_new(std::ptr::null()) });
    static DISPLAY: RefCell<*mut c_void> = RefCell::new(unsafe { XOpenDisplay(std::ptr::null())});
}

extern "C" {
    fn xdo_get_mouse_location(
        xdo: Xdo,
        x: *mut c_int,
        y: *mut c_int,
        screen_num: *mut c_int,
    ) -> c_int;
    fn xdo_new(display: *const c_char) -> Xdo;
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
    pub cursor_serial: libc::c_long,
    pub pixels: *const libc::c_long,
}

pub fn get_cursor_pos() -> Option<(i32, i32)> {
    let mut res = None;
    XDO.with(|xdo| {
        if let Ok(xdo) = xdo.try_borrow_mut() {
            if xdo.is_null() {
                return;
            }
            let mut x: c_int = 0;
            let mut y: c_int = 0;
            unsafe {
                xdo_get_mouse_location(*xdo, &mut x as _, &mut y as _, std::ptr::null_mut());
            }
            res = Some((x, y));
        }
    });
    res
}

pub fn reset_input_cache() {}

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

fn stop_server(server: &mut Option<std::process::Child>) {
    if let Some(mut ps) = server.take() {
        allow_err!(ps.kill());
        std::thread::sleep(std::time::Duration::from_millis(30));
        match ps.try_wait() {
            Ok(Some(_status)) => {}
            Ok(None) => {
                let _res = ps.wait();
            }
            Err(e) => log::error!("error attempting to wait: {e}"),
        }
    }
}

fn set_x11_env(uid: &str) {
    log::info!("uid of seat0: {}", uid);
    let gdm = format!("/run/user/{}/gdm/Xauthority", uid);
    let mut auth = get_env_tries("XAUTHORITY", uid, 10);
    // auth is another user's when uid = 0, https://github.com/rustdesk/rustdesk/issues/2468
    if auth.is_empty() || uid == "0" {
        auth = if std::path::Path::new(&gdm).exists() {
            gdm
        } else {
            let username = get_active_username();
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
        };
    }
    let mut d = get_env("DISPLAY", uid);
    if d.is_empty() {
        d = get_display();
    }
    if d.is_empty() {
        d = ":0".to_owned();
    }
    d = d.replace(&whoami::hostname(), "").replace("localhost", "");
    log::info!("DISPLAY: {}", d);
    log::info!("XAUTHORITY: {}", auth);
    std::env::set_var("XAUTHORITY", auth);
    std::env::set_var("DISPLAY", d);
}

fn stop_rustdesk_servers() {
    let _ = run_cmds(format!(
        r##"ps -ef | grep -E 'rustdesk +--server' | awk '{{printf("kill -9 %d\n", $2)}}' | bash"##,
    ));
}

fn should_start_server(
    try_x11: bool,
    uid: &mut String,
    cur_uid: String,
    cm0: &mut bool,
    last_restart: &mut std::time::Instant,
    server: &mut Option<std::process::Child>,
) -> bool {
    let cm = get_cm();
    let mut start_new = false;
    if cur_uid != *uid && !cur_uid.is_empty() {
        *uid = cur_uid;
        if try_x11 {
            set_x11_env(&uid);
        }
        if let Some(ps) = server.as_mut() {
            allow_err!(ps.kill());
            std::thread::sleep(std::time::Duration::from_millis(30));
            *last_restart = std::time::Instant::now();
        }
    } else if !cm
        && ((*cm0 && last_restart.elapsed().as_secs() > 60)
            || last_restart.elapsed().as_secs() > 3600)
    {
        // restart server if new connections all closed, or every one hour,
        // as a workaround to resolve "SpotUdp" (dns resolve)
        // and x server get displays failure issue
        if let Some(ps) = server.as_mut() {
            allow_err!(ps.kill());
            std::thread::sleep(std::time::Duration::from_millis(30));
            *last_restart = std::time::Instant::now();
            log::info!("restart server");
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

pub fn start_os_service() {
    stop_rustdesk_servers();
    start_uinput_service();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let mut uid = "".to_owned();
    let mut server: Option<std::process::Child> = None;
    let mut user_server: Option<std::process::Child> = None;
    if let Err(err) = ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }) {
        println!("Failed to set Ctrl-C handler: {}", err);
    }

    let mut cm0 = false;
    let mut last_restart = std::time::Instant::now();
    while running.load(Ordering::SeqCst) {
        let (cur_uid, cur_user) = get_active_user_id_name();
        let is_wayland = current_is_wayland();

        if cur_user == "root" || !is_wayland {
            stop_server(&mut user_server);
            // try start subprocess "--server"
            if should_start_server(
                true,
                &mut uid,
                cur_uid,
                &mut cm0,
                &mut last_restart,
                &mut server,
            ) {
                // to-do: stop_server(&mut user_server); may not stop child correctly
                // stop_rustdesk_servers() is just a temp solution here.
                stop_rustdesk_servers();
                std::thread::sleep(std::time::Duration::from_millis(super::SERVICE_INTERVAL));
                match crate::run_me(vec!["--server"]) {
                    Ok(ps) => server = Some(ps),
                    Err(err) => {
                        log::error!("Failed to start server: {}", err);
                    }
                }
            }
        } else if cur_user != "" {
            if cur_user != "gdm" {
                // try kill subprocess "--server"
                stop_server(&mut server);

                // try start subprocess "--server"
                if should_start_server(
                    false,
                    &mut uid,
                    cur_uid.clone(),
                    &mut cm0,
                    &mut last_restart,
                    &mut user_server,
                ) {
                    stop_rustdesk_servers();
                    std::thread::sleep(std::time::Duration::from_millis(super::SERVICE_INTERVAL));
                    match run_as_user(vec!["--server"], Some((cur_uid, cur_user))) {
                        Ok(ps) => user_server = ps,
                        Err(err) => {
                            log::error!("Failed to start server: {}", err);
                        }
                    }
                }
            }
        } else {
            stop_rustdesk_servers();
            std::thread::sleep(std::time::Duration::from_millis(super::SERVICE_INTERVAL));
            stop_server(&mut user_server);
            stop_server(&mut server);
        }
        std::thread::sleep(std::time::Duration::from_millis(super::SERVICE_INTERVAL));
    }

    if let Some(ps) = user_server.take().as_mut() {
        allow_err!(ps.kill());
    }
    if let Some(ps) = server.take().as_mut() {
        allow_err!(ps.kill());
    }
    log::info!("Exit");
}

pub fn get_active_user_id_name() -> (String, String) {
    let vec_id_name = get_values_of_seat0([1, 2].to_vec());
    (vec_id_name[0].clone(), vec_id_name[1].clone())
}

pub fn get_active_userid() -> String {
    get_values_of_seat0([1].to_vec())[0].clone()
}

fn get_cm() -> bool {
    if let Ok(output) = std::process::Command::new("ps").args(vec!["aux"]).output() {
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

fn get_display() -> String {
    let user = get_active_username();
    log::debug!("w {}", &user);
    if let Ok(output) = std::process::Command::new("w").arg(&user).output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            log::debug!("  {}", line);
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
    log::debug!("ls -l /tmp/.X11-unix/");
    let mut last = "".to_owned();
    if let Ok(output) = std::process::Command::new("ls")
        .args(vec!["-l", "/tmp/.X11-unix/"])
        .output()
    {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            log::debug!("  {}", line);
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

pub fn is_login_wayland() -> bool {
    if let Ok(contents) = std::fs::read_to_string("/etc/gdm3/custom.conf") {
        contents.contains("#WaylandEnable=false") || contents.contains("WaylandEnable=true")
    } else if let Ok(contents) = std::fs::read_to_string("/etc/gdm/custom.conf") {
        contents.contains("#WaylandEnable=false") || contents.contains("WaylandEnable=true")
    } else {
        false
    }
}

pub fn fix_login_wayland() {
    let mut file = "/etc/gdm3/custom.conf".to_owned();
    if !std::path::Path::new(&file).exists() {
        file = "/etc/gdm/custom.conf".to_owned();
    }
    match std::process::Command::new("pkexec")
        .args(vec![
            "sed",
            "-i",
            "s/#WaylandEnable=false/WaylandEnable=false/g",
            &file,
        ])
        .output()
    {
        Ok(x) => {
            let x = String::from_utf8_lossy(&x.stderr);
            if !x.is_empty() {
                log::error!("fix_login_wayland failed: {}", x);
            }
        }
        Err(err) => {
            log::error!("fix_login_wayland failed: {}", err);
        }
    }
}

pub fn current_is_wayland() -> bool {
    let dtype = get_display_server();
    return "wayland" == dtype && unsafe { UNMODIFIED };
}

pub fn modify_default_login() -> String {
    let dsession = std::env::var("DESKTOP_SESSION").unwrap();
    let user_name = std::env::var("USERNAME").unwrap();
    if let Ok(x) = run_cmds("ls /usr/share/* | grep ${DESKTOP_SESSION}-xorg.desktop".to_owned()) {
        if x.trim_end().to_string() != "" {
            match std::process::Command::new("pkexec")
                .args(vec![
                    "sed",
                    "-i",
                    &format!("s/={0}$/={0}-xorg/g", &dsession),
                    &format!("/var/lib/AccountsService/users/{}", &user_name),
                ])
                .output()
            {
                Ok(x) => {
                    let x = String::from_utf8_lossy(&x.stderr);
                    if !x.is_empty() {
                        log::error!("modify_default_login failed: {}", x);
                        return "Fix failed! Please re-login with X server manually".to_owned();
                    } else {
                        unsafe {
                            UNMODIFIED = false;
                        }
                        return "".to_owned();
                    }
                }
                Err(err) => {
                    log::error!("modify_default_login failed: {}", err);
                    return "Fix failed! Please re-login with X server manually".to_owned();
                }
            }
        } else if let Ok(z) =
            run_cmds("ls /usr/share/* | grep ${DESKTOP_SESSION:0:-8}.desktop".to_owned())
        {
            if z.trim_end().to_string() != "" {
                match std::process::Command::new("pkexec")
                    .args(vec![
                        "sed",
                        "-i",
                        &format!("s/={}$/={}/g", &dsession, &dsession[..dsession.len() - 8]),
                        &format!("/var/lib/AccountsService/users/{}", &user_name),
                    ])
                    .output()
                {
                    Ok(x) => {
                        let x = String::from_utf8_lossy(&x.stderr);
                        if !x.is_empty() {
                            log::error!("modify_default_login failed: {}", x);
                            return "Fix failed! Please re-login with X server manually".to_owned();
                        } else {
                            unsafe {
                                UNMODIFIED = false;
                            }
                            return "".to_owned();
                        }
                    }
                    Err(err) => {
                        log::error!("modify_default_login failed: {}", err);
                        return "Fix failed! Please re-login with X server manually".to_owned();
                    }
                }
            }
        }
    }
    return "Fix failed! Please re-login with X server manually".to_owned();
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

pub fn get_active_username() -> String {
    get_values_of_seat0([2].to_vec())[0].clone()
}

pub fn get_active_user_home() -> Option<PathBuf> {
    let username = get_active_username();
    if !username.is_empty() {
        let home = PathBuf::from(format!("/home/{}", username));
        if home.exists() {
            return Some(home);
        }
    }
    None
}

pub fn is_prelogin() -> bool {
    let n = get_active_userid().len();
    n < 4 && n > 1
}

pub fn is_root() -> bool {
    crate::username() == "root"
}

fn is_opensuse() -> bool {
    if let Ok(res) = run_cmds("cat /etc/os-release | grep opensuse".to_owned()) {
        if !res.is_empty() {
            return true;
        }
    }
    false
}

pub fn run_as_user(
    arg: Vec<&str>,
    user: Option<(String, String)>,
) -> ResultType<Option<std::process::Child>> {
    let (uid, username) = match user {
        Some(id_name) => id_name,
        None => get_active_user_id_name(),
    };
    let cmd = std::env::current_exe()?;
    let xdg = &format!("XDG_RUNTIME_DIR=/run/user/{}", uid) as &str;
    let mut args = vec![xdg, "-u", &username, cmd.to_str().unwrap_or("")];
    args.append(&mut arg.clone());
    // -E required for opensuse
    if is_opensuse() {
        args.insert(0, "-E");
    }

    let task = std::process::Command::new("sudo").args(args).spawn()?;
    Ok(Some(task))
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

pub fn lock_screen() {
    std::process::Command::new("xdg-screensaver")
        .arg("lock")
        .spawn()
        .ok();
}

pub fn toggle_blank_screen(_v: bool) {
    // https://unix.stackexchange.com/questions/17170/disable-keyboard-mouse-input-on-unix-under-x
}

pub fn block_input(_v: bool) -> bool {
    true
}

pub fn is_installed() -> bool {
    true
}

fn get_env_tries(name: &str, uid: &str, n: usize) -> String {
    for _ in 0..n {
        let x = get_env(name, uid);
        if !x.is_empty() {
            return x;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    "".to_owned()
}

fn get_env(name: &str, uid: &str) -> String {
    let cmd = format!("ps -u {} -o pid= | xargs -I__ cat /proc/__/environ 2>/dev/null | tr '\\0' '\\n' | grep '^{}=' | tail -1 | sed 's/{}=//g'", uid, name, name);
    log::debug!("Run: {}", &cmd);
    if let Ok(x) = run_cmds(cmd) {
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

pub fn check_super_user_permission() -> ResultType<bool> {
    let file = "/usr/share/rustdesk/files/polkit";
    let arg;
    if std::path::Path::new(file).is_file() {
        arg = file;
    } else {
        arg = "echo";
    }
    let status = std::process::Command::new("pkexec").arg(arg).status()?;
    Ok(status.success() && status.code() == Some(0))
}

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
        let property = std::ffi::CString::new("gtk-double-click-time").unwrap();
        let setings = gtk_settings_get_default();
        g_object_get(
            setings,
            property.as_ptr(),
            &mut double_click_time as *mut u32,
            0 as *const libc::c_void,
        );
        double_click_time
    }
}

/// forever: may not work
pub fn system_message(title: &str, msg: &str, forever: bool) -> ResultType<()> {
    let cmds: HashMap<&str, Vec<&str>> = HashMap::from([
        ("notify-send", [title, msg].to_vec()),
        (
            "zenity",
            [
                "--info",
                "--timeout",
                if forever { "0" } else { "3" },
                "--title",
                title,
                "--text",
                msg,
            ]
            .to_vec(),
        ),
        ("kdialog", ["--title", title, "--msgbox", msg].to_vec()),
        (
            "xmessage",
            [
                "-center",
                "-timeout",
                if forever { "0" } else { "3" },
                title,
                msg,
            ]
            .to_vec(),
        ),
    ]);
    for (k, v) in cmds {
        if std::process::Command::new(k).args(v).spawn().is_ok() {
            return Ok(());
        }
    }
    bail!("failed to post system message");
}

extern "C" fn breakdown_signal_handler(sig: i32) {
    let mut stack = vec![];
    backtrace::trace(|frame| {
        backtrace::resolve_frame(frame, |symbol| {
            if let Some(name) = symbol.name() {
                stack.push(name.to_string());
            }
        });
        true // keep going to the next frame
    });
    let mut info = String::default();
    if stack.iter().any(|s| {
        s.contains(&"nouveau_pushbuf_kick")
            || s.to_lowercase().contains("nvidia")
            || s.contains("gdk_window_end_draw_frame")
    }) {
        hbb_common::config::Config::set_option(
            "allow-always-software-render".to_string(),
            "Y".to_string(),
        );
        info = "Always use software rendering will be set.".to_string();
        log::info!("{}", info);
    }
    log::error!(
        "Got signal {} and exit. stack:\n{}",
        sig,
        stack.join("\n").to_string()
    );
    system_message(
        "RustDesk",
        &format!("Got signal {} and exit.{}", sig, info),
        true,
    )
    .ok();
    std::process::exit(0);
}

pub fn register_breakdown_handler() {
    unsafe {
        libc::signal(libc::SIGSEGV, breakdown_signal_handler as _);
    }
}
