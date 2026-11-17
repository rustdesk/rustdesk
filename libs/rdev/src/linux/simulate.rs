use crate::linux::common::{FALSE, TRUE};
use crate::keycodes::linux::code_from_key;
use crate::rdev::{Button, EventType, RawKey, SimulateError};
use std::convert::TryInto;
use std::os::raw::c_int;
use std::ptr::null;
use x11::xlib;
use x11::xtest;

unsafe fn send_native(event_type: &EventType, display: *mut xlib::Display) -> Option<()> {
    let res = match event_type {
        EventType::KeyPress(key) => match key {
            crate::Key::RawKey(rawkey) => {
                if let RawKey::LinuxXorgKeycode(keycode) = rawkey {
                    xtest::XTestFakeKeyEvent(display, *keycode as _, TRUE, 0)
                } else {
                    return None;
                }
            }
            _ => {
                let code = code_from_key(*key)?;
                xtest::XTestFakeKeyEvent(display, code, TRUE, 0)
            }
        },
        EventType::KeyRelease(key) => match key {
            crate::Key::RawKey(rawkey) => {
                if let RawKey::LinuxXorgKeycode(keycode) = rawkey {
                    xtest::XTestFakeKeyEvent(display, *keycode as _, FALSE, 0)
                } else {
                    return None;
                }
            }
            _ => {
                let code = code_from_key(*key)?;
                xtest::XTestFakeKeyEvent(display, code, FALSE, 0)
            }
        },
        EventType::ButtonPress(button) => match button {
            Button::Left => xtest::XTestFakeButtonEvent(display, 1, TRUE, 0),
            Button::Middle => xtest::XTestFakeButtonEvent(display, 2, TRUE, 0),
            Button::Right => xtest::XTestFakeButtonEvent(display, 3, TRUE, 0),
            Button::Unknown(code) => {
                xtest::XTestFakeButtonEvent(display, (*code).try_into().ok()?, TRUE, 0)
            }
        },
        EventType::ButtonRelease(button) => match button {
            Button::Left => xtest::XTestFakeButtonEvent(display, 1, FALSE, 0),
            Button::Middle => xtest::XTestFakeButtonEvent(display, 2, FALSE, 0),
            Button::Right => xtest::XTestFakeButtonEvent(display, 3, FALSE, 0),
            Button::Unknown(code) => {
                xtest::XTestFakeButtonEvent(display, (*code).try_into().ok()?, FALSE, 0)
            }
        },
        EventType::MouseMove { x, y } => {
            //TODO: replace with clamp if it is stabalized
            let x = if x.is_finite() {
                x.min(c_int::max_value().into())
                    .max(c_int::min_value().into())
                    .round() as c_int
            } else {
                0
            };
            let y = if y.is_finite() {
                y.min(c_int::max_value().into())
                    .max(c_int::min_value().into())
                    .round() as c_int
            } else {
                0
            };
            xtest::XTestFakeMotionEvent(display, 0, x, y, 0)
            //     xlib::XWarpPointer(display, 0, root, 0, 0, 0, 0, *x as i32, *y as i32);
        }
        EventType::Wheel { delta_y, .. } => {
            let code = if *delta_y > 0 { 4 } else { 5 };
            xtest::XTestFakeButtonEvent(display, code, TRUE, 0)
                & xtest::XTestFakeButtonEvent(display, code, FALSE, 0)
        }
    };
    if res == 0 {
        None
    } else {
        Some(())
    }
}

pub fn simulate(event_type: &EventType) -> Result<(), SimulateError> {
    unsafe {
        let dpy = xlib::XOpenDisplay(null());
        if dpy.is_null() {
            return Err(SimulateError);
        }
        match send_native(event_type, dpy) {
            Some(_) => {
                xlib::XFlush(dpy);
                xlib::XSync(dpy, 0);
                xlib::XCloseDisplay(dpy);
                Ok(())
            }
            None => {
                xlib::XCloseDisplay(dpy);
                Err(SimulateError)
            }
        }
    }
}

unsafe fn send_native_char(chr: char, pressed: bool, display: *mut xlib::Display) -> Option<()> {
    // unuse keycode: F24 -> 194
    let keycode: u32 = 194;

    // char to keysym
    let ordinal: u32 = chr.into();
    let mut keysym = if ordinal < 0x100 {
        ordinal
    } else {
        ordinal | 0x01000000
    } as libc::c_ulong;

    // remap keycode to keysym
    x11::xlib::XChangeKeyboardMapping(display, keycode as _, 1, &mut keysym, 1);

    let res = if pressed {
        xtest::XTestFakeKeyEvent(display, keycode as _, TRUE, 0)
    } else {
        xtest::XTestFakeKeyEvent(display, keycode as _, FALSE, 0)
    };

    if res == 0 {
        None
    } else {
        Some(())
    }
}

pub fn simulate_char(chr: char, pressed: bool) -> Result<(), SimulateError> {
    unsafe {
        let dpy = xlib::XOpenDisplay(null());
        if dpy.is_null() {
            return Err(SimulateError);
        }
        match send_native_char(chr, pressed, dpy) {
            Some(_) => {
                xlib::XFlush(dpy);
                xlib::XSync(dpy, 0);
                xlib::XCloseDisplay(dpy);
                Ok(())
            }
            None => {
                xlib::XCloseDisplay(dpy);
                Err(SimulateError)
            }
        }
    }
}

pub fn simulate_unicode(_unicode: u16) -> Result<(), SimulateError> {
    Err(SimulateError)
}
