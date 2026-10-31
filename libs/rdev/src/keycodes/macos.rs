#![allow(non_upper_case_globals)]

use super::macos_virtual_keycodes::*;
use crate::rdev::Key;

pub use super::macos_virtual_keycodes as virtual_keycodes;

macro_rules! decl_keycodes {
    ($($key:ident, $code:ident),*) => {
        //TODO: make const when rust lang issue #49146 is fixed
        pub fn code_from_key(key: Key) -> Option<CGKeyCode> {
            match key {
                $(
                    Key::$key => Some($code),
                )*
                Key::Unknown(code) => Some(code as _),
                _ => None,
            }
        }

        //TODO: make const when rust lang issue #49146 is fixed
        #[allow(dead_code)]
        pub fn key_from_code(code: CGKeyCode) -> Key {
            match code {
                $(
                    $code => Key::$key,
                )*
                _ => Key::Unknown(code as _)
            }
        }
    };
}

#[rustfmt::skip]
decl_keycodes!(
    KeyA, kVK_ANSI_A,
    KeyS, kVK_ANSI_S,
    KeyD, kVK_ANSI_D,
    KeyF, kVK_ANSI_F,
    KeyH, kVK_ANSI_H,
    KeyG, kVK_ANSI_G,
    KeyZ, kVK_ANSI_Z,
    KeyX, kVK_ANSI_X,
    KeyC, kVK_ANSI_C,
    KeyV, kVK_ANSI_V,
    IntlBackslash, kVK_ISO_Section,
    KeyB, kVK_ANSI_B,
    KeyQ, kVK_ANSI_Q,
    KeyW, kVK_ANSI_W,
    KeyE, kVK_ANSI_E,
    KeyR, kVK_ANSI_R,
    KeyY, kVK_ANSI_Y,
    KeyT, kVK_ANSI_T,
    Num1, kVK_ANSI_1,
    Num2, kVK_ANSI_2,
    Num3, kVK_ANSI_3,
    Num4, kVK_ANSI_4,
    Num6, kVK_ANSI_6,
    Num5, kVK_ANSI_5,
    Equal, kVK_ANSI_Equal,
    Num9, kVK_ANSI_9,
    Num7, kVK_ANSI_7,
    Minus, kVK_ANSI_Minus,
    Num8, kVK_ANSI_8,
    Num0, kVK_ANSI_0,
    RightBracket, kVK_ANSI_RightBracket,
    KeyO, kVK_ANSI_O,
    KeyU, kVK_ANSI_U,
    LeftBracket, kVK_ANSI_LeftBracket,
    KeyI, kVK_ANSI_I,
    KeyP, kVK_ANSI_P,
    Return, kVK_Return,
    KeyL, kVK_ANSI_L,
    KeyJ, kVK_ANSI_J,
    Quote, kVK_ANSI_Quote,
    KeyK, kVK_ANSI_K,
    SemiColon, kVK_ANSI_Semicolon,
    BackSlash, kVK_ANSI_Backslash,
    Comma, kVK_ANSI_Comma,
    Slash, kVK_ANSI_Slash,
    KeyN, kVK_ANSI_N,
    KeyM, kVK_ANSI_M,
    Dot, kVK_ANSI_Period,
    Tab, kVK_Tab,
    Space, kVK_Space,
    BackQuote, kVK_ANSI_Grave,
    Backspace, kVK_Delete,
    Escape, kVK_Escape,
    MetaRight, kVK_RightCommand,
    MetaLeft, kVK_Command,
    ShiftLeft, kVK_Shift,
    CapsLock, kVK_CapsLock,
    Alt, kVK_Option,
    ControlLeft, kVK_Control,
    ShiftRight, kVK_RightShift,
    AltGr, kVK_RightOption,
    ControlRight, kVK_RightControl,
    Function, kVK_Function,
    F17, kVK_F17,
    KpDecimal, kVK_ANSI_KeypadDecimal,
    KpMultiply, kVK_ANSI_KeypadMultiply,
    KpPlus, kVK_ANSI_KeypadPlus,
    NumLock, kVK_ANSI_KeypadClear,
    VolumeUp, kVK_VolumeUp,
    VolumeDown, kVK_VolumeDown,
    VolumeMute, kVK_Mute,
    KpDivide, kVK_ANSI_KeypadDivide,
    KpReturn, kVK_ANSI_KeypadEnter,
    KpMinus, kVK_ANSI_KeypadMinus,
    F18, kVK_F18,
    F19, kVK_F19,
    KpEqual, kVK_ANSI_KeypadEquals,
    Kp0, kVK_ANSI_Keypad0,
    Kp1, kVK_ANSI_Keypad1,
    Kp2, kVK_ANSI_Keypad2,
    Kp3, kVK_ANSI_Keypad3,
    Kp4, kVK_ANSI_Keypad4,
    Kp5, kVK_ANSI_Keypad5,
    Kp6, kVK_ANSI_Keypad6,
    Kp7, kVK_ANSI_Keypad7,
    F20, kVK_F20,
    Kp8, kVK_ANSI_Keypad8,
    Kp9, kVK_ANSI_Keypad9,
    IntlYen, kVK_JIS_Yen,
    IntlRo, kVK_JIS_Underscore,
    KpComma, kVK_JIS_KeypadComma,
    F5, kVK_F5,
    F6, kVK_F6,
    F7, kVK_F7,
    F3, kVK_F3,
    F8, kVK_F8,
    F9, kVK_F9,
    Lang2, kVK_JIS_Eisu,
    F11, kVK_F11,
    Lang1, kVK_JIS_Kana,
    // PrintScreen, kVK_F13,
    F13, kVK_F13,
    F16, kVK_F16,
    // ScrollLock, kVK_F14,
    F14, kVK_F14,
    F10, kVK_F10,
    F12, kVK_F12,
    // Pause, kVK_F15,
    F15, kVK_F15,
    Insert, kVK_Help,
    Home, kVK_Home,
    PageUp, kVK_PageUp,
    Delete, kVK_ForwardDelete,
    F4, kVK_F4,
    End, kVK_End,
    F2, kVK_F2,
    PageDown, kVK_PageDown,
    F1, kVK_F1,
    LeftArrow, kVK_LeftArrow,
    RightArrow, kVK_RightArrow,
    DownArrow, kVK_DownArrow,
    UpArrow, kVK_UpArrow,
    Apps, kVK_Context_Menu
    // KanaMode, kVK_Unknown,
    // Lang3, kVK_Unknown,
    // Lang4, kVK_Unknown,
    // Lang5, kVK_Unknown
);

#[cfg(test)]
mod test {
    use super::{code_from_key, key_from_code};
    #[test]
    fn test_reversible() {
        for code in 0..=65535 {
            let key = key_from_code(code);
            match code_from_key(key) {
                Some(code2) => assert_eq!(code, code2),
                None => panic!("Could not convert back code: {:?}", code),
            }
        }
    }
}
