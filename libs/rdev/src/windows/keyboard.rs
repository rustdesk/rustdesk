use crate::rdev::{EventType, Key, KeyboardState, UnicodeInfo};
use crate::windows::common::{get_code, get_scan_code, FALSE, TRUE};
use std::collections::HashMap;
use std::ptr::null_mut;
use winapi::ctypes::c_int;
use winapi::shared::minwindef::{BYTE, HKL, LPARAM, UINT};
use winapi::um::processthreadsapi::GetCurrentThreadId;
use winapi::um::winuser::{
    self, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RWIN,
};
use winapi::um::winuser::{
    GetForegroundWindow, GetKeyState, GetKeyboardLayout, GetKeyboardState,
    GetWindowThreadProcessId, ToUnicodeEx, VK_CAPITAL, VK_LSHIFT, VK_RSHIFT, VK_SHIFT,
};

const VK_SHIFT_: usize = VK_SHIFT as usize;
const VK_CAPITAL_: usize = VK_CAPITAL as usize;
const VK_LSHIFT_: usize = VK_LSHIFT as usize;
const VK_RSHIFT_: usize = VK_RSHIFT as usize;
const HIGHBIT: u8 = 0x80;

pub struct Keyboard {
    last_code: UINT,
    last_scan_code: UINT,
    last_state: [BYTE; 256],
    modifiers: HashMap<c_int, bool>,
    last_is_dead: bool,
    event_popup: bool,
}

impl Keyboard {
    pub fn new() -> Keyboard {
        Keyboard {
            last_code: 0,
            last_scan_code: 0,
            last_state: [0; 256],
            modifiers: HashMap::new(),
            last_is_dead: false,
            event_popup: true,
        }
    }

    pub(crate) fn get_modifier(&self, key: Key) -> bool {
        match key {
            Key::ShiftLeft => *self.modifiers.get(&VK_LSHIFT).unwrap_or(&false),
            Key::ShiftRight => *self.modifiers.get(&VK_RSHIFT).unwrap_or(&false),
            Key::ControlLeft => *self.modifiers.get(&VK_LCONTROL).unwrap_or(&false),
            Key::ControlRight => *self.modifiers.get(&VK_RCONTROL).unwrap_or(&false),
            Key::Alt => *self.modifiers.get(&VK_LMENU).unwrap_or(&false),
            Key::AltGr => *self.modifiers.get(&VK_RMENU).unwrap_or(&false),
            Key::MetaLeft => *self.modifiers.get(&VK_LWIN).unwrap_or(&false),
            Key::MetaRight => *self.modifiers.get(&VK_RWIN).unwrap_or(&false),
            _ => false,
        }
    }

    #[inline]
    fn set_modifier_(&mut self, key1: c_int, key2: c_int, key: c_int, down: bool) {
        self.modifiers.insert(key1, down);
        let key_down = if down {
            true
        } else {
            *self.modifiers.get(&key2).unwrap_or(&false)
        };
        self.modifiers.insert(key, key_down);
    }

    pub(crate) fn set_modifier(&mut self, key: Key, down: bool) {
        match key {
            Key::ControlLeft => {
                self.set_modifier_(VK_LCONTROL, VK_RCONTROL, VK_CONTROL, down);
            }
            Key::ControlRight => {
                self.set_modifier_(VK_RCONTROL, VK_LCONTROL, VK_CONTROL, down);
            }
            Key::ShiftLeft => {
                self.set_modifier_(VK_LSHIFT, VK_RSHIFT, VK_SHIFT, down);
            }
            Key::ShiftRight => {
                self.set_modifier_(VK_RSHIFT, VK_LSHIFT, VK_SHIFT, down);
            }
            Key::Alt => {
                self.set_modifier_(VK_LMENU, VK_RMENU, VK_MENU, down);
            }
            Key::AltGr => {
                self.set_modifier_(VK_RMENU, VK_LMENU, VK_MENU, down);
            }
            Key::MetaLeft => {
                self.modifiers.insert(VK_LWIN, down);
            }
            Key::MetaRight => {
                self.modifiers.insert(VK_RWIN, down);
            }
            _ => {
                // ignore
            }
        }
    }

    #[inline]
    pub(crate) fn set_event_popup(&mut self, b: bool) {
        self.event_popup = b;
    }

    pub(crate) unsafe fn get_unicode(&mut self, lpdata: LPARAM) -> Option<UnicodeInfo> {
        // https://gist.github.com/akimsko/2011327
        // https://www.experts-exchange.com/questions/23453780/LowLevel-Keystroke-Hook-removes-Accents-on-French-Keyboard.html
        let code = get_code(lpdata);
        let scan_code = get_scan_code(lpdata);

        self.set_global_state()?;
        self.get_code_name_unicode(code, scan_code)
    }

