use serde::de::Error;

use crate::ResultType;
use std::{
    borrow::BorrowMut, collections::HashMap, convert::TryFrom, fmt::Display, ops::Index,
    process::Command,
};

lazy_static::lazy_static! {
    pub static ref DISTRO: Distro = Distro::new();
}

pub struct Distro {
    pub name: String,
    pub version_id: String,
}

impl Distro {
    fn new() -> Self {
        let name = run_cmds("awk -F'=' '/^NAME=/ {print $2}' /etc/os-release".to_owned())
            .unwrap_or_default()
            .trim()
            .trim_matches('"')
            .to_string();
        let version_id =
            run_cmds("awk -F'=' '/^VERSION_ID=/ {print $2}' /etc/os-release".to_owned())
                .unwrap_or_default()
                .trim()
                .trim_matches('"')
                .to_string();
        Self { name, version_id }
    }
}

/// Loginctl is the most reliable, but frequently call DBUS,
/// avoid using the network.
///
/// 1. XDG_SESSION_TYPE: Get TTY When using ssh.
fn get_display_server_str() -> String {
    let mut session = get_values_of_seat0([0].to_vec())[0].clone();
    if session.is_empty() {
        // loginctl has not given the expected output.  try something else.
        if let Ok(sid) = std::env::var("XDG_SESSION_ID") {
            // could also execute "cat /proc/self/sessionid"
            session = sid;
        }
        if session.is_empty() {
            session = run_cmds("cat /proc/self/sessionid".to_owned()).unwrap_or_default();
        }
    }

    get_display_server_of_session(&session)
}

