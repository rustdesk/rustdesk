use core_graphics;

// TODO(dustin): use only the things i need

use self::core_graphics::display::*;
use self::core_graphics::event::*;
use self::core_graphics::event_source::*;

use crate::macos::keycodes::*;
use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};
use objc::runtime::Class;
use std::ffi::CStr;
use std::os::raw::*;

// required for pressedMouseButtons on NSEvent
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

struct MyCGEvent;

#[allow(improper_ctypes)]
#[allow(non_snake_case)]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventPost(tapLocation: CGEventTapLocation, event: *mut MyCGEvent);
    // not present in servo/core-graphics
    fn CGEventCreateScrollWheelEvent(
        source: &CGEventSourceRef,
        units: ScrollUnit,
        wheelCount: u32,
        wheel1: i32,
        ...
    ) -> *mut MyCGEvent;
    fn CGEventSourceKeyState(stateID: i32, key: u16) -> bool;
}

pub type CFDataRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
pub struct __TISInputSource;
pub type TISInputSourceRef = *const __TISInputSource;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __CFString([u8; 0]);
pub type CFStringRef = *const __CFString;
pub type Boolean = c_uchar;
pub type UInt8 = c_uchar;
pub type SInt32 = c_int;
pub type UInt16 = c_ushort;
pub type UInt32 = c_uint;
pub type UniChar = UInt16;
pub type UniCharCount = c_ulong;

pub type OptionBits = UInt32;
pub type OSStatus = SInt32;

pub type CFStringEncoding = UInt32;

#[allow(non_upper_case_globals)]
pub const kUCKeyActionDisplay: _bindgen_ty_702 = _bindgen_ty_702::kUCKeyActionDisplay;

#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum _bindgen_ty_702 {
    // kUCKeyActionDown = 0,
    // kUCKeyActionUp = 1,
    // kUCKeyActionAutoKey = 2,
    kUCKeyActionDisplay = 3,
}

#[allow(non_snake_case)]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UCKeyboardTypeHeader {
    pub keyboardTypeFirst: UInt32,
    pub keyboardTypeLast: UInt32,
    pub keyModifiersToTableNumOffset: UInt32,
    pub keyToCharTableIndexOffset: UInt32,
    pub keyStateRecordsIndexOffset: UInt32,
    pub keyStateTerminatorsOffset: UInt32,
    pub keySequenceDataIndexOffset: UInt32,
}

#[allow(non_snake_case)]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UCKeyboardLayout {
    pub keyLayoutHeaderFormat: UInt16,
    pub keyLayoutDataVersion: UInt16,
    pub keyLayoutFeatureInfoOffset: UInt32,
    pub keyboardTypeCount: UInt32,
    pub keyboardTypeList: [UCKeyboardTypeHeader; 1usize],
}

#[allow(non_upper_case_globals)]
pub const kUCKeyTranslateNoDeadKeysBit: _bindgen_ty_703 =
    _bindgen_ty_703::kUCKeyTranslateNoDeadKeysBit;

#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum _bindgen_ty_703 {
    kUCKeyTranslateNoDeadKeysBit = 0,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct __CFAllocator([u8; 0]);
pub type CFAllocatorRef = *const __CFAllocator;

// #[repr(u32)]
// #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
// pub enum _bindgen_ty_15 {
//     kCFStringEncodingMacRoman = 0,
//     kCFStringEncodingWindowsLatin1 = 1280,
//     kCFStringEncodingISOLatin1 = 513,
//     kCFStringEncodingNextStepLatin = 2817,
//     kCFStringEncodingASCII = 1536,
//     kCFStringEncodingUnicode = 256,
//     kCFStringEncodingUTF8 = 134217984,
//     kCFStringEncodingNonLossyASCII = 3071,
//     kCFStringEncodingUTF16BE = 268435712,
//     kCFStringEncodingUTF16LE = 335544576,
//     kCFStringEncodingUTF32 = 201326848,
//     kCFStringEncodingUTF32BE = 402653440,
//     kCFStringEncodingUTF32LE = 469762304,
// }

#[allow(non_upper_case_globals)]
pub const kCFStringEncodingUTF8: u32 = 134_217_984;

