use crate::rdev::Key;

macro_rules! decl_keycodes {
    ($($key:ident, $code:literal, $scancode:literal),*) => {
        //TODO: make const when rust lang issue #49146 is fixed
        pub fn code_from_key(key: Key) -> Option<u32> {
            match key {
                $(
                    Key::$key => Some($code),
                )*
                Key::Unknown(code) => Some(code as _),
                _ => None,
            }
        }

        //TODO: make const when rust lang issue #49146 is fixed
        pub fn key_from_code(code: u32) -> Key {
            #[allow(unreachable_patterns)]
            match code {
                $(
                    $code => Key::$key,
                )*
                _ => Key::Unknown(code as _)
            }
        }

        pub fn scancode_from_key(key: Key) -> Option<u32> {
            match key {
                $(
                    Key::$key => Some($scancode),
                )*
                Key::Unknown(code) => Some(code as u32),
                _ => None,
            }
        }

        pub fn key_from_scancode(scancode: u32) -> Key{
            #[allow(unreachable_patterns)]
            match scancode {
                0 => Key::Unknown(0),
                $(
                    $scancode => Key::$key,
                )*
                _ => Key::Unknown(scancode as _)
            }
        }

        pub fn get_win_key(keycode: u32, scancode: u32) -> Key{
            let key = key_from_code(keycode);
            let scancode_key = key_from_scancode(scancode);

            if key == Key::AltGr || key == Key::KpDivide || key == Key::ControlRight {
                // note: alt and altgr have same scancode.
                // slash and divide.
                // left control and right control .
                key
            } else if scancode_key != Key::Unknown(scancode) {
                // note: numpad should use scancode directly,
                scancode_key
            } else {
                key
            }
        }

        pub fn get_win_codes(key: Key) -> Option<(u32, u32)>{
            let keycode = code_from_key(key)?;
            let key = if key == Key::Unknown(keycode){
                key_from_code(keycode)
            }else{
                key
            };
            let scancode = scancode_from_key(key)?;
            Some((keycode, scancode))
        }
    };
}

// TODO: 0

