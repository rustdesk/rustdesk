use hbb_common::regex::Regex;
use lazy_static::lazy_static;
use std::sync::Mutex;
use std::{
    process::{Command, Output, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::warn;

use hbb_common::platform::linux::{get_wayland_displays, WaylandDisplayInfo};

lazy_static! {
    static ref DISPLAYS: Mutex<Option<Arc<Displays>>> = Mutex::new(None);
}

const COMMAND_TIMEOUT: Duration = Duration::from_millis(1000);

pub struct Displays {
    pub primary: usize,
    pub displays: Vec<WaylandDisplayInfo>,
}

// We need this helper to run commands with a timeout, as some commands may hang.
// `kscreen-doctor -o` is known to hang when:
// 1. On Archlinux, Both GNOME and KDE Plasma are installed.
// 2. Run this command in a GNOME session.
fn run_with_timeout(
    program: &str,
    args: &[&str],
    timeout: Duration,
    label: &str,
) -> Option<Output> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let start = Instant::now();
    loop {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        if start.elapsed() >= timeout {
            warn!("{} command timed out after {:?}", label, timeout);
            if let Err(e) = child.kill() {
                warn!("Failed to kill child process for '{}': {}", label, e);
            }
            if let Err(e) = child.wait() {
                warn!("Failed to wait for child process for '{}': {}", label, e);
            }
            return None;
        }
        std::thread::sleep(Duration::from_millis(30));
    }

    match child.wait_with_output() {
        Ok(output) => {
            if !output.status.success() {
                warn!("{} command failed with status: {}", label, output.status);
                return None;
            }
            Some(output)
        }
        Err(_) => None,
    }
}

// There are some limitations with xrandr method:
// 1. It only works when XWayland is running.
// 2. The distro may not have xrandr installed by default.
// 3. xrandr may not report "primary" in its output. eg. openSUSE Leap 15.6 KDE Plasma.
fn try_xrandr_primary() -> Option<String> {
    let output = Command::new("xrandr").output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if line.contains("primary") && line.contains("connected") {
            if let Some(name) = line.split_whitespace().next() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn try_kscreen_primary() -> Option<String> {
    if !hbb_common::platform::linux::is_kde_session() {
        return None;
    }

    let output = run_with_timeout(
        "kscreen-doctor",
        &["-o"],
        COMMAND_TIMEOUT,
        "kscreen-doctor -o",
    )?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);

    // Remove ANSI color codes
    let re_ansi = Regex::new(r"\x1b\[[0-9;]*m").ok()?;
    let clean_text = re_ansi.replace_all(&text, "");

    // Split the text into blocks, each starting with "Output:".
    // The first element of the split will be empty, so we skip it.
    for block in clean_text.split("Output:").skip(1) {
        // Check if this block describes the primary monitor.
        if block.contains("priority 1") {
            // The monitor name is the second piece of text in the block, after the ID.
            // e.g., " 1 eDP-1 enabled..." -> "eDP-1"
            if let Some(name) = block.split_whitespace().nth(1) {
                return Some(name.to_string());
            }
        }
    }

    None
}

fn try_gdbus_primary() -> Option<String> {
    let output = run_with_timeout(
        "gdbus",
        &[
            "call",
            "--session",
            "--dest",
            "org.gnome.Mutter.DisplayConfig",
            "--object-path",
            "/org/gnome/Mutter/DisplayConfig",
            "--method",
            "org.gnome.Mutter.DisplayConfig.GetCurrentState",
        ],
        COMMAND_TIMEOUT,
        "gdbus DisplayConfig.GetCurrentState",
    )?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);

    // Match logical monitor entries with primary=true
    // Pattern: (x, y, scale, transform, true, [('connector-name', ...), ...], ...)
    // Use regex to find entries where 5th field is true, then extract connector name
    // Example matched text: "(0, 0, 1.5, 0, true, [('HDMI-1', 'MHH', 'Monitor', '0x00000000')], ...)"
    let re = Regex::new(r"\([^()]*,\s*true,\s*\[\('([^']+)'").ok()?;

    if let Some(captures) = re.captures(&text) {
        return captures.get(1).map(|m| m.as_str().to_string());
    }

    None
}

fn get_primary_monitor() -> Option<String> {
    try_xrandr_primary()
        .or_else(try_kscreen_primary)
        .or_else(try_gdbus_primary)
}

pub fn get_displays() -> Arc<Displays> {
    let mut lock = DISPLAYS.lock().unwrap();
    match lock.as_ref() {
        Some(displays) => displays.clone(),
        None => match get_wayland_displays() {
            Ok(displays) => {
                let mut primary_index = None;
                if let Some(name) = get_primary_monitor() {
                    for (i, display) in displays.iter().enumerate() {
                        if display.name == name {
                            primary_index = Some(i);
                            break;
                        }
                    }
                };
                if primary_index.is_none() {
                    for (i, display) in displays.iter().enumerate() {
                        if display.x == 0 && display.y == 0 {
                            primary_index = Some(i);
                            break;
                        }
                    }
                }
                let displays = Arc::new(Displays {
                    primary: primary_index.unwrap_or(0),
                    displays,
                });
                *lock = Some(displays.clone());
                displays
            }
            Err(err) => {
                warn!("Failed to get wayland displays: {}", err);
                Arc::new(Displays {
                    primary: 0,
                    displays: Vec::new(),
                })
            }
        },
    }
}

#[inline]
pub fn clear_wayland_displays_cache() {
    let _ = DISPLAYS.lock().unwrap().take();
}

// Return (min_x, max_x, min_y, max_y)
pub fn get_desktop_rect_for_uinput() -> Option<(i32, i32, i32, i32)> {
    let wayland_displays = get_displays();
    let displays = &wayland_displays.displays;
    if displays.is_empty() {
        return None;
    }

    // For compatibility, if only one display, we use the physical size for `uinput`.
    // Otherwise, we use the logical size for `uinput`.
    if displays.len() == 1 {
        let d = &displays[0];
        return Some((d.x, d.x + d.width, d.y, d.y + d.height));
    }

    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for d in displays.iter() {
        min_x = min_x.min(d.x);
        min_y = min_y.min(d.y);
        let size = if let Some(logical_size) = d.logical_size {
            logical_size
        } else {
            // When `logical_size` is None, we cannot obtain the correct desktop rectangle.
            // This may occur if the Wayland compositor does not provide logical size information,
            // or if display information is incomplete. We fall back to physical size, which provides
            // usable dimensions, but may not always be correct depending on compositor behavior.
            warn!(
                    "Display at ({}, {}) is missing logical_size; falling back to physical size ({}, {}).",
                    d.x, d.y, d.width, d.height
                );
            (d.width, d.height)
        };
        max_x = max_x.max(d.x + size.0);
        max_y = max_y.max(d.y + size.1);
    }
    Some((min_x, max_x, min_y, max_y))
}
