use crate::rdev::{Button, EventType, RawKey, SimulateError};
use crate::keycodes::windows::{get_win_codes, scancode_from_key};
use crate::Key;
use std::convert::TryFrom;
use std::mem::size_of;
use std::ptr::null_mut;
use winapi::ctypes::{c_int, c_short};
use winapi::shared::minwindef::{DWORD, HKL, LOWORD, UINT, WORD};
use winapi::shared::ntdef::LONG;
use winapi::um::winuser::{
    GetForegroundWindow, GetKeyboardLayout, GetSystemMetrics, GetWindowThreadProcessId, INPUT_u,
    MapVirtualKeyExW, SendInput, VkKeyScanExW, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC,
    MAPVK_VSC_TO_VK_EX, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL,
    MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    WHEEL_DELTA,
};
/// Not defined in win32 but define here for clarity
#[allow(dead_code)]
static KEYEVENTF_KEYDOWN: DWORD = 0;
// KEYBDINPUT
static mut DW_MOUSE_EXTRA_INFO: usize = 0;
static mut DW_KEYBOARD_EXTRA_INFO: usize = 0;

pub fn set_mouse_extra_info(extra: usize) {
    unsafe { DW_MOUSE_EXTRA_INFO = extra }
}

pub fn set_keyboard_extra_info(extra: usize) {
    unsafe { DW_KEYBOARD_EXTRA_INFO = extra }
}

fn sim_mouse_event(flags: DWORD, data: DWORD, dx: LONG, dy: LONG) -> Result<(), SimulateError> {
    let mut union: INPUT_u = unsafe { std::mem::zeroed() };
    let inner_union = unsafe { union.mi_mut() };
    unsafe {
        *inner_union = MOUSEINPUT {
            dx,
            dy,
            mouseData: data,
            dwFlags: flags,
            time: 0,
            dwExtraInfo: DW_MOUSE_EXTRA_INFO,
        };
    }
    let mut input = [INPUT {
        type_: INPUT_MOUSE,
        u: union,
    }; 1];
    let value = unsafe {
        SendInput(
            input.len() as UINT,
            input.as_mut_ptr(),
            size_of::<INPUT>() as c_int,
        )
    };
    if value != 1 {
        Err(SimulateError)
    } else {
        Ok(())
    }
}

fn sim_keyboard_event(flags: DWORD, vk: WORD, scan: WORD) -> Result<(), SimulateError> {
    let mut union: INPUT_u = unsafe { std::mem::zeroed() };
    let inner_union = unsafe { union.ki_mut() };
    unsafe {
        *inner_union = KEYBDINPUT {
            wVk: vk,
            wScan: scan,
            dwFlags: flags,
            time: 0,
            dwExtraInfo: DW_KEYBOARD_EXTRA_INFO,
        };
    }
    let mut input = [INPUT {
        type_: INPUT_KEYBOARD,
        u: union,
    }; 1];
    let value = unsafe {
        SendInput(
            input.len() as UINT,
            input.as_mut_ptr(),
            size_of::<INPUT>() as c_int,
        )
    };
    if value != 1 {
        Err(SimulateError)
    } else {
        Ok(())
    }
}

#[inline]
fn get_layout() -> HKL {
    unsafe {
        let current_window_thread_id = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
        GetKeyboardLayout(current_window_thread_id)
    }
}

fn simulate_key_event_rawkey(key: &RawKey, is_press: bool) -> Result<(), SimulateError> {
    match key {
        RawKey::ScanCode(scancode) => simulate_code(None, Some(*scancode), is_press),
        RawKey::WinVirtualKeycode(vk) => {
            let scancode =
                unsafe { MapVirtualKeyExW(*vk as _, MAPVK_VK_TO_VSC, get_layout()) as _ };
            simulate_code(None, Some(scancode), is_press)
        }
        _ => Err(SimulateError),
    }
}

fn simulate_key_event_not_rawkey(key: &Key, is_press: bool) -> Result<(), SimulateError> {
    let layout = get_layout();
    let (vk, scan) = {
        let (code, scancode) = get_win_codes(*key).ok_or(SimulateError)?;
        if scancode != 0 {
            (None, Some(scancode))
        } else {
            let code = if code == 165 && LOWORD(layout as usize as u32) == 0x0412 {
                winapi::um::winuser::VK_HANGUL as u32
            } else if scancode != 0 {
                unsafe { MapVirtualKeyExW(scancode as _, MAPVK_VSC_TO_VK_EX, layout) }
            } else {
                code
            };
            (Some(code as _), None)
        }
    };
    simulate_code(vk, scan, is_press)
}

