use hbb_common::{anyhow::Context, bail, log, ResultType};
use std::{
    cell::{Cell, RefCell},
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use x11rb::{protocol::xproto::ConnectionExt, rust_connection::RustConnection, NONE};

mod xsettings;

#[cfg(test)]
mod x11_tests;

thread_local! {
    static SETTINGS: RefCell<Option<(RustConnection, usize)>> = const { RefCell::new(None) };
    static X11_SCALE: Cell<Option<f64>> = const { Cell::new(Some(0.0)) };
}

pub(super) fn cache_id(id: u64, scale: f64) -> u64 {
    // Legacy Web decoders require JS-safe integers; zero is the service's initial ID.
    const MAX_CURSOR_ID: u64 = (1 << 53) - 1;
    if scale == 0.0 {
        return id;
    }
    let mut hash = DefaultHasher::new();
    (id, scale.to_bits()).hash(&mut hash);
    hash.finish() % MAX_CURSOR_ID + 1
}

pub(super) fn x11_cursor_id(id: u64) -> u64 {
    let scale = X11_SCALE.with(|last| match x11_scale() {
        Ok(scale) => {
            last.set(Some(scale));
            scale
        }
        Err(err) => {
            // XSETTINGS is optional; warn once per failure streak without
            // turning valid XFixes cursor updates into service errors.
            if last.replace(None).is_some() {
                log::warn!("Failed to read XSETTINGS cursor density; using unknown density: {err}");
            }
            0.0
        }
    });
    cache_id(id, scale)
}

pub(super) fn x11_cursor_scale() -> f64 {
    // The cursor service reads the ID and bitmap on the same thread. Reuse
    // that poll's density even if the settings manager changes between them.
    X11_SCALE.with(|last| last.get().unwrap_or(0.0))
}

pub(super) fn x11_scale() -> ResultType<f64> {
    if !super::is_x11() {
        return Ok(0.0);
    }
    SETTINGS.with(|settings| {
        let mut state = settings.try_borrow_mut()?;
        if state.is_none() {
            *state = Some(x11rb::connect(None)?);
        }
        let (connection, screen) = state.as_ref().context("Missing XSETTINGS connection")?;
        let result = read_settings(connection, *screen);
        if result.is_err() {
            *state = None;
        }
        result
    })
}

fn read_settings(connection: &RustConnection, screen: usize) -> ResultType<f64> {
    let selection = connection
        .intern_atom(true, format!("_XSETTINGS_S{screen}").as_bytes())?
        .reply()?
        .atom;
    if selection == NONE {
        return Ok(0.0);
    }
    let owner = connection.get_selection_owner(selection)?.reply()?.owner;
    if owner == NONE {
        return Ok(0.0);
    }
    let property = connection
        .intern_atom(true, b"_XSETTINGS_SETTINGS")?
        .reply()?
        .atom;
    let reply = connection
        .get_property(false, owner, property, property, 0, u32::MAX)?
        .reply()?;
    if reply.format != 8 || reply.bytes_after != 0 {
        bail!("Incomplete XSETTINGS property");
    }
    // Xft/DPI includes text scaling; it is not the cursor's pixel density.
    // Zero explicitly keeps the existing policy on desktops without a window scale.
    Ok(xsettings::scale(&reply.value)?.unwrap_or(0.0))
}

#[cfg(feature = "drm")]
pub(super) fn drm_snapshot<T>(
    f: impl Fn(&crate::server::drm_capturer::DrmCursorData) -> T,
) -> ResultType<Option<(T, f64)>> {
    crate::server::drm_capturer::drm_cursor_snapshot(f)
        .map(|(cursor, display)| {
            // A hidden cursor or an unavailable display probe has no density metadata.
            let scale = display
                .as_ref()
                .map(wayland_scale)
                .transpose()?
                .unwrap_or(0.0);
            Ok((cursor, scale))
        })
        .transpose()
}

#[cfg(feature = "drm")]
fn wayland_scale(display: &base::platform::linux::WaylandDisplayInfo) -> ResultType<f64> {
    // Missing logical geometry means unknown density, as with older senders.
    let Some((logical_width, logical_height)) = display.logical_size else {
        return Ok(0.0);
    };
    if logical_width <= 0 || logical_height <= 0 || display.width <= 0 || display.height <= 0 {
        bail!("Invalid Wayland cursor display dimensions");
    }
    // Logical geometry is already rotated; the physical mode dimensions are not.
    let width = if matches!(display.transform, 90 | 270) {
        display.height
    } else {
        display.width
    };
    let scale = f64::from(width) / f64::from(logical_width);
    // Mutter can report physical desktop coordinates even at 2x/3x output scale.
    // Keep geometry-derived fractional densities for logically scaled desktops.
    Ok(if scale == 1.0 && display.scale_factor > 1 {
        f64::from(display.scale_factor)
    } else {
        scale
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_cache_ids_fit_legacy_web_numbers() {
        for cursor in [1, 123, u64::MAX] {
            assert_eq!(cache_id(cursor, 0.0), cursor);
            for scale in [1.0, 1.25, 2.0] {
                assert!((1..=9_007_199_254_740_991).contains(&cache_id(cursor, scale)));
            }
        }
    }

    #[cfg(feature = "drm")]
    #[test]
    fn cursor_density_tracks_fractional_rotation_and_cache_identity() {
        let display = base::platform::linux::WaylandDisplayInfo {
            name: "test".into(),
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
            logical_size: Some((600, 960)),
            scale_factor: 2,
            refresh_rate: 60000,
            transform: 90,
        };
        assert_eq!(wayland_scale(&display).unwrap(), 4.0 / 3.0);
        assert_ne!(cache_id(1, 1.0), cache_id(1, 2.0));
        assert_eq!(cache_id(1, 0.0), 1);
    }

    #[cfg(feature = "drm")]
    #[test]
    fn cursor_density_distinguishes_output_scale_from_desktop_coordinates() {
        for (scale_factor, logical_size, expected) in [
            (3, (2560, 1600), 3.0),  // GNOME can use physical desktop coordinates.
            (2, (2048, 1280), 1.25), // Keep fractional scaling from logical geometry.
            (0, (2560, 1600), 1.0),  // Older probe snapshots omit wl_output.scale.
        ] {
            let display = base::platform::linux::WaylandDisplayInfo {
                name: "eDP-1".into(),
                x: 0,
                y: 0,
                width: 2560,
                height: 1600,
                logical_size: Some(logical_size),
                scale_factor,
                refresh_rate: 120000,
                transform: 0,
            };
            assert_eq!(wayland_scale(&display).unwrap(), expected);
        }
    }
}
