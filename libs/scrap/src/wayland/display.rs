use hbb_common::regex::Regex;
use lazy_static::lazy_static;
use std::sync::Mutex;
use std::{process::Command, sync::Arc};
use tracing::warn;

use hbb_common::platform::linux::{get_wayland_displays, WaylandDisplayInfo};

lazy_static! {
    static ref DISPLAYS: Mutex<Option<Arc<Displays>>> = Mutex::new(None);
}

pub struct Displays {
    pub primary: usize,
    pub displays: Vec<WaylandDisplayInfo>,
}

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
    let output = Command::new("kscreen-doctor").args(["-o"]).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);

    // Remove ANSI color codes
    let re_ansi = Regex::new(r"\x1b\[[0-9;]*m").ok()?;
    let clean_text = re_ansi.replace_all(&text, "");

    // Split into output blocks and find the one with priority 1
    // Each block starts with "Output:"
    let re_block = Regex::new(r"Output:\s+\d+\s+(\S+)").ok()?;
    let re_priority = Regex::new(r"priority\s+1\b").ok()?;

    let mut last_name = None;
    for line in clean_text.lines() {
        if let Some(caps) = re_block.captures(line) {
            last_name = caps.get(1).map(|m| m.as_str().to_string());
        } else if re_priority.is_match(line) && last_name.is_some() {
            return last_name;
        }
    }

    None
}

fn try_gdbus_primary() -> Option<String> {
    let output = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.Mutter.DisplayConfig",
            "--object-path",
            "/org/gnome/Mutter/DisplayConfig",
            "--method",
            "org.gnome.Mutter.DisplayConfig.GetCurrentState",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);

    // Match logical monitor entries with primary=true
    // Pattern: (x, y, scale, transform, true, [('connector-name', ...), ...], ...)
    // Use regex to find entries where 5th field is true, then extract connector name
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