    pub(crate) unsafe fn set_global_state(&mut self) -> Option<()> {
        let mut state = [0_u8; 256];
        let state_ptr = state.as_mut_ptr();

        // to-do
        // GetKeyState should be called before GetKeyboardState.
        // https://stackoverflow.com/questions/45719020/winapi-getkeyboardstate-behavior-modified-by-getkeystate-when-application-is-out
        // But this causing accents errors. Typing ö turns out ô.
        // https://github.com/rustdesk/rustdesk/issues/2670
        let _shift = GetKeyState(VK_SHIFT);
        let current_window_thread_id = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
        let thread_id = GetCurrentThreadId();
        // Attach to active thread so we can get that keyboard state
        let status = if winuser::AttachThreadInput(thread_id, current_window_thread_id, TRUE) == 1 {
            // Current state of the modifiers in keyboard
            let status = GetKeyboardState(state_ptr);
            // Detach
            winuser::AttachThreadInput(thread_id, current_window_thread_id, FALSE);
            status
        } else {
            // Could not attach, perhaps it is this process?
            GetKeyboardState(state_ptr)
        };

        if status != 1 {
            return None;
        }

        let press_state = 129;
        // let release_state = 0;
        let control_left = self.get_modifier(Key::ControlLeft);
        let control_right = self.get_modifier(Key::ControlRight);
        if control_left {
            state[VK_LCONTROL as usize] = press_state;
        }
        if control_right {
            state[VK_RCONTROL as usize] = press_state;
        }
        if control_left || control_right {
            state[VK_CONTROL as usize] = press_state;
        }
        // state[VK_LCONTROL as usize] = if control_left {press_state} else {release_state};
        // state[VK_RCONTROL as usize] = if control_right {press_state} else {release_state};
        // state[VK_CONTROL as usize] = if control_left || control_right {press_state} else {release_state};

        let shift_left = self.get_modifier(Key::ShiftLeft);
        let shift_right = self.get_modifier(Key::ShiftRight);
        if shift_left {
            state[VK_LSHIFT as usize] = press_state;
        }
        if shift_right {
            state[VK_RSHIFT as usize] = press_state;
        }
        if shift_left || shift_right {
            state[VK_SHIFT as usize] = press_state;
        }
        // state[VK_LSHIFT as usize] = if shift_left {press_state} else {release_state};
        // state[VK_RSHIFT as usize] = if shift_right {press_state} else {release_state};
        // state[VK_SHIFT as usize] = if shift_left || shift_right {press_state} else {release_state};

        let alt_left = self.get_modifier(Key::Alt);
        let alt_right = self.get_modifier(Key::AltGr);
        if alt_left {
            state[VK_LMENU as usize] = press_state;
        }
        if alt_right {
            state[VK_RMENU as usize] = press_state;
        }
        if alt_left || alt_right {
            state[VK_MENU as usize] = press_state;
        }
        // state[VK_LMENU as usize] = if alt_left {press_state} else {release_state};
        // state[VK_RMENU as usize] = if alt_right {press_state} else {release_state};
        // state[VK_MENU as usize] = if alt_left || alt_right {press_state} else {release_state};

        let win_left = self.get_modifier(Key::MetaLeft);
        let win_right = self.get_modifier(Key::MetaRight);
        if win_left {
            state[VK_LWIN as usize] = press_state;
        }
        if win_right {
            state[VK_RWIN as usize] = press_state;
        }
        // state[VK_LWIN as usize] = if win_left {press_state} else {release_state};
        // state[VK_RWIN as usize] = if win_right {press_state} else {release_state};

        self.last_state = state;
        Some(())
    }