#[allow(improper_ctypes)]
#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;

    //     extern void *
    // TISGetInputSourceProperty(
    //   TISInputSourceRef   inputSource,
    //   CFStringRef         propertyKey)

    #[allow(non_upper_case_globals)]
    #[link_name = "kTISPropertyUnicodeKeyLayoutData"]
    pub static kTISPropertyUnicodeKeyLayoutData: CFStringRef;

    #[allow(non_snake_case)]
    pub fn TISGetInputSourceProperty(
        inputSource: TISInputSourceRef,
        propertyKey: CFStringRef,
    ) -> *mut c_void;

    #[allow(non_snake_case)]
    pub fn CFDataGetBytePtr(theData: CFDataRef) -> *const UInt8;

    #[allow(non_snake_case)]
    pub fn UCKeyTranslate(
        keyLayoutPtr: *const UInt8, //*const UCKeyboardLayout,
        virtualKeyCode: UInt16,
        keyAction: UInt16,
        modifierKeyState: UInt32,
        keyboardType: UInt32,
        keyTranslateOptions: OptionBits,
        deadKeyState: *mut UInt32,
        maxStringLength: UniCharCount,
        actualStringLength: *mut UniCharCount,
        unicodeString: *mut UniChar,
    ) -> OSStatus;

    pub fn LMGetKbdType() -> UInt8;

    #[allow(non_snake_case)]
    pub fn CFStringCreateWithCharacters(
        alloc: CFAllocatorRef,
        chars: *const UniChar,
        numChars: CFIndex,
    ) -> CFStringRef;

    #[allow(non_upper_case_globals)]
    #[link_name = "kCFAllocatorDefault"]
    pub static kCFAllocatorDefault: CFAllocatorRef;

    #[allow(non_snake_case)]
    pub fn CFStringGetCString(
        theString: CFStringRef,
        buffer: *mut c_char,
        bufferSize: CFIndex,
        encoding: CFStringEncoding,
    ) -> Boolean;
}

// not present in servo/core-graphics
#[allow(dead_code)]
#[derive(Debug)]
enum ScrollUnit {
    Pixel = 0,
    Line = 1,
}
// hack

/// The main struct for handling the event emitting
pub struct Enigo {
    event_source: Option<CGEventSource>,
    keycode_to_string_map: std::collections::HashMap<String, CGKeyCode>,
    double_click_interval: u32,
    last_click_time: Option<std::time::Instant>,
    multiple_click: i64,
    flags: CGEventFlags,
}

impl Enigo {
    ///
    pub fn reset_flag(&mut self) {
        self.flags = CGEventFlags::CGEventFlagNull;
    }

    ///
    pub fn add_flag(&mut self, key: &Key) {
        let flag = match key {
            &Key::CapsLock => CGEventFlags::CGEventFlagAlphaShift,
            &Key::Shift => CGEventFlags::CGEventFlagShift,
            &Key::Control => CGEventFlags::CGEventFlagControl,
            &Key::Alt => CGEventFlags::CGEventFlagAlternate,
            &Key::Meta => CGEventFlags::CGEventFlagCommand,
            &Key::NumLock => CGEventFlags::CGEventFlagNumericPad,
            _ => CGEventFlags::CGEventFlagNull,
        };
        self.flags |= flag;
    }

    fn post(&self, event: CGEvent) {
        event.set_flags(self.flags);
        event.post(CGEventTapLocation::HID);
    }
}

impl Default for Enigo {
    fn default() -> Self {
        let mut double_click_interval = 500;
        if let Some(ns_event) = Class::get("NSEvent") {
            let tm: f64 = unsafe { msg_send![ns_event, doubleClickInterval] };
            if tm > 0. {
                double_click_interval = (tm * 1000.) as u32;
                log::info!("double click interval: {}ms", double_click_interval);
            }
        }
        Self {
            // TODO(dustin): return error rather than panic here
            event_source: if let Ok(src) =
                CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            {
                Some(src)
            } else {
                None
            },
            keycode_to_string_map: Default::default(),
            double_click_interval,
            multiple_click: 1,
            last_click_time: None,
            flags: CGEventFlags::CGEventFlagNull,
        }
    }
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        let pressed = Self::pressed_buttons();

        let event_type = if pressed & 1 > 0 {
            CGEventType::LeftMouseDragged
        } else if pressed & 2 > 0 {
            CGEventType::RightMouseDragged
        } else {
            CGEventType::MouseMoved
        };

        let dest = CGPoint::new(x as f64, y as f64);
        if let Some(src) = self.event_source.as_ref() {
            if let Ok(event) =
                CGEvent::new_mouse_event(src.clone(), event_type, dest, CGMouseButton::Left)
            {
                self.post(event);
            }
        }
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        let (display_width, display_height) = Self::main_display_size();
        let (current_x, y_inv) = Self::mouse_location_raw_coords();
        let current_y = (display_height as i32) - y_inv;
        let new_x = current_x + x;
        let new_y = current_y + y;

