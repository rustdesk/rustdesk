//! XDO-based input emulation for Linux.
//!
//! This module uses libxdo-sys (patched to use dynamic loading stub) for input emulation.
//! The stub handles dynamic loading of libxdo, so we just call the functions directly.
//!
//! If libxdo is not available at runtime, all operations become no-ops.

use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};

use hbb_common::libc::c_int;
use libxdo_sys::{self, xdo_t, CURRENTWINDOW};
use std::{borrow::Cow, ffi::CString};

/// Default delay per keypress in microseconds.
/// This value is passed to libxdo functions and must fit in `useconds_t` (u32).
const DEFAULT_DELAY: u64 = 12000;

/// Maximum allowed delay value (u32::MAX as u64).
const MAX_DELAY: u64 = u32::MAX as u64;

fn mousebutton(button: MouseButton) -> c_int {
    match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
        MouseButton::ScrollUp => 4,
        MouseButton::ScrollDown => 5,
        MouseButton::ScrollLeft => 6,
        MouseButton::ScrollRight => 7,
        MouseButton::Back => 8,
        MouseButton::Forward => 9,
    }
}

/// The main struct for handling the event emitting
pub(super) struct EnigoXdo {
    xdo: *mut xdo_t,
    delay: u64,
}
// This is safe, we have a unique pointer.
// TODO: use Unique<c_char> once stable.
unsafe impl Send for EnigoXdo {}

impl Default for EnigoXdo {
    /// Create a new EnigoXdo instance.
    ///
    /// If libxdo is not available, the xdo pointer will be null and all
    /// input operations will be no-ops.
    fn default() -> Self {
        let xdo = unsafe { libxdo_sys::xdo_new(std::ptr::null()) };
        if xdo.is_null() {
            log::warn!("Failed to create xdo context, xdo functions will be disabled");
        } else {
            log::info!("xdo context created successfully");
        }
        Self {
            xdo,
            delay: DEFAULT_DELAY,
        }
    }
}

impl EnigoXdo {
    /// Get the delay per keypress in microseconds.
    ///
    /// Default value is 12000 (12ms). This is Linux-specific.
    pub fn delay(&self) -> u64 {
        self.delay
    }

    /// Set the delay per keypress in microseconds.
    ///
    /// This is Linux-specific. The value is clamped to `u32::MAX` (approximately
    /// 4295 seconds) because libxdo uses `useconds_t` which is typically `u32`.
    ///
    /// # Arguments
    /// * `delay` - Delay in microseconds. Values exceeding `u32::MAX` will be clamped.
    pub fn set_delay(&mut self, delay: u64) {
        self.delay = delay.min(MAX_DELAY);
        if delay > MAX_DELAY {
            log::warn!(
                "delay value {} exceeds maximum {}, clamped",
                delay,
                MAX_DELAY
            );
        }
    }
}

impl Drop for EnigoXdo {
    fn drop(&mut self) {
        if !self.xdo.is_null() {
            unsafe {
                libxdo_sys::xdo_free(self.xdo);
            }
        }
    }
}

impl MouseControllable for EnigoXdo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn mouse_move_to(&mut self, x: i32, y: i32) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            libxdo_sys::xdo_move_mouse(self.xdo as *const _, x, y, 0);
        }
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            libxdo_sys::xdo_move_mouse_relative(self.xdo as *const _, x, y);
        }
    }

    fn mouse_down(&mut self, button: MouseButton) -> crate::ResultType {
        if self.xdo.is_null() {
            return Ok(());
        }
        unsafe {
            libxdo_sys::xdo_mouse_down(self.xdo as *const _, CURRENTWINDOW, mousebutton(button));
        }
        Ok(())
    }

    fn mouse_up(&mut self, button: MouseButton) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            libxdo_sys::xdo_mouse_up(self.xdo as *const _, CURRENTWINDOW, mousebutton(button));
        }
    }

    fn mouse_click(&mut self, button: MouseButton) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            libxdo_sys::xdo_click_window(self.xdo as *const _, CURRENTWINDOW, mousebutton(button));
        }
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = MouseButton::ScrollLeft;
        } else {
            button = MouseButton::ScrollRight;
        }

        if length < 0 {
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }

    fn mouse_scroll_y(&mut self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = MouseButton::ScrollUp;
        } else {
            button = MouseButton::ScrollDown;
        }

        if length < 0 {
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }
}

