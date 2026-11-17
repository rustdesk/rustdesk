#[cfg(target_os = "linux")]
use crate::keycodes::linux::key_from_code as linux_key_from_code;
#[cfg(target_os = "macos")]
use crate::keycodes::macos::key_from_code as macos_key_from_code;
use crate::keycodes::macos::virtual_keycodes::*;
#[cfg(target_os = "windows")]
use crate::keycodes::windows::key_from_scancode as win_key_from_scancode;
#[cfg(target_os = "macos")]
use crate::macos::map_keycode;
use crate::{
    keycodes::{
        android::code_from_key as android_code_from_key,
        linux::code_from_key as linux_code_from_key, macos::code_from_key as macos_code_from_key,
        usb_hid::key_from_code as usb_hid_key_from_code,
        windows::scancode_from_key as win_scancode_from_key,
    },
    Key, KeyCode,
};

macro_rules! conv_keycodes {
    ($fnname:ident, $key_from_code:ident, $code_from_key:ident) => {
        pub fn $fnname(code: u32) -> Option<KeyCode> {
            let key = $key_from_code(code as _);
            match key {
                Key::Unknown(..) => None,
                Key::RawKey(..) => None,
                _ => $code_from_key(key).map(|c| c as KeyCode),
            }
        }
    };
}

// JIS conversion keys occupy the same physical positions as the corresponding
// macOS input-mode keys.
#[allow(non_upper_case_globals)]
fn macos_target_code_from_key(key: Key) -> Option<KeyCode> {
    match key {
        Key::NonConvert => Some(kVK_JIS_Eisu as _),
        Key::Convert => Some(kVK_JIS_Kana as _),
        _ => macos_code_from_key(key).map(|code| code as _),
    }
}

#[allow(non_upper_case_globals)]
fn macos_iso_code_from_key(key: Key) -> Option<KeyCode> {
    match macos_target_code_from_key(key)? {
        kVK_ISO_Section => Some(kVK_ANSI_Grave),
        kVK_ANSI_Grave => Some(kVK_ISO_Section),
        code => Some(code as _),
    }
}

#[cfg(target_os = "macos")]
#[allow(non_upper_case_globals)]
fn macos_keycode_from_code_(code: KeyCode) -> Key {
    // Preserve the Japanese bottom-row positions when macOS is the source.
    match macos_key_from_code(map_keycode(code)) {
        Key::Lang2 => Key::NonConvert,
        Key::Lang1 => Key::Convert,
        key => key,
    }
}

#[cfg(target_os = "windows")]
conv_keycodes!(
    win_scancode_to_linux_code,
    win_key_from_scancode,
    linux_code_from_key
);
#[cfg(target_os = "windows")]
conv_keycodes!(
    win_scancode_to_macos_code,
    win_key_from_scancode,
    macos_target_code_from_key
);
#[cfg(target_os = "windows")]
// From Win scancode to MacOS keycode(ISO Layout)
conv_keycodes!(
    win_scancode_to_macos_iso_code,
    win_key_from_scancode,
    macos_iso_code_from_key
);
#[cfg(target_os = "windows")]
// From Win scancode to android keycode
conv_keycodes!(
    win_scancode_to_android_key_code,
    win_key_from_scancode,
    android_code_from_key
);
#[cfg(target_os = "linux")]
conv_keycodes!(
    linux_code_to_win_scancode,
    linux_key_from_code,
    win_scancode_from_key
);
#[cfg(target_os = "linux")]
conv_keycodes!(
    linux_code_to_macos_code,
    linux_key_from_code,
    macos_target_code_from_key
);
#[cfg(target_os = "linux")]
// From Linux scancode to MacOS keycode(ISO Layout)
conv_keycodes!(
    linux_code_to_macos_iso_code,
    linux_key_from_code,
    macos_iso_code_from_key
);
#[cfg(target_os = "linux")]
conv_keycodes!(
    linux_code_to_android_key_code,
    linux_key_from_code,
    android_code_from_key
);
#[cfg(target_os = "macos")]
conv_keycodes!(
    macos_code_to_win_scancode,
    macos_keycode_from_code_,
    win_scancode_from_key
);
#[cfg(target_os = "macos")]
conv_keycodes!(
    macos_code_to_linux_code,
    macos_keycode_from_code_,
    linux_code_from_key
);
#[cfg(target_os = "macos")]
conv_keycodes!(
    macos_code_to_android_key_code,
    macos_keycode_from_code_,
    android_code_from_key
);
conv_keycodes!(
    usb_hid_code_to_win_scancode,
    usb_hid_key_from_code,
    win_scancode_from_key
);
conv_keycodes!(
    usb_hid_code_to_linux_code,
    usb_hid_key_from_code,
    linux_code_from_key
);
conv_keycodes!(
    usb_hid_code_to_macos_code,
    usb_hid_key_from_code,
    macos_target_code_from_key
);
conv_keycodes!(
    usb_hid_code_to_macos_iso_code,
    usb_hid_key_from_code,
    macos_iso_code_from_key
);
conv_keycodes!(
    usb_hid_code_to_android_key_code,
    usb_hid_key_from_code,
    android_code_from_key
);

#[cfg(test)]
mod test {
    use crate::keycodes::macos::virtual_keycodes::{kVK_JIS_Eisu, kVK_JIS_Kana};

