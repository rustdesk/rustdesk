use crate::keycodes::macos::{code_from_key, virtual_keycodes::*};
use crate::macos::common::CGEventSourceKeyState;
use crate::rdev::{Button, EventType, RawKey, SimulateError};
use core_graphics::{
    event::{
        CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton,
        EventField, ScrollEventUnit,
    },
    event_source::{CGEventSource, CGEventSourceStateID},
    geometry::CGPoint,
};
use std::convert::TryInto;

const SIMULATION_ERROR_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

macro_rules! log_simulation_error {
    ($($arg:tt)+) => {{
        static LOG_STATE: std::sync::Mutex<(Option<std::time::Instant>, u64)> =
            std::sync::Mutex::new((None, 0));
        let count = {
            let mut state = LOG_STATE.lock().unwrap();
            state.1 += 1;
            if state.0.map_or(true, |last| last.elapsed() >= SIMULATION_ERROR_LOG_INTERVAL) {
                state.0 = Some(std::time::Instant::now());
                Some(std::mem::replace(&mut state.1, 0))
            } else {
                None
            }
        };
        if let Some(count) = count {
            log::error!(
                "{} failures at this log site since last report; latest error: {}",
                count,
                format_args!($($arg)+)
            );
        }
    }};
}

static mut MOUSE_EXTRA_INFO: i64 = 0;
static mut KEYBOARD_EXTRA_INFO: i64 = 0;

pub fn set_mouse_extra_info(extra: i64) {
    unsafe { MOUSE_EXTRA_INFO = extra }
}

pub fn set_keyboard_extra_info(extra: i64) {
    unsafe { KEYBOARD_EXTRA_INFO = extra }
}

fn event_type_name(event_type: &EventType) -> &'static str {
    match event_type {
        EventType::KeyPress(_) => "KeyPress",
        EventType::KeyRelease(_) => "KeyRelease",
        EventType::ButtonPress(_) => "ButtonPress",
        EventType::ButtonRelease(_) => "ButtonRelease",
        EventType::MouseMove { .. } => "MouseMove",
        EventType::Wheel { .. } => "Wheel",
    }
}

// https://github.com/rustdesk/rustdesk/issues/16227
#[allow(non_upper_case_globals)]
fn normalize_text_key_event(event: CGEvent, keycode: CGKeyCode) -> CGEvent {
    // Clear only NumericPad on text-key down/up; preserve the keycode and all other bits.
    // No OS/layout/dead-key checks or character inspection: ordinary text keys count too.
    //
    // In RustDesk's Map path, the sender supplies the key position and down/up; Quartz sets
    // NumericPad on macOS. It identifies keypad events, including key-up, not Windows Num Lock.
    // On macOS 27, keypad 0 down/up left NumericPad in CombinedSessionState, and
    // CGEventCreateKeyboardEvent inherited it for the Spanish acute dead key. This retention
    // is observed behavior, not an API guarantee. NSMenu then indexed the dead key's empty
    // charactersIgnoringModifiers, raising NSInvalidArgumentException. Apple's guide checks
    // for empty strings before characterAtIndex:0 (Listing 5-4):
    // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/HandlingKeyEvents/HandlingKeyEvents.html
    //
    // Clearing only on keypad release would alter the keypad key-up's identification flag,
    // risking inconsistent down/up handling in apps. It would also miss overlapping input:
    // keypad 0 down -> acute down -> keypad 0 up; the dead key arrives before release cleanup.
    // These are compatibility/timing concerns; the release-only alternative is untested.
    // Existing arrow-key release workarounds are separate and are not extended to keypad keys.
    //
    // This helper leaves keypad/navigation/function keys and events without NumericPad unchanged.
    // Existing key-up workarounds still apply. Dead keys outside the filter and NumericPad added
    // later are not covered. Apps/IMEs/remappers using this flag on text keys may change text/shortcuts.
    // The ANSI range includes ISO Section; exclude Return/Tab. JIS Yen and Underscore are also text keys.
    let is_text_key = matches!(
        keycode,
        kVK_ANSI_A..=kVK_ANSI_Grave | kVK_JIS_Yen | kVK_JIS_Underscore
    ) && !matches!(keycode, kVK_Return | kVK_Tab);
    let flags = event.get_flags();
    if is_text_key && flags.contains(CGEventFlags::CGEventFlagNumericPad) {
        log::debug!("Clearing NumericPad flag from macOS text key event");
        // Sub preserves unknown system flags on older bitflags 1.x; ! truncates them.
        event.set_flags(flags - CGEventFlags::CGEventFlagNumericPad);
    }
    event
}

