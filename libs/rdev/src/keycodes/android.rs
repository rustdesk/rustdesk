use crate::rdev::Key;

macro_rules! decl_keycodes {
    ($($key:ident, $code:literal),*) => {
        //TODO: make const when rust lang issue #49146 is fixed
        #[allow(dead_code)]
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
    Alt, 57,
    AltGr, 58,
    Backspace, 67,
    CapsLock, 115,
    ControlLeft, 113,
    ControlRight, 114,
    Delete, 112,
    DownArrow, 20,
    End, 123,
    Escape, 111,
    F1, 131,
    F10, 140,
    F11, 141,
    F12, 142,
    F2, 132,
    F3, 133,
    F4, 134,
    F5, 135,
    F6, 136,
    F7, 137,
    F8, 138,
    F9, 139,
    Home, 3,
    LeftArrow, 21,
    MetaLeft, 117,
    PageDown, 93,
    PageUp, 92,
    Return, 66,
    RightArrow, 22,
    ShiftLeft, 59,
    ShiftRight, 60,
    Space, 62,
    Tab, 61,
    UpArrow, 19,
    PrintScreen, 120,
    ScrollLock, 116,
    NumLock, 143,
    Pause, 121,
    BackQuote, 75,
    Num1, 8,
    Num2, 9,
    Num3, 10,
    Num4, 11,
    Num5, 12,
    Num6, 13,
    Num7, 14,
    Num8, 15,
    Num9, 16,
    Num0, 7,
    Minus, 69,
    Equal, 70,
    KeyA, 29,
    KeyB, 30,
    KeyC, 31,
    KeyD, 32,
    KeyE, 33,
    KeyF, 34,
    KeyG, 35,
    KeyH, 36,
    KeyI, 37,
    KeyJ, 38,
    KeyK, 39,
    KeyL, 40,
    KeyM, 41,
    KeyN, 42,
    KeyO, 43,
    KeyP, 44,
    KeyQ, 45,
    KeyR, 46,
    KeyS, 47,
    KeyT, 48,
    KeyU, 49,
    KeyV, 50,
    KeyW, 51,
    KeyX, 52,
    KeyY, 53,
    KeyZ, 54,
    LeftBracket, 71,
    RightBracket, 72,

    SemiColon, 74,
    Quote, 75,
    BackSlash, 73,
    KanaMode, 218,

    Comma, 55,
    Dot, 56,
    Slash, 76,
    Insert, 124
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
