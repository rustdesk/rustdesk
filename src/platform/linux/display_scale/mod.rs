use crate::platform::display_scale::{token, validate, Display, State, UNSUPPORTED};
use dbus::blocking::Connection;
use hbb_common::{bail, libc, ResultType};
use std::time::Duration;

mod gnome;
mod kde;
mod layout;

const TIMEOUT: Duration = Duration::from_secs(2);

fn connection() -> ResultType<Connection> {
    // Never address a greeter, root's bus, or another logged-in user's compositor.
    let uid = unsafe { libc::geteuid() };
    let greeter = matches!(
        hbb_common::whoami::username().as_str(),
        "gdm" | "_gdm" | "sddm" | "lightdm"
    );
    if uid == 0
        || greeter
        || crate::platform::linux::get_active_userid_fresh()
            .parse::<u32>()
            .ok()
            != Some(uid)
    {
        bail!(UNSUPPORTED);
    }
    Ok(Connection::new_session()?)
}

fn has_owner(connection: &Connection, name: &str) -> ResultType<bool> {
    let proxy = connection.with_proxy("org.freedesktop.DBus", "/org/freedesktop/DBus", TIMEOUT);
    let (owner,): (bool,) = proxy.method_call("org.freedesktop.DBus", "NameHasOwner", (name,))?;
    Ok(owner)
}

pub fn read(display: &Display) -> ResultType<State> {
    let connection = connection()?;
    let captured = resolve_capture(display)?;
    let display = captured.as_ref().unwrap_or(display);
    if has_owner(&connection, "org.gnome.Mutter.DisplayConfig")? {
        gnome::read(&connection, display)
    } else if has_owner(&connection, "org.kde.KWin")? && crate::platform::current_is_wayland() {
        kde::read(display)
    } else {
        bail!(UNSUPPORTED)
    }
}

pub fn apply(display: &Display, percent: f64, expected: &str) -> ResultType<State> {
    let connection = connection()?;
    let captured = resolve_capture(display)?;
    let display = captured.as_ref().unwrap_or(display);
    if has_owner(&connection, "org.gnome.Mutter.DisplayConfig")? {
        gnome::apply(&connection, display, percent, expected)
    } else if has_owner(&connection, "org.kde.KWin")? && crate::platform::current_is_wayland() {
        kde::apply(display, percent, expected)
    } else {
        bail!(UNSUPPORTED)
    }
}

fn resolve_capture(display: &Display) -> ResultType<Option<Display>> {
    if !display.name.is_empty() || !crate::platform::current_is_wayland() {
        return Ok(None);
    }
    // PipeWire keeps its initial stream positions when scaling moves another
    // monitor. Resolve against the same cached layout used to position capture.
    captured_display(display, &scrap::wayland::display::get_displays().displays)
}

fn captured_display(
    display: &Display,
    captured: &[base::platform::linux::WaylandDisplayInfo],
) -> ResultType<Option<Display>> {
    let mut matches = captured.iter().filter(|output| {
        let size = if matches!(output.transform, 90 | 270) {
            (output.height, output.width)
        } else {
            (output.width, output.height)
        };
        display.origin == (output.x, output.y)
            && size.0 > 0
            && size.1 > 0
            && display.size == (size.0 as usize, size.1 as usize)
    });
    let Some(output) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        bail!(UNSUPPORTED);
    }
    Ok((!output.name.is_empty()).then(|| Display {
        name: output.name.clone(),
        origin: display.origin,
        size: display.size,
    }))
}

fn percent(scale: f64) -> Option<f64> {
    (scale.is_finite() && (0.5..=5.0).contains(&scale)).then(|| scale * 100.0)
}

fn same_connector(a: &str, b: &str) -> bool {
    fn normalize(name: &str) -> String {
        let parts: Vec<_> = name.split('-').collect();
        if parts.len() == 3
            && parts[1].len() == 1
            && parts[1].bytes().all(|c| c.is_ascii_alphabetic())
        {
            format!("{}-{}", parts[0], parts[2])
        } else {
            name.into()
        }
    }
    // DRM may report HDMI-A-1 where Mutter reports HDMI-1. Keep MST port numbers.
    !a.is_empty() && normalize(a) == normalize(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_connector_requires_unique_oriented_geometry() {
        let display = Display {
            name: String::new(),
            origin: (1920, 0),
            size: (2160, 3840),
        };
        let mut captured = base::platform::linux::WaylandDisplayInfo {
            name: "DP-1".into(),
            x: 1920,
            y: 0,
            width: 3840,
            height: 2160,
            logical_size: Some((1080, 1920)),
            refresh_rate: 60,
            transform: 90,
        };
        assert_eq!(
            captured_display(&display, &[captured.clone()])
                .unwrap()
                .unwrap()
                .name,
            "DP-1"
        );
        assert!(captured_display(&display, &[captured.clone(), captured.clone()]).is_err());
        captured.transform = 0;
        assert!(captured_display(&display, &[captured.clone()])
            .unwrap()
            .is_none());
        captured.transform = 270;
        captured.name.clear();
        assert!(captured_display(&display, &[captured]).unwrap().is_none());
    }

    #[test]
    fn connector_matching_preserves_mst_ports() {
        assert!(same_connector("HDMI-A-1", "HDMI-1"));
        assert!(same_connector("DP-1-2", "DP-1-2"));
        assert!(!same_connector("DP-1-2", "DP-2"));
        assert!(!same_connector("", ""));
    }
}
