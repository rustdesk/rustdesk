use super::{
    layout, percent, Display, SnapshotChanged, State, Target, STALE, TIMEOUT, UNSUPPORTED,
};
use dbus::{
    arg::{PropMap, Variant},
    blocking::{stdintf::org_freedesktop_dbus::Properties, Connection},
};
use hbb_common::{bail, ResultType};
use std::time::Duration;

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

fn matching(current: &Current, display: &Display) -> ResultType<Vec<usize>> {
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
    Ok(matches)
}

fn selected(current: &Current, display: &Display) -> ResultType<usize> {
    let matches = matching(current, display)?;
    if matches.len() != 1 {
        bail!(UNSUPPORTED);
    }
    Ok(matches[0])
}

fn state(current: &Current, display: &Display) -> ResultType<(State, usize)> {
    let index = selected(current, display)?;
    Ok((state_at(current, index)?, index))
}

fn state_at(current: &Current, index: usize) -> ResultType<State> {
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
    if mode.1 <= 0 || mode.2 <= 0 {
        bail!(UNSUPPORTED);
    }
    let resolution = if logical.3 % 2 == 1 {
        (mode.2 as u32, mode.1 as u32)
    } else {
        (mode.1 as u32, mode.2 as u32)
    };
    let Some(value) = percent(logical.2) else {
        bail!(UNSUPPORTED);
    };
    let mut options: Vec<_> = mode.5.iter().filter_map(|s| percent(*s)).collect();
    options.sort_by(f64::total_cmp);
    options.dedup();
    if !options.contains(&value) {
        bail!(UNSUPPORTED);
    }
    Ok(State {
        identity: super::token(("gnome", &monitor.0)),
        resolution,
        percent: value,
        custom: None,
        recommended: percent(mode.4).filter(|p| options.contains(p)),
        options,
        token: super::token((current.0, &monitor.0, &mode.0, logical.2.to_bits())),
    })
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

fn confirmed(current: &Current, display: Option<&Display>, identity: &str) -> ResultType<State> {
    let mut targets = current.2.iter().enumerate().filter(|(_, logical)| {
        logical
            .5
            .iter()
            .any(|spec| super::token(("gnome", spec)) == identity)
    });
    let Some((index, logical)) = targets.next() else {
        bail!(STALE);
    };
    if targets.next().is_some() || logical.5.len() != 1 {
        bail!(UNSUPPORTED);
    }
    // Validate the same native target before interpreting missing capture geometry
    // as a lagging snapshot rather than unavailable scaling.
    let state = state_at(current, index)?;
    let Some(display) = display else {
        bail!(SnapshotChanged);
    };
    match matching(current, display)?.as_slice() {
        [selected] if *selected == index => Ok(state),
        [] if display.name.is_empty() => bail!(SnapshotChanged),
        [] | [_] => bail!(STALE),
        _ => bail!(UNSUPPORTED),
    }
}

pub fn read_confirmed(
    connection: &Connection,
    display: Option<&Display>,
    identity: &str,
) -> ResultType<State> {
    confirmed(&current(connection)?, display, identity)
}

fn logical_size(current: &Current, logical: &Logical, scale: f64) -> ResultType<(i32, i32)> {
    if !scale.is_finite() || scale <= 0.0 || logical.3 > 7 {
        bail!(UNSUPPORTED);
    }
    let mut size = None;
    for spec in &logical.5 {
        let Some(monitor) = current.1.iter().find(|m| &m.0 == spec) else {
            bail!(UNSUPPORTED);
        };
        let mode = mode(monitor)?;
        let dimensions = (mode.1, mode.2);
        if dimensions.0 <= 0 || dimensions.1 <= 0 || size.is_some_and(|size| size != dimensions) {
            bail!(UNSUPPORTED);
        }
        size = Some(dimensions);
    }
    let Some((width, height)) = size else {
        bail!(UNSUPPORTED);
    };
    let (width, height) = if logical.3 % 2 == 1 {
        (height, width)
    } else {
        (width, height)
    };
    let width = (f64::from(width) / scale).round();
    let height = (f64::from(height) / scale).round();
    if !width.is_finite()
        || !height.is_finite()
        || !(1.0..=f64::from(i32::MAX)).contains(&width)
        || !(1.0..=f64::from(i32::MAX)).contains(&height)
    {
        bail!(UNSUPPORTED);
    }
    Ok((width as i32, height as i32))
}

fn batch_configuration(
    current: &Current,
    targets: &[Target],
) -> ResultType<(Configuration, PropMap)> {
    let layout_mode = current
        .3
        .get("layout-mode")
        .and_then(|p| p.0.as_u64())
        .unwrap_or(1);
    if !matches!(layout_mode, 1 | 2) {
        bail!(UNSUPPORTED);
    }
    // Mutter omits this property when its backend has no screen-size limit.
    let limit = if let Some(value) = current.3.get("max-screen-size") {
        let Some(mut fields) = value.0.as_iter() else {
            bail!(UNSUPPORTED);
        };
        let width = fields
            .next()
            .and_then(|v| v.as_i64())
            .and_then(|v| i32::try_from(v).ok());
        let height = fields
            .next()
            .and_then(|v| v.as_i64())
            .and_then(|v| i32::try_from(v).ok());
        match (width, height) {
            (Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
            _ => bail!(UNSUPPORTED),
        }
    } else {
        (i32::MAX, i32::MAX)
    };
    let mut found = vec![false; targets.len()];
    let mut logicals = Vec::new();
    let mut rectangles = Vec::new();
    let mut sizes = Vec::new();
    let mut maximum = Vec::new();
    let mut identities = Vec::new();
    for (index, logical) in current.2.iter().enumerate() {
        let mut specs = logical.5.clone();
        specs.sort();
        identities.push(super::token(&specs));
        let identity = (logical.5.len() == 1).then(|| super::token(("gnome", &logical.5[0])));
        let matches: Vec<_> = targets
            .iter()
            .enumerate()
            .filter(|(_, t)| identity.as_ref() == Some(&t.identity))
            .collect();
        if matches.len() > 1 {
            bail!(UNSUPPORTED);
        }
        let target = matches.first().map(|&(i, t)| {
            found[i] = true;
            t
        });
        if target.is_some() {
            state_at(current, index)?;
        }
        let scale = target.map_or(logical.2, |t| t.percent / 100.0);
        if percent(scale).is_none() {
            bail!(UNSUPPORTED);
        }
        let (mut max_width, mut max_height) = (0, 0);
        let mut desired_size = None;
        let mut monitors = Vec::new();
        for spec in &logical.5 {
            let Some(monitor) = current.1.iter().find(|m| &m.0 == spec) else {
                bail!(UNSUPPORTED);
            };
            let current_mode = mode(monitor)?;
            let oriented = |m: &Mode| {
                if logical.3 % 2 == 1 {
                    (m.2, m.1)
                } else {
                    (m.1, m.2)
                }
            };
            let supported = |m: &&Mode| {
                target.map_or(true, |t| {
                    oriented(m) == (t.resolution.0 as i32, t.resolution.1 as i32)
                        && m.5.iter().any(|s| (*s - scale).abs() < 0.000001)
                })
            };
            let selected = if supported(&current_mode) {
                current_mode
            } else {
                monitor
                    .1
                    .iter()
                    .filter(supported)
                    .max_by(|a, b| a.3.total_cmp(&b.3))
                    .ok_or(UNSUPPORTED)?
            };
            let dimensions = |m: &Mode, scale: f64| -> ResultType<(i32, i32)> {
                let (w, h) = oriented(m);
                if w <= 0 || h <= 0 {
                    bail!(UNSUPPORTED);
                }
                let divisor = if layout_mode == 1 { scale } else { 1.0 };
                let (w, h) = (
                    (f64::from(w) / divisor).round(),
                    (f64::from(h) / divisor).round(),
                );
                if !(1.0..=f64::from(i32::MAX)).contains(&w)
                    || !(1.0..=f64::from(i32::MAX)).contains(&h)
                {
                    bail!(UNSUPPORTED);
                }
                Ok((w as i32, h as i32))
            };
            let size = dimensions(selected, scale)?;
            if desired_size.is_some_and(|before| before != size) {
                bail!(UNSUPPORTED);
            }
            desired_size = Some(size);
            // Include every advertised mode and restorable scale, not just the
            // current one: resolution changes use this same admitted range.
            for m in &monitor.1 {
                for &s in m.5.iter().filter(|s| percent(**s).is_some()) {
                    let (w, h) = dimensions(m, s)?;
                    max_width = max_width.max(w);
                    max_height = max_height.max(h);
                }
            }
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
            monitors.push((spec.0.clone(), selected.0.clone(), props));
        }
        let before_scale = if layout_mode == 1 { logical.2 } else { 1.0 };
        let (width, height) = logical_size(current, logical, before_scale)?;
        rectangles.push(layout::Rect {
            x: logical.0,
            y: logical.1,
            width,
            height,
        });
        sizes.push(desired_size.ok_or(UNSUPPORTED)?);
        maximum.push((max_width, max_height));
        logicals.push((logical.0, logical.1, scale, logical.3, logical.4, monitors));
    }
    if found.iter().any(|found| !found) {
        bail!(STALE);
    }
    for (logical, rect) in logicals.iter_mut().zip(layout::plan(
        &rectangles,
        &sizes,
        &maximum,
        limit,
        &identities,
    )?) {
        logical.0 = rect.x;
        logical.1 = rect.y;
    }
    let mut props = PropMap::new();
    if flag(&current.3, "supports-changing-layout-mode") {
        props.insert("layout-mode".into(), Variant(Box::new(layout_mode as u32)));
    }
    Ok((logicals, props))
}

fn applied(current: &Current, configuration: &Configuration, before: &Current) -> bool {
    current.2.len() == configuration.len()
        && before
            .2
            .iter()
            .flat_map(|l| &l.5)
            .all(|spec| current.2.iter().any(|l| l.5.contains(spec)))
        && configuration.iter().all(|expected| {
            current.2.iter().any(|actual| {
                (actual.0, actual.1, actual.3, actual.4)
                    == (expected.0, expected.1, expected.3, expected.4)
                    && (actual.2 - expected.2).abs() < 0.000001
                    && actual.5.len() == expected.5.len()
                    && expected.5.iter().all(|(connector, mode_id, _)| {
                        actual.5.iter().any(|spec| {
                            &spec.0 == connector
                                && current
                                    .1
                                    .iter()
                                    .find(|m| &m.0 == spec)
                                    .and_then(|m| mode(m).ok())
                                    .is_some_and(|m| &m.0 == mode_id)
                        })
                    })
            })
        })
}

fn commit(connection: &Connection, before: &Current, targets: &[Target]) -> ResultType<Vec<State>> {
    let (logicals, props) = batch_configuration(before, targets)?;
    let proxy = connection.with_proxy(BUS, PATH, TIMEOUT);
    let _: () = proxy.method_call(
        BUS,
        "ApplyMonitorsConfig",
        (before.0, 0u32, &logicals, &props),
    )?;
    let _: () = proxy.method_call(
        BUS,
        "ApplyMonitorsConfig",
        (before.0, 1u32, &logicals, props),
    )?;
    let deadline = std::time::Instant::now() + TIMEOUT;
    loop {
        let after = current(connection)?;
        if applied(&after, &logicals, before) {
            return targets
                .iter()
                .map(|target| {
                    let index = after
                        .2
                        .iter()
                        .position(|l| {
                            l.5.len() == 1 && super::token(("gnome", &l.5[0])) == target.identity
                        })
                        .ok_or_else(|| hbb_common::anyhow::anyhow!(STALE))?;
                    state_at(&after, index)
                })
                .collect();
        }
        if std::time::Instant::now() >= deadline {
            bail!("Display settings timed out. Refresh and try again.");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn restore(connection: &Connection, targets: &[Target]) -> ResultType<Vec<State>> {
    commit(connection, &current(connection)?, targets)
}

fn resolution_target(
    current: &Current,
    index: usize,
    resolution: (u32, u32),
) -> ResultType<Target> {
    let state = state_at(current, index)?;
    let logical = &current.2[index];
    let monitor = current
        .1
        .iter()
        .find(|m| m.0 == logical.5[0])
        .ok_or(UNSUPPORTED)?;
    let size = if logical.3 % 2 == 1 {
        (resolution.1, resolution.0)
    } else {
        resolution
    };
    let modes: Vec<_> = monitor
        .1
        .iter()
        .filter(|m| (m.1 as u32, m.2 as u32) == size)
        .collect();
    let percent = if modes
        .iter()
        .any(|m| m.5.iter().any(|s| percent(*s) == Some(state.percent)))
    {
        state.percent
    } else {
        let selected = modes
            .into_iter()
            .max_by(|a, b| a.3.total_cmp(&b.3))
            .ok_or(UNSUPPORTED)?;
        percent(selected.4)
            .filter(|_| selected.5.contains(&selected.4))
            .or_else(|| {
                selected
                    .5
                    .iter()
                    .filter_map(|s| percent(*s))
                    .min_by(f64::total_cmp)
            })
            .ok_or(UNSUPPORTED)?
    };
    Ok(Target {
        identity: state.identity,
        resolution,
        percent,
    })
}

pub fn set_resolution(
    connection: &Connection,
    display: &Display,
    resolution: (u32, u32),
) -> ResultType<State> {
    let before = current(connection)?;
    let (state, index) = state(&before, display)?;
    if state.resolution == resolution {
        return Ok(state);
    }
    let target = resolution_target(&before, index, resolution)?;
    commit(connection, &before, &[target])?
        .into_iter()
        .next()
        .ok_or_else(|| UNSUPPORTED.into())
}

pub fn apply(
    connection: &Connection,
    display: &Display,
    value: f64,
    expected: &str,
) -> ResultType<State> {
    let current = current(connection)?;
    let (state, _) = state(&current, display)?;
    super::validate(&state, value, expected)?;
    if state.percent == value {
        return Ok(state);
    }
    commit(
        connection,
        &current,
        &[Target {
            identity: state.identity.clone(),
            resolution: state.resolution,
            percent: value,
        }],
    )?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configuration(
        current: &Current,
        index: usize,
        scale: f64,
    ) -> ResultType<(Configuration, PropMap)> {
        let state = state_at(current, index)?;
        batch_configuration(
            current,
            &[Target {
                identity: state.identity,
                resolution: state.resolution,
                percent: scale * 100.0,
            }],
        )
    }

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
        assert!(confirmed(&current, Some(&display), &before.identity)
            .unwrap_err()
            .is::<crate::platform::display_scale::Unsupported>());
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
        let mut current = fixture();
        let (config, _) = configuration(&current, 0, 1.5).unwrap();
        assert_eq!(config[0].2, 1.5);
        assert_eq!((config[1].0, config[1].1, config[1].2), (2560, 0, 2.0));
        assert!(config[0].4);
        assert_eq!(config[0].5[0].1, "3840x2160@60");
        assert!(flag(&config[0].5[0].2, "underscanning"));
        assert_eq!(config[0].5[0].2["color-mode"].0.as_u64(), Some(1));
        assert_eq!(current.2[1].0, 1920);

        let mut extra = fixture();
        let mut monitor = extra.1.remove(0);
        monitor.0 .0 = "HDMI-1".into();
        monitor.0 .3 = "third".into();
        current.2.push((
            0,
            1080,
            2.0,
            0,
            false,
            vec![monitor.0.clone()],
            PropMap::new(),
        ));
        current.1.push(monitor);
        current.1[1].1[0].2 = 4320;
        current.1[0].1[0].5.push(3.0);
        let (config, _) = configuration(&current, 0, 3.0).unwrap();
        assert_eq!((config[1].0, config[1].1), (1920, 0));
        assert_eq!((config[2].0, config[2].1), (0, 720));
        let mut changed_mode = fixture();
        changed_mode.1[0].1.push((
            "2560x1440@60".into(),
            2560,
            1440,
            60.0,
            1.0,
            vec![1.0],
            PropMap::new(),
        ));
        let target = resolution_target(&changed_mode, 0, (2560, 1440)).unwrap();
        assert_eq!(target.percent, 100.0);
        let (config, _) = batch_configuration(&changed_mode, &[target]).unwrap();
        assert_eq!(config[0].5[0].1, "2560x1440@60");
        assert_eq!((config[0].2, config[1].2), (1.0, 2.0));
        let original = state_at(&changed_mode, 0).unwrap();
        assert!(batch_configuration(
            &changed_mode,
            &[Target {
                identity: original.identity,
                resolution: (2560, 1440),
                percent: 200.0,
            }]
        )
        .is_err());
    }

    #[test]
    fn refuses_global_scale_on_multiple_displays_and_mirrored_target() {
        let mut current = fixture();
        let display = Display {
            name: "eDP-1".into(),
            origin: (0, 0),
            size: (3840, 2160),
        };
        let identity = state(&current, &display).unwrap().0.identity;
        let capture = Display {
            name: String::new(),
            origin: (-1, 0),
            size: display.size,
        };
        current
            .3
            .insert("global-scale-required".into(), Variant(Box::new(true)));
        assert!(state(&current, &display).is_err());
        for capture in [Some(&capture), None] {
            assert!(confirmed(&current, capture, &identity)
                .unwrap_err()
                .is::<crate::platform::display_scale::Unsupported>());
        }
        current.3.clear();
        current.2[0].5.push(current.1[1].0.clone());
        assert!(state(&current, &display).is_err());
        for capture in [Some(&capture), None] {
            assert!(confirmed(&current, capture, &identity)
                .unwrap_err()
                .is::<crate::platform::display_scale::Unsupported>());
        }
    }

    #[test]
    fn native_identity_survives_mode_changes_but_not_monitor_replacement() {
        let mut current = fixture();
        let display = Display {
            name: "eDP-1".into(),
            origin: (0, 0),
            size: (3840, 2160),
        };
        let before = state(&current, &display).unwrap().0;
        assert_eq!(before.resolution, (3840, 2160));
        current.1[0].1[0].1 = 2560;
        current.1[0].1[0].2 = 1440;
        current.2[0].2 = 1.5;
        current.2[0].3 = 1;
        let after = state(&current, &display).unwrap().0;
        assert_eq!(after.identity, before.identity);
        assert_eq!(after.resolution, (1440, 2560));
        assert_eq!(
            confirmed(&current, Some(&display), &before.identity)
                .unwrap()
                .resolution,
            after.resolution
        );
        let mut capture = Display {
            name: String::new(),
            origin: display.origin,
            size: display.size,
        };
        for capture in [Some(&capture), None] {
            assert!(confirmed(&current, capture, &before.identity)
                .unwrap_err()
                .is::<SnapshotChanged>());
        }
        capture.size = (1440, 2560);
        assert!(confirmed(&current, Some(&capture), &before.identity).is_ok());
        capture.origin = (1920, 0);
        capture.size = (3840, 2160);
        for name in ["", "missing"] {
            capture.name = name.into();
            assert_eq!(
                confirmed(&current, Some(&capture), &before.identity)
                    .unwrap_err()
                    .to_string(),
                STALE
            );
        }
        let removed = current.2.remove(0);
        for capture in [Some(&display), None] {
            assert_eq!(
                confirmed(&current, capture, &before.identity)
                    .unwrap_err()
                    .to_string(),
                STALE
            );
        }
        current.2.insert(0, removed);
        current.1[0].0.3 = "replacement".into();
        current.2[0].5[0] = current.1[0].0.clone();
        assert_ne!(state(&current, &display).unwrap().0.identity, after.identity);
        for capture in [Some(&display), None] {
            assert_eq!(
                confirmed(&current, capture, &before.identity)
                    .unwrap_err()
                    .to_string(),
                STALE
            );
        }
    }
}