    const USB_HID_JIS_HENKAN: u32 = 0x8A;
    const USB_HID_JIS_MUHENKAN: u32 = 0x8B;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    const WIN_JIS_HENKAN_SCANCODE: u32 = 0x79;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    const WIN_JIS_MUHENKAN_SCANCODE: u32 = 0x7B;
    #[cfg(target_os = "macos")]
    const LINUX_X11_JIS_HENKAN_KEYCODE: u32 = 0x64;
    #[cfg(target_os = "macos")]
    const LINUX_X11_JIS_MUHENKAN_KEYCODE: u32 = 0x66;

    #[test]
    fn test_usb_hid_code_to_macos_code() {
        for code in 0..=65535 {
            let key = crate::keycodes::macos::key_from_code(code);
            if matches!(key, crate::Key::Unknown(..) | crate::Key::RawKey(..)) {
                continue;
            }
            let usb_hid = crate::keycodes::usb_hid::code_from_key(key);
            if let Some(usb_hid) = usb_hid {
                if usb_hid == 0 {
                    continue;
                }
                if let Some(code2) = super::usb_hid_code_to_macos_code(usb_hid) {
                    assert_eq!(u64::from(code), u64::from(code2))
                } else {
                    assert!(false, "We could not convert back code: {:?}", code);
                }
            }
        }
    }

    #[test]
    fn test_usb_hid_jis_keys_to_macos_code() {
        assert_eq!(
            super::usb_hid_code_to_macos_code(USB_HID_JIS_MUHENKAN),
            Some(kVK_JIS_Eisu as _)
        );
        assert_eq!(
            super::usb_hid_code_to_macos_code(USB_HID_JIS_HENKAN),
            Some(kVK_JIS_Kana as _)
        );
    }

    #[test]
    fn test_usb_hid_code_to_windows_scan_code() {
        for code in 1..=65535 {
            let key = crate::keycodes::windows::key_from_scancode(code);
            if matches!(key, crate::Key::Unknown(..) | crate::Key::RawKey(..)) {
                continue;
            }
            let usb_hid = crate::keycodes::usb_hid::code_from_key(key);
            if let Some(usb_hid) = usb_hid {
                if usb_hid == 0 {
                    continue;
                }
                if let Some(code2) = super::usb_hid_code_to_win_scancode(usb_hid) {
                    assert_eq!(code, code2 as u32)
                } else {
                    assert!(false, "We could not convert back code: {:?}", code);
                }
            }
        }
    }

    #[test]
    fn test_usb_hid_code_to_linux_key_code() {
        for code in 0..=65535 {
            let key = crate::keycodes::linux::key_from_code(code);
            if matches!(key, crate::Key::Unknown(..) | crate::Key::RawKey(..)) {
                continue;
            }
            let usb_hid = crate::keycodes::usb_hid::code_from_key(key);
            if let Some(usb_hid) = usb_hid {
                if usb_hid == 0 {
                    continue;
                }
                if let Some(code2) = super::usb_hid_code_to_linux_code(usb_hid) {
                    assert_eq!(code, code2 as u32)
                } else {
                    assert!(false, "We could not convert back code: {:?}", code);
                }
            }
        }
    }

    // Regression test: Windows JIS Muhenkan/Henkan keys must map to the
    // matching macOS keys when controlling a macOS peer (Map mode).
    //   無変換 (Muhenkan / NonConvert, scancode 0x7B) -> macOS 英数 (Eisu, 102)
    //   変換   (Henkan  / Convert,    scancode 0x79) -> macOS かな (Kana, 104)
    #[cfg(target_os = "windows")]
    #[test]
    fn test_jis_muhenkan_henkan_to_macos_code() {
        assert_eq!(
            super::win_scancode_to_macos_code(WIN_JIS_MUHENKAN_SCANCODE),
            Some(kVK_JIS_Eisu as _),
            "Muhenkan (0x7B) should map to macOS Eisu"
        );
        assert_eq!(
            super::win_scancode_to_macos_code(WIN_JIS_HENKAN_SCANCODE),
            Some(kVK_JIS_Kana as _),
            "Henkan (0x79) should map to macOS Kana"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_jis_keys_to_windows_scancode() {
        assert_eq!(
            super::macos_code_to_win_scancode(kVK_JIS_Eisu as _),
            Some(WIN_JIS_MUHENKAN_SCANCODE as _),
            "macOS Eisu should map to Windows Muhenkan"
        );
        assert_eq!(
            super::macos_code_to_win_scancode(kVK_JIS_Kana as _),
            Some(WIN_JIS_HENKAN_SCANCODE as _),
            "macOS Kana should map to Windows Henkan"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_jis_keys_to_linux_keycode() {
        assert_eq!(
            super::macos_code_to_linux_code(kVK_JIS_Eisu as _),
            Some(LINUX_X11_JIS_MUHENKAN_KEYCODE as _),
            "macOS Eisu should map to Linux Muhenkan"
        );
        assert_eq!(
            super::macos_code_to_linux_code(kVK_JIS_Kana as _),
            Some(LINUX_X11_JIS_HENKAN_KEYCODE as _),
            "macOS Kana should map to Linux Henkan"
        );
    }
}
