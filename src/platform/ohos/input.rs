use std::{
    ffi::CString,
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
    time::{Duration, Instant},
};

use hbb_common::message_proto::{key_event, ControlKey, KeyEvent, KeyboardMode};

const INPUT_SUCCESS: i32 = 0;
const INPUT_INJECTION_AUTHORIZING: i32 = 3_900_005;
const INPUT_INJECTION_AUTHORIZED: i32 = 3_900_007;
const AUTHORIZED: i32 = 2;
const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(120);
const LOG_INFO: i32 = 4;
const LOG_ERROR: i32 = 6;

const KEY_ACTION_DOWN: i32 = 1;
const KEY_ACTION_UP: i32 = 2;
const MOUSE_ACTION_MOVE: i32 = 1;
const MOUSE_ACTION_BUTTON_DOWN: i32 = 2;
const MOUSE_ACTION_BUTTON_UP: i32 = 3;
const MOUSE_ACTION_AXIS_BEGIN: i32 = 4;
const MOUSE_ACTION_AXIS_UPDATE: i32 = 5;
const MOUSE_ACTION_AXIS_END: i32 = 6;
const MOUSE_AXIS_SCROLL_VERTICAL: i32 = 0;
const MOUSE_AXIS_SCROLL_HORIZONTAL: i32 = 1;
const MOUSE_BUTTON_NONE: i32 = -1;
const MOUSE_BUTTON_LEFT: i32 = 0;
const MOUSE_BUTTON_MIDDLE: i32 = 1;
const MOUSE_BUTTON_RIGHT: i32 = 2;
const MOUSE_BUTTON_FORWARD: i32 = 3;
const MOUSE_BUTTON_BACK: i32 = 4;

#[repr(C)]
struct InputKeyEvent {
    _private: [u8; 0],
}

#[repr(C)]
struct InputMouseEvent {
    _private: [u8; 0],
}

#[link(name = "ohinput")]
unsafe extern "C" {
    fn OH_Input_RequestInjection(callback: unsafe extern "C" fn(i32)) -> i32;
    fn OH_Input_QueryAuthorizedStatus(status: *mut i32) -> i32;
    fn OH_Input_CancelInjection();

    fn OH_Input_CreateKeyEvent() -> *mut InputKeyEvent;
    fn OH_Input_DestroyKeyEvent(event: *mut *mut InputKeyEvent);
    fn OH_Input_SetKeyEventAction(event: *mut InputKeyEvent, action: i32);
    fn OH_Input_SetKeyEventKeyCode(event: *mut InputKeyEvent, key_code: i32);
    fn OH_Input_SetKeyEventActionTime(event: *mut InputKeyEvent, action_time: i64);
    fn OH_Input_SetKeyEventDisplayId(event: *mut InputKeyEvent, display_id: i32);
    fn OH_Input_InjectKeyEvent(event: *const InputKeyEvent) -> i32;

    fn OH_Input_CreateMouseEvent() -> *mut InputMouseEvent;
    fn OH_Input_DestroyMouseEvent(event: *mut *mut InputMouseEvent);
    fn OH_Input_SetMouseEventAction(event: *mut InputMouseEvent, action: i32);
    fn OH_Input_SetMouseEventDisplayX(event: *mut InputMouseEvent, x: i32);
    fn OH_Input_SetMouseEventDisplayY(event: *mut InputMouseEvent, y: i32);
    fn OH_Input_SetMouseEventGlobalX(event: *mut InputMouseEvent, x: i32);
    fn OH_Input_SetMouseEventGlobalY(event: *mut InputMouseEvent, y: i32);
    fn OH_Input_SetMouseEventButton(event: *mut InputMouseEvent, button: i32);
    fn OH_Input_SetMouseEventAxisType(event: *mut InputMouseEvent, axis_type: i32);
    fn OH_Input_SetMouseEventAxisValue(event: *mut InputMouseEvent, value: f32);
    fn OH_Input_SetMouseEventActionTime(event: *mut InputMouseEvent, action_time: i64);
    fn OH_Input_SetMouseEventDisplayId(event: *mut InputMouseEvent, display_id: i32);
    fn OH_Input_InjectMouseEventGlobal(event: *const InputMouseEvent) -> i32;
}

