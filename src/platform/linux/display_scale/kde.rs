use super::{layout, percent, Display, State, UNSUPPORTED};
use hbb_common::{allow_err, bail, log, ResultType};
use serde_json::Value;
use std::{
    io::Read,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

fn stop(child: &mut Child) {
    allow_err!(child.kill());
    allow_err!(child.wait());
}

fn doctor(args: &[&str]) -> ResultType<Vec<u8>> {
    let mut child = Command::new("kscreen-doctor")
        .args(args)
        .env("QT_QPA_PLATFORM", "wayland")
        // Isolate KScreen's backend to avoid Qt Wayland connection teardown crashes.
        .env("KSCREEN_BACKEND_INPROCESS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let Some(stdout) = child.stdout.take() else {
        stop(&mut child);
        bail!(UNSUPPORTED);
    };
    let (send, receive) = mpsc::sync_channel(1);
    let reader = std::thread::Builder::new()
        .name("display-scale-output".into())
        .spawn(move || {
            let mut data = Vec::new();
            let result = stdout
                .take(1024 * 1024)
                .read_to_end(&mut data)
                .map(|_| data);
            let _ = send.send(result); // The receiver is dropped after a timeout.
        });
    if let Err(error) = reader {
        stop(&mut child);
        return Err(error.into());
    }
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    bail!("Failed to change display settings: kscreen-doctor ({status})");
                }
                let output =
                    receive.recv_timeout(deadline.saturating_duration_since(Instant::now()))??;
                if output.len() >= 1024 * 1024 {
                    bail!("Display configuration response is too large.");
                }
                return Ok(output);
            }
            Err(error) => {
                stop(&mut child);
                return Err(error.into());
            }
            Ok(None) if Instant::now() >= deadline => {
                stop(&mut child);
                bail!("Display settings timed out. Refresh and try again.");
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn snapshot(data: &Value, display: &Display) -> ResultType<(State, i64)> {
    let Some(outputs) = data["outputs"].as_array() else {
        bail!(UNSUPPORTED);
    };
    let mut matches = Vec::new();
    let mut topology = Vec::new();
    for output in outputs {
        topology.push(
            serde_json::json!([
                output["id"],
                output["name"],
                output["connected"],
                output["enabled"],
                output["pos"],
                output["currentModeId"],
                output["scale"],
                output["rotation"],
                output["replicationSource"],
                output["size"],
                output["sizeMM"],
                output["modes"].as_array().and_then(|modes| modes
                    .iter()
                    .find(|mode| mode["id"] == output["currentModeId"]))
            ])
            .to_string(),
        );
        if output["connected"] != true || output["enabled"] == false {
            continue;
        }
        let Some(modes) = output["modes"].as_array() else {
            continue;
        };
        let Some(mode) = modes.iter().find(|m| m["id"] == output["currentModeId"]) else {
            continue;
        };
        let size = (
            mode["size"]["width"].as_u64(),
            mode["size"]["height"].as_u64(),
        );
        let size = if matches!(output["rotation"].as_u64(), Some(2 | 8 | 32 | 128)) {
            (size.1, size.0)
        } else {
            size
        };
        if output["name"]
            .as_str()
            .is_some_and(|name| super::same_connector(&display.name, name))
            || (display.name.is_empty()
                && output["pos"]["x"].as_i64() == Some(display.origin.0 as i64)
                && output["pos"]["y"].as_i64() == Some(display.origin.1 as i64)
                && size == (Some(display.size.0 as u64), Some(display.size.1 as u64)))
        {
            matches.push((output, size));
        }
    }
    if matches.len() != 1 {
        bail!(UNSUPPORTED);
    }
    let (output, size) = matches[0];
    let Some(id) = output["id"].as_i64().filter(|id| *id > 0) else {
        bail!(UNSUPPORTED);
    };
    let Some(name) = output["name"].as_str().filter(|name| !name.is_empty()) else {
        bail!(UNSUPPORTED);
    };
    let (Some(width), Some(height)) = size else {
        bail!(UNSUPPORTED);
    };
    if width == 0 || height == 0 || width > u32::MAX as u64 || height > u32::MAX as u64 {
        bail!(UNSUPPORTED);
    }
    if output["replicationSource"].as_i64().unwrap_or(0) != 0
        || outputs.iter().any(|o| {
            o["replicationSource"].as_i64() == Some(id)
                || (o["id"] != output["id"]
                    && o["connected"] == true
                    && o["enabled"] != false
                    && o["pos"] == output["pos"])
        })
    {
        bail!(UNSUPPORTED);
    }
    let Some(value) = output["scale"].as_f64().and_then(percent) else {
        bail!(UNSUPPORTED);
    };
    // Capture and input coordinates do not support scales below 100%. Keep an
    // existing lower value readable so it can be raised to a supported scale.
    let mut options: Vec<_> = (100..=300).step_by(25).map(f64::from).collect();
    options.push(value);
    options.sort_by(f64::total_cmp);
    options.dedup();
    topology.sort_unstable();
    Ok((
        State {
            identity: super::token(("kde", id, name)),
            resolution: (width as u32, height as u32),
            percent: value,
            custom: Some([100.0, 300.0, 100.0 / 120.0]),
            recommended: None,
            options,
            token: super::token((id, topology)),
        },
        id,
    ))
}

fn logical_size(output: &Value, scale: f64) -> ResultType<(i32, i32)> {
    let Some(mode) = output["modes"]
        .as_array()
        .and_then(|modes| modes.iter().find(|m| m["id"] == output["currentModeId"]))
    else {
        bail!(UNSUPPORTED);
    };
    let (Some(w), Some(h), Some(rotation)) = (
        mode["size"]["width"].as_f64(),
        mode["size"]["height"].as_f64(),
        output["rotation"].as_u64(),
    ) else {
        bail!(UNSUPPORTED);
    };
    if !matches!(rotation, 1 | 2 | 4 | 8 | 16 | 32 | 64 | 128)
        || !scale.is_finite()
        || scale <= 0.0
    {
        bail!(UNSUPPORTED);
    }
    let (w, h) = if matches!(rotation, 2 | 8 | 32 | 128) {
        (h, w)
    } else {
        (w, h)
    };
    let (w, h) = ((w / scale).round(), (h / scale).round());
    if !(1.0..=i32::MAX as f64).contains(&w) || !(1.0..=i32::MAX as f64).contains(&h) {
        bail!(UNSUPPORTED);
    }
    Ok((w as i32, h as i32))
}

fn arguments(data: &Value, id: i64, percent: f64) -> ResultType<Vec<String>> {
    let Some(outputs) = data["outputs"].as_array() else {
        bail!(UNSUPPORTED);
    };
    let active: Vec<_> = outputs
        .iter()
        .filter(|o| o["connected"] == true && o["enabled"] != false)
        .collect();
    let mut ids: Vec<Vec<i64>> = Vec::new();
    let mut rects: Vec<layout::Rect> = Vec::new();
    for output in &active {
        let source = output["replicationSource"].as_i64().unwrap_or(0);
        if source != 0 {
            if !active.iter().any(|o| {
                o["id"].as_i64() == Some(source)
                    && o["replicationSource"].as_i64().unwrap_or(0) == 0
            }) {
                bail!(UNSUPPORTED);
            }
            continue;
        }
        let (Some(other), Some(x), Some(y), Some(scale)) = (
            output["id"].as_i64(),
            output["pos"]["x"]
                .as_i64()
                .and_then(|v| i32::try_from(v).ok()),
            output["pos"]["y"]
                .as_i64()
                .and_then(|v| i32::try_from(v).ok()),
            output["scale"].as_f64(),
        ) else {
            bail!(UNSUPPORTED);
        };
        if other <= 0 || ids.iter().any(|group| group.contains(&other)) {
            bail!(UNSUPPORTED);
        }
        let (width, height) = logical_size(output, scale)?;
        // Older KScreen backends represent cloned outputs only by equal geometry.
        if let Some(index) = rects
            .iter()
            .position(|r| (r.x, r.y, r.width, r.height) == (x, y, width, height))
        {
            ids[index].push(other);
        } else {
            ids.push(vec![other]);
            rects.push(layout::Rect {
                x,
                y,
                width,
                height,
            });
        }
    }
    let Some(index) = ids.iter().position(|group| group.contains(&id)) else {
        bail!(UNSUPPORTED);
    };
    if ids[index].len() != 1 {
        bail!(UNSUPPORTED);
    }
    let Some(target) = active.iter().find(|o| o["id"].as_i64() == Some(id)) else {
        bail!(UNSUPPORTED);
    };
    // Match KWin's scale quantization before rounding logical dimensions.
    let scale = (percent / 100.0 * 120.0).round() / 120.0;
    let next = layout::resize(&rects, index, logical_size(target, scale)?)?;
    let mut args = vec![format!("output.{id}.scale.{scale}")];
    for ((group, before), after) in ids.iter().zip(&rects).zip(next) {
        if (before.x, before.y) != (after.x, after.y) {
            for other in group {
                args.push(format!("output.{other}.position.{},{}", after.x, after.y));
            }
        }
    }
    Ok(args)
}

pub fn read(display: &Display) -> ResultType<State> {
    Ok(snapshot(&serde_json::from_slice(&doctor(&["--json"])?)?, display)?.0)
}

pub fn apply(display: &Display, percent: f64, expected: &str) -> ResultType<State> {
    let data = serde_json::from_slice(&doctor(&["--json"])?)?;
    let (state, id) = snapshot(&data, display)?;
    super::validate(&state, percent, expected)?;
    if state.percent == percent {
        return Ok(state);
    }
    let args = arguments(&data, id, percent)?;
    doctor(&args.iter().map(String::as_str).collect::<Vec<_>>())?;
    // Some kscreen-doctor versions return exit code 0 when applying fails.
    // The caller reads the compositor state again before reporting success.
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn output(id: u32, x: i32) -> Value {
        json!({"id": id, "name": format!("DP-{id}"), "connected": true, "enabled": true,
            "pos": {"x": x, "y": 0}, "scale": 1.5, "rotation": 1, "replicationSource": 0,
            "currentModeId": "mode", "modes": [{"id": "mode", "size": {"width": 3840, "height": 2160}}]})
    }

    #[test]
    fn lower_current_scales_can_recover_without_allowing_new_sub_one_scales() {
        let display = Display {
            name: "DP-1".into(),
            origin: (0, 0),
            size: (3840, 2160),
        };
        let mut data = json!({"outputs": [output(1, 0)]});
        data["outputs"][0]["scale"] = json!(0.75);
        let (before, _) = snapshot(&data, &display).unwrap();
        assert_eq!(before.percent, 75.0);
        assert!(before.options.contains(&75.0));
        assert!(before.options.iter().all(|p| *p >= 100.0 || *p == 75.0));
        assert!(super::super::validate(&before, 99.16666666666667, &before.token).is_err());
        assert!(super::super::validate(&before, 100.0, &before.token).is_ok());
        assert!(super::super::validate(&before, 151.0 / 120.0 * 100.0, &before.token).is_ok());

        data["outputs"][0]["scale"] = json!(1.0);
        let (after, _) = snapshot(&data, &display).unwrap();
        assert!(after.options.iter().all(|p| *p >= 100.0));
        assert!(super::super::validate(&after, 75.0, &after.token).is_err());
    }

    #[test]
    fn reused_mode_ids_do_not_hide_geometry_or_refresh_changes() {
        let display = Display {
            name: "DP-1".into(),
            origin: (0, 0),
            size: (3840, 2160),
        };
        let mut output = output(1, 0);
        let before = snapshot(&json!({"outputs": [output.clone()]}), &display)
            .unwrap()
            .0;
        output["modes"][0]["size"]["width"] = json!(2560);
        let resized = snapshot(&json!({"outputs": [output.clone()]}), &display)
            .unwrap()
            .0;
        assert_ne!(before.token, resized.token);
        output["modes"][0]["refreshRate"] = json!(120);
        let refreshed = snapshot(&json!({"outputs": [output]}), &display).unwrap().0;
        assert_ne!(resized.token, refreshed.token);
    }

    #[test]
    fn maps_physical_mode_and_detects_reordered_topology_without_false_staleness() {
        let display = Display {
            name: String::new(),
            origin: (2560, 0),
            size: (3840, 2160),
        };
        let a = output(1, 0);
        let b = output(2, 2560);
        let (before, id) = snapshot(&json!({"outputs": [a, b]}), &display).unwrap();
        assert_eq!(id, 2);
        assert_eq!(before.percent, 150.0);
        assert!(before.options.contains(&125.0));
        assert_eq!(before.recommended, None);
        let mut other = output(1, 0);
        let b = output(2, 2560);
        assert_eq!(
            before.token,
            snapshot(&json!({"outputs": [b, other]}), &display)
                .unwrap()
                .0
                .token
        );
        other["scale"] = json!(2.0);
        assert_ne!(
            before.token,
            snapshot(&json!({"outputs": [b, other]}), &display)
                .unwrap()
                .0
                .token
        );
    }

    #[test]
    fn rejects_ambiguous_capture_and_replication() {
        let mut display = Display {
            name: String::new(),
            origin: (0, 0),
            size: (3840, 2160),
        };
        assert!(snapshot(&json!({"outputs": [output(1, 0), output(2, 0)]}), &display).is_err());
        let mut mirror = output(2, 2560);
        mirror["replicationSource"] = json!(1);
        assert!(snapshot(&json!({"outputs": [output(1, 0), mirror]}), &display).is_err());

        let mut data = json!({"outputs": [output(1, 0), output(2, 0), output(3, 2560)]});
        display.name = "DP-1".into();
        assert!(snapshot(&data, &display)
            .unwrap_err()
            .is::<crate::platform::display_scale::Unsupported>());
        for key in ["enabled", "connected"] {
            data["outputs"][1][key] = json!(false);
            assert!(snapshot(&data, &display).is_ok());
            data["outputs"][1][key] = json!(true);
        }
        display.name = "DP-3".into();
        assert!(snapshot(&data, &display).is_ok());
    }

    #[test]
    fn capture_identity_survives_an_adjacent_monitor_scale_change() {
        let display = Display {
            name: String::new(),
            origin: (2560, 0),
            size: (3840, 2160),
        };
        let captured = base::platform::linux::WaylandDisplayInfo {
            name: "DP-2".into(),
            x: 2560,
            y: 0,
            width: 3840,
            height: 2160,
            logical_size: Some((2560, 1440)),
            refresh_rate: 60,
            transform: 0,
        };
        let mut first = output(1, 0);
        first["scale"] = json!(2.0);
        let current = json!({"outputs": [first, output(2, 1920)]});
        assert!(snapshot(&current, &display).is_err());
        let resolved = super::super::captured_display(&display, &[captured])
            .unwrap()
            .unwrap();
        let (state, id) = snapshot(&current, &resolved).unwrap();
        assert_eq!(id, 2);
        assert_eq!(state.percent, 150.0);
        assert_eq!(
            arguments(&current, id, 200.0).unwrap(),
            ["output.2.scale.2"]
        );
    }

    #[test]
    fn scale_and_adjacent_positions_are_applied_as_one_configuration() {
        let args = arguments(
            &json!({"outputs": [output(1, 0), output(2, 2560)]}),
            1,
            200.0,
        )
        .unwrap();
        assert_eq!(args, ["output.1.scale.2", "output.2.position.1920,0"]);
        let args = arguments(
            &json!({"outputs": [output(1, 0), output(2, 2560)]}),
            2,
            200.0,
        )
        .unwrap();
        assert_eq!(args, ["output.2.scale.2"]);

        let mut below = output(3, 0);
        below["pos"]["y"] = json!(1440);
        below["modes"][0]["size"]["width"] = json!(7680);
        let mut replica = below.clone();
        replica["id"] = json!(4);
        replica["replicationSource"] = json!(3);
        let mut data = json!({"outputs": [output(1, 0), output(2, 2560), below, replica]});
        assert_eq!(
            arguments(&data, 1, 200.0).unwrap(),
            ["output.1.scale.2", "output.2.position.1920,0"]
        );
        data["outputs"][1]["currentModeId"] = json!("missing");
        assert!(arguments(&data, 1, 200.0).is_err());

        let mut data = json!({"outputs": [output(1, 0), output(2, 2560), output(3, 2560)]});
        assert_eq!(
            arguments(&data, 1, 200.0).unwrap(),
            [
                "output.1.scale.2",
                "output.2.position.1920,0",
                "output.3.position.1920,0"
            ]
        );
        assert!(arguments(&data, 2, 200.0).is_err());
        data["outputs"][0]["pos"]["x"] = json!(2560);
        data["outputs"][1]["pos"]["x"] = json!(0);
        data["outputs"][2]["pos"]["x"] = json!(0);
        assert_eq!(arguments(&data, 1, 200.0).unwrap(), ["output.1.scale.2"]);
        data["outputs"][2]["pos"]["x"] = json!(1);
        assert!(arguments(&data, 1, 200.0).is_err());

        let mut target = output(1, 0);
        target["modes"][0]["size"] = json!({"width": 1922, "height": 1080});
        target["scale"] = json!(1.0);
        assert_eq!(
            arguments(
                &json!({"outputs": [target, output(2, 1922)]}),
                1,
                160.0 * (100.0 / 120.0)
            )
            .unwrap(),
            [
                "output.1.scale.1.3333333333333333",
                "output.2.position.1442,0"
            ]
        );
    }

    #[test]
    fn native_identity_survives_mode_changes_but_not_output_replacement() {
        let display = Display {
            name: "DP-1".into(),
            origin: (0, 0),
            size: (3840, 2160),
        };
        let mut output = output(1, 0);
        let before = snapshot(&json!({"outputs": [output.clone()]}), &display)
            .unwrap()
            .0;
        assert_eq!(before.resolution, (3840, 2160));
        output["modes"][0]["size"] = json!({"width": 2560, "height": 1440});
        output["scale"] = json!(2.0);
        for rotation in [2, 32, 128] {
            output["rotation"] = json!(rotation);
            let after = snapshot(&json!({"outputs": [output.clone()]}), &display)
                .unwrap()
                .0;
            assert_eq!(after.identity, before.identity);
            assert_eq!(after.resolution, (1440, 2560));
            for (x, y) in [(1440, 0), (0, 2560)] {
                let mut adjacent = output.clone();
                adjacent["id"] = json!(2);
                adjacent["name"] = json!("DP-2");
                adjacent["pos"] = json!({"x": x / 2, "y": y / 2});
                let args =
                    arguments(&json!({"outputs": [output.clone(), adjacent]}), 1, 100.0).unwrap();
                assert_eq!(
                    args,
                    ["output.1.scale.1", &format!("output.2.position.{x},{y}")]
                );
            }
        }
        output["id"] = json!(42);
        assert_ne!(
            snapshot(&json!({"outputs": [output.clone()]}), &display)
                .unwrap()
                .0
                .identity,
            before.identity
        );
    }
}
