use hbb_common::{bail, ResultType};
use serde::Serialize;
use std::hash::{Hash, Hasher};

#[cfg(target_os = "linux")]
use super::linux::display_scale as backend;
#[cfg(windows)]
use super::windows::display_scale as backend;

pub const UNSUPPORTED: &str =
    "System scaling is unavailable for this display or desktop environment.";
pub const STALE: &str = "Display settings changed. Reopen the resolution menu and try again.";

#[derive(Debug, Serialize)]
pub struct State {
    pub identity: String,
    pub resolution: (u32, u32),
    pub percent: f64,
    pub recommended: Option<f64>,
    pub options: Vec<f64>,
    pub token: String,
    pub custom: Option<[f64; 3]>,
}

pub struct Display {
    pub name: String,
    pub origin: (i32, i32),
    pub size: (usize, usize),
}

// Detect topology, mode and scale changes between opening the editor and applying.
pub(super) fn token(value: impl Hash) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hash);
    format!("{:016x}", hash.finish())
}

pub fn configure(display: &Display, percent: f64, expected: &str) -> ResultType<State> {
    if !percent.is_finite() || (percent != 0.0 && !(50.0..=500.0).contains(&percent)) {
        bail!("Invalid display scaling request.");
    }
    if percent == 0.0 {
        return backend::read(display);
    }
    let before = backend::apply(display, percent, expected)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let after = backend::read(display);
        if let Ok(state) = &after {
            if state.identity != before.identity || state.resolution != before.resolution {
                bail!(STALE);
            }
            if (state.percent - percent).abs() < 0.000001 {
                return after;
            }
        }
        if std::time::Instant::now() >= deadline {
            after?;
            bail!("The system did not apply the requested scale. Refresh and try again.");
        }
        // Runs only on the display-settings blocking worker. Native scale notifications
        // may arrive after the operating system's setter has returned.
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

pub(super) fn validate(state: &State, percent: f64, expected: &str) -> ResultType<()> {
    if expected.is_empty() || expected != state.token {
        bail!(STALE);
    }
    let custom = state.custom.is_some_and(|[min, max, step]| {
        percent.is_finite()
            && percent >= min
            && percent <= max
            && ((percent / step).round() * step - percent).abs() < 0.000001
    });
    if !state.options.contains(&percent) && !custom {
        bail!("Select a scaling level supported by the display.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_percentages_are_rejected_before_accessing_the_desktop() {
        let display = Display {
            name: String::new(),
            origin: (0, 0),
            size: (0, 0),
        };
        for percent in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            49.0,
            501.0,
        ] {
            let error = configure(&display, percent, "").unwrap_err();
            assert_eq!(error.to_string(), "Invalid display scaling request.");
        }
    }

    #[test]
    fn decimal_levels_and_native_steps_are_not_rounded_to_integers() {
        let mut state = State {
            identity: "display-1".into(),
            resolution: (3840, 2160),
            percent: 125.5,
            recommended: None,
            options: vec![100.0, 125.5],
            token: "fractional".into(),
            custom: None,
        };
        assert!(validate(&state, 125.5, "fractional").is_ok());
        assert!(validate(&state, 126.0, "fractional").is_err());
        state.custom = Some([50.0, 300.0, 100.0 / 120.0]);
        assert!(validate(&state, 151.0 / 120.0 * 100.0, "fractional").is_ok());
        for value in [125.6, 49.0, 301.0, f64::NAN, f64::INFINITY] {
            assert!(validate(&state, value, "fractional").is_err());
        }
    }

    #[test]
    fn rejects_stale_and_unadvertised_changes() {
        let state = State {
            identity: "display-1".into(),
            resolution: (3840, 2160),
            percent: 150.0,
            recommended: Some(150.0),
            options: vec![100.0, 125.0, 150.0],
            token: "current".into(),
            custom: None,
        };
        assert!(validate(&state, 125.0, "current").is_ok());
        assert!(validate(&state, 125.0, "").is_err());
        assert!(validate(&state, 125.0, "previous").is_err());
        assert!(validate(&state, 137.0, "current").is_err());
        assert!(validate(&state, 0.0, "current").is_err());
    }
}
