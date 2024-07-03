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
    // Check for forced display server environment variable first
    if let Ok(forced_display) = std::env::var("RUSTDESK_FORCED_DISPLAY_SERVER") {
        return forced_display;
    }

    // Check if `loginctl` can be called successfully
    if run_loginctl(None).is_err() {
        return DISPLAY_SERVER_X11.to_owned();
    }

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
        std::env::var("XDG_SESSION_TYPE").unwrap_or("x11".to_owned())
    } else {
        get_display_server_of_session(&session)
    }
}

pub fn get_display_server_of_session(session: &str) -> String {
    let mut display_server = if let Ok(output) =
        run_loginctl(Some(vec!["show-session", "-p", "Type", session]))
    // Check session type of the session
    {
        String::from_utf8_lossy(&output.stdout)
            .replace("Type=", "")
            .trim_end()
            .into()
    } else {
        "".to_owned()
    };
    if display_server.is_empty() || display_server == "tty" {
        if let Ok(sestype) = std::env::var("XDG_SESSION_TYPE") {
            if !sestype.is_empty() {
                return sestype.to_lowercase();
            }
        }
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

// Ignore "3 sessions listed."
fn ignore_loginctl_line(line: &str) -> bool {
    line.contains("sessions") || line.split(" ").count() < 4
}

fn _get_values_of_seat0(indices: &[usize], ignore_gdm_wayland: bool) -> Vec<String> {
    if let Ok(output) = run_loginctl(None) {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if ignore_loginctl_line(line) {
                continue;
            }
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
            if ignore_loginctl_line(line) {
                continue;
            }
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

pub fn is_active_and_seat0(sid: &str) -> bool {
    if let Ok(output) = run_loginctl(Some(vec!["show-session", sid])) {
        String::from_utf8_lossy(&output.stdout).contains("State=active")
            && String::from_utf8_lossy(&output.stdout).contains("Seat=seat0")
    } else {
        false
    }
}

// **Note** that the return value here, the last character is '\n'.
// Use `run_cmds_trim_newline()` if you want to remove '\n' at the end.
pub fn run_cmds(cmds: &str) -> ResultType<String> {
    let output = std::process::Command::new("sh")
        .args(vec!["-c", cmds])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn run_cmds_trim_newline(cmds: &str) -> ResultType<String> {
    let output = std::process::Command::new("sh")
        .args(vec!["-c", cmds])
        .output()?;
    let out = String::from_utf8_lossy(&output.stdout);
    Ok(if out.ends_with('\n') {
        out[..out.len() - 1].to_string()
    } else {
        out.to_string()
    })
}

fn run_loginctl(args: Option<Vec<&str>>) -> std::io::Result<std::process::Output> {
    if std::env::var("FLATPAK_ID").is_ok() {
        let mut l_args = String::from("loginctl");
        if let Some(a) = args.as_ref() {
            l_args = format!("{} {}", l_args, a.join(" "));
        }
        let res = std::process::Command::new("flatpak-spawn")
            .args(vec![String::from("--host"), l_args])
            .output();
        if res.is_ok() {
            return res;
        }
    }
    let mut cmd = std::process::Command::new("loginctl");
    if let Some(a) = args {
        return cmd.args(a).output();
    }
    cmd.output()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_cmds_trim_newline() {
        assert_eq!(run_cmds_trim_newline("echo -n 123").unwrap(), "123");
        assert_eq!(run_cmds_trim_newline("echo 123").unwrap(), "123");
        assert_eq!(
            run_cmds_trim_newline("whoami").unwrap() + "\n",
            run_cmds("whoami").unwrap()
        );
    }
}
