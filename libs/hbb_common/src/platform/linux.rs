use crate::ResultType;

lazy_static::lazy_static! {
    pub static ref DISTRO: Disto = Disto::new();
}

pub struct Disto {
    pub name: String,
    pub version_id: String,
}

impl Disto {
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

pub fn get_display_server() -> String {
    let session = get_value_of_seat0(0);
    get_display_server_of_session(&session)
}

fn get_display_server_of_session(session: &str) -> String {
    if let Ok(output) = run_loginctl(Some(vec!["show-session", "-p", "Type", session]))
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
                if let Ok(xorg_results) = run_cmds(format!("ps -e | grep \"{}.\\\\+Xorg\"", tty))
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
            // loginctl has not given the expected output.  try something else.
            if let Ok(sestype) = std::env::var("XDG_SESSION_TYPE") {
                return sestype.to_owned();
            }
            // If the session is not a tty, then just return the type as usual
            display_server
        }
    } else {
        "".to_owned()
    }
}

pub fn get_value_of_seat0(i: usize) -> String {
    if let Ok(output) = run_loginctl(None) {
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
    if let Ok(output) = run_loginctl(None) {
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

    // loginctl has not given the expected output.  try something else.
    if let Ok(sid) = std::env::var("XDG_SESSION_ID") {
        // could also execute "cat /proc/self/sessionid"
        return sid.to_owned();
    }

    return "".to_owned();
}

fn is_active(sid: &str) -> bool {
    if let Ok(output) = run_loginctl(Some(vec!["show-session", "-p", "State", sid]))
    {
        String::from_utf8_lossy(&output.stdout).contains("active")
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
