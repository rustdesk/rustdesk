use super::{percent, Display, State, TIMEOUT, UNSUPPORTED};
use dbus::{
    arg::{PropMap, Variant},
    blocking::{stdintf::org_freedesktop_dbus::Properties, Connection},
};
use hbb_common::{bail, ResultType};

const BUS: &str = "org.gnome.Mutter.DisplayConfig";
const PATH: &str = "/org/gnome/Mutter/DisplayConfig";
type Spec = (String, String, String, String);
type Mode = (String, i32, i32, f64, f64, Vec<f64>, PropMap);
type Monitor = (Spec, Vec<Mode>, PropMap);
type Logical = (i32, i32, f64, u32, bool, Vec<Spec>, PropMap);
type Current = (u32, Vec<Monitor>, Vec<Logical>, PropMap);
type Configuration = Vec<(i32, i32, f64, u32, bool, Vec<(String, String, PropMap)>)>;

fn flag(props: &PropMap, key: &str) -> bool {
    props.get(key).and_then(|v| v.0.as_i64()) == Some(1)
}

fn mode(monitor: &Monitor) -> ResultType<&Mode> {
    monitor
        .1
        .iter()
        .find(|m| flag(&m.6, "is-current"))
        .ok_or_else(|| hbb_common::anyhow::anyhow!(UNSUPPORTED))
}

fn selected(current: &Current, display: &Display) -> ResultType<usize> {
    let mut matches = Vec::new();
    for (i, logical) in current.2.iter().enumerate() {
        if logical.5.len() != 1 {
            continue;
        }
        let Some(monitor) = current.1.iter().find(|m| m.0 == logical.5[0]) else {
            continue;
        };
        let m = mode(monitor)?;
        let size = if logical.3 % 2 == 1 {
            (m.2, m.1)
        } else {
            (m.1, m.2)
        };
        if super::same_connector(&display.name, &monitor.0 .0)
            || (display.name.is_empty()
                && display.origin == (logical.0, logical.1)
                && display.size == (size.0 as usize, size.1 as usize))
        {
            matches.push(i);
        }
    }
    if matches.len() != 1 {
        bail!(UNSUPPORTED);
    }
    Ok(matches[0])
}

fn state(current: &Current, display: &Display) -> ResultType<(State, usize)> {
    let index = selected(current, display)?;
    if flag(&current.3, "global-scale-required") && current.2.len() > 1 {
        bail!(UNSUPPORTED);
    }
    if current.1.iter().any(|m| flag(&m.2, "is-for-lease")) {
        bail!(UNSUPPORTED);
    }
    let logical = &current.2[index];
    let Some(monitor) = current.1.iter().find(|m| m.0 == logical.5[0]) else {
        bail!(UNSUPPORTED);
    };
    let mode = mode(monitor)?;
    let Some(value) = percent(logical.2) else {
        bail!(UNSUPPORTED);
    };
    let mut options: Vec<_> = mode.5.iter().filter_map(|s| percent(*s)).collect();
    options.sort_by(f64::total_cmp);
    options.dedup();
    if !options.contains(&value) {
        bail!(UNSUPPORTED);
    }
    Ok((
        State {
            percent: value,
            custom: None,
            recommended: percent(mode.4).filter(|p| options.contains(p)),
            options,
            token: super::token((current.0, &monitor.0, &mode.0, logical.2.to_bits())),
        },
        index,
    ))
}

fn current(connection: &Connection) -> ResultType<Current> {
    let proxy = connection.with_proxy(BUS, PATH, TIMEOUT);
    match proxy.get::<bool>(BUS, "ApplyMonitorsConfigAllowed") {
        Ok(false) => bail!(UNSUPPORTED),
        Ok(true) => {}
        // Older Mutter versions expose GetCurrentState without this property.
        Err(error)
            if matches!(
                error.name(),
                Some(
                    "org.freedesktop.DBus.Error.UnknownProperty"
                        | "org.freedesktop.DBus.Error.InvalidArgs"
                )
            ) => {}
        Err(error) => return Err(error.into()),
    }
    Ok(proxy.method_call(BUS, "GetCurrentState", ())?)
}

