extern crate x11;
use crate::keycodes::linux::code_from_key;
use crate::rdev::{EventType, KeyboardState, UnicodeInfo};
use std::convert::TryInto;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_ulong, c_void};
use std::ptr::{null, null_mut, NonNull};
use x11::xlib::{self, KeySym, XKeyEvent, XKeysymToString, XSupportsLocale};

#[derive(Debug)]
pub struct MyXIM(xlib::XIM);
unsafe impl Sync for MyXIM {}
unsafe impl Send for MyXIM {}

#[derive(Debug)]
pub struct MyXIC(xlib::XIC);
unsafe impl Sync for MyXIC {}
unsafe impl Send for MyXIC {}

#[derive(Debug)]
pub struct MyDisplay(*mut xlib::Display);
unsafe impl Sync for MyDisplay {}
unsafe impl Send for MyDisplay {}

#[derive(Debug)]
pub struct Keyboard {
    pub xim: Box<MyXIM>,
    pub xic: Box<MyXIC>,
    pub display: Box<MyDisplay>,
    window: Box<xlib::Window>,
    keysym: Box<c_ulong>,
    status: Box<i32>,
    serial: c_ulong,
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        unsafe {
            let MyDisplay(display) = *self.display;
            xlib::XCloseDisplay(display);
        }
    }
}

impl Keyboard {
    pub fn new() -> Option<Keyboard> {
        unsafe {
            let dpy = xlib::XOpenDisplay(null());
            if dpy.is_null() {
                return None;
            }
            // https://stackoverflow.com/questions/18246848/get-utf-8-input-with-x11-display#
            // Try system localle first
            let string = CString::new("").ok()?;
            libc::setlocale(libc::LC_ALL, string.as_ptr());
            // If not supported try C.UTF-8
            if XSupportsLocale() == 0 {
                let string = CString::new("C.UTF-8").ok()?;
                libc::setlocale(libc::LC_ALL, string.as_ptr());
            }
            if XSupportsLocale() == 0 {
                let string = CString::new("C").ok()?;
                libc::setlocale(libc::LC_ALL, string.as_ptr());
            }
            let string = CString::new("@im=none").ok()?;
            let ret = xlib::XSetLocaleModifiers(string.as_ptr());
            NonNull::new(ret)?;

            let xim = xlib::XOpenIM(dpy, null_mut(), null_mut(), null_mut());
            NonNull::new(xim)?;

            let mut win_attr = xlib::XSetWindowAttributes {
                background_pixel: 0,
                background_pixmap: 0,
                border_pixel: 0,
                border_pixmap: 0,
                bit_gravity: 0,
                win_gravity: 0,
                backing_store: 0,
                backing_planes: 0,
                backing_pixel: 0,
                event_mask: 0,
                save_under: 0,
                do_not_propagate_mask: 0,
                override_redirect: 0,
                colormap: 0,
                cursor: 0,
            };

            let window = xlib::XCreateWindow(
                dpy,
                xlib::XDefaultRootWindow(dpy),
                0,
                0,
                1,
                1,
                0,
                xlib::CopyFromParent,
                xlib::InputOnly as c_uint,
                null_mut(),
                xlib::CWOverrideRedirect,
                &mut win_attr,
            );

            let input_style = CString::new(xlib::XNInputStyle).ok()?;
            let window_client = CString::new(xlib::XNClientWindow).ok()?;
            let style = xlib::XIMPreeditNothing | xlib::XIMStatusNothing;

            let xic = xlib::XCreateIC(
                xim,
                input_style.as_ptr(),
                style,
                window_client.as_ptr(),
                window,
                null::<c_void>(),
            );
            NonNull::new(xic)?;

            xlib::XSetICFocus(xic);

            Some(Keyboard {
                xim: Box::new(MyXIM(xim)),
                xic: Box::new(MyXIC(xic)),
                display: Box::new(MyDisplay(dpy)),
                window: Box::new(window),
                keysym: Box::new(0),
                status: Box::new(0),
                serial: 0,
            })
        }
    }

    pub(crate) unsafe fn get_current_modifiers(&mut self) -> Option<u32> {
        let MyDisplay(display) = *self.display;
        let screen_number = xlib::XDefaultScreen(display);
        let screen = xlib::XScreenOfDisplay(display, screen_number);
        let window = xlib::XRootWindowOfScreen(screen);
        // Passing null pointers for the things we don't need results in a
        // segfault.
        let mut root_return: xlib::Window = 0;
        let mut child_return: xlib::Window = 0;
        let mut root_x_return = 0;
        let mut root_y_return = 0;
        let mut win_x_return = 0;
        let mut win_y_return = 0;
        let mut mask_return = 0;
        xlib::XQueryPointer(
            display,
            window,
            &mut root_return,
            &mut child_return,
            &mut root_x_return,
            &mut root_y_return,
            &mut win_x_return,
            &mut win_y_return,
            &mut mask_return,
        );
        Some(mask_return)
    }

