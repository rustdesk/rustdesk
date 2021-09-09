use super::{CursorData, ResultType};
use hbb_common::{allow_err, bail, log};
use libc::{c_char, c_int, c_void};
use std::io::prelude::*;
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
type Xdo = *const c_void;

pub const PA_SAMPLE_RATE: u32 = 24000;
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
                        cd.colors.resize(pixels.len() * 4, 0);
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
                                cd.colors[pos] = r as _;
                                cd.colors[pos + 1] = g as _;
                                cd.colors[pos + 2] = b as _;
                                cd.colors[pos + 3] = a as _;
                            }
                        }
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

pub fn start_os_service() {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let mut uid = "".to_owned();
    let mut server: Option<std::process::Child> = None;
    if let Err(err) = ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }) {
        println!("Failed to set Ctrl-C handler: {}", err);
    }

    let mut cm0 = false;
    let mut last_restart = std::time::Instant::now();
    while running.load(Ordering::SeqCst) {
        let cm = get_cm();
        let tmp = get_active_userid();
        let mut start_new = false;
        if tmp != uid && !tmp.is_empty() {
            uid = tmp;
            log::info!("uid of seat0: {}", uid);
            let gdm = format!("/run/user/{}/gdm/Xauthority", uid);
            let mut auth = get_env_tries("XAUTHORITY", &uid, 10);
            if auth.is_empty() {
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
            let mut d = get_env("DISPLAY", &uid);
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
            if let Some(ps) = server.as_mut() {
                allow_err!(ps.kill());
                std::thread::sleep(std::time::Duration::from_millis(30));
                last_restart = std::time::Instant::now();
            }
        } else if !cm
            && ((cm0 && last_restart.elapsed().as_secs() > 60)
                || last_restart.elapsed().as_secs() > 3600)
        {
            // restart server if new connections all closed, or every one hour,
            // as a workaround to resolve "SpotUdp" (dns resolve)
            // and x server get displays failure issue
            if let Some(ps) = server.as_mut() {
                allow_err!(ps.kill());
                std::thread::sleep(std::time::Duration::from_millis(30));
                last_restart = std::time::Instant::now();
                log::info!("restart server");
            }
        }
        if let Some(ps) = server.as_mut() {
            match ps.try_wait() {
                Ok(Some(_)) => {
                    server = None;
                    start_new = true;
                }
                _ => {}
            }
        } else {
            start_new = true;
        }
        if start_new {
            match crate::run_me(vec!["--server"]) {
                Ok(ps) => server = Some(ps),
                Err(err) => {
                    log::error!("Failed to start server: {}", err);
                }
            }
        }
        cm0 = cm;
        std::thread::sleep(std::time::Duration::from_millis(super::SERVICE_INTERVAL));
    }

    if let Some(ps) = server.take().as_mut() {
        allow_err!(ps.kill());
    }
    println!("Exit");
}

fn get_active_userid() -> String {
    get_value_of_seat0(1)
}

fn is_active(sid: &str) -> bool {
    if let Ok(output) = std::process::Command::new("loginctl")
        .args(vec!["show-session", "-p", "State", sid])
        .output()
    {
        String::from_utf8_lossy(&output.stdout).contains("active")
    } else {
        false
    }
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

fn get_value_of_seat0(i: usize) -> String {
    if let Ok(output) = std::process::Command::new("loginctl").output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.contains("seat0") {
                if let Some(sid) = line.split_whitespace().nth(0) {
                    if is_active(sid) {
                        if let Some(uid) = line.split_whitespace().nth(i) {
                            return uid.to_owned();
                        }
                    }
                }
            }
        }
    }

    // some case, there is no seat0 https://github.com/rustdesk/rustdesk/issues/73
    if let Ok(output) = std::process::Command::new("loginctl").output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(sid) = line.split_whitespace().nth(0) {
                let d = get_display_server_of_session(sid);
                if is_active(sid) && d != "tty" {
                    if let Some(uid) = line.split_whitespace().nth(i) {
                        return uid.to_owned();
                    }
                }
            }
        }
    }

    return "".to_owned();
}

pub fn get_display_server() -> String {
    let session = get_value_of_seat0(0);
    get_display_server_of_session(&session)
}

fn get_display_server_of_session(session: &str) -> String {
    if let Ok(output) = std::process::Command::new("loginctl")
        .args(vec!["show-session", "-p", "Type", session])
        .output()
    // Check session type of the session
    {
        let display_server = String::from_utf8_lossy(&output.stdout)
            .replace("Type=", "")
            .trim_end()
            .into();
        if display_server == "tty" {
            // If the type is tty...
            if let Ok(output) = std::process::Command::new("loginctl")
                .args(vec!["show-session", "-p", "TTY", session])
                .output()
            // Get the tty number
            {
                let tty: String = String::from_utf8_lossy(&output.stdout)
                    .replace("TTY=", "")
                    .trim_end()
                    .into();
                if let Ok(Some(xorg_results)) =
                    run_cmds(format!("ps -e | grep \"{}.\\\\+Xorg\"", tty))
                // And check if Xorg is running on that tty
                {
                    if xorg_results.trim_end().to_string() != "" {
                        // If it is, manually return "x11", otherwise return tty
                        "x11".to_owned()
                    } else {
                        display_server
                    }
                } else {
                    // If any of these commands fail just fall back to the display server
                    display_server
                }
            } else {
                display_server
            }
        } else {
            // If the session is not a tty, then just return the type as usual
            display_server
        }
    } else {
        "".to_owned()
    }
}

