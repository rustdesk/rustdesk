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
    let mut session = get_values_of_seat0([0].to_vec())[0].clone();
    if session.is_empty() {
        // loginctl has not given the expected output.  try something else.
        if let Ok(sid) = std::env::var("XDG_SESSION_ID") {
            // could also execute "cat /proc/self/sessionid"
            session = sid.to_owned();
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
            "".to_owned()
        }
    } else {
        "".to_owned()
    };
    if display_server.is_empty() {
        // loginctl has not given the expected output.  try something else.
        if let Ok(sestype) = std::env::var("XDG_SESSION_TYPE") {
            display_server = sestype;
        }
    }
    // If the session is not a tty, then just return the type as usual
    display_server
}

pub fn get_values_of_seat0(indices: Vec<usize>) -> Vec<String> {
    if let Ok(output) = run_loginctl(None) {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.contains("seat0") {
                if let Some(sid) = line.split_whitespace().nth(0) {
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
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(sid) = line.split_whitespace().nth(0) {
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
