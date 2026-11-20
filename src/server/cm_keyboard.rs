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
        // Block activation before notifying the loop, which may await network I/O.
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
        if !event.down
            && !event.press
            && !matches!(
                event.union,
                Some(key_event::Union::Unicode(_) | key_event::Union::Seq(_))
            )
        {
            return true;
        }
        !is_activation_event(event)
    }
}

fn is_activation_event(event: &KeyEvent) -> bool {
    let mode = event.mode.enum_value_or(KeyboardMode::Legacy);
    // Legacy modifier entries are injected as keys before the main key.
    if matches!(mode, KeyboardMode::Legacy | KeyboardMode::Auto)
        && event
            .modifiers
            .iter()
            .any(|key| ACTIVATION_KEYS.contains(&key.enum_value_or(ControlKey::Unknown)))
    {
        return true;
    }
    match &event.union {
        Some(key_event::Union::ControlKey(key)) => {
            ACTIVATION_KEYS.contains(&key.enum_value_or(ControlKey::Unknown))
        }
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
    #[cfg(target_os = "linux")]
    use rdev::linux_keycode_from_key as raw_code;
    #[cfg(target_os = "macos")]
    use rdev::macos_keycode_from_key as raw_code;
    #[cfg(target_os = "windows")]
    use rdev::win_scancode_from_key as raw_code;

    #[cfg(target_os = "linux")]
    const LINUX_SELECT_KEYCODE: u32 = 361;

    #[test]
    fn focused_cm_blocks_activation_but_allows_editing_and_shortcuts() {
        let gate = CmKeyboardGate::default();
        let mut key = KeyEvent::new();
        key.down = true;
        for control in [
            ControlKey::Return,
            ControlKey::NumpadEnter,
            ControlKey::Space,
            ControlKey::Select,
        ] {
            key.set_control_key(control);
            gate.set_blocked(false);
            assert!(gate.allows(&key));
            gate.set_blocked(true);
            assert!(!gate.allows(&key));
            key.down = false;
            assert!(gate.allows(&key));
            key.down = true;
        }
        for control in [
            ControlKey::Control,
            ControlKey::Alt,
            ControlKey::Meta,
            ControlKey::Tab,
        ] {
            key.set_control_key(control);
            assert!(gate.allows(&key));
        }
        key.set_chr(u32::from('v'));
        key.modifiers = vec![ControlKey::Control.into()];
        assert!(gate.allows(&key));
        key.modifiers.push(ControlKey::Return.into());
        assert!(!gate.allows(&key));
        key.modifiers.clear();
        key.set_control_key(ControlKey::CtrlAltDel);
        assert!(gate.allows(&key));
    }

    #[test]
    fn focused_cm_checks_raw_activation_keys() {
        let gate = CmKeyboardGate::default();
        gate.set_blocked(true);
        let mut key = KeyEvent::new();
        for mode in [KeyboardMode::Map, KeyboardMode::Translate] {
            key.mode = mode.into();
            for (native, allowed) in [
                (rdev::Key::Return, false),
                (rdev::Key::KpReturn, false),
                (rdev::Key::Space, false),
                #[cfg(target_os = "linux")]
                (rdev::Key::Unknown(LINUX_SELECT_KEYCODE), false),
                (rdev::Key::Tab, true),
                (rdev::Key::KeyV, true),
            ] {
                key.down = true;
                key.set_chr(raw_code(native).unwrap() as _);
                assert_eq!(gate.allows(&key), allowed, "{mode:?}: {native:?}");
                if mode == KeyboardMode::Map && !allowed {
                    key.set_chr(key.chr() | (1 << u16::BITS));
                    assert!(!gate.allows(&key));
                }
                key.down = false;
                assert!(gate.allows(&key));
            }
        }
    }

    #[test]
    fn focused_cm_checks_text_even_without_key_down() {
        let gate = CmKeyboardGate::default();
        gate.set_blocked(true);
        let mut key = KeyEvent::new();
        for (character, allowed) in [
            ('a', true),
            ('界', true),
            ('\u{20020}', true),
            (' ', false),
            ('\r', false),
            ('\n', false),
        ] {
            key.set_unicode(u32::from(character));
            assert_eq!(gate.allows(&key), allowed);
            key.set_chr(u32::from(character));
            key.press = true;
            assert_eq!(gate.allows(&key), allowed);
            key.press = false;
        }
        for mode in [KeyboardMode::Legacy, KeyboardMode::Translate] {
            key.mode = mode.into();
            key.set_seq("remote\u{20020}".to_owned());
            assert_eq!(
                gate.allows(&key),
                !cfg!(target_os = "windows") || mode != KeyboardMode::Translate
            );
            key.set_seq("remote text".to_owned());
            assert!(!gate.allows(&key));
            gate.set_blocked(false);
            assert!(gate.allows(&key));
            gate.set_blocked(true);
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn focused_cm_checks_windows_virtual_keys_and_text() {
        use winapi::um::winuser::{VK_GAMEPAD_A, VK_RETURN, VK_SELECT, VK_SPACE};
        let gate = CmKeyboardGate::default();
        gate.set_blocked(true);
        let mut key = KeyEvent::new();
        key.mode = KeyboardMode::Translate.into();
        key.down = true;
        for vk in [VK_RETURN, VK_SPACE, VK_SELECT, VK_GAMEPAD_A] {
            let code = (vk as u32) << u16::BITS;
            key.set_chr(code);
            assert!(!gate.allows(&key));
            key.set_win2win_hotkey(code | u32::from('v'));
            assert!(!gate.allows(&key));
        }
        let code = rdev::win_keycode_from_key(rdev::Key::KeyV).unwrap() << u16::BITS;
        key.set_chr(code);
        assert!(gate.allows(&key));
        key.set_win2win_hotkey(code | u32::from(' '));
        assert!(!gate.allows(&key));
        key.down = false;
        assert!(gate.allows(&key));
        key.set_seq("\u{10020}".to_owned());
        assert!(!gate.allows(&key));
        key.mode = KeyboardMode::Legacy.into();
        key.down = true;
        key.set_chr(u32::from('\u{20020}'));
        key.modifiers.push(ControlKey::Control.into());
        assert!(!gate.allows(&key));
    }

    #[test]
    fn cm_ipc_disconnect_blocks_before_connection_receiver_runs() {
        for blocked in [false, true] {
            let gate = CmKeyboardGate::default();
            gate.set_blocked(blocked);
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut key = KeyEvent::new();
            key.down = true;
            key.set_control_key(ControlKey::Return);

            gate.on_cm_ipc_disconnect(&tx).unwrap();
            assert!(!gate.allows(&key));
            assert!(matches!(rx.try_recv().unwrap(), Data::CmIpcDisconnected));

            gate.set_blocked(false);
            rx.close();
            assert!(gate.on_cm_ipc_disconnect(&tx).is_err());
            assert!(!gate.allows(&key));
        }
    }
}
