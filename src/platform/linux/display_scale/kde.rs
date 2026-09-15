use super::{percent, Display, State, UNSUPPORTED};
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
        let size = if matches!(output["rotation"].as_u64(), Some(2 | 8)) {
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
            matches.push(output);
        }
    }
    if matches.len() != 1 {
        bail!(UNSUPPORTED);
    }
    let output = matches[0];
    let Some(id) = output["id"].as_i64().filter(|id| *id > 0) else {
        bail!(UNSUPPORTED);
    };
    if output["replicationSource"].as_i64().unwrap_or(0) != 0
        || outputs
            .iter()
            .any(|o| o["replicationSource"].as_i64() == Some(id))
    {
        bail!(UNSUPPORTED);
    }
    let Some(value) = output["scale"].as_f64().and_then(percent) else {
        bail!(UNSUPPORTED);
    };
    // Match Plasma's native scale slider. X11's global font DPI is not per-output scaling.
    let mut options: Vec<_> = (50..=300).step_by(25).map(f64::from).collect();
    options.push(value);
    options.sort_by(f64::total_cmp);
    options.dedup();
    topology.sort_unstable();
    Ok((
        State {
            percent: value,
            custom: Some([50.0, 300.0, 100.0 / 120.0]),
            recommended: None,
            options,
            token: super::token((id, topology)),
        },
        id,
    ))
}

fn arguments(data: &Value, id: i64, percent: f64) -> ResultType<Vec<String>> {
    let Some(outputs) = data["outputs"].as_array() else {
        bail!(UNSUPPORTED);
    };
    let Some(target) = outputs.iter().find(|o| o["id"].as_i64() == Some(id)) else {
        bail!(UNSUPPORTED);
    };
    let Some(mode) = target["modes"]
        .as_array()
        .and_then(|modes| modes.iter().find(|m| m["id"] == target["currentModeId"]))
    else {
        bail!(UNSUPPORTED);
    };
    let (Some(w), Some(h), Some(x), Some(y), Some(scale)) = (
        mode["size"]["width"].as_f64(),
        mode["size"]["height"].as_f64(),
        target["pos"]["x"].as_i64(),
        target["pos"]["y"].as_i64(),
        target["scale"].as_f64(),
    ) else {
        bail!(UNSUPPORTED);
    };
    let (w, h) = if matches!(target["rotation"].as_u64(), Some(2 | 8)) {
        (h, w)
    } else {
        (w, h)
    };
    let old = ((w / scale).round() as i64, (h / scale).round() as i64);
    let new = (
        (w * 100.0 / percent).round() as i64,
        (h * 100.0 / percent).round() as i64,
    );
    let mut args = vec![format!("output.{id}.scale.{}", percent / 100.0)];
    for output in outputs {
        let (Some(other), Some(ox), Some(oy)) = (
            output["id"].as_i64(),
            output["pos"]["x"].as_i64(),
            output["pos"]["y"].as_i64(),
        ) else {
            continue;
        };
        if other == id
            || output["connected"] != true
            || output["enabled"] == false
            || output["replicationSource"].as_i64().unwrap_or(0) != 0
        {
            continue;
        }
        let nx = ox + if ox >= x + old.0 { new.0 - old.0 } else { 0 };
        let ny = oy + if oy >= y + old.1 { new.1 - old.1 } else { 0 };
        if (ox, oy) != (nx, ny) {
            args.push(format!("output.{other}.position.{nx},{ny}"));
        }
    }
    Ok(args)
}

pub fn read(display: &Display) -> ResultType<State> {
    Ok(snapshot(&serde_json::from_slice(&doctor(&["--json"])?)?, display)?.0)
}

pub fn apply(display: &Display, percent: f64, expected: &str) -> ResultType<()> {
    let data = serde_json::from_slice(&doctor(&["--json"])?)?;
    let (state, id) = snapshot(&data, display)?;
    super::validate(&state, percent, expected)?;
    if state.percent == percent {
        return Ok(());
    }
    let args = arguments(&data, id, percent)?;
    doctor(&args.iter().map(String::as_str).collect::<Vec<_>>())?;
    // Some kscreen-doctor versions return exit code 0 when applying fails.
    // The caller reads the compositor state again before reporting success.
    Ok(())
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
        let display = Display {
            name: String::new(),
            origin: (0, 0),
            size: (3840, 2160),
        };
        assert!(snapshot(&json!({"outputs": [output(1, 0), output(2, 0)]}), &display).is_err());
        let mut mirror = output(2, 2560);
        mirror["replicationSource"] = json!(1);
        assert!(snapshot(&json!({"outputs": [output(1, 0), mirror]}), &display).is_err());
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
    }
}
