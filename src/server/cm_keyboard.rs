use crate::ipc::Data;
use base::message_proto::{key_event, ControlKey, KeyEvent, KeyboardMode};
use hbb_common::{tokio::sync::mpsc::UnboundedSender, ResultType};
use std::sync::atomic::{AtomicBool, Ordering};

const ACTIVATION_KEYS: [ControlKey; 4] = [
    ControlKey::Return,
    ControlKey::NumpadEnter,
    ControlKey::Space,
    ControlKey::Select,
];

#[derive(Default)]
pub(super) struct CmKeyboardGate {
    blocked: AtomicBool,
}

impl CmKeyboardGate {
    pub(super) fn on_cm_ipc_disconnect(&self, tx: &UnboundedSender<Data>) -> ResultType<()> {
        // Block before notifying the connection loop, which may await network I/O.
        self.set_blocked(true);
        tx.send(Data::CmIpcDisconnected)?;
        Ok(())
    }

    pub(super) fn set_blocked(&self, blocked: bool) {
        self.blocked.store(blocked, Ordering::SeqCst);
    }

    pub(super) fn allows(&self, event: &KeyEvent) -> bool {
        if !self.blocked.load(Ordering::SeqCst) {
            return true;
        }
        // Text injection can generate key presses even when down=false.
        let is_press = event.down
            || event.press
            || matches!(
                event.union,
                Some(key_event::Union::Unicode(_) | key_event::Union::Seq(_))
            );
        !is_press || !is_activation_event(event)
    }
}

fn is_activation_event(event: &KeyEvent) -> bool {
    let mode = event.mode.enum_value_or(KeyboardMode::Legacy);
    // Legacy modifier entries are injected as keys before the main key.
    if ACTIVATION_KEYS.contains(&event.control_key())
        || (matches!(mode, KeyboardMode::Legacy | KeyboardMode::Auto)
            && event
                .modifiers
                .iter()
                .any(|key| ACTIVATION_KEYS.contains(&key.enum_value_or(ControlKey::Unknown))))
    {
        return true;
    }
    match &event.union {
        Some(key_event::Union::Chr(code))
            if matches!(mode, KeyboardMode::Map | KeyboardMode::Translate) =>
        {
            is_activation_raw_key(*code, mode)
        }
        Some(key_event::Union::Chr(code) | key_event::Union::Unicode(code)) => {
            is_activation_character(*code, event)
        }
        Some(key_event::Union::Seq(text)) => text
            .chars()
            .any(|c| is_activation_character(c as u32, event)),
        #[cfg(target_os = "windows")]
        Some(key_event::Union::Win2winHotkey(code)) => {
            is_activation_character(*code, event)
                || is_activation_raw_key(*code & !u32::from(u16::MAX), KeyboardMode::Translate)
        }
        _ => false,
    }
}

fn is_activation_character(code: u32, _event: &KeyEvent) -> bool {
    // Legacy Unicode/sequence input preserves surrogate pairs; these paths do not.
    #[cfg(target_os = "windows")]
    let code = if _event.mode.enum_value() == Ok(KeyboardMode::Translate)
        || (matches!(_event.union, Some(key_event::Union::Chr(_)))
            && super::input_service::has_hotkey_modifiers(_event))
    {
        u32::from(code as u16)
    } else {
        code
    };
    matches!(char::from_u32(code), Some(' ' | '\r' | '\n'))
}