pub fn is_login_wayland() -> bool {
    if let Ok(contents) = std::fs::read_to_string("/etc/gdm3/custom.conf") {
        contents.contains("#WaylandEnable=false")
    } else if let Ok(contents) = std::fs::read_to_string("/etc/gdm/custom.conf") {
        contents.contains("#WaylandEnable=false")
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
            &file
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
    return "wayland" == dtype && unsafe{UNMODIFIED};
}

pub fn modify_default_login() -> String {
    let dsession = std::env::var("DESKTOP_SESSION").unwrap();
    let user_name = std::env::var("USERNAME").unwrap();
    if let Ok(Some(x)) = run_cmds("ls /usr/share/* | grep ${DESKTOP_SESSION}-xorg.desktop".to_owned()) {
        if x.trim_end().to_string() != "" {
            match std::process::Command::new("pkexec")
                .args(vec![
                    "sed",
                    "-i",
                    &format!("s/={0}$/={0}-xorg/g", &dsession),
                    &format!("/var/lib/AccountsService/users/{}", &user_name)
                ])
                .output()
            {
                Ok(x) => {
                    let x = String::from_utf8_lossy(&x.stderr);
                    if !x.is_empty() {
                        log::error!("modify_default_login failed: {}", x);
                        return "Fix failed! Please re-login with X server manually".to_owned();
                    } else {
                        unsafe {UNMODIFIED = false;}
                        return "".to_owned();
                    }
                }
                Err(err) => {
                    log::error!("modify_default_login failed: {}", err);
                    return "Fix failed! Please re-login with X server manually".to_owned();
                }
            }
        } else if let Ok(Some(z)) = run_cmds("ls /usr/share/* | grep ${DESKTOP_SESSION:0:-8}.desktop".to_owned()) {
            if z.trim_end().to_string() != "" {
                match std::process::Command::new("pkexec")
                    .args(vec![
                        "sed",
                        "-i",
                        &format!("s/={}$/={}/g", &dsession, &dsession[..dsession.len()-8]),
                        &format!("/var/lib/AccountsService/users/{}", &user_name)
                    ])
                    .output()
                {
                    Ok(x) => {
                        let x = String::from_utf8_lossy(&x.stderr);
                        if !x.is_empty() {
                            log::error!("modify_default_login failed: {}", x);
                            return "Fix failed! Please re-login with X server manually".to_owned();
                        } else {
                            unsafe {UNMODIFIED = false;}
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
    get_value_of_seat0(2)
}

pub fn is_prelogin() -> bool {
    let n = get_active_userid().len();
    n < 4 && n > 1
}

pub fn is_root() -> bool {
    crate::username() == "root"
}

pub fn run_as_user(arg: &str) -> ResultType<Option<std::process::Child>> {
    let uid = get_active_userid();
    let cmd = std::env::current_exe()?;
    let task = std::process::Command::new("sudo")
        .args(vec![
            &format!("XDG_RUNTIME_DIR=/run/user/{}", uid) as &str,
            "-u",
            &get_active_username(),
            cmd.to_str().unwrap_or(""),
            arg,
        ])
        .spawn()?;
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
    std::thread::spawn(move || {
        use crate::server::input_service::handle_key;
        use hbb_common::message_proto::*;
        let mut evt = KeyEvent {
            down: true,
            modifiers: vec![ControlKey::Meta.into()],
            ..Default::default()
        };
        evt.set_chr('l' as _);
        handle_key(&evt);
        evt.down = false;
        handle_key(&evt);
    });
}

pub fn toggle_privacy_mode(_v: bool) {
    // https://unix.stackexchange.com/questions/17170/disable-keyboard-mouse-input-on-unix-under-x
}

pub fn block_input(_v: bool) {
    //
}

pub fn is_installed() -> bool {
    true
}

fn run_cmds(cmds: String) -> ResultType<Option<String>> {
    let mut tmp = std::env::temp_dir();
    tmp.push(format!(
        "{}_{}",
        hbb_common::config::APP_NAME,
        crate::get_time()
    ));
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(cmds.as_bytes())?;
    file.sync_all()?;
    if let Ok(output) = std::process::Command::new("bash")
        .arg(tmp.to_str().unwrap_or(""))
        .output()
    {
        Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
    } else {
        Ok(None)
    }
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
    let cmd = format!("ps -u {} -o pid= | xargs -I__ cat /proc/__/environ 2>/dev/null | tr '\\0' '\\n' | grep -m1 '^{}=' | sed 's/{}=//g'", uid, name, name);
    log::debug!("Run: {}", &cmd);
    if let Ok(Some(x)) = run_cmds(cmd) {
        x.trim_end().to_string()
    } else {
        "".to_owned()
    }
}
