#![allow(clippy::upper_case_acronyms)]
use crate::keycodes::macos::virtual_keycodes::*;
use crate::macos::keyboard::Keyboard;
use crate::rdev::{Button, Event, EventType, Key};
use cocoa::base::id;
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGKeyCode, EventField},
    event_source::CGEventSourceStateID,
};
use lazy_static::lazy_static;
use std::convert::TryInto;
use std::os::raw::c_void;
use std::sync::Mutex;
use std::time::SystemTime;

use crate::keycodes::macos::key_from_code;

pub type CFMachPortRef = *const c_void;
pub type CFIndex = u64;
pub type CFAllocatorRef = id;
pub type CFRunLoopSourceRef = id;
pub type CFRunLoopRef = id;
pub type CFRunLoopMode = id;
pub type CGEventTapProxy = id;
pub type CGEventRef = CGEvent;
pub type FourCharCode = ::std::os::raw::c_uint;
pub type OSType = FourCharCode;
pub type PhysicalKeyboardLayoutType = OSType;
pub type SInt16 = ::std::os::raw::c_short;

#[allow(non_upper_case_globals, dead_code)]
pub const kUnknownType: FourCharCode = 1061109567;
#[allow(non_upper_case_globals, dead_code)]
pub const kKeyboardJIS: PhysicalKeyboardLayoutType = 1246319392;
#[allow(non_upper_case_globals, dead_code)]
pub const kKeyboardANSI: PhysicalKeyboardLayoutType = 1095652169;
#[allow(non_upper_case_globals, dead_code)]
pub const kKeyboardISO: PhysicalKeyboardLayoutType = 1230196512;
#[allow(non_upper_case_globals, dead_code)]
pub const kKeyboardUnknown: PhysicalKeyboardLayoutType = 1061109567;

// https://developer.apple.com/documentation/coregraphics/cgeventtapplacement?language=objc
pub type CGEventTapPlacement = u32;
#[allow(non_upper_case_globals)]
pub const kCGHeadInsertEventTap: u32 = 0;

// https://developer.apple.com/documentation/coregraphics/cgeventtapoptions?language=objc
#[allow(non_upper_case_globals)]
#[repr(u32)]
pub enum CGEventTapOption {
    Default = 0,
    ListenOnly = 1,
}

pub static mut LAST_FLAGS: CGEventFlags = CGEventFlags::CGEventFlagNull;
lazy_static! {
    pub static ref KEYBOARD_STATE: Mutex<Option<Keyboard>> = Mutex::new(Keyboard::new());
}

// https://developer.apple.com/documentation/coregraphics/cgeventmask?language=objc
pub type CGEventMask = u64;
#[allow(non_upper_case_globals)]
pub const kCGEventMaskForAllEvents: u64 = (1 << CGEventType::LeftMouseDown as u64)
    + (1 << CGEventType::LeftMouseUp as u64)
    + (1 << CGEventType::RightMouseDown as u64)
    + (1 << CGEventType::RightMouseUp as u64)
    + (1 << CGEventType::OtherMouseDown as u64)
    + (1 << CGEventType::OtherMouseUp as u64)
    + (1 << CGEventType::MouseMoved as u64)
    + (1 << CGEventType::LeftMouseDragged as u64)
    + (1 << CGEventType::RightMouseDragged as u64)
    + (1 << CGEventType::KeyDown as u64)
    + (1 << CGEventType::KeyUp as u64)
    + (1 << CGEventType::FlagsChanged as u64)
    + (1 << CGEventType::ScrollWheel as u64);

#[cfg(target_os = "macos")]
#[link(name = "Cocoa", kind = "framework")]
extern "C" {
    #[allow(improper_ctypes)]
    pub fn CGEventTapCreate(
        tap: CGEventTapLocation,
        place: CGEventTapPlacement,
        options: CGEventTapOption,
        eventsOfInterest: CGEventMask,
        callback: QCallback,
        user_info: id,
    ) -> CFMachPortRef;
    pub fn CGEventSourceKeyState(state_id: CGEventSourceStateID, key: CGKeyCode) -> bool;
    pub fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        tap: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFRunLoopMode);
    pub fn CFRunLoopGetMain() -> CFRunLoopRef;
    pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    pub fn CFRunLoopRun();
    pub fn CFRunLoopStop(rl: CFRunLoopRef);

    pub static kCFRunLoopCommonModes: CFRunLoopMode;
}

#[allow(improper_ctypes)]
#[allow(non_snake_case)]
#[link(name = "Carbon", kind = "framework")]
extern "C" {
    pub fn LMGetKbdType() -> u8;
    pub fn KBGetLayoutType(iKeyboardType: SInt16) -> PhysicalKeyboardLayoutType;
}

pub type QCallback = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    _type: CGEventType,
    cg_event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

#[cfg(target_os = "macos")]
#[inline]
fn kb_get_layout_type() -> PhysicalKeyboardLayoutType {
    unsafe { KBGetLayoutType(LMGetKbdType() as _) }
}

#[cfg(target_os = "macos")]
#[allow(non_upper_case_globals)]
pub fn map_keycode(code: CGKeyCode) -> CGKeyCode {
    match code {
        kVK_ISO_Section => {
            if kb_get_layout_type() == kKeyboardISO {
                kVK_ANSI_Grave
            } else {
                kVK_ISO_Section
            }
        }
        kVK_ANSI_Grave => {
            if kb_get_layout_type() == kKeyboardISO {
                kVK_ISO_Section
            } else {
                kVK_ANSI_Grave
            }
        }
        _ => code,
    }
}