        if new_x < 0
            || new_x as usize > display_width
            || new_y < 0
            || new_y as usize > display_height
        {
            return;
        }

        self.mouse_move_to(new_x, new_y);
    }

    fn mouse_down(&mut self, button: MouseButton) -> crate::ResultType {
        let now = std::time::Instant::now();
        if let Some(t) = self.last_click_time {
            if t.elapsed().as_millis() as u32 <= self.double_click_interval {
                self.multiple_click += 1;
            } else {
                self.multiple_click = 1;
            }
        }
        self.last_click_time = Some(now);
        let (current_x, current_y) = Self::mouse_location();
        let (button, event_type) = match button {
            MouseButton::Left => (CGMouseButton::Left, CGEventType::LeftMouseDown),
            MouseButton::Middle => (CGMouseButton::Center, CGEventType::OtherMouseDown),
            MouseButton::Right => (CGMouseButton::Right, CGEventType::RightMouseDown),
            _ => unimplemented!(),
        };
        let dest = CGPoint::new(current_x as f64, current_y as f64);
        if let Some(src) = self.event_source.as_ref() {
            if let Ok(event) = CGEvent::new_mouse_event(src.clone(), event_type, dest, button) {
                if self.multiple_click > 1 {
                    event.set_integer_value_field(
                        EventField::MOUSE_EVENT_CLICK_STATE,
                        self.multiple_click,
                    );
                }
                self.post(event);
            }
        }
        Ok(())
    }

    fn mouse_up(&mut self, button: MouseButton) {
        let (current_x, current_y) = Self::mouse_location();
        let (button, event_type) = match button {
            MouseButton::Left => (CGMouseButton::Left, CGEventType::LeftMouseUp),
            MouseButton::Middle => (CGMouseButton::Center, CGEventType::OtherMouseUp),
            MouseButton::Right => (CGMouseButton::Right, CGEventType::RightMouseUp),
            _ => unimplemented!(),
        };
        let dest = CGPoint::new(current_x as f64, current_y as f64);
        if let Some(src) = self.event_source.as_ref() {
            if let Ok(event) = CGEvent::new_mouse_event(src.clone(), event_type, dest, button) {
                if self.multiple_click > 1 {
                    event.set_integer_value_field(
                        EventField::MOUSE_EVENT_CLICK_STATE,
                        self.multiple_click,
                    );
                }
                self.post(event);
            }
        }
    }

    fn mouse_click(&mut self, button: MouseButton) {
        self.mouse_down(button).ok();
        self.mouse_up(button);
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        let mut scroll_direction = -1; // 1 left -1 right;
        let mut length = length;

        if length < 0 {
            length *= -1;
            scroll_direction *= -1;
        }

        if let Some(src) = self.event_source.as_ref() {
            for _ in 0..length {
                unsafe {
                    let mouse_ev = CGEventCreateScrollWheelEvent(
                        &src,
                        ScrollUnit::Line,
                        2, // CGWheelCount 1 = y 2 = xy 3 = xyz
                        0,
                        scroll_direction,
                    );

                    CGEventPost(CGEventTapLocation::HID, mouse_ev);
                    CFRelease(mouse_ev as *const std::ffi::c_void);
                }
            }
        }
    }

    fn mouse_scroll_y(&mut self, length: i32) {
        let mut scroll_direction = -1; // 1 left -1 right;
        let mut length = length;

        if length < 0 {
            length *= -1;
            scroll_direction *= -1;
        }

        if let Some(src) = self.event_source.as_ref() {
            for _ in 0..length {
                unsafe {
                    let mouse_ev = CGEventCreateScrollWheelEvent(
                        &src,
                        ScrollUnit::Line,
                        1, // CGWheelCount 1 = y 2 = xy 3 = xyz
                        scroll_direction,
                    );

                    CGEventPost(CGEventTapLocation::HID, mouse_ev);
                    CFRelease(mouse_ev as *const std::ffi::c_void);
                }
            }
        }
    }
}

// https://stackoverflow.
// com/questions/1918841/how-to-convert-ascii-character-to-cgkeycode

impl KeyboardControllable for Enigo {
    fn key_sequence(&mut self, sequence: &str) {
        // NOTE(dustin): This is a fix for issue https://github.com/enigo-rs/enigo/issues/68
        // TODO(dustin): This could be improved by aggregating 20 bytes worth of graphemes at a time
        // but i am unsure what would happen for grapheme clusters greater than 20 bytes ...
        use unicode_segmentation::UnicodeSegmentation;
        let clusters = UnicodeSegmentation::graphemes(sequence, true).collect::<Vec<&str>>();
        for cluster in clusters {
            if let Some(src) = self.event_source.as_ref() {
                if let Ok(event) = CGEvent::new_keyboard_event(src.clone(), 0, true) {
                    event.set_string(cluster);
                    self.post(event);
                }
            }
        }
    }