pub fn read(connection: &Connection, display: &Display) -> ResultType<State> {
    Ok(state(&current(connection)?, display)?.0)
}

fn configuration(
    current: &Current,
    index: usize,
    scale: f64,
) -> ResultType<(Configuration, PropMap)> {
    let target = &current.2[index];
    let mut logicals = Vec::new();
    for (i, logical) in current.2.iter().enumerate() {
        let mut monitors = Vec::new();
        for spec in &logical.5 {
            let Some(monitor) = current.1.iter().find(|m| &m.0 == spec) else {
                bail!(UNSUPPORTED);
            };
            let mode = mode(monitor)?;
            let mut props = PropMap::new();
            for (from, to) in [
                ("is-underscanning", "underscanning"),
                ("color-mode", "color-mode"),
                ("rgb-range", "rgb-range"),
            ] {
                if let Some(value) = monitor.2.get(from) {
                    props.insert(to.into(), Variant(value.0.box_clone()));
                }
            }
            monitors.push((spec.0.clone(), mode.0.clone(), props));
        }
        logicals.push((
            logical.0,
            logical.1,
            if i == index { scale } else { logical.2 },
            logical.3,
            logical.4,
            monitors,
        ));
    }
    let layout = current
        .3
        .get("layout-mode")
        .and_then(|p| p.0.as_u64())
        .unwrap_or(1);
    if layout != 1 && layout != 2 {
        bail!(UNSUPPORTED);
    }
    if layout == 1 {
        let Some(monitor) = current.1.iter().find(|m| m.0 == target.5[0]) else {
            bail!(UNSUPPORTED);
        };
        let mode = mode(monitor)?;
        let size = if target.3 % 2 == 1 {
            (mode.2, mode.1)
        } else {
            (mode.1, mode.2)
        };
        let old = (
            (size.0 as f64 / target.2).round() as i32,
            (size.1 as f64 / target.2).round() as i32,
        );
        let new = (
            (size.0 as f64 / scale).round() as i32,
            (size.1 as f64 / scale).round() as i32,
        );
        // Keep adjacent rows/columns attached when the target's logical size changes.
        // Mutter verifies the full layout before any settings are changed.
        for (i, logical) in logicals.iter_mut().enumerate() {
            if i == index {
                continue;
            }
            if logical.0 >= target.0 + old.0 {
                logical.0 += new.0 - old.0;
            }
            if logical.1 >= target.1 + old.1 {
                logical.1 += new.1 - old.1;
            }
        }
    }
    let mut props = PropMap::new();
    if flag(&current.3, "supports-changing-layout-mode") {
        props.insert("layout-mode".into(), Variant(Box::new(layout as u32)));
    }
    Ok((logicals, props))
}

