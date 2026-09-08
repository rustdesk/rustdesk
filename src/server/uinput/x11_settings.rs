use hbb_common::x11::{xinput2, xlib};

const PROPERTY_FORMAT: i32 = 8;
// X11 property lengths are in 32-bit units; these settings contain at most three bytes.
const PROPERTY_LENGTH: std::os::raw::c_long = 1;
const TWO_FINGER_SCROLL: &[u8] = &[1, 0, 0];
const SEND_EVENTS_ENABLED: &[u8] = &[0, 0];
const HORIZONTAL_SCROLL_ENABLED: &[u8] = &[1];

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
    )
}

unsafe fn property_matches(
    display: *mut xlib::Display,
    device_id: i32,
    property: (&[u8], &[u8]),
) -> bool {
    let (name, expected) = property;
    let atom = xlib::XInternAtom(display, name.as_ptr().cast(), xlib::True);
    if atom == 0 {
        return false;
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
    let matches = status == i32::from(xlib::Success)
        && actual_type == xlib::XA_INTEGER
        && format == PROPERTY_FORMAT
        && items == expected.len() as _
        && remaining == 0
        && !data.is_null()
        && std::slice::from_raw_parts(data, expected.len()) == expected;
    if !data.is_null() {
        xlib::XFree(data.cast());
    }
    matches
}
