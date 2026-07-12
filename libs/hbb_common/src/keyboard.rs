use std::{fmt, slice::Iter, str::FromStr};

use crate::protos::message::KeyboardMode;

impl fmt::Display for KeyboardMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KeyboardMode::Legacy => write!(f, "legacy"),
            KeyboardMode::Map => write!(f, "map"),
            KeyboardMode::Translate => write!(f, "translate"),
            KeyboardMode::Auto => write!(f, "auto"),
        }
    }
}

impl FromStr for KeyboardMode {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "legacy" => Ok(KeyboardMode::Legacy),
            "map" => Ok(KeyboardMode::Map),
            "translate" => Ok(KeyboardMode::Translate),
            "auto" => Ok(KeyboardMode::Auto),
            _ => Err(()),
        }
    }
}

impl KeyboardMode {
    pub fn iter() -> Iter<'static, KeyboardMode> {
        static KEYBOARD_MODES: [KeyboardMode; 4] = [
            KeyboardMode::Legacy,
            KeyboardMode::Map,
            KeyboardMode::Translate,
            KeyboardMode::Auto,
        ];
        KEYBOARD_MODES.iter()
    }
}