pub fn set_is_main_thread(b: bool) {
    if let Some(keyboard_state) = KEYBOARD_STATE.lock().unwrap().as_mut() {
        keyboard_state.set_is_main_thread(b);
    }
}

#[inline]
unsafe fn get_code(cg_event: &CGEvent) -> Option<CGKeyCode> {
    cg_event
        .get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
        .try_into()
        .ok()
}

pub unsafe fn convert(
    _type: CGEventType,
    cg_event: &CGEvent,
    keyboard_state: &mut Keyboard,
) -> Option<Event> {
    let mut code = 0;
    let option_type = match _type {
        CGEventType::TapDisabledByTimeout => {
            log::error!(
                "macOS event tap disabled by timeout; capture through this tap has stopped and pending key-up events may be lost"
            );
            None
        }
        CGEventType::TapDisabledByUserInput => {
            log::error!(
                "macOS event tap disabled by user input; capture through this tap has stopped and pending key-up events may be lost"
            );
            None
        }
        CGEventType::LeftMouseDown => Some(EventType::ButtonPress(Button::Left)),
        CGEventType::LeftMouseUp => Some(EventType::ButtonRelease(Button::Left)),
        CGEventType::RightMouseDown => Some(EventType::ButtonPress(Button::Right)),
        CGEventType::RightMouseUp => Some(EventType::ButtonRelease(Button::Right)),
        CGEventType::OtherMouseDown => {
            match cg_event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER) {
                2 => Some(EventType::ButtonPress(Button::Middle)),
                event => Some(EventType::ButtonPress(Button::Unknown(event as u8))),
            }
        }
        CGEventType::OtherMouseUp => {
            match cg_event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER) {
                2 => Some(EventType::ButtonRelease(Button::Middle)),
                event => Some(EventType::ButtonRelease(Button::Unknown(event as u8))),
            }
        }
        CGEventType::MouseMoved => {
            let point = cg_event.location();
            Some(EventType::MouseMove {
                x: point.x,
                y: point.y,
            })
        }
        CGEventType::KeyDown => {
            code = get_code(cg_event)?;
            Some(EventType::KeyPress(key_from_code(code)))
        }
        CGEventType::KeyUp => {
            code = get_code(cg_event)?;
            Some(EventType::KeyRelease(key_from_code(code)))
        }
        CGEventType::FlagsChanged => {
            code = get_code(cg_event)?;
            let flags = cg_event.get_flags();
            if flags < LAST_FLAGS {
                LAST_FLAGS = flags;
                Some(EventType::KeyRelease(key_from_code(code)))
            } else {
                LAST_FLAGS = flags;
                Some(EventType::KeyPress(key_from_code(code)))
            }
        }
        CGEventType::ScrollWheel => {
            let delta_y =
                cg_event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_1);
            let delta_x =
                cg_event.get_integer_value_field(EventField::SCROLL_WHEEL_EVENT_POINT_DELTA_AXIS_2);
            Some(EventType::Wheel { delta_x, delta_y })
        }
        _ => None,
    };
    if let Some(event_type) = option_type {
        let unicode = match event_type {
            EventType::KeyPress(..) => {
                let code =
                    cg_event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                #[allow(non_upper_case_globals)]
                let skip_unicode = match code as CGKeyCode {
                    kVK_Shift | kVK_RightShift | kVK_ForwardDelete => true,
                    _ => false,
                };
                if skip_unicode {
                    None
                } else {
                    let flags = cg_event.get_flags();
                    let s = keyboard_state.create_unicode_for_key(code, flags);
                    // if s.is_none() {
                    //     s = Some(key_to_name(_k).to_owned())
                    // }
                    s
                }
            }
            EventType::KeyRelease(..) => None,
            _ => None,
        };
        return Some(Event {
            event_type,
            time: SystemTime::now(),
            unicode,
            platform_code: code as _,
            position_code: 0 as _,
            usb_hid: 0,
            extra_data: cg_event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA),
        });
    }
    None
}

#[allow(dead_code)]
#[inline]
fn key_to_name(key: Key) -> &'static str {
    use Key::*;
    match key {
        KeyA => "a",
        KeyB => "b",
        KeyC => "c",
        KeyD => "d",
        KeyE => "e",
        KeyF => "f",
        KeyG => "g",
        KeyH => "h",
        KeyI => "i",
        KeyJ => "j",
        KeyK => "k",
        KeyL => "l",
        KeyM => "m",
        KeyN => "n",
        KeyO => "o",
        KeyP => "p",
        KeyQ => "q",
        KeyR => "r",
        KeyS => "s",
        KeyT => "t",
        KeyU => "u",
        KeyV => "v",
        KeyW => "w",
        KeyX => "x",
        KeyY => "y",
        KeyZ => "z",
        Num0 => "0",
        Num1 => "1",
        Num2 => "2",
        Num3 => "3",
        Num4 => "4",
        Num5 => "5",
        Num6 => "6",
        Num7 => "7",
        Num8 => "8",
        Num9 => "9",
        Minus => "-",
        Equal => "=",
        LeftBracket => "[",
        RightBracket => "]",
        BackSlash => "\\",
        SemiColon => ";",
        Quote => "\"",
        Comma => ",",
        Dot => ".",
        Slash => "/",
        BackQuote => "`",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[allow(non_snake_case)]
    fn test_KBGetLayoutType() {
        unsafe {
            let t1 = LMGetKbdType();
            let t2 = KBGetLayoutType(t1 as _);
            println!("LMGetKbdType: {}, KBGetLayoutType: {}", t1, t2);
        }
    }
}
