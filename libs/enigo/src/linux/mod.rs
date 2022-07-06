mod nix_impl;
mod pynput;
mod xdo;

pub use self::nix_impl::Enigo;

/// Check if display manager is x11.
pub fn is_x11() -> bool {
    let stdout = 
    match std::process::Command::new("sh")
    .arg("-c")
    .arg("loginctl show-session $(loginctl | awk '/tty/ {print $1}') -p Type | awk -F= '{print $2}'")
    .output() {
        Ok(output) => {
            output.stdout
        },
        Err(_) => {
            match std::process::Command::new("sh")
            .arg("-c")
            .arg("echo $XDG_SESSION_TYPE")
            .output() {
                Ok(output) => {
                    output.stdout
                },
                Err(_) => {
                    return false;
                }
            }
        }
    };

    if let Ok(display_manager) = std::str::from_utf8(&stdout) {
        display_manager.trim() == "x11"
    } else {
        false
    }
}