    pub(crate) unsafe fn unicode_from_code(
        &mut self,
        keycode: c_uint,
        state: c_uint,
    ) -> Option<UnicodeInfo> {
        let MyDisplay(display) = *self.display;
        let MyXIC(xic) = *self.xic;
        if display.is_null() || xic.is_null() {
            println!("We don't seem to have a display or a xic");
            return None;
        }
        const BUF_LEN: usize = 4;
        let mut buf = [0_u8; BUF_LEN];
        let MyDisplay(display) = *self.display;
        let mut key = xlib::XKeyEvent {
            display,
            root: 0,
            window: *self.window,
            subwindow: 0,
            x: 0,
            y: 0,
            x_root: 0,
            y_root: 0,
            state,
            keycode,
            same_screen: 0,
            send_event: 0,
            serial: self.serial,
            type_: xlib::KeyPress,
            time: xlib::CurrentTime,
        };
        self.serial += 1;

        let mut event = xlib::XEvent { key };

        // -----------------------------------------------------------------
        // XXX: This is **OMEGA IMPORTANT** This is what enables us to receive
        // the correct keyvalue from the utf8LookupString !!
        // https://stackoverflow.com/questions/18246848/get-utf-8-input-with-x11-display#
        // -----------------------------------------------------------------
        xlib::XFilterEvent(&mut event, 0);

        let MyXIC(xic) = *self.xic;
        let ret = xlib::Xutf8LookupString(
            xic,
            &mut event.key,
            buf.as_mut_ptr() as *mut c_char,
            BUF_LEN as c_int,
            &mut *self.keysym,
            &mut *self.status,
        );

        let keysym = xlookup_string(&mut key);
        self.keysym = Box::new(keysym);
        if self.is_dead() {
            return Some(UnicodeInfo {
                name: None,
                unicode: Vec::new(),
                is_dead: true,
            });
        }
        if ret == xlib::NoSymbol {
            return None;
        }

        let len = buf.iter().position(|ch| ch == &0).unwrap_or(BUF_LEN);

        // C0 controls
        if len == 1 {
            match String::from_utf8(buf[..len].to_vec()) {
                Ok(s) => {
                    if let Some(c) = s.chars().next() {
                        if ('\u{1}'..='\u{1f}').contains(&c) {
                            return None;
                        }
                    }
                }
                Err(_) => {}
            }
        }

        Some(UnicodeInfo {
            name: String::from_utf8(buf[..len].to_vec()).ok(),
            unicode: Vec::new(),
            is_dead: false,
        })
    }

    pub fn is_dead(&mut self) -> bool {
        let ptr = unsafe { XKeysymToString(*self.keysym) };
        if ptr.is_null() {
            false
        } else {
            let res = unsafe { CStr::from_ptr(ptr).to_str() };
            res.unwrap_or_default().to_owned().starts_with("dead")
        }
    }

    pub fn keysym(&self) -> u32 {
        (*self.keysym).try_into().unwrap_or_default()
    }
}

impl KeyboardState for Keyboard {
    fn add(&mut self, event_type: &EventType) -> Option<UnicodeInfo> {
        match event_type {
            EventType::KeyPress(key) => {
                let keycode = code_from_key(*key)?;
                // let state = self.state.value();
                let state = unsafe { self.get_current_modifiers().unwrap_or_default() };
                // !!!: Igore Control
                let state = state & 0xFFFB;
                unsafe { self.unicode_from_code(keycode, state) }
            }
            EventType::KeyRelease(_key) => None,
            _ => None,
        }
    }
}

/// refs:
/// 1. https://github.com/mechpen/rterm/blob/b2d04defc13b5688bf75c5de72c0b8810f982dc1/src/x11_wrapper.rs#L357
/// 2. https://github.com/freedesktop/xev/blob/a92082cb05bb3d6d3f0bebb951133774ca2dd412/xev.c#L125
pub fn xlookup_string(event: &mut XKeyEvent) -> KeySym {
    let mut buf = [0u8; 64];

    let mut ksym: KeySym = 0;
    let _len = unsafe {
        xlib::XLookupString(
            event,
            buf.as_mut_ptr() as *mut _,
            buf.len() as _,
            &mut ksym,
            null_mut(),
        )
    };
    ksym
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    /// If the following tests run, they *will* cause a crash because xlib
    /// is *not* thread safe. Ignoring the tests for now.
    /// XCB *could* be an option but not even sure we can get dead keys again.
    /// XCB doc is sparse on the web let's say.
    fn test_thread_safety() {
        let mut keyboard = Keyboard::new().unwrap();
        let char_s = keyboard
            .add(&EventType::KeyPress(crate::rdev::Key::KeyS))
            .unwrap()
            .name
            .unwrap();
        assert_eq!(
            char_s,
            "s".to_string(),
            "This test should pass only on Qwerty layout !"
        );
    }

    #[test]
    #[ignore]
    fn test_thread_safety_2() {
        let mut keyboard = Keyboard::new().unwrap();
        let char_s = keyboard
            .add(&EventType::KeyPress(crate::rdev::Key::KeyS))
            .unwrap()
            .name
            .unwrap();
        assert_eq!(
            char_s,
            "s".to_string(),
            "This test should pass only on Qwerty layout !"
        );
    }
}
