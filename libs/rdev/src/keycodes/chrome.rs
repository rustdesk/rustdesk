use crate::rdev::Key;

pub const RESERVED_UNKNOWN_CODE: u32 = 0;

macro_rules! decl_keycodes {
    ($($key:ident, $code:expr),*) => {
        pub fn code_from_key(key: Key) -> Option<&'static str> {
            match key {
                $(
                    Key::$key => Some($code),
                )*
                _ => None,
            }
        }

        pub fn key_from_code(code: &str) -> Key {
            match code {
                $(
                    $code => Key::$key,
                )*
                _ => Key::Unknown(RESERVED_UNKNOWN_CODE)
            }
        }
    };
}

// https://developer.mozilla.org/en-US/docs/Web/API/UI_Events/Keyboard_event_code_values
decl_keycodes! {
    Alt, "AltLeft",
    AltGr, "AltRight",
    Backspace, "Backspace",
    CapsLock, "CapsLock",
    ControlLeft, "ControlLeft",
    ControlRight, "ControlRight",
    Delete, "Delete",
    UpArrow, "ArrowUp",
    DownArrow, "ArrowDown",
    LeftArrow, "ArrowLeft",
    RightArrow, "ArrowRight",
    End, "End",
    Escape, "Escape",
    F1, "F1",
    F2, "F2",
    F3, "F3",
    F4, "F4",
    F5, "F5",
    F6, "F6",
    F7, "F7",
    F8, "F8",
    F9, "F9",
    F10, "F10",
    F11, "F11",
    F12, "F12",
    F13, "F13",
    F14, "F14",
    F15, "F15",
    F16, "F16",
    F17, "F17",
    F18, "F18",
    F19, "F19",
    F20, "F20",
    F21, "F21",
    F22, "F22",
    F23, "F23",
    F24, "F24",
    Home, "Home",
    MetaLeft, "MetaLeft",   // "MetaLeft" (was "OSLeft" prior to Chrome 52)
    PageDown, "PageDown",
    PageUp, "PageUp",
    Return, "Enter",
    ShiftLeft, "ShiftLeft",
    ShiftRight, "ShiftRight",
    Space, "Space",
    Tab, "Tab",
    PrintScreen, "PrintScreen",
    ScrollLock, "ScrollLock",
    NumLock, "NumLock",            // The scan code is 0x0045, but https://developer.mozilla.org/en-US/docs/Web/API/UI_Events/Keyboard_event_code_values gives 0xE045
    BackQuote, "Backquote",
    Num1, "Digit1",
    Num2, "Digit2",
    Num3, "Digit3",
    Num4, "Digit4",
    Num5, "Digit5",
    Num6, "Digit6",
    Num7, "Digit7",
    Num8, "Digit8",
    Num9, "Digit9",
    Num0, "Digit0",
    Minus, "Minus",
    Equal, "Equal",
    KeyQ, "KeyQ",
    KeyW, "KeyW",
    KeyE, "KeyE",
    KeyR, "KeyR",
    KeyT, "KeyT",
    KeyY, "KeyY",
    KeyU, "KeyU",
    KeyI, "KeyI",
    KeyO, "KeyO",
    KeyP, "KeyP",
    LeftBracket, "BracketLeft",
    RightBracket, "BracketRight",
    BackSlash, "Backslash",
    KeyA, "KeyA",
    KeyS, "KeyS",
    KeyD, "KeyD",
    KeyF, "KeyF",
    KeyG, "KeyG",
    KeyH, "KeyH",
    KeyJ, "KeyJ",
    KeyK, "KeyK",
    KeyL, "KeyL",
    SemiColon, "Semicolon",
    Quote, "Quote",
    IntlBackslash, "IntlBackslash",
    IntlRo, "IntlRo", // "IntlRo" (was "" prior to Chrome 48)
    IntlYen, "IntlYen",
    KanaMode, "KanaMode", // "KanaMode" (was "" prior to Chrome 48)
    KeyZ, "KeyZ",
    KeyX, "KeyX",
    KeyC, "KeyC",
    KeyV, "KeyV",
    KeyB, "KeyB",
    KeyN, "KeyN",
    KeyM, "KeyM",
    Comma, "Comma",
    Dot, "Period",
    Slash, "Slash",
    Insert, "Insert",
    KpMinus, "NumpadSubtract",
    KpPlus, "NumpadAdd",
    KpMultiply, "NumpadMultiply",
    KpDivide, "NumpadDivide",
    KpDecimal, "NumpadDecimal",
    KpReturn, "NumpadEnter",
    KpEqual, "NumpadEqual", // 	"NumpadEqual" (was "" prior to Chrome 48)
    KpComma, "NumpadComma", // "NumpadComma" (was "" prior to Chrome 48)
    Kp0, "Numpad0",
    Kp1, "Numpad1",
    Kp2, "Numpad2",
    Kp3, "Numpad3",
    Kp4, "Numpad4",
    Kp5, "Numpad5",
    Kp6, "Numpad6",
    Kp7, "Numpad7",
    Kp8, "Numpad8",
    Kp9, "Numpad9",
    MetaRight, "MetaRight", // "MetaRight" (was "OSRight" prior to Chrome 52)
    Apps, "ContextMenu",
    VolumeUp, "AudioVolumeUp", // "AudioVolumeUp" (was "VolumeUp" prior to Chrome 52) (⚠️ Not the same on Firefox)
    VolumeDown, "AudioVolumeDown", // "AudioVolumeDown" (was "VolumeDown" prior to Chrome 52) (⚠️ Not the same on Firefox)
    VolumeMute, "AudioVolumeMute", // "AudioVolumeMute" (was "VolumeMute" prior to Chrome 52) (⚠️ Not the same on Firefox)
    NonConvert, "NonConvert", // "NonConvert" (was "" prior to Chrome 48)
    Convert, "Convert", // "Convert" (was "" prior to Chrome 48)
    Lang1, "Lang1",
    Lang2, "Lang2",
    Lang3, "Lang3", // "Lang3" (was "" prior to Chrome 48)
    Lang4, "Lang4", // "Lang4" (was "" prior to Chrome 48)
    Lang5, "Lang5", // "Lang5" (was "" prior to Chrome 48) (⚠️ Not the same on Firefox) Lang5 in https://download.microsoft.com/download/1/6/1/161ba512-40e2-4cc9-843a-923143f3456c/translate.pdf is 0x0075, while is "Lang5" in https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/code is 0x005D
    Cancel, "",
    Clear, "",
    Kana, "",
    Junja, "",
    Final, "",
    Hanja, "",
    Select, "",
    Print, "",
    Execute, "",
    Help, "",
    Sleep, "",
    Separator, "",
    Pause, ""
}

#[cfg(test)]
mod test {
    use super::{code_from_key, key_from_code};
    #[test]
    fn test_reversible() {
        for code in ["KeyA", "KeyB", "KeyC"] {
            let key = key_from_code(code);
            if let Some(code2) = code_from_key(key) {
                assert_eq!(code, code2)
            } else {
                assert!(false, "We could not convert back code: {:?}", code);
            }
        }
    }
}
