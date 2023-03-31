use crate::ResultType;
use std::{collections::HashMap, process::Command};

lazy_static::lazy_static! {
    pub static ref DISTRO: Distro = Distro::new();
}

pub const DISPLAY_SERVER_WAYLAND: &str = "wayland";
pub const DISPLAY_SERVER_X11: &str = "x11";

pub struct Distro {
    pub name: String,
    pub version_id: String,
}

impl Distro {
    fn new() -> Self {
        let name = run_cmds("awk -F'=' '/^NAME=/ {print $2}' /etc/os-release")
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .to_string();
        let version_id = run_cmds("awk -F'=' '/^VERSION_ID=/ {print $2}' /etc/os-release")
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .to_string();
        Self { name, version_id }
    }
}

#[inline]
pub fn is_gdm_user(username: &str) -> bool {
    username == "gdm"
    // || username == "lightgdm"
}

#[inline]
pub fn is_desktop_wayland() -> bool {
    get_display_server() == DISPLAY_SERVER_WAYLAND
}

#[inline]
pub fn is_x11_or_headless() -> bool {
    !is_desktop_wayland()
}

// -1
const INVALID_SESSION: &str = "4294967295";

pub fn get_display_server() -> String {
    let mut session = get_values_of_seat0(&[0])[0].clone();
    if session.is_empty() {
        // loginctl has not given the expected output.  try something else.
        if let Ok(sid) = std::env::var("XDG_SESSION_ID") {
            // could also execute "cat /proc/self/sessionid"
            session = sid;
        }
        if session.is_empty() {
            session = run_cmds("cat /proc/self/sessionid").unwrap_or_default();
            if session == INVALID_SESSION {
                session = "".to_owned();
            }
        }
    }
    if session.is_empty() {
        "".to_owned()
    } else {
        get_display_server_of_session(&session)
    }
}

pub fn get_display_server_of_session(session: &str) -> String {
    let mut display_server = if let Ok(output) =
        run_loginctl(Some(vec!["show-session", "-p", "Type", session]))
    // Check session type of the session
    {
        let display_server = String::from_utf8_lossy(&output.stdout)
            .replace("Type=", "")
            .trim_end()
            .into();
        if display_server == "tty" {
            // If the type is tty...
            if let Ok(output) = run_loginctl(Some(vec!["show-session", "-p", "TTY", session]))
            // Get the tty number
            {
                let tty: String = String::from_utf8_lossy(&output.stdout)
                    .replace("TTY=", "")
                    .trim_end()
                    .into();
                if let Ok(xorg_results) = run_cmds(&format!("ps -e | grep \"{tty}.\\\\+Xorg\""))
                // And check if Xorg is running on that tty
                {
                    if xorg_results.trim_end() != "" {
                        // If it is, manually return "x11", otherwise return tty
                        return "x11".to_owned();
                    }
                }
            }
        }
        display_server
    } else {
        "".to_owned()
    };
    if display_server.is_empty() || display_server == "tty" {
        // loginctl has not given the expected output.  try something else.
        if let Ok(sestype) = std::env::var("XDG_SESSION_TYPE") {
            display_server = sestype;
        }
    }
    if display_server == "" {
        display_server = "x11".to_owned();
    }
    display_server.to_lowercase()
}

#[inline]
fn line_values(indices: &[usize], line: &str) -> Vec<String> {
    indices
        .into_iter()
        .map(|idx| line.split_whitespace().nth(*idx).unwrap_or("").to_owned())
        .collect::<Vec<String>>()
}

#[inline]
pub fn get_values_of_seat0(indices: &[usize]) -> Vec<String> {
    _get_values_of_seat0(indices, true)
}

#[inline]
pub fn get_values_of_seat0_with_gdm_wayland(indices: &[usize]) -> Vec<String> {
    _get_values_of_seat0(indices, false)
}

fn _get_values_of_seat0(indices: &[usize], ignore_gdm_wayland: bool) -> Vec<String> {
    if let Ok(output) = run_loginctl(None) {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.contains("seat0") {
                if let Some(sid) = line.split_whitespace().next() {
                    if is_active(sid) {
                        if ignore_gdm_wayland {
                            if is_gdm_user(line.split_whitespace().nth(2).unwrap_or(""))
                                && get_display_server_of_session(sid) == DISPLAY_SERVER_WAYLAND
                            {
                                continue;
                            }
                        }
                        return line_values(indices, line);
                    }
                }
            }
        }

        // some case, there is no seat0 https://github.com/rustdesk/rustdesk/issues/73
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(sid) = line.split_whitespace().next() {
                if is_active(sid) {
                    let d = get_display_server_of_session(sid);
                    if ignore_gdm_wayland {
                        if is_gdm_user(line.split_whitespace().nth(2).unwrap_or(""))
                            && d == DISPLAY_SERVER_WAYLAND
                        {
                            continue;
                        }
                    }
                    if d == "tty" {
                        continue;
                    }
                    return line_values(indices, line);
                }
            }
        }
    }

    line_values(indices, "")
}

pub fn is_active(sid: &str) -> bool {
    if let Ok(output) = run_loginctl(Some(vec!["show-session", "-p", "State", sid])) {
        String::from_utf8_lossy(&output.stdout).contains("active")
    } else {
        false
    }
}

pub fn run_cmds(cmds: &str) -> ResultType<String> {
    let output = std::process::Command::new("sh")
        .args(vec!["-c", cmds])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(not(feature = "flatpak"))]
fn run_loginctl(args: Option<Vec<&str>>) -> std::io::Result<std::process::Output> {
    let mut cmd = std::process::Command::new("loginctl");
    if let Some(a) = args {
        return cmd.args(a).output();
    }
    cmd.output()
}

#[cfg(feature = "flatpak")]
fn run_loginctl(args: Option<Vec<&str>>) -> std::io::Result<std::process::Output> {
    let mut l_args = String::from("loginctl");
    if let Some(a) = args {
        l_args = format!("{} {}", l_args, a.join(" "));
    }
    std::process::Command::new("flatpak-spawn")
        .args(vec![String::from("--host"), l_args])
        .output()
}

/// forever: may not work
#[cfg(target_os = "linux")]
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
        if Command::new(k).args(v).spawn().is_ok() {
            return Ok(());
        }
    }
    crate::bail!("failed to post system message");
}