fn get_display_server_of_session(session: &str) -> String {
    let mut display_server = if let Ok(output) =
        run_loginctl(Some(vec!["show-session", "-p", "Type", session]))
    // Check session type of the session
    {
        let display_server = output.replace("Type=", "").trim_end().into();
        if display_server == "tty" {
            // If the type is tty...
            if let Ok(output) = run_loginctl(Some(vec!["show-session", "-p", "TTY", session]))
            // Get the tty number
            {
                let tty: String = output.replace("TTY=", "").trim_end().into();
                if let Ok(xorg_results) = run_cmds(format!("ps -e | grep \"{tty}.\\\\+Xorg\""))
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
    }
    // If the session is not a tty, then just return the type as usual
    display_server
}

fn get_values_of_seat0(indices: Vec<usize>) -> Vec<String> {
    if let Ok(output) = run_loginctl(None) {
        for line in output.lines() {
            if line.contains("seat0") {
                if let Some(sid) = line.split_whitespace().next() {
                    if is_active(sid) {
                        return indices
                            .into_iter()
                            .map(|idx| line.split_whitespace().nth(idx).unwrap_or("").to_owned())
                            .collect::<Vec<String>>();
                    }
                }
            }
        }
    }

    // some case, there is no seat0 https://github.com/rustdesk/rustdesk/issues/73
    if let Ok(output) = run_loginctl(None) {
        for line in output.lines() {
            if let Some(sid) = line.split_whitespace().next() {
                let d = get_display_server_of_session(sid);
                if is_active(sid) && d != "tty" {
                    return indices
                        .into_iter()
                        .map(|idx| line.split_whitespace().nth(idx).unwrap_or("").to_owned())
                        .collect::<Vec<String>>();
                }
            }
        }
    }

    return indices
        .iter()
        .map(|_x| "".to_owned())
        .collect::<Vec<String>>();
}

fn is_active(sid: &str) -> bool {
    if let Ok(output) = run_loginctl(Some(vec!["show-session", "-p", "State", sid])) {
        output.contains("active")
    } else {
        false
    }
}

pub fn run_cmds(cmds: String) -> ResultType<String> {
    let output = std::process::Command::new("sh")
        .args(vec!["-c", &cmds])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(not(feature = "flatpak"))]
fn run_loginctl(args: Option<Vec<&str>>) -> anyhow::Result<String> {
    let mut cmd = std::process::Command::new("loginctl");
    let mut stdout = vec![];

    if let Some(args) = args {
        stdout = cmd.args(args).output()?.stdout;
    } else {
        stdout = cmd.output()?.stdout;
    }

    Ok(String::from_utf8_lossy(&stdout).into())
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

#[derive(Debug, PartialEq)]
pub enum DisplayServer {
    X11,
    Wayland,
    UnKnown,
}

impl Display for DisplayServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl TryFrom<&String> for DisplayServer {
    type Error = ();
    fn try_from(value: &String) -> Result<Self, Self::Error> {
        let mut server = Err(());
        let value = value.to_lowercase();

        if value.contains("x11") || value.contains("xorg") {
            server = Ok(DisplayServer::X11);
        } else if value.contains("wayland") {
            server = Ok(DisplayServer::Wayland);
        }
        server
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinuxSession {
    pub session_id: u8,
    pub uid: u32,
    pub username: String,
    pub seat: Option<String>,
    pub tty: Option<String>,
}

impl TryFrom<Vec<String>> for LinuxSession {
    type Error = anyhow::Error;
    fn try_from(values: Vec<String>) -> anyhow::Result<Self, Self::Error> {
        if values.len() < 3 {
            anyhow::bail!("Not enough information in Vec");
        }

        Ok(Self {
            session_id: values[0].parse::<u8>()?,
            uid: values[1].parse::<u32>()?,
            username: values[2].clone(),
            seat: values.get(3).map(|s| s.to_owned()),
            tty: values.get(4).map(|s| s.to_owned()),
        })
    }
}

impl LinuxSession {
    pub fn get_properties(&self) -> SessionProperties {
        let mut r#type = String::new();
        let mut tty = String::new();
        let mut state = String::new();

        if let Ok(output) = run_loginctl(Some(vec![
            "show-session",
            "-p",
            "Type",
            "-p",
            "TTY",
            "-p",
            "State",
            &self.session_id.to_string(),
        ])) {
            let lines: Vec<&str> = output.lines().map(|line| line.trim()).collect();
            for line in lines {
                let key_value: Vec<&str> = line.split('=').map(|s| s).collect();

                if line.contains("Type") {
                    if let Some(value) = key_value.get(1) {
                        r#type = value.to_string();
                    }
                }
                if line.contains("TTY") {
                    if let Some(value) = key_value.get(1) {
                        tty = value.to_string();
                    }
                }
                if line.contains("State") {
                    if let Some(value) = key_value.get(1) {
                        state = value.to_string();
                    }
                }
            }
        }

        SessionProperties { r#type, tty, state }
    }

    pub fn is_seat0(&self) -> bool {
        if let Some(s) = &self.seat {
            if s.contains("seat0") {
                return true;
            } else {
                return false;
            }
        } else {
            return false;
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SessionProperties {
    pub r#type: String,
    pub tty: String,
    pub state: String,
}

impl SessionProperties {
    pub fn is_active(&self) -> bool {
        self.state.contains("active")
    }

    pub fn get_display_server(&self) -> DisplayServer {
        if self.r#type.contains("x11") {
            DisplayServer::X11
        } else if self.r#type.contains("wayland") {
            DisplayServer::Wayland
        } else {
            DisplayServer::UnKnown
        }
    }
}

/// Loginctl is the most reliable, but frequently call DBUS,
/// avoid using the network.
///
/// 1. XDG_SESSION_TYPE
/// 2. ps -h
/// 3. loginctl
///
/// refs: <https://unix.stackexchange.com/questions/236498/how-to-get-information-about-which-display-server-is-running>
pub fn get_display_server() -> DisplayServer {
    let mut display_server = None;
    // WARNING: When using ssh, XDG_SESSION_TYPE will get tty. We need to distinguish between desktop servers, X11, or Wayland.
    if let Ok(server_type_str) = std::env::var("XDG_SESSION_TYPE") {
        if let Ok(server_type) = DisplayServer::try_from(&server_type_str) {
            display_server = Some(server_type);
        }
    }
    if let Ok(server_type_str) = run_cmds("ps -h | grep tty2".to_owned()) {
        if let Ok(server_type) = DisplayServer::try_from(&server_type_str) {
            display_server = Some(server_type);
        }
    }
    if display_server.is_none() {
        if let Ok((session, properties)) = get_seat0_session_properties() {
            display_server = Some(properties.get_display_server());
        }
    }

    if let Some(display_server) = display_server {
        display_server
    } else {
        log::warn!("Faild to get display server, default display server is x11");
        DisplayServer::X11
    }
}

pub fn get_linux_sessions() -> anyhow::Result<Vec<LinuxSession>> {
    let output = run_loginctl(None)?;

    let display_server: String = output.into();
    let mut linux_sessions: Vec<LinuxSession> = vec![];
    for (l, line_str) in display_server.lines().enumerate() {
        if l == 0 {
            continue;
        }
        if line_str == "" {
            break;
        }
        let session_values: Vec<String> =
            line_str.split_whitespace().map(|s| s.to_owned()).collect();

        let linux_session = LinuxSession::try_from(session_values)?;
        linux_sessions.push(linux_session);
    }

    Ok(linux_sessions)
}

/// Returns session & properties of seat0.
/// refs: <https://github.com/rustdesk/rustdesk/issues/73>
pub fn get_seat0_session_properties() -> anyhow::Result<(LinuxSession, SessionProperties)> {
    let sessions = get_linux_sessions()?;
    let mut session_properties = None;

    for session in &sessions {
        if session.is_seat0() {
            let props = session.get_properties();
            if props.is_active() {
                session_properties.replace((session.clone(), props));
            }
        }
    }
    if session_properties.is_none() {
        for session in &sessions {
            if let Some(s) = &session.seat {
                let props = session.get_properties();

                if props.r#type != "tty" && props.is_active() {
                    session_properties.replace((session.clone(), props));
                }
            }
        }
    }

    if let Some(session_properties) = session_properties {
        Ok(session_properties)
    } else {
        anyhow::bail!("Failed to get properties of seat0");
    }
}

#[test]
fn test_get_display_server() -> anyhow::Result<()> {
    let server = get_display_server();

    let sessions = get_linux_sessions()?;
    let properties = sessions[0].get_properties();
    let session_display_server = properties.get_display_server();
    let seat0_properties = get_seat0_session_properties()?;

    let (session, properties) = get_seat0_session_properties()?;
    let (cur_uid, cur_user) = (session.uid, session.username);
    let is_wayland = properties.get_display_server() == DisplayServer::Wayland;

    dbg!(sessions);
    dbg!(&properties);
    dbg!(&seat0_properties);

    Ok(())
}