    pub(crate) unsafe fn get_code_name_unicode(
        &mut self,
        code: UINT,
        scan_code: UINT,
    ) -> Option<UnicodeInfo> {
        let current_window_thread_id = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
        let state_ptr = self.last_state.as_mut_ptr();
        const BUF_LEN: i32 = 32;
        let mut buff = [0_u16; BUF_LEN as usize];
        let buff_ptr = buff.as_mut_ptr();
        let layout = GetKeyboardLayout(current_window_thread_id);
        // https://github.com/rustdesk/rustdesk/issues/2670
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-tounicodeex
        // For Portuguese (Brazil) layout,
        // Shift + [ returns -1. (`)
        // Shift + ' returns -1. (^)
        // Shift + 6 does not return -1(dead code)(¨).
        let len = ToUnicodeEx(code, scan_code, state_ptr, buff_ptr, 8 - 1, 0, layout);
        let mut is_dead = false;
        let result = match len {
            0 => None,
            -1 => {
                is_dead = true;
                self.clear_keyboard_buffer(code, scan_code, layout);
                Some(UnicodeInfo {
                    name: None,
                    unicode: Vec::new(),
                    is_dead: true,
                })
            }
            len if len > 0 => {
                let unicode = buff[..len as usize].to_vec();
                Some(UnicodeInfo {
                    name: String::from_utf16(&unicode).ok(),
                    unicode,
                    is_dead: false,
                })
            }
            _ => None,
        };

        if self.event_popup {
            if self.last_code != 0 && self.last_is_dead {
                buff = [0; 32];
                let buff_ptr = buff.as_mut_ptr();
                let last_state_ptr = self.last_state.as_mut_ptr();
                ToUnicodeEx(
                    self.last_code,
                    self.last_scan_code,
                    last_state_ptr,
                    buff_ptr,
                    BUF_LEN,
                    0,
                    layout,
                );
                self.last_code = 0;
            } else {
                self.last_code = code;
                self.last_scan_code = scan_code;
            }
        } else {
            // I cannot understand why ToUnicodeEx is called.
            // But ToUnicodeEx is needed here.
            if is_dead {
                buff = [0; 32];
                let buff_ptr = buff.as_mut_ptr();
                let last_state_ptr = self.last_state.as_mut_ptr();
                ToUnicodeEx(
                    code,
                    scan_code,
                    last_state_ptr,
                    buff_ptr,
                    BUF_LEN,
                    0,
                    layout,
                );
                self.last_code = 0;
            } else {
                self.last_code = code;
                self.last_scan_code = scan_code;
            }
        }
        self.last_is_dead = is_dead;

        // C0 controls
        if len == 1
            && matches!(
                String::from_utf16(&buff[..len as usize])
                    .ok()?
                    .chars()
                    .next()?,
                '\u{1}'..='\u{1f}'
            )
        {
            return None;
        }
        result
    }

    unsafe fn clear_keyboard_buffer(&self, code: UINT, scan_code: UINT, layout: HKL) {
        const BUF_LEN: i32 = 32;
        let mut buff = [0_u16; BUF_LEN as usize];
        let buff_ptr = buff.as_mut_ptr();
        let mut state = [0_u8; 256];
        let state_ptr = state.as_mut_ptr();

        let mut len = -1;
        while len < 0 {
            len = ToUnicodeEx(code, scan_code, state_ptr, buff_ptr, BUF_LEN, 0, layout);
        }
    }

    pub fn is_dead(&mut self) -> bool {
        self.last_is_dead
    }
}

impl KeyboardState for Keyboard {
    fn add(&mut self, event_type: &EventType) -> Option<UnicodeInfo> {
        match event_type {
            EventType::KeyPress(key) => match key {
                Key::ShiftLeft => {
                    self.last_state[VK_SHIFT_] |= HIGHBIT;
                    self.last_state[VK_LSHIFT_] |= HIGHBIT;
                    None
                }
                Key::ShiftRight => {
                    self.last_state[VK_SHIFT_] |= HIGHBIT;
                    self.last_state[VK_RSHIFT_] |= HIGHBIT;
                    None
                }
                Key::CapsLock => {
                    self.last_state[VK_CAPITAL_] ^= HIGHBIT;
                    None
                }
                key => {
                    let (code, scan_code) = crate::get_win_codes(*key)?;

                    unsafe {
                        let _control = GetKeyState(winuser::VK_CONTROL) & 0x8000_u16 as i16;
                        let _altgr = GetKeyState(winuser::VK_RMENU) & 0x8000_u16 as i16;
                        // If control is pressed, global state cannot be used, otherwise no character will be generated.
                        // note: AltGR => ControlLeft + AltGR
                        if _control < 0 && _altgr >= 0 {
                            self.get_code_name_unicode(code as _, scan_code)
                        } else {
                            self.set_global_state()?;
                            self.get_code_name_unicode(code, scan_code)
                        }
                    }
                }
            },
            EventType::KeyRelease(key) => match key {
                Key::ShiftLeft => {
                    self.last_state[VK_SHIFT_] &= !HIGHBIT;
                    self.last_state[VK_LSHIFT_] &= !HIGHBIT;
                    None
                }
                Key::ShiftRight => {
                    self.last_state[VK_SHIFT_] &= !HIGHBIT;
                    self.last_state[VK_RSHIFT_] &= HIGHBIT;
                    None
                }
                _ => None,
            },

            _ => None,
        }
    }
}