pub fn simulate(event_type: &EventType) -> Result<(), SimulateError> {
    match event_type {
        EventType::KeyPress(key) => match key {
            crate::Key::RawKey(raw_key) => simulate_key_event_rawkey(raw_key, true),
            _ => simulate_key_event_not_rawkey(key, true),
        },
        EventType::KeyRelease(key) => match key {
            crate::Key::RawKey(raw_key) => simulate_key_event_rawkey(raw_key, false),
            _ => simulate_key_event_not_rawkey(key, false),
        },
        EventType::ButtonPress(button) => match button {
            Button::Left => sim_mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0),
            Button::Middle => sim_mouse_event(MOUSEEVENTF_MIDDLEDOWN, 0, 0, 0),
            Button::Right => sim_mouse_event(MOUSEEVENTF_RIGHTDOWN, 0, 0, 0),
            Button::Unknown(code) => sim_mouse_event(MOUSEEVENTF_XDOWN, 0, 0, (*code).into()),
        },
        EventType::ButtonRelease(button) => match button {
            Button::Left => sim_mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0),
            Button::Middle => sim_mouse_event(MOUSEEVENTF_MIDDLEUP, 0, 0, 0),
            Button::Right => sim_mouse_event(MOUSEEVENTF_RIGHTUP, 0, 0, 0),
            Button::Unknown(code) => sim_mouse_event(MOUSEEVENTF_XUP, 0, 0, (*code).into()),
        },
        EventType::Wheel { delta_x, delta_y } => {
            if *delta_x != 0 {
                sim_mouse_event(
                    MOUSEEVENTF_HWHEEL,
                    (c_short::try_from(*delta_x).map_err(|_| SimulateError)? * WHEEL_DELTA) as u32,
                    0,
                    0,
                )?;
            }

            if *delta_y != 0 {
                sim_mouse_event(
                    MOUSEEVENTF_WHEEL,
                    (c_short::try_from(*delta_y).map_err(|_| SimulateError)? * WHEEL_DELTA) as u32,
                    0,
                    0,
                )?;
            }
            Ok(())
        }
        EventType::MouseMove { x, y } => {
            let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
            let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
            if width == 0 || height == 0 {
                return Err(SimulateError);
            }

            sim_mouse_event(
                MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                0,
                (*x as i32 + 1) * 65535 / width,
                (*y as i32 + 1) * 65535 / height,
            )
        }
    }
}

pub fn simulate_code(
    vk: Option<u16>,
    scan: Option<u32>,
    pressed: bool,
) -> Result<(), SimulateError> {
    let keycode;
    let scancode;
    let mut flags;

    if let Some(scan) = scan {
        keycode = 0;
        scancode = scan;
        flags = KEYEVENTF_SCANCODE;
    } else if let Some(vk) = vk {
        keycode = vk;
        scancode = 0;
        flags = 0;
    } else {
        return Err(SimulateError);
    }

    if (scancode >> 8) == 0xE0 || (scancode >> 8) == 0xE1 {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }

    if !pressed {
        flags |= KEYEVENTF_KEYUP;
    }
    sim_keyboard_event(flags as _, keycode, scancode as _)
}

pub fn simulate_key_unicode(unicode_16: u16, try_unicode: bool) -> Result<(), SimulateError> {
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-vkkeyscanexw
    let current_window_thread_id =
        unsafe { GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut()) };
    let layout = unsafe { GetKeyboardLayout(current_window_thread_id) };
    let res = unsafe { VkKeyScanExW(unicode_16, layout) as u16 };
    if res == 0xFFFF {
        if try_unicode {
            simulate_unicode(unicode_16)
        } else {
            Err(SimulateError)
        }
    } else {
        let vk = res & 0x00FF;
        let flag = res >> 8;
        let modifiers_scancode = [
            scancode_from_key(Key::ShiftLeft).unwrap_or(0x2A),
            scancode_from_key(Key::ControlLeft).unwrap_or(0x1D),
            scancode_from_key(Key::Alt).unwrap_or(0x38),
        ];
        let mod_len = modifiers_scancode.len();
        for pos in 0..mod_len {
            if flag & (0x0001 << pos) != 0 {
                let _ = simulate_code(None, Some(modifiers_scancode[pos]), true);
            }
        }
        let scan = unsafe { MapVirtualKeyExW(vk as _, MAPVK_VK_TO_VSC, layout) as _ };
        let down_res = simulate_code(Some(vk as _), Some(scan), true);
        let _ = simulate_code(Some(vk as _), Some(scan), false);
        for pos in 0..mod_len {
            let rpos = mod_len - 1 - pos;
            if flag & (0x0001 << rpos) != 0 {
                let _ = simulate_code(None, Some(modifiers_scancode[rpos]), false);
            }
        }
        down_res
    }
}

#[inline]
pub fn simulate_char(chr: char, try_unicode: bool) -> Result<(), SimulateError> {
    simulate_key_unicode(chr as _, try_unicode)
}

pub fn simulate_unicode(unicode: u16) -> Result<(), SimulateError> {
    sim_keyboard_event(KEYEVENTF_UNICODE, 0, unicode)?;
    sim_keyboard_event(KEYEVENTF_UNICODE | KEYEVENTF_KEYUP, 0, unicode)
}

#[inline]
pub fn simulate_unistr(unistr: &str) -> Result<(), SimulateError> {
    for unicode in unistr.encode_utf16() {
        simulate_unicode(unicode)?;
    }
    Ok(())
}