fn is_activation_raw_key(code: u32, _mode: KeyboardMode) -> bool {
    #[cfg(target_os = "linux")]
    {
        const XKB_KEYCODE_OFFSET: u16 = 8;
        if code as u16 == evdev::Key::KEY_SELECT.code() + XKB_KEYCODE_OFFSET {
            return true;
        }
    }
    #[cfg(target_os = "windows")]
    let key = if _mode == KeyboardMode::Translate && code >> u16::BITS != 0 {
        let vk = code >> u16::BITS;
        if vk == winapi::um::winuser::VK_GAMEPAD_A as u32 {
            return true;
        }
        rdev::win_key_from_keycode(vk)
    } else {
        // Enter and Space keep their base scan code with either extended prefix.
        crate::keyboard::keycode_to_rdev_key(u32::from(code as u8))
    };
    #[cfg(not(target_os = "windows"))]
    let key = crate::keyboard::keycode_to_rdev_key(u32::from(code as u16));
    matches!(
        key,
        rdev::Key::Return | rdev::Key::KpReturn | rdev::Key::Space | rdev::Key::Select
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::tokio::sync::mpsc;

    #[test]
    fn focused_cm_blocks_queued_keys_and_text_until_blur() {
        let gate = CmKeyboardGate::default();
        let mut key = KeyEvent::new();
        key.down = true;
        key.set_control_key(ControlKey::Return);
        assert!(gate.allows(&key));

        gate.set_blocked(true);
        for control in [
            ControlKey::Return,
            ControlKey::NumpadEnter,
            ControlKey::Space,
            ControlKey::Select,
        ] {
            key.set_control_key(control);
            assert!(!gate.allows(&key));
        }
        key.down = false;
        assert!(gate.allows(&key));
        key.set_unicode(u32::from('\r'));
        assert!(!gate.allows(&key));
        key.set_unicode(u32::from('\u{20020}'));
        assert!(gate.allows(&key));
        key.set_seq("remote\u{20020}".to_owned());
        assert!(gate.allows(&key));
        key.set_seq("remote text".to_owned());
        assert!(!gate.allows(&key));

        gate.set_blocked(false);
        assert!(gate.allows(&key));

        let (tx, mut rx) = mpsc::unbounded_channel();
        gate.on_cm_ipc_disconnect(&tx).unwrap();
        assert!(!gate.allows(&key));
        assert!(matches!(rx.try_recv().unwrap(), Data::CmIpcDisconnected));
        gate.set_blocked(false);
        rx.close();
        assert!(gate.on_cm_ipc_disconnect(&tx).is_err());
        assert!(!gate.allows(&key));
    }

    #[test]
    fn focused_cm_allows_editing_and_shortcuts() {
        let gate = CmKeyboardGate::default();
        gate.set_blocked(true);
        let mut key = KeyEvent::new();
        key.set_chr(u32::from('v'));
        key.press = true;
        key.modifiers.push(ControlKey::Control.into());
        assert!(gate.allows(&key));
        key.modifiers.push(ControlKey::Return.into());
        assert!(!gate.allows(&key));
        key.modifiers = vec![ControlKey::Alt.into()];
        key.set_control_key(ControlKey::Tab);
        assert!(gate.allows(&key));
        key.modifiers.clear();
        key.set_control_key(ControlKey::CtrlAltDel);
        assert!(gate.allows(&key));
        key.set_chr(u32::from('\u{20020}'));
        key.modifiers.push(ControlKey::Control.into());
        assert_eq!(gate.allows(&key), !cfg!(target_os = "windows"));
        key.press = false;
        key.mode = KeyboardMode::Translate.into();
        key.set_seq("\u{20020}".to_owned());
        assert_eq!(gate.allows(&key), !cfg!(target_os = "windows"));
    }

    #[test]
    fn focused_cm_checks_raw_activation_keys() {
        // Native Return, numpad Enter, Space (plus Linux Select), and Tab.
        #[cfg(target_os = "windows")]
        let (activation_codes, tab_code) = (&[0x1c, 0xe01c, 0x39][..], 0x0f);
        #[cfg(target_os = "macos")]
        let (activation_codes, tab_code) = (&[36, 76, 49][..], 48);
        #[cfg(target_os = "linux")]
        let (activation_codes, tab_code) = (&[36, 104, 65, 361][..], 23);
        let gate = CmKeyboardGate::default();
        gate.set_blocked(true);
        let mut key = KeyEvent::new();
        key.down = true;
        for mode in [KeyboardMode::Map, KeyboardMode::Translate] {
            key.mode = mode.into();
            for &code in activation_codes {
                key.set_chr(code);
                assert!(!gate.allows(&key), "{mode:?}: {code}");
            }
            key.down = false;
            assert!(gate.allows(&key));
            key.down = true;
            key.set_chr(tab_code);
            assert!(gate.allows(&key));
        }
        #[cfg(target_os = "windows")]
        {
            let enter = (winapi::um::winuser::VK_RETURN as u32) << u16::BITS;
            key.set_chr(enter);
            assert!(!gate.allows(&key));
            key.set_win2win_hotkey(enter | u32::from('v'));
            assert!(!gate.allows(&key));
            let letter = rdev::win_keycode_from_key(rdev::Key::KeyV).unwrap() << u16::BITS;
            key.set_chr(letter);
            assert!(gate.allows(&key));
            key.set_win2win_hotkey(letter | u32::from(' '));
            assert!(!gate.allows(&key));
            key.down = false;
            assert!(gate.allows(&key));
        }
    }
}