// https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
// https://learn.microsoft.com/en-us/windows/win32/inputdev/about-keyboard-input
// https://download.microsoft.com/download/1/6/1/161ba512-40e2-4cc9-843a-923143f3456c/translate.pdf
// We redefined here for Letter and number keys which are not in winapi crate (and don't have a name either in win32)
decl_keycodes! {
    Alt, 164, 0x38,
    AltGr, 165, 0xE038,
    Backspace, 0x08, 0x0E,
    CapsLock, 20, 0x3A,
    ControlLeft, 162, 0x1D,
    ControlRight, 163, 0xE01D,
    Delete, 46, 0xE053,     // Note 1
    UpArrow, 38, 0xE048,    // Note 1
    DownArrow, 40, 0xE050,  // Note 1
    LeftArrow, 37, 0xE04B,  // Note 1
    RightArrow, 39, 0xE04D, // Note 1
    End, 35, 0xE04F,        // Note 1
    Escape, 27, 0x01,
    F1, 112, 0x3B,
    F2, 113, 0x3C,
    F3, 114, 0x3D,
    F4, 115, 0x3E,
    F5, 116, 0x3F,
    F6, 117, 0x40,
    F7, 118, 0x41,
    F8, 119, 0x42,
    F9, 120, 0x43,
    F10, 121, 0x44,
    F11, 122, 0x57,
    F12, 123, 0x58,
    F13, 0x7C, 0x64,
    F14, 0x7D, 0x65,
    F15, 0x7E, 0x66,
    F16, 0x7F, 0x67,
    F17, 0x80, 0x68,
    F18, 0x81, 0x69,
    F19, 0x82, 0x6A,
    F20, 0x83, 0x6B,
    F21, 0x84, 0x6C,
    F22, 0x85, 0x6D,
    F23, 0x86, 0x6E,
    F24, 0x87, 0x76,
    Home, 36, 0xE047,       // Note 1
    MetaLeft, 91, 0xE05B,
    PageDown, 34, 0xE051,   // Note 1
    PageUp, 33, 0xE049,
    Return, 13, 0x1C,
    ShiftLeft, 160, 0x2A,
    ShiftRight, 161, 0x36,
    Space, 32, 0x39,
    Tab, 0x09, 0x0F,
    PrintScreen, 44, 0xE037,    // Note 4. Make: E0 2A  E0 37, Break E0 B7  E0 AA
    ScrollLock, 145, 0x46,
    NumLock, 144, 0x45,
    BackQuote, 192, 0x29,
    Num1, 49, 0x02,
    Num2, 50, 0x03,
    Num3, 51, 0x04,
    Num4, 52, 0x05,
    Num5, 53, 0x06,
    Num6, 54, 0x07,
    Num7, 55, 0x08,
    Num8, 56, 0x09,
    Num9, 57, 0x0A,
    Num0, 48, 0x0B,
    Minus, 189, 0x0C,
    Equal, 187, 0x0D,
    KeyQ, 81, 0x10,
    KeyW, 87, 0x11,
    KeyE, 69, 0x12,
    KeyR, 82, 0x13,
    KeyT, 84, 0x14,
    KeyY, 89, 0x15,
    KeyU, 85, 0x16,
    KeyI, 73, 0x17,
    KeyO, 79, 0x18,
    KeyP, 80, 0x19,
    LeftBracket, 219, 0x1A,
    RightBracket, 221, 0x1B,
    BackSlash, 220, 0x2B,
    KeyA, 65, 0x1E,
    KeyS, 83, 0x1F,
    KeyD, 68, 0x20,
    KeyF, 70, 0x21,
    KeyG, 71, 0x22,
    KeyH, 72, 0x23,
    KeyJ, 74, 0x24,
    KeyK, 75, 0x25,
    KeyL, 76, 0x26,
    SemiColon, 186, 0x27,
    Quote, 222, 0x28,
    IntlBackslash, 226, 0x56,
    IntlRo, 0x00E2, 0x0073,
    IntlYen, 0x00DC, 0x007D,
    KanaMode, 0x0000, 0x70,
    KeyZ, 90, 0x2C,
    KeyX, 88, 0x2D,
    KeyC, 67, 0x2E,
    KeyV, 86, 0x2F,
    KeyB, 66, 0x30,
    KeyN, 78, 0x31,
    KeyM, 77, 0x32,
    Comma, 188, 0x33,
    Dot, 190, 0x34,
    Slash, 191, 0x35,
    Insert, 45, 0xE052,     // Note 1
    KpMinus, 109, 0x4A,
    KpPlus, 107, 0x4E,
    KpMultiply, 106, 0x37,
    KpDivide, 111, 0xE035,
    KpDecimal, 110, 0x53,
    KpReturn, 13, 0xE01C,
    KpEqual, 0x0000, 0x59,
    KpComma, 0x0000, 0x7E,
    Kp0, 96, 0x52,
    Kp1, 97, 0x4F,
    Kp2, 98, 0x50,
    Kp3, 99, 0x51,
    Kp4, 100, 0x4B,
    Kp5, 101, 0x4C,
    Kp6, 102, 0x4D,
    Kp7, 103, 0x47,
    Kp8, 104, 0x48,
    Kp9, 105, 0x49,
    MetaRight, 92, 0xE05C,
    Apps, 93, 0xE05D,
    VolumeUp, 0x00AF, 0xE030,
    VolumeDown, 0x00AE, 0xE02E,
    VolumeMute, 0x00AD, 0xE020,
    NonConvert, 0x1D, 0x007b,
    Convert, 0x1C, 0x0079,
    Lang3, 0x0000, 0x0078,
    Lang4, 0x0000, 0x0077,
    Lang5, 0x0000, 0x0076,
    Cancel, 0x03, 0x0000,
    Clear, 12, 0x0000,
    Kana, 0x15, 0x0080,
    // Windows virtual-key names and USB HID-to-scan-code mappings:
    // https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
    // https://download.microsoft.com/download/1/6/1/161ba512-40e2-4cc9-843a-923143f3456c/translate.pdf
    // Keep native keys first for scan-code decoding; LANG keys are encoding aliases.
    Hangul, 0x15, 0x00f2,
    Lang1, 0x15, 0x00f2,
    Junja, 0x17, 0x0000,
    Final, 0x18, 0x0000,
    Hanja, 0x19, 0x00f1,
    Lang2, 0x19, 0x00f1,
    Select, 0x29, 0x0000,
    Print, 0x2A, 0x0000,
    Execute, 0x2B, 0x0000,
    Help, 0x2F, 0x0000,
    Sleep, 0x5F, 0x0000,
    Separator, 0x6C, 0x0000,
    Pause, 19, 0x0000
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
