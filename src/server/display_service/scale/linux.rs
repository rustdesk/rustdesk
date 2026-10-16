use super::{Disabled, OriginalScale};
use crate::platform::display_scale::{self, Display, State, Target};
use base::config::keys;
use hbb_common::{bail, config::Config, throttled_log, ResultType};
use std::{collections::HashMap, time::Duration};

pub(super) fn change_resolution(
    display: &Display,
    name: &str,
    original: (i32, i32),
    requested: (i32, i32),
) {
    // Record before applying so a failed native readback still participates in cleanup.
    super::super::set_last_changed_resolution(name, original, requested);
    if let Err(error) = display_scale::set_resolution(display, (requested.0 as _, requested.1 as _))
    {
        throttled_log!(
            Duration::from_secs(60),
            error,
            "Failed to change resolution '{}' to ({},{}): {}",
            name,
            requested.0,
            requested.1,
            error
        );
    }
}

pub(super) fn restore_one(
    current: State,
    original: &OriginalScale,
    enabled: bool,
) -> ResultType<State> {
    if current.resolution == original.resolution
        && (current.percent - original.percent).abs() < 0.000001
    {
        return Ok(current);
    }
    if !enabled || !Config::get_bool_option(keys::OPTION_ALLOW_DISPLAY_SCALING) {
        bail!(Disabled);
    }
    display_scale::restore(&[target(&current.identity, original)])?
        .into_iter()
        .find(|state| state.identity == current.identity)
        .ok_or_else(|| hbb_common::anyhow::anyhow!(display_scale::STALE))
}

fn target(identity: &str, original: &OriginalScale) -> Target {
    Target {
        identity: identity.to_owned(),
        resolution: original.resolution,
        percent: original.percent,
    }
}

pub(super) fn restore(originals: &HashMap<String, OriginalScale>) {
    if originals.is_empty() {
        return;
    }
    let targets: Vec<_> = originals
        .iter()
        .map(|(identity, original)| target(identity, original))
        .collect();
    if let Err(error) = display_scale::restore(&targets) {
        throttled_log!(
            Duration::from_secs(60),
            warn,
            "Could not restore original display settings: {error}"
        );
    }
    // A separate resolution-only pass would reintroduce invalid intermediate layouts.
    let mut resolutions = super::super::CHANGED_RESOLUTIONS.write().unwrap();
    for original in originals.values() {
        resolutions.remove(&original.display_name);
    }
}