#[allow(non_upper_case_globals)]
fn workaround_fn(event: CGEvent, keycode: CGKeyCode) -> CGEvent {
    match keycode {
        // https://github.com/rustdesk/rustdesk/issues/10126
        // https://stackoverflow.com/questions/74938870/sticky-fn-after-home-is-simulated-programmatically-macos
        // `kVK_F20` does not stick `CGEventFlags::CGEventFlagSecondaryFn`
        kVK_F1 | kVK_F2 | kVK_F3 | kVK_F4 | kVK_F5 | kVK_F6 | kVK_F7 | kVK_F8 | kVK_F9
        | kVK_F10 | kVK_F11 | kVK_F12 | kVK_F13 | kVK_F14 | kVK_F15 | kVK_F16 | kVK_F17
        | kVK_F18 | kVK_F19 | kVK_ANSI_KeypadClear | kVK_ForwardDelete | kVK_Home
        | kVK_End | kVK_PageDown | kVK_PageUp
        | 129 // Spotlight Search
        | 130 // Application
        | 131 // Launchpad
        | 144 // Brightness Up
        | 145 // Brightness Down
        => {
            let flags = event.get_flags();
            event.set_flags(flags & (!(CGEventFlags::CGEventFlagSecondaryFn)));
        }
        kVK_UpArrow | kVK_DownArrow | kVK_LeftArrow | kVK_RightArrow => {
            let flags = event.get_flags();
            event.set_flags(
                flags
                    & (!(CGEventFlags::CGEventFlagSecondaryFn
                        | CGEventFlags::CGEventFlagNumericPad)),
            );
        }
        kVK_Help => {
            let flags = event.get_flags();
            event.set_flags(
                flags
                    & (!(CGEventFlags::CGEventFlagSecondaryFn
                        | CGEventFlags::CGEventFlagHelp)),
            );
        }
        _ => {}
    }
    normalize_text_key_event(event, keycode)
}

unsafe fn convert_native_with_source(
    event_type: &EventType,
    source: CGEventSource,
) -> Option<CGEvent> {
    match event_type {
        EventType::KeyPress(key) => match key {
            crate::Key::RawKey(rawkey) => {
                if let RawKey::MacVirtualKeycode(keycode) = rawkey {
                    CGEvent::new_keyboard_event(source, *keycode as _, true)
                        .map_err(|_| {
                            log_simulation_error!(
                                "Failed to create macOS {}: CGEventCreateKeyboardEvent returned null",
                                event_type_name(event_type)
                            );
                        })
                        // Don't use `workaround_fn()` for `KeyPress`, or `F11` will not work.
                        // .and_then(|event| Ok(workaround_fn(event, *keycode)))
                        .map(|event| normalize_text_key_event(event, *keycode))
                        .ok()
                } else {
                    log_simulation_error!(
                        "Failed to convert macOS {}: raw key is not a MacVirtualKeycode",
                        event_type_name(event_type)
                    );
                    None
                }
            }
            _ => {
                let code = code_from_key(*key).or_else(|| {
                    log_simulation_error!(
                        "Failed to convert macOS {}: no macOS keycode mapping",
                        event_type_name(event_type)
                    );
                    None
                })?;
                CGEvent::new_keyboard_event(source, code as _, true)
                    .map_err(|_| {
                        log_simulation_error!(
                            "Failed to create macOS {}: CGEventCreateKeyboardEvent returned null",
                            event_type_name(event_type)
                        );
                    })
                    // Don't use `workaround_fn()` for `KeyPress`, or `F11` will not work.
                    // .and_then(|event| Ok(workaround_fn(event, code as _)))
                    .map(|event| normalize_text_key_event(event, code))
                    .ok()
            }
        },
        EventType::KeyRelease(key) => match key {
            crate::Key::RawKey(rawkey) => {
                if let RawKey::MacVirtualKeycode(keycode) = rawkey {
                    CGEvent::new_keyboard_event(source, *keycode as _, false)
                        .map_err(|_| {
                            log_simulation_error!(
                                "Failed to create macOS {}: CGEventCreateKeyboardEvent returned null",
                                event_type_name(event_type)
                            );
                        })
                        .and_then(|event| Ok(workaround_fn(event, *keycode)))
                        .ok()
                } else {
                    log_simulation_error!(
                        "Failed to convert macOS {}: raw key is not a MacVirtualKeycode",
                        event_type_name(event_type)
                    );
                    None
                }
            }
            _ => {
                let code = code_from_key(*key).or_else(|| {
                    log_simulation_error!(
                        "Failed to convert macOS {}: no macOS keycode mapping",
                        event_type_name(event_type)
                    );
                    None
                })?;
                CGEvent::new_keyboard_event(source, code as _, false)
                    .map_err(|_| {
                        log_simulation_error!(
                            "Failed to create macOS {}: CGEventCreateKeyboardEvent returned null",
                            event_type_name(event_type)
                        );
                    })
                    .and_then(|event| Ok(workaround_fn(event, code as _)))
                    .ok()
            }
        },
        EventType::ButtonPress(button) => {
            let point = get_current_mouse_location()?;
            let event = match button {
                Button::Left => CGEventType::LeftMouseDown,
                Button::Right => CGEventType::RightMouseDown,
                _ => return None,
            };
            CGEvent::new_mouse_event(
                source,
                event,
                point,
                CGMouseButton::Left, // ignored because we don't use OtherMouse EventType
            )
            .ok()
        }
        EventType::ButtonRelease(button) => {
            let point = get_current_mouse_location()?;
            let event = match button {
                Button::Left => CGEventType::LeftMouseUp,
                Button::Right => CGEventType::RightMouseUp,
                _ => return None,
            };
            CGEvent::new_mouse_event(
                source,
                event,
                point,
                CGMouseButton::Left, // ignored because we don't use OtherMouse EventType
            )
            .ok()
        }
        EventType::MouseMove { x, y } => {
            let point = CGPoint { x: (*x), y: (*y) };
            CGEvent::new_mouse_event(source, CGEventType::MouseMoved, point, CGMouseButton::Left)
                .ok()
        }
        EventType::Wheel { delta_x, delta_y } => {
            let wheel_count = 2;
            CGEvent::new_scroll_event(
                source,
                ScrollEventUnit::PIXEL,
                wheel_count,
                (*delta_y).try_into().ok()?,
                (*delta_x).try_into().ok()?,
                0,
            )
            .ok()
        }
    }
}

