use crate::rdev::Key;

macro_rules! decl_keycodes {
    ($($key:ident, $code:literal),*) => {
        //TODO: make const when rust lang issue #49146 is fixed
        pub fn code_from_key(key: Key) -> Option<u32> {
            match key {
                $(
                    Key::$key => Some($code),
                )*
                Key::Unknown(code) => Some(code),
                _ => None,
            }
        }

        //TODO: make const when rust lang issue #49146 is fixed
        #[allow(dead_code)]
        pub fn key_from_code(code: u32) -> Key {
            #[allow(unreachable_patterns)]
            match code {
                $(
                    $code => Key::$key,
                )*
                _ => Key::Unknown(code)
            }
        }
    };
}

#[rustfmt::skip]
decl_keycodes!(
    Alt, 64,
    AltGr, 108,
    Backspace, 22,
    CapsLock, 66,
    ControlLeft, 37,
    ControlRight, 105,
    Delete, 119,
    DownArrow, 116,
    End, 115,
    Escape, 9,
    F1, 67,
    F10, 76,
    F11, 95,
    F12, 96,
    F13, 0xBF,
    F14, 0xC0,
    F15, 0xC1,
    F16, 0xC2,
    F17, 0xC3,
    F18, 0xC4,
    F19, 0xC5,
    F20, 0xC6,
    F21, 0xC7,
    F22, 0xC8,
    F23, 0xC9,
    F24, 0xCA,
    F2, 68,
    F3, 69,
    F4, 70,
    F5, 71,
    F6, 72,
    F7, 73,
    F8, 74,
    F9, 75,
    Home, 110,
    LeftArrow, 113,
    MetaLeft, 133,
    PageDown, 117,
    PageUp, 112,
    Return, 36,
    RightArrow, 114,
    ShiftLeft, 50,
    ShiftRight, 62,
    Space, 65,
    Tab, 23,
    UpArrow, 111,
    PrintScreen, 107,
    ScrollLock, 78,
    Pause, 127,
    NumLock, 77,
    BackQuote, 49,
    Num1, 10,
    Num2, 11,
    Num3, 12,
    Num4, 13,
    Num5, 14,
    Num6, 15,
    Num7, 16,
    Num8, 17,
    Num9, 18,
    Num0, 19,
    Minus, 20,
    Equal, 21,
    KeyQ, 24,
    KeyW, 25,
    KeyE, 26,
    KeyR, 27,
    KeyT, 28,
    KeyY, 29,
    KeyU, 30,
    KeyI, 31,
    KeyO, 32,
    KeyP, 33,
    LeftBracket, 34,
    RightBracket, 35,
    KeyA, 38,
    KeyS, 39,
    KeyD, 40,
    KeyF, 41,
    KeyG, 42,
    KeyH, 43,
    KeyJ, 44,
    KeyK, 45,
    KeyL, 46,
    SemiColon, 47,
    Quote, 48,
    BackSlash, 51,
    IntlBackslash, 94,
    IntlRo, 0x61,
    IntlYen, 0x84,
    KanaMode, 0x65,
    KeyZ, 52,
    KeyX, 53,
    KeyC, 54,
    KeyV, 55,
    KeyB, 56,
    KeyN, 57,
    KeyM, 58,
    Comma, 59,
    Dot, 60,
    Slash, 61,
    Insert, 118,
    KpDecimal, 91,
    KpReturn, 104,
    KpMinus, 82,
    KpPlus, 86,
    KpMultiply, 63,
    KpDivide, 106,
    KpEqual, 0x7D,
    KpComma, 0x81,
    Kp0, 90,
    Kp1, 87,
    Kp2, 88,
    Kp3, 89,
    Kp4, 83,
    Kp5, 84,
    Kp6, 85,
    Kp7, 79,
    Kp8, 80,
    Kp9, 81,
    MetaRight, 134,
    Apps, 135,
    VolumeUp, 0x007B,
    VolumeDown, 0x007A,
    VolumeMute, 0x0079,
    NonConvert, 0x0066,
    Convert, 0x0064,
    Lang3, 0x0062,
    Lang4, 0x0063,
    Lang5, 0x005d,
    // Linux KEY_HANGEUL (122) and KEY_HANJA (123), plus the X11 offset of 8:
    // https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/input-event-codes.h
    // https://gitlab.freedesktop.org/xkeyboard-config/xkeyboard-config/-/blob/master/keycodes/evdev
    // Keep native keys first for X11 decoding; LANG keys are encoding aliases.
    Hangul, 0x0082,
    Lang1, 0x0082,
    Hanja, 0x0083,
    Lang2, 0x0083
);

#[cfg(test)]
mod test {
    use super::{code_from_key, key_from_code};
    #[test]
    fn test_reversible() {
        for code in 0..65636 {
            let key = key_from_code(code);
            match code_from_key(key) {
                Some(code2) => assert_eq!(code, code2),
                None => panic!("Could not convert back code: {:?}", code),
            }
        }
    }
}
