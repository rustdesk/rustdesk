use hbb_common::x11::{xinput2, xlib};

const PROPERTY_FORMAT: i32 = 8;
// X11 property lengths are in 32-bit units; these settings contain at most three bytes.
const PROPERTY_LENGTH: std::os::raw::c_long = 1;
const TWO_FINGER_SCROLL: &[u8] = &[1, 0, 0];
const SEND_EVENTS_ENABLED: &[u8] = &[0, 0];
const HORIZONTAL_SCROLL_ENABLED: &[u8] = &[1];
const NATURAL_SCROLL_PROPERTY: &[u8] = b"libinput Natural Scrolling Enabled\0";
const NATURAL_SCROLL_DISABLED: [u8; 1] = [0];
const NATURAL_SCROLL_ENABLED: [u8; 1] = [1];

pub(super) unsafe fn scrolling_enabled(display: *mut xlib::Display, device_id: i32) -> bool {
    // Scroll valuators remain present even when the driver disables their event delivery.
    property_matches(
        display,
        device_id,
        (b"libinput Scroll Method Enabled\0", TWO_FINGER_SCROLL),
    ) && property_matches(
        display,
        device_id,
        (b"libinput Send Events Mode Enabled\0", SEND_EVENTS_ENABLED),
    ) && property_matches(
        display,
        device_id,
        (
            b"libinput Horizontal Scroll Enabled\0",
            HORIZONTAL_SCROLL_ENABLED,
        ),
    ) && disable_natural_scrolling(display, device_id)
}

pub(super) unsafe fn disable_natural_scrolling(
    display: *mut xlib::Display,
    device_id: i32,
) -> bool {
    let property = NATURAL_SCROLL_PROPERTY;
    // Compare one snapshot; the desktop may change the property concurrently.
    let Some(value) = read_property(
        display,
        device_id,
        (property, NATURAL_SCROLL_DISABLED.len()),
    ) else {
        return false;
    };
    if value == NATURAL_SCROLL_DISABLED {
        return true;
    }
    if value != NATURAL_SCROLL_ENABLED {
        return false;
    }
    // The controller already chooses the scroll direction. Normalize only our virtual device.
    let atom = xlib::XInternAtom(display, property.as_ptr().cast(), xlib::True);
    let mut value = NATURAL_SCROLL_DISABLED;
    xinput2::XIChangeProperty(
        display,
        device_id,
        atom,
        xlib::XA_INTEGER,
        PROPERTY_FORMAT,
        xlib::PropModeReplace,
        value.as_mut_ptr(),
        value.len() as std::os::raw::c_int,
    );
    // XIChangeProperty has no status result; read back the effective driver property.
    let disabled = property_matches(display, device_id, (property, &NATURAL_SCROLL_DISABLED));
    if disabled {
        hbb_common::log::info!("Disabled natural scrolling on RustDesk X11 device {device_id}");
    } else {
        hbb_common::log::error!(
            "Failed to disable natural scrolling on RustDesk X11 device {device_id}"
        );
    }
    disabled
}

unsafe fn property_matches(
    display: *mut xlib::Display,
    device_id: i32,
    property: (&[u8], &[u8]),
) -> bool {
    let (name, expected) = property;
    read_property(display, device_id, (name, expected.len())).is_some_and(|value| value == expected)
}

unsafe fn read_property(
    display: *mut xlib::Display,
    device_id: i32,
    property: (&[u8], usize),
) -> Option<Vec<u8>> {
    let (name, length) = property;
    let atom = xlib::XInternAtom(display, name.as_ptr().cast(), xlib::True);
    if atom == 0 {
        return None;
    }
    let (mut actual_type, mut format, mut items, mut remaining) = (0, 0, 0, 0);
    let mut data = std::ptr::null_mut();
    let status = xinput2::XIGetProperty(
        display,
        device_id,
        atom,
        0,
        PROPERTY_LENGTH,
        xlib::False,
        xlib::XA_INTEGER,
        &mut actual_type,
        &mut format,
        &mut items,
        &mut remaining,
        &mut data,
    );
    let value = (status == i32::from(xlib::Success)
        && actual_type == xlib::XA_INTEGER
        && format == PROPERTY_FORMAT
        && items == length as std::os::raw::c_ulong
        && remaining == 0
        && !data.is_null())
    .then(|| std::slice::from_raw_parts(data, length).to_vec());
    if !data.is_null() {
        xlib::XFree(data.cast());
    }
    value
}