#[link(name = "hilog_ndk.z")]
unsafe extern "C" {
    fn OH_LOG_PrintMsg(
        log_type: i32,
        level: i32,
        domain: u32,
        tag: *const std::ffi::c_char,
        message: *const std::ffi::c_char,
    ) -> i32;
}

static AUTHORIZATION_RESULT: AtomicI32 = AtomicI32::new(-1);
static INPUT_AUTHORIZED: AtomicBool = AtomicBool::new(false);
static INPUT_ERROR_REPORTED: AtomicBool = AtomicBool::new(false);
static INPUT_SUCCESS_REPORTED: AtomicBool = AtomicBool::new(false);

fn system_log(level: i32, message: &str) {
    let Ok(message) = CString::new(message) else {
        return;
    };
    unsafe {
        OH_LOG_PrintMsg(0, level, 0xD00, c"RustDeskInput".as_ptr(), message.as_ptr());
    }
}

unsafe extern "C" fn authorization_callback(status: i32) {
    AUTHORIZATION_RESULT.store(status, Ordering::Release);
    INPUT_AUTHORIZED.store(status == AUTHORIZED, Ordering::Release);
}

pub(super) fn request_authorization() -> Result<(), String> {
    let mut status = 0;
    let query_result = unsafe { OH_Input_QueryAuthorizedStatus(&mut status) };
    if query_result == INPUT_SUCCESS && status == AUTHORIZED {
        INPUT_AUTHORIZED.store(true, Ordering::Release);
        INPUT_ERROR_REPORTED.store(false, Ordering::Release);
        system_log(
            LOG_INFO,
            "HarmonyOS input injection authorization is active",
        );
        return Ok(());
    }

    AUTHORIZATION_RESULT.store(-1, Ordering::Release);
    INPUT_AUTHORIZED.store(false, Ordering::Release);
    let request_result = unsafe { OH_Input_RequestInjection(authorization_callback) };
    if request_result == INPUT_INJECTION_AUTHORIZED {
        INPUT_AUTHORIZED.store(true, Ordering::Release);
        INPUT_ERROR_REPORTED.store(false, Ordering::Release);
        system_log(
            LOG_INFO,
            "HarmonyOS input injection authorization is active",
        );
        return Ok(());
    }
    if request_result != INPUT_SUCCESS && request_result != INPUT_INJECTION_AUTHORIZING {
        return Err(format!(
            "OH_Input_RequestInjection failed: {request_result}"
        ));
    }

    let deadline = Instant::now() + AUTHORIZATION_TIMEOUT;
    loop {
        let result = AUTHORIZATION_RESULT.load(Ordering::Acquire);
        if result == AUTHORIZED {
            INPUT_AUTHORIZED.store(true, Ordering::Release);
            INPUT_ERROR_REPORTED.store(false, Ordering::Release);
            system_log(
                LOG_INFO,
                "HarmonyOS input injection authorization is active",
            );
            return Ok(());
        }
        if result == 0 {
            return Err("HarmonyOS input injection authorization was denied".to_owned());
        }
        if Instant::now() >= deadline {
            return Err("HarmonyOS input injection authorization timed out".to_owned());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(super) fn cancel_authorization() {
    unsafe {
        OH_Input_CancelInjection();
    }
    AUTHORIZATION_RESULT.store(-1, Ordering::Release);
    INPUT_AUTHORIZED.store(false, Ordering::Release);
    INPUT_ERROR_REPORTED.store(false, Ordering::Release);
    INPUT_SUCCESS_REPORTED.store(false, Ordering::Release);
}

pub(super) fn is_authorized() -> bool {
    INPUT_AUTHORIZED.load(Ordering::Acquire)
}

fn report_input_error(error: &str) {
    INPUT_AUTHORIZED.store(false, Ordering::Release);
    crate::ui_cm_interface::switch_permission_all("keyboard".to_owned(), false);
    if !INPUT_ERROR_REPORTED.swap(true, Ordering::AcqRel) {
        hbb_common::log::error!("HarmonyOS input injection failed: {error}");
        system_log(
            LOG_ERROR,
            &format!("HarmonyOS input injection failed: {error}"),
        );
    }
}

fn report_input_success(kind: &str) {
    if !INPUT_SUCCESS_REPORTED.swap(true, Ordering::AcqRel) {
        system_log(LOG_INFO, &format!("HarmonyOS {kind} injection succeeded"));
    }
}

pub(super) enum InjectionResult {
    Injected,
    RetryInFrontend,
    Partial,
}

pub(super) fn inject_pointer(
    kind: &str,
    mask: i32,
    x: i32,
    y: i32,
    display_id: u64,
) -> InjectionResult {
    if !is_authorized() {
        return InjectionResult::RetryInFrontend;
    }
    let event_type = mask & crate::common::input::MOUSE_TYPE_MASK;
    let result = if event_type == crate::common::input::MOUSE_TYPE_WHEEL
        || event_type == crate::common::input::MOUSE_TYPE_TRACKPAD
    {
        inject_scroll(x, y, display_id)
    } else {
        let action = match event_type {
            crate::common::input::MOUSE_TYPE_MOVE => MOUSE_ACTION_MOVE,
            crate::common::input::MOUSE_TYPE_DOWN => MOUSE_ACTION_BUTTON_DOWN,
            crate::common::input::MOUSE_TYPE_UP => MOUSE_ACTION_BUTTON_UP,
            _ => return InjectionResult::Injected,
        };
        let button = if action == MOUSE_ACTION_MOVE {
            MOUSE_BUTTON_NONE
        } else {
            mouse_button(mask >> 3)
        };
        inject_mouse(action, button, x, y, display_id, None)
    };
    match result {
        Ok(()) => {
            report_input_success(kind);
            InjectionResult::Injected
        }
        Err(error) => {
            report_input_error(&format!("{kind} event: {error}"));
            InjectionResult::Partial
        }
    }
}

fn mouse_button(buttons: i32) -> i32 {
    if buttons & crate::common::input::MOUSE_BUTTON_LEFT != 0 {
        MOUSE_BUTTON_LEFT
    } else if buttons & crate::common::input::MOUSE_BUTTON_RIGHT != 0 {
        MOUSE_BUTTON_RIGHT
    } else if buttons & crate::common::input::MOUSE_BUTTON_WHEEL != 0 {
        MOUSE_BUTTON_MIDDLE
    } else if buttons & crate::common::input::MOUSE_BUTTON_FORWARD != 0 {
        MOUSE_BUTTON_FORWARD
    } else if buttons & crate::common::input::MOUSE_BUTTON_BACK != 0 {
        MOUSE_BUTTON_BACK
    } else {
        MOUSE_BUTTON_NONE
    }
}

fn inject_scroll(x: i32, y: i32, display_id: u64) -> Result<(), String> {
    if y != 0 {
        inject_axis(MOUSE_AXIS_SCROLL_VERTICAL, -(y as f32), display_id)?;
    }
    if x != 0 {
        inject_axis(MOUSE_AXIS_SCROLL_HORIZONTAL, -(x as f32), display_id)?;
    }
    Ok(())
}

fn inject_axis(axis_type: i32, value: f32, display_id: u64) -> Result<(), String> {
    for action in [
        MOUSE_ACTION_AXIS_BEGIN,
        MOUSE_ACTION_AXIS_UPDATE,
        MOUSE_ACTION_AXIS_END,
    ] {
        inject_mouse(
            action,
            MOUSE_BUTTON_NONE,
            0,
            0,
            display_id,
            Some((axis_type, value)),
        )?;
    }
    Ok(())
}

fn inject_mouse(
    action: i32,
    button: i32,
    x: i32,
    y: i32,
    display_id: u64,
    axis: Option<(i32, f32)>,
) -> Result<(), String> {
    let mut event = unsafe { OH_Input_CreateMouseEvent() };
    if event.is_null() {
        return Err("OH_Input_CreateMouseEvent returned null".to_owned());
    }
    unsafe {
        OH_Input_SetMouseEventAction(event, action);
        OH_Input_SetMouseEventDisplayX(event, x);
        OH_Input_SetMouseEventDisplayY(event, y);
        OH_Input_SetMouseEventGlobalX(event, x);
        OH_Input_SetMouseEventGlobalY(event, y);
        OH_Input_SetMouseEventButton(event, button);
        OH_Input_SetMouseEventActionTime(event, hbb_common::get_time() as i64);
        OH_Input_SetMouseEventDisplayId(event, display_id as i32);
        if let Some((axis_type, value)) = axis {
            OH_Input_SetMouseEventAxisType(event, axis_type);
            OH_Input_SetMouseEventAxisValue(event, value);
        }
    }
    let result = unsafe { OH_Input_InjectMouseEventGlobal(event) };
    unsafe {
        OH_Input_DestroyMouseEvent(&mut event);
    }
    if result == INPUT_SUCCESS {
        Ok(())
    } else {
        Err(format!("OH_Input_InjectMouseEventGlobal failed: {result}"))
    }
}

pub(super) fn inject_key(event: &KeyEvent, display_id: u64) -> InjectionResult {
    if !is_authorized() {
        return InjectionResult::RetryInFrontend;
    }
    let Some(key_code) = key_code(event) else {
        return InjectionResult::Injected;
    };
    if event.press {
        if let Err(error) = inject_key_action(key_code, KEY_ACTION_DOWN, display_id) {
            report_input_error(&error);
            return InjectionResult::RetryInFrontend;
        }
        if let Err(error) = inject_key_action(key_code, KEY_ACTION_UP, display_id) {
            if let Err(retry_error) = inject_key_action(key_code, KEY_ACTION_UP, display_id) {
                report_input_error(&format!(
                    "key release failed ({error}); compensating release failed ({retry_error})"
                ));
                return InjectionResult::Partial;
            }
        }
    } else {
        if let Err(error) = inject_key_action(
            key_code,
            if event.down {
                KEY_ACTION_DOWN
            } else {
                KEY_ACTION_UP
            },
            display_id,
        ) {
            report_input_error(&error);
            return InjectionResult::RetryInFrontend;
        }
    }
    report_input_success("key");
    InjectionResult::Injected
}

fn inject_key_action(key_code: i32, action: i32, display_id: u64) -> Result<(), String> {
    let mut event = unsafe { OH_Input_CreateKeyEvent() };
    if event.is_null() {
        return Err("OH_Input_CreateKeyEvent returned null".to_owned());
    }
    unsafe {
        OH_Input_SetKeyEventAction(event, action);
        OH_Input_SetKeyEventKeyCode(event, key_code);
        OH_Input_SetKeyEventActionTime(event, hbb_common::get_time() as i64);
        OH_Input_SetKeyEventDisplayId(event, display_id as i32);
    }
    let result = unsafe { OH_Input_InjectKeyEvent(event) };
    unsafe {
        OH_Input_DestroyKeyEvent(&mut event);
    }
    if result == INPUT_SUCCESS {
        Ok(())
    } else {
        Err(format!("OH_Input_InjectKeyEvent failed: {result}"))
    }
}

fn key_code(event: &KeyEvent) -> Option<i32> {
    match event.union.as_ref()? {
        key_event::Union::ControlKey(value) => value.enum_value().ok().and_then(control_key_code),
        key_event::Union::Chr(value) => {
            if event.mode.enum_value() == Ok(KeyboardMode::Map) {
                linux_key_code(*value)
            } else {
                char_key_code(char::from_u32(*value & 0xffff)?)
            }
        }
        key_event::Union::Unicode(value) => char_key_code(char::from_u32(*value)?),
        key_event::Union::Seq(value) => {
            let mut chars = value.chars();
            let value = chars.next()?;
            if chars.next().is_none() {
                char_key_code(value)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn control_key_code(key: ControlKey) -> Option<i32> {
    Some(match key {
        ControlKey::Alt | ControlKey::Option => 2045,
        ControlKey::RAlt => 2046,
        ControlKey::Backspace => 2055,
        ControlKey::CapsLock => 2074,
        ControlKey::Control => 2072,
        ControlKey::RControl => 2073,
        ControlKey::Delete => 2071,
        ControlKey::DownArrow => 2013,
        ControlKey::End => 2082,
        ControlKey::Escape => 2070,
        ControlKey::F1 => 2090,
        ControlKey::F2 => 2091,
        ControlKey::F3 => 2092,
        ControlKey::F4 => 2093,
        ControlKey::F5 => 2094,
        ControlKey::F6 => 2095,
        ControlKey::F7 => 2096,
        ControlKey::F8 => 2097,
        ControlKey::F9 => 2098,
        ControlKey::F10 => 2099,
        ControlKey::F11 => 2100,
        ControlKey::F12 => 2101,
        ControlKey::Home => 2081,
        ControlKey::LeftArrow => 2014,
        ControlKey::Meta => 2076,
        ControlKey::RWin => 2077,
        ControlKey::PageDown => 2069,
        ControlKey::PageUp => 2068,
        ControlKey::Return => 2054,
        ControlKey::RightArrow => 2015,
        ControlKey::Shift => 2047,
        ControlKey::RShift => 2048,
        ControlKey::Space => 2050,
        ControlKey::Tab => 2049,
        ControlKey::UpArrow => 2012,
        ControlKey::Numpad0 => 2103,
        ControlKey::Numpad1 => 2104,
        ControlKey::Numpad2 => 2105,
        ControlKey::Numpad3 => 2106,
        ControlKey::Numpad4 => 2107,
        ControlKey::Numpad5 => 2108,
        ControlKey::Numpad6 => 2109,
        ControlKey::Numpad7 => 2110,
        ControlKey::Numpad8 => 2111,
        ControlKey::Numpad9 => 2112,
        ControlKey::Pause => 2080,
        ControlKey::Snapshot | ControlKey::Print => 2079,
        ControlKey::Insert => 2083,
        ControlKey::Scroll => 2075,
        ControlKey::NumLock => 2102,
        ControlKey::Multiply => 2114,
        ControlKey::Add => 2116,
        ControlKey::Subtract => 2115,
        ControlKey::Decimal => 2117,
        ControlKey::Divide => 2113,
        ControlKey::Equals => 2120,
        ControlKey::NumpadEnter => 2119,
        ControlKey::VolumeMute => 22,
        ControlKey::VolumeUp => 16,
        ControlKey::VolumeDown => 17,
        ControlKey::Power => 18,
        _ => return None,
    })
}

fn char_key_code(value: char) -> Option<i32> {
    match value {
        'a'..='z' => Some(2017 + (value as i32 - 'a' as i32)),
        'A'..='Z' => Some(2017 + (value as i32 - 'A' as i32)),
        '0'..='9' => Some(2000 + (value as i32 - '0' as i32)),
        ',' => Some(2043),
        '.' => Some(2044),
        ' ' => Some(2050),
        '\t' => Some(2049),
        '\n' | '\r' => Some(2054),
        '`' => Some(2056),
        '-' => Some(2057),
        '=' => Some(2058),
        '[' => Some(2059),
        ']' => Some(2060),
        '\\' => Some(2061),
        ';' => Some(2062),
        '\'' => Some(2063),
        '/' => Some(2064),
        '@' => Some(2065),
        '+' => Some(2066),
        _ => None,
    }
}

fn linux_key_code(value: u32) -> Option<i32> {
    match value {
        1 => Some(2070),
        2..=11 => Some(2001 + value as i32 - 2),
        12 => Some(2057),
        13 => Some(2058),
        14 => Some(2055),
        15 => Some(2049),
        16..=25 => Some(2033 + value as i32 - 16),
        26 => Some(2059),
        27 => Some(2060),
        28 => Some(2054),
        29 => Some(2072),
        30..=38 => Some(2017 + value as i32 - 30),
        39 => Some(2062),
        40 => Some(2063),
        41 => Some(2056),
        42 => Some(2047),
        43 => Some(2061),
        44 => Some(2042),
        45 => Some(2040),
        46 => Some(2019),
        47 => Some(2038),
        48 => Some(2018),
        49 => Some(2030),
        50 => Some(2029),
        51 => Some(2043),
        52 => Some(2044),
        53 => Some(2064),
        54 => Some(2048),
        55 => Some(2114),
        56 => Some(2045),
        57 => Some(2050),
        58 => Some(2074),
        59..=68 => Some(2090 + value as i32 - 59),
        69 => Some(2102),
        70 => Some(2075),
        71 => Some(2110),
        72 => Some(2111),
        73 => Some(2112),
        74 => Some(2115),
        75 => Some(2107),
        76 => Some(2108),
        77 => Some(2109),
        78 => Some(2116),
        79 => Some(2104),
        80 => Some(2105),
        81 => Some(2106),
        82 => Some(2103),
        83 => Some(2117),
        87 => Some(2100),
        88 => Some(2101),
        96 => Some(2119),
        97 => Some(2073),
        98 => Some(2113),
        100 => Some(2046),
        102 => Some(2081),
        103 => Some(2012),
        104 => Some(2068),
        105 => Some(2014),
        106 => Some(2015),
        107 => Some(2082),
        108 => Some(2013),
        109 => Some(2069),
        110 => Some(2083),
        111 => Some(2071),
        125 => Some(2076),
        126 => Some(2077),
        _ => None,
    }
}