unsafe fn convert_native(event_type: &EventType) -> Option<CGEvent> {
    // https://developer.apple.com/documentation/coregraphics/cgeventsourcestateid#:~:text=kCGEventSourceStatePrivate
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| log_simulation_error!(
            "Failed to create macOS input source for {}: CGEventSourceCreate(HIDSystemState) returned null",
            event_type_name(event_type)
        ))
        .ok()?;
    convert_native_with_source(event_type, source)
}

unsafe fn get_current_mouse_location() -> Option<CGPoint> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
    let event = CGEvent::new(source).ok()?;
    Some(event.location())
}

pub fn simulate(event_type: &EventType) -> Result<(), SimulateError> {
    unsafe {
        if let Some(cg_event) = convert_native(event_type) {
            cg_event.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, MOUSE_EXTRA_INFO);
            cg_event.post(CGEventTapLocation::HID);
            if matches!(
                event_type,
                EventType::KeyPress(_) | EventType::KeyRelease(_)
            ) {
                log::trace!(
                    "macOS CGEventPost returned for {} at HID; delivery is unconfirmed",
                    event_type_name(event_type)
                );
            }
            Ok(())
        } else {
            Err(SimulateError)
        }
    }
}

pub struct VirtualInput {
    source: CGEventSource,
    tap_loc: CGEventTapLocation,
}

impl VirtualInput {
    pub fn new(state_id: CGEventSourceStateID, tap_loc: CGEventTapLocation) -> Result<Self, ()> {
        Ok(Self {
            source: CGEventSource::new(state_id).inspect_err(|_| {
                log_simulation_error!(
                    "Failed to create macOS input source {:?}: CGEventSourceCreate returned null",
                    state_id
                );
            })?,
            tap_loc,
        })
    }

    pub fn simulate(&self, event_type: &EventType) -> Result<(), SimulateError> {
        unsafe {
            if let Some(cg_event) = convert_native_with_source(event_type, self.source.clone()) {
                cg_event.post(self.tap_loc);
                if matches!(
                    event_type,
                    EventType::KeyPress(_) | EventType::KeyRelease(_)
                ) {
                    log::trace!(
                        "macOS CGEventPost returned for {} at {:?}; delivery is unconfirmed",
                        event_type_name(event_type),
                        self.tap_loc
                    );
                }
                Ok(())
            } else {
                Err(SimulateError)
            }
        }
    }

    // keycode is defined in rdev::macos::virtual_keycodes
    pub fn get_key_state(state_id: CGEventSourceStateID, keycode: CGKeyCode) -> bool {
        unsafe { CGEventSourceKeyState(state_id, keycode) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_with_numeric_pad(keycode: CGKeyCode, down: bool) -> CGEvent {
        const SYSTEM_EVENT_FLAG: u64 = 0x20000000;
        let source = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
        let event = CGEvent::new_keyboard_event(source, keycode, down).unwrap();
        let flags = CGEventFlags::CGEventFlagNumericPad
            | CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagSecondaryFn;
        // SAFETY: CGEventFlags is a repr(C) u64 wrapper, including with bitflags 1.0.
        event.set_flags(unsafe { std::mem::transmute(flags.bits() | SYSTEM_EVENT_FLAG) });
        event
    }

    #[test]
    fn text_key_events_clear_only_numeric_pad() {
        for down in [false, true] {
            let event = event_with_numeric_pad(kVK_ANSI_Quote, down);
            let bits = event.get_flags().bits();
            let event = if down {
                normalize_text_key_event(event, kVK_ANSI_Quote)
            } else {
                workaround_fn(event, kVK_ANSI_Quote)
            };
            assert_eq!(
                event.get_flags().bits(),
                bits & !CGEventFlags::CGEventFlagNumericPad.bits(),
                "key down: {down}"
            );
        }
    }

    #[test]
    fn normalization_preserves_non_text_key_flags() {
        let keys = [
            kVK_ANSI_Keypad0,
            kVK_LeftArrow,
            kVK_F11,
            kVK_Return,
            kVK_Tab,
        ];
        for keycode in keys {
            let event = event_with_numeric_pad(keycode, true);
            let flags = event.get_flags();
            let event = normalize_text_key_event(event, keycode);
            assert_eq!(event.get_flags(), flags, "keycode {keycode}");
        }
    }
}