    fn key_click(&mut self, key: Key) {
        let keycode = self.key_to_keycode(key);
        if keycode == 0 {
            return;
        }

        if let Some(src) = self.event_source.as_ref() {
            if let Ok(event) = CGEvent::new_keyboard_event(src.clone(), keycode, true) {
                self.post(event);
            }

            if let Ok(event) = CGEvent::new_keyboard_event(src.clone(), keycode, false) {
                self.post(event);
            }
        }
    }

    fn key_down(&mut self, key: Key) -> crate::ResultType {
        if let Some(src) = self.event_source.as_ref() {
            if let Ok(event) =
                CGEvent::new_keyboard_event(src.clone(), self.key_to_keycode(key), true)
            {
                self.post(event);
            }
        }
        Ok(())
    }

    fn key_up(&mut self, key: Key) {
        if let Some(src) = self.event_source.as_ref() {
            if let Ok(event) =
                CGEvent::new_keyboard_event(src.clone(), self.key_to_keycode(key), false)
            {
                self.post(event);
            }
        }
    }

    fn get_key_state(&mut self, key: Key) -> bool {
        let keycode = self.key_to_keycode(key);
        unsafe { CGEventSourceKeyState(1, keycode) }
    }
}

impl Enigo {
    fn pressed_buttons() -> usize {
        if let Some(ns_event) = Class::get("NSEvent") {
            unsafe { msg_send![ns_event, pressedMouseButtons] }
        } else {
            0
        }
    }

    /// Fetches the `(width, height)` in pixels of the main display
    pub fn main_display_size() -> (usize, usize) {
        let display_id = unsafe { CGMainDisplayID() };
        let width = unsafe { CGDisplayPixelsWide(display_id) };
        let height = unsafe { CGDisplayPixelsHigh(display_id) };
        (width, height)
    }

    /// Returns the current mouse location in Cocoa coordinates which have Y
    /// inverted from the Carbon coordinates used in the rest of the API.
    /// This function exists so that mouse_move_relative only has to fetch
    /// the screen size once.
    fn mouse_location_raw_coords() -> (i32, i32) {
        if let Some(ns_event) = Class::get("NSEvent") {
            let pt: NSPoint = unsafe { msg_send![ns_event, mouseLocation] };
            (pt.x as i32, pt.y as i32)
        } else {
            (0, 0)
        }
    }

    /// The mouse coordinates in points, only works on the main display
    pub fn mouse_location() -> (i32, i32) {
        let (x, y_inv) = Self::mouse_location_raw_coords();
        let (_, display_height) = Self::main_display_size();
        (x, (display_height as i32) - y_inv)
    }

    fn key_to_keycode(&mut self, key: Key) -> CGKeyCode {
        #[allow(deprecated)]
        // I mean duh, we still need to support deprecated keys until they're removed
        match key {
            Key::Alt => kVK_Option,
            Key::Backspace => kVK_Delete,
            Key::CapsLock => kVK_CapsLock,
            Key::Control => kVK_Control,
            Key::Delete => kVK_ForwardDelete,
            Key::DownArrow => kVK_DownArrow,
            Key::End => kVK_End,
            Key::Escape => kVK_Escape,
            Key::F1 => kVK_F1,
            Key::F10 => kVK_F10,
            Key::F11 => kVK_F11,
            Key::F12 => kVK_F12,
            Key::F2 => kVK_F2,
            Key::F3 => kVK_F3,
            Key::F4 => kVK_F4,
            Key::F5 => kVK_F5,
            Key::F6 => kVK_F6,
            Key::F7 => kVK_F7,
            Key::F8 => kVK_F8,
            Key::F9 => kVK_F9,
            Key::Home => kVK_Home,
            Key::LeftArrow => kVK_LeftArrow,
            Key::Option => kVK_Option,
            Key::PageDown => kVK_PageDown,
            Key::PageUp => kVK_PageUp,
            Key::Return => kVK_Return,
            Key::RightArrow => kVK_RightArrow,
            Key::Shift => kVK_Shift,
            Key::Space => kVK_Space,
            Key::Tab => kVK_Tab,
            Key::UpArrow => kVK_UpArrow,
            Key::Numpad0 => kVK_ANSI_Keypad0,
            Key::Numpad1 => kVK_ANSI_Keypad1,
            Key::Numpad2 => kVK_ANSI_Keypad2,
            Key::Numpad3 => kVK_ANSI_Keypad3,
            Key::Numpad4 => kVK_ANSI_Keypad4,
            Key::Numpad5 => kVK_ANSI_Keypad5,
            Key::Numpad6 => kVK_ANSI_Keypad6,
            Key::Numpad7 => kVK_ANSI_Keypad7,
            Key::Numpad8 => kVK_ANSI_Keypad8,
            Key::Numpad9 => kVK_ANSI_Keypad9,
            Key::Mute => kVK_Mute,
            Key::VolumeDown => kVK_VolumeUp,
            Key::VolumeUp => kVK_VolumeDown,
            Key::Help => kVK_Help,
            Key::Snapshot => kVK_F13,
            Key::Clear => kVK_ANSI_KeypadClear,
            Key::Decimal => kVK_ANSI_KeypadDecimal,
            Key::Multiply => kVK_ANSI_KeypadMultiply,
            Key::Add => kVK_ANSI_KeypadPlus,
            Key::Divide => kVK_ANSI_KeypadDivide,
            Key::NumpadEnter => kVK_ANSI_KeypadEnter,
            Key::Subtract => kVK_ANSI_KeypadMinus,
            Key::Equals => kVK_ANSI_KeypadEquals,
            Key::NumLock => kVK_ANSI_KeypadClear,

            Key::Raw(raw_keycode) => raw_keycode,
            Key::Layout(c) => self.get_layoutdependent_keycode(c.to_string()),

            Key::Super | Key::Command | Key::Windows | Key::Meta => kVK_Command,
            _ => 0,
        }
    }