fn keysequence<'a>(key: Key) -> Cow<'a, str> {
    if let Key::Layout(c) = key {
        return Cow::Owned(format!("U{:X}", c as u32));
    }
    if let Key::Raw(k) = key {
        return Cow::Owned(format!("{}", k as u16));
    }
    #[allow(deprecated)]
    // I mean duh, we still need to support deprecated keys until they're removed
    // https://www.rubydoc.info/gems/xdo/XDo/Keyboard
    // https://gitlab.com/cunidev/gestures/-/wikis/xdotool-list-of-key-codes
    Cow::Borrowed(match key {
        Key::Alt => "Alt",
        Key::Backspace => "BackSpace",
        Key::CapsLock => "Caps_Lock",
        Key::Control => "Control",
        Key::Delete => "Delete",
        Key::DownArrow => "Down",
        Key::End => "End",
        Key::Escape => "Escape",
        Key::F1 => "F1",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::Home => "Home",
        //Key::Layout(_) => unreachable!(),
        Key::LeftArrow => "Left",
        Key::Option => "Option",
        Key::PageDown => "Page_Down",
        Key::PageUp => "Page_Up",
        //Key::Raw(_) => unreachable!(),
        Key::Return => "Return",
        Key::RightArrow => "Right",
        Key::Shift => "Shift",
        Key::Space => "space",
        Key::Tab => "Tab",
        Key::UpArrow => "Up",
        Key::Numpad0 => "U30", //"KP_0",
        Key::Numpad1 => "U31", //"KP_1",
        Key::Numpad2 => "U32", //"KP_2",
        Key::Numpad3 => "U33", //"KP_3",
        Key::Numpad4 => "U34", //"KP_4",
        Key::Numpad5 => "U35", //"KP_5",
        Key::Numpad6 => "U36", //"KP_6",
        Key::Numpad7 => "U37", //"KP_7",
        Key::Numpad8 => "U38", //"KP_8",
        Key::Numpad9 => "U39", //"KP_9",
        Key::Decimal => "U2E", //"KP_Decimal",
        Key::Cancel => "Cancel",
        Key::Clear => "Clear",
        Key::Pause => "Pause",
        Key::Kana => "Kana",
        Key::Hangul => "Hangul",
        Key::Junja => "",
        Key::Final => "",
        Key::Hanja => "Hanja",
        Key::Kanji => "Kanji",
        Key::Convert => "",
        Key::Select => "Select",
        Key::Print => "Print",
        Key::Execute => "Execute",
        Key::Snapshot => "3270_PrintScreen",
        Key::Insert => "Insert",
        Key::Help => "Help",
        Key::Sleep => "",
        Key::Separator => "KP_Separator",
        Key::VolumeUp => "",
        Key::VolumeDown => "",
        Key::Mute => "",
        Key::Scroll => "Scroll_Lock",
        Key::NumLock => "Num_Lock",
        Key::RWin => "Super_R",
        Key::Apps => "Menu",
        Key::Multiply => "KP_Multiply",
        Key::Add => "KP_Add",
        Key::Subtract => "KP_Subtract",
        Key::Divide => "KP_Divide",
        Key::Equals => "KP_Equal",
        Key::NumpadEnter => "KP_Enter",
        Key::RightShift => "Shift_R",
        Key::RightControl => "Control_R",
        Key::RightAlt => "Alt_R",

        Key::Command | Key::Super | Key::Windows | Key::Meta => "Super",

        _ => "",
    })
}

impl KeyboardControllable for EnigoXdo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_key_state(&mut self, key: Key) -> bool {
        if self.xdo.is_null() {
            return false;
        }
        /*
        // modifier keys mask
        pub const ShiftMask: c_uint = 0x01;
        pub const LockMask: c_uint = 0x02;
        pub const ControlMask: c_uint = 0x04;
        pub const Mod1Mask: c_uint = 0x08;
        pub const Mod2Mask: c_uint = 0x10;
        pub const Mod3Mask: c_uint = 0x20;
        pub const Mod4Mask: c_uint = 0x40;
        pub const Mod5Mask: c_uint = 0x80;
        */
        let mod_shift = 1 << 0;
        let mod_lock = 1 << 1;
        let mod_control = 1 << 2;
        let mod_alt = 1 << 3;
        let mod_numlock = 1 << 4;
        let mod_meta = 1 << 6;
        let mask = unsafe { libxdo_sys::xdo_get_input_state(self.xdo as *const _) };
        match key {
            Key::Shift => mask & mod_shift != 0,
            Key::CapsLock => mask & mod_lock != 0,
            Key::Control => mask & mod_control != 0,
            Key::Alt => mask & mod_alt != 0,
            Key::NumLock => mask & mod_numlock != 0,
            Key::Meta => mask & mod_meta != 0,
            _ => false,
        }
    }

    fn key_sequence(&mut self, sequence: &str) {
        if self.xdo.is_null() {
            return;
        }
        if let Ok(string) = CString::new(sequence) {
            unsafe {
                libxdo_sys::xdo_enter_text_window(
                    self.xdo as *const _,
                    CURRENTWINDOW,
                    string.as_ptr(),
                    self.delay as libxdo_sys::useconds_t,
                );
            }
        }
    }

    fn key_down(&mut self, key: Key) -> crate::ResultType {
        if self.xdo.is_null() {
            return Ok(());
        }
        let string = CString::new(&*keysequence(key))?;
        unsafe {
            libxdo_sys::xdo_send_keysequence_window_down(
                self.xdo as *const _,
                CURRENTWINDOW,
                string.as_ptr(),
                self.delay as libxdo_sys::useconds_t,
            );
        }
        Ok(())
    }

    fn key_up(&mut self, key: Key) {
        if self.xdo.is_null() {
            return;
        }
        if let Ok(string) = CString::new(&*keysequence(key)) {
            unsafe {
                libxdo_sys::xdo_send_keysequence_window_up(
                    self.xdo as *const _,
                    CURRENTWINDOW,
                    string.as_ptr(),
                    self.delay as libxdo_sys::useconds_t,
                );
            }
        }
    }

    fn key_click(&mut self, key: Key) {
        if self.xdo.is_null() {
            return;
        }
        if let Ok(string) = CString::new(&*keysequence(key)) {
            unsafe {
                libxdo_sys::xdo_send_keysequence_window(
                    self.xdo as *const _,
                    CURRENTWINDOW,
                    string.as_ptr(),
                    self.delay as libxdo_sys::useconds_t,
                );
            }
        }
    }

    fn key_sequence_parse(&mut self, sequence: &str)
    where
        Self: Sized,
    {
        if let Err(..) = self.key_sequence_parse_try(sequence) {
            println!("Could not parse sequence");
        }
    }

    fn key_sequence_parse_try(&mut self, sequence: &str) -> Result<(), crate::dsl::ParseError>
    where
        Self: Sized,
    {
        crate::dsl::eval(self, sequence)
    }
}
