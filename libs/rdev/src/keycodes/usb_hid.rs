use crate::rdev::Key;

macro_rules! decl_keycodes {
    ($($key:ident, $code:literal),*) => {
        pub fn code_from_key(key: Key) -> Option<u32> {
            match key {
                $(
                    Key::$key => Some($code),
                )*
                Key::Unknown(code) => Some(code as _),
                _ => None,
            }
        }

        pub fn key_from_code(code: u32) -> Key {
            #[allow(unreachable_patterns)]
            match code {
                $(
                    $code => Key::$key,
                )*
                _ => Key::Unknown(code as _)
            }
        }
    };
}

// TODO: 0

// https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
// https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input
// https://www.usb.org/sites/default/files/hut1_7.pdf
// https://download.microsoft.com/download/1/6/1/161ba512-40e2-4cc9-843a-923143f3456c/translate.pdf
// We redefined here for Letter and number keys which are not in winapi crate (and don't have a name either in win32)
decl_keycodes! {
    Alt, 0xE2,
    AltGr, 0xE6,
    Backspace, 0x2A,
    CapsLock, 0x39,
    ControlLeft, 0xE0,
    ControlRight, 0xE4,
    Delete, 0x4C,     // Note 1
    UpArrow, 0x52,    // Note 1
    DownArrow, 0x51,  // Note 1
    LeftArrow, 0x50,  // Note 1
    RightArrow, 0x4F, // Note 1
    End, 0x4D,        // Note 1
    Escape, 0x29,
    F1, 0x3A,
    F2, 0x3B,
    F3, 0x3C,
    F4, 0x3D,
    F5, 0x3E,
    F6, 0x3F,
    F7, 0x40,
    F8, 0x41,
    F9, 0x42,
    F10, 0x43,
    F11, 0x44,
    F12, 0x45,
    F13, 0x68,
    F14, 0x69,
    F15, 0x6A,
    F16, 0x6B,
    F17, 0x6C,
    F18, 0x6D,
    F19, 0x6E,
    F20, 0x6F,
    F21, 0x70,
    F22, 0x71,
    F23, 0x72,
    F24, 0x73,
    Home, 0x4A,       // Note 1
    MetaLeft, 0xE3,
    PageDown, 0x4E,   // Note 1
    PageUp, 0x4B,
    Return, 0x28,
    ShiftLeft, 0xE1,
    ShiftRight, 0xE5,
    Space, 0x2C,
    Tab, 0x2B,
    PrintScreen, 0x46,    // Note 4. Make: E0 2A  E0 37, Break E0 B7  E0 AA
    ScrollLock, 0x47,
    NumLock, 0x53,
    BackQuote, 0x35,
    Num1, 0x1E,
    Num2, 0x1F,
    Num3, 0x20,
    Num4, 0x21,
    Num5, 0x22,
    Num6, 0x23,
    Num7, 0x24,
    Num8, 0x25,
    Num9, 0x26,
    Num0, 0x27,
    Minus, 0x2D,
    Equal, 0x2E,
    KeyQ, 0x14,
    KeyW, 0x1A,
    KeyE, 0x08,
    KeyR, 0x15,
    KeyT, 0x17,
    KeyY, 0x1C,
    KeyU, 0x18,
    KeyI, 0x0C,
    KeyO, 0x12,
    KeyP, 0x13,
    LeftBracket, 0x2F,
    RightBracket, 0x30,
    BackSlash, 0x31,
    KeyA, 0x04,
    KeyS, 0x16,
    KeyD, 0x07,
    KeyF, 0x09,
    KeyG, 0x0A,
    KeyH, 0x0B,
    KeyJ, 0x0D,
    KeyK, 0x0E,
    KeyL, 0x0F,
    SemiColon, 0x33,
    Quote, 0x34,
    IntlBackslash, 0x64,
    IntlRo, 0x87,
    IntlYen, 0x89,
    // KanaMode, 0x88,
    KeyZ, 0x1D,
    KeyX, 0x1B,
    KeyC, 0x06,
    KeyV, 0x19,
    KeyB, 0x05,
    KeyN, 0x11,
    KeyM, 0x10,
    Comma, 0x36,
    Dot, 0x37,
    Slash, 0x38,
    Insert, 0x49,     // Note 1
    KpMinus, 0x56,
    KpPlus, 0x57,
    KpMultiply, 0x55,
    KpDivide, 0x54,
    KpDecimal, 0x63,
    KpReturn, 0x58,
    KpEqual, 0x67,
    KpComma, 0x85,
    Kp0, 0x62,
    Kp1, 0x59,
    Kp2, 0x5A,
    Kp3, 0x5B,
    Kp4, 0x5C,
    Kp5, 0x5D,
    Kp6, 0x5E,
    Kp7, 0x5F,
    Kp8, 0x60,
    Kp9, 0x61,
    MetaRight, 0xE7,
    // Apps, 0xE7,
    Apps, 0x65,
    VolumeUp, 0x80,
    VolumeDown, 0x81,
    VolumeMute, 0x7F,
    Convert, 0x8A,
    NonConvert, 0x8B,
    // USB HID Usage Tables, Keyboard/Keypad Page (0x07):
    // https://www.usb.org/sites/default/files/hut1_7.pdf
    // Keep LANG keys first for HID decoding; Hangul/Hanja are encoding aliases.
    Lang1, 0x90,
    Hangul, 0x90,
    Lang2, 0x91,
    Hanja, 0x91,
    Lang3, 0x92,
    Lang4, 0x93,
    Lang5, 0x94,
    Cancel, 0x9B,
    Clear, 0x9C,
    Kana, 0x88,
    Junja, 0x00,
    Final, 0x00,
    Select, 0x77,
    Print, 0x00,
    Execute, 0x74,
    Help, 0x75,
    Sleep, 0x00,
    Separator, 0x9f,
    Pause, 0x48
}

#[cfg(test)]
mod test {
    use super::{code_from_key, key_from_code};
    #[test]
    fn test_reversible() {
        for code in 0..65535 {
            let key = key_from_code(code);
            if let Some(code2) = code_from_key(key) {
                assert_eq!(code, code2)
            } else {
                assert!(false, "We could not convert back code: {:?}", code);
            }
        }
    }
}