    fn get_layoutdependent_keycode(&mut self, string: String) -> CGKeyCode {
        if self.keycode_to_string_map.is_empty() {
            self.init_map();
        }
        *self.keycode_to_string_map.get(&string).unwrap_or(&0)
    }

    fn init_map(&mut self) {
        self.keycode_to_string_map.insert("".to_owned(), 0);
        // loop through every keycode (0 - 127)
        for keycode in 0..128 {
            // no modifier
            if let Some(key_string) = self.keycode_to_string(keycode, 0x100) {
                self.keycode_to_string_map.insert(key_string, keycode);
            }

            // shift modifier
            if let Some(key_string) = self.keycode_to_string(keycode, 0x20102) {
                self.keycode_to_string_map.insert(key_string, keycode);
            }

            // alt modifier
            // if let Some(string) = self.keycode_to_string(keycode, 0x80120) {
            //     println!("{:?}", string);
            // }
            // alt + shift modifier
            // if let Some(string) = self.keycode_to_string(keycode, 0xa0122) {
            //     println!("{:?}", string);
            // }
        }
    }

    fn keycode_to_string(&self, keycode: u16, modifier: u32) -> Option<String> {
        let cf_string = self.create_string_for_key(keycode, modifier);
        unsafe {
            if !cf_string.is_null() {
                let mut buf: [i8; 255] = [0; 255];
                let success = CFStringGetCString(
                    cf_string,
                    buf.as_mut_ptr(),
                    buf.len() as _,
                    kCFStringEncodingUTF8,
                );
                if success != 0 {
                    let name: &CStr = CStr::from_ptr(buf.as_ptr());
                    if let Ok(name) = name.to_str() {
                        return Some(name.to_owned());
                    }
                }
            }
        }

        None
    }

    fn create_string_for_key(&self, keycode: u16, modifier: u32) -> CFStringRef {
        let current_keyboard = unsafe { TISCopyCurrentKeyboardInputSource() };
        let layout_data = unsafe {
            TISGetInputSourceProperty(current_keyboard, kTISPropertyUnicodeKeyLayoutData)
        };
        let keyboard_layout = unsafe { CFDataGetBytePtr(layout_data) };

        let mut keys_down: UInt32 = 0;
        // let mut chars: *mut c_void;//[UniChar; 4];
        let mut chars: u16 = 0;
        let mut real_length: UniCharCount = 0;
        unsafe {
            UCKeyTranslate(
                keyboard_layout,
                keycode,
                kUCKeyActionDisplay as u16,
                modifier,
                LMGetKbdType() as u32,
                kUCKeyTranslateNoDeadKeysBit as u32,
                &mut keys_down,
                8, // sizeof(chars) / sizeof(chars[0]),
                &mut real_length,
                &mut chars,
            );
        }

        unsafe { CFStringCreateWithCharacters(kCFAllocatorDefault, &chars, 1) }
    }
}

unsafe impl Send for Enigo {}