pub fn apply(
    connection: &Connection,
    display: &Display,
    value: f64,
    expected: &str,
) -> ResultType<()> {
    let current = current(connection)?;
    let (state, index) = state(&current, display)?;
    super::validate(&state, value, expected)?;
    if state.percent == value {
        return Ok(());
    }
    let target = &current.2[index];
    let Some(monitor) = current.1.iter().find(|m| m.0 == target.5[0]) else {
        bail!(UNSUPPORTED);
    };
    let Some(scale) = mode(monitor)?
        .5
        .iter()
        .find(|s| percent(**s) == Some(value))
    else {
        bail!(UNSUPPORTED);
    };
    let (logicals, props) = configuration(&current, index, *scale)?;
    let proxy = connection.with_proxy(BUS, PATH, TIMEOUT);
    let _: () = proxy.method_call(
        BUS,
        "ApplyMonitorsConfig",
        (current.0, 0u32, &logicals, &props),
    )?;
    // Session-scoped application avoids a second local confirmation dialog.
    let _: () = proxy.method_call(
        BUS,
        "ApplyMonitorsConfig",
        (current.0, 1u32, logicals, props),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Current {
        let monitors: Vec<_> = ["eDP-1", "DP-1"]
            .into_iter()
            .map(|name| {
                let spec = (name.into(), "vendor".into(), "model".into(), name.into());
                let mut props = PropMap::new();
                props.insert("is-current".into(), Variant(Box::new(true)));
                let mode = (
                    "3840x2160@60".into(),
                    3840,
                    2160,
                    60.0,
                    2.0,
                    vec![1.0, 1.25, 1.5, 2.0],
                    props,
                );
                let mut props = PropMap::new();
                props.insert("is-underscanning".into(), Variant(Box::new(true)));
                props.insert("color-mode".into(), Variant(Box::new(1u32)));
                (spec, vec![mode], props)
            })
            .collect();
        let logicals = monitors
            .iter()
            .enumerate()
            .map(|(i, m)| {
                (
                    1920 * i as i32,
                    0,
                    2.0,
                    0,
                    i == 0,
                    vec![m.0.clone()],
                    PropMap::new(),
                )
            })
            .collect();
        (42, monitors, logicals, PropMap::new())
    }

    #[test]
    fn preserves_fractional_native_levels() {
        let mut current = fixture();
        current.1[0].1[0].5.push(1.255);
        current.2[0].2 = 1.255;
        let display = Display {
            name: String::new(),
            origin: (0, 0),
            size: (3840, 2160),
        };
        let (state, _) = state(&current, &display).unwrap();
        assert!((state.percent - 125.5).abs() < 0.000001);
        assert!(state
            .options
            .iter()
            .any(|value| (*value - 125.5).abs() < 0.000001));
    }

    #[test]
    fn maps_nameless_capture_uniquely_and_invalidates_old_serial() {
        let mut current = fixture();
        let display = Display {
            name: String::new(),
            origin: (1920, 0),
            size: (3840, 2160),
        };
        let (before, index) = state(&current, &display).unwrap();
        assert_eq!(index, 1);
        assert_eq!(before.percent, 200.0);
        assert_eq!(before.options, [100.0, 125.0, 150.0, 200.0]);
        current.0 += 1;
        assert_ne!(before.token, state(&current, &display).unwrap().0.token);
        current.2[0].0 = 1920;
        assert!(state(&current, &display).is_err());
    }

    #[test]
    fn capture_identity_survives_an_adjacent_monitor_scale_change() {
        let mut current = fixture();
        let display = Display {
            name: String::new(),
            origin: (1920, 0),
            size: (3840, 2160),
        };
        let captured = base::platform::linux::WaylandDisplayInfo {
            name: "DP-1".into(),
            x: 1920,
            y: 0,
            width: 3840,
            height: 2160,
            logical_size: Some((1920, 1080)),
            refresh_rate: 60,
            transform: 0,
        };
        current.2[0].2 = 1.5;
        current.2[1].0 = 2560;
        assert!(state(&current, &display).is_err());
        let resolved = super::super::captured_display(&display, &[captured])
            .unwrap()
            .unwrap();
        let (state, index) = state(&current, &resolved).unwrap();
        assert_eq!(index, 1);
        assert_eq!(state.percent, 200.0);
        let (config, _) = configuration(&current, index, 1.25).unwrap();
        assert_eq!(config[0].2, 1.5);
        assert_eq!((config[1].0, config[1].2), (2560, 1.25));
    }

    #[test]
    fn resizing_preserves_modes_color_rotation_and_adjacent_layout() {
        let current = fixture();
        let (config, _) = configuration(&current, 0, 1.5).unwrap();
        assert_eq!(config[0].2, 1.5);
        assert_eq!((config[1].0, config[1].1, config[1].2), (2560, 0, 2.0));
        assert!(config[0].4);
        assert_eq!(config[0].5[0].1, "3840x2160@60");
        assert!(flag(&config[0].5[0].2, "underscanning"));
        assert_eq!(config[0].5[0].2["color-mode"].0.as_u64(), Some(1));
        assert_eq!(current.2[1].0, 1920);
    }

    #[test]
    fn refuses_global_scale_on_multiple_displays_and_mirrored_target() {
        let mut current = fixture();
        let display = Display {
            name: "eDP-1".into(),
            origin: (0, 0),
            size: (3840, 2160),
        };
        current
            .3
            .insert("global-scale-required".into(), Variant(Box::new(true)));
        assert!(state(&current, &display).is_err());
        current.3.clear();
        current.2[0].5.push(current.1[1].0.clone());
        assert!(state(&current, &display).is_err());
    }
}
