//! User-defined key remapping, configured per remote OS.
//!
//! A key is remapped only when its event is encoded for the peer, after the
//! local modifier and pressed-key bookkeeping. Shortcuts, long-press filtering
//! and the keys released on focus loss therefore keep working on physical keys,
//! and a stored key is remapped exactly once whenever it is replayed.

use std::{collections::HashMap, time::Duration};

use base::message_proto::KeyboardMode;
use rdev::{Event, EventType, Key};
use serde::Deserialize;

/// LocalConfig key, shared with `flutter/lib/models/key_mapping_model.dart`.
pub const LOCAL_CONFIG_KEY: &str = "key-mapping";

/// Stable ids shared with the Flutter settings page. Only keys that produce no
/// text are listed: a remapped event carries no unicode. Caps Lock is left out
/// because its lock state is synced to the peer separately from the key.
const KEYS: &[(&str, Key)] = &[
    ("ControlLeft", Key::ControlLeft),
    ("ControlRight", Key::ControlRight),
    ("ShiftLeft", Key::ShiftLeft),
    ("ShiftRight", Key::ShiftRight),
    ("Alt", Key::Alt),
    ("AltGr", Key::AltGr),
    ("MetaLeft", Key::MetaLeft),
    ("MetaRight", Key::MetaRight),
    ("Escape", Key::Escape),
    ("Tab", Key::Tab),
    ("Backspace", Key::Backspace),
    ("Return", Key::Return),
    ("Delete", Key::Delete),
    ("Insert", Key::Insert),
    ("Home", Key::Home),
    ("End", Key::End),
    ("PageUp", Key::PageUp),
    ("PageDown", Key::PageDown),
    ("UpArrow", Key::UpArrow),
    ("DownArrow", Key::DownArrow),
    ("LeftArrow", Key::LeftArrow),
    ("RightArrow", Key::RightArrow),
    ("F1", Key::F1),
    ("F2", Key::F2),
    ("F3", Key::F3),
    ("F4", Key::F4),
    ("F5", Key::F5),
    ("F6", Key::F6),
    ("F7", Key::F7),
    ("F8", Key::F8),
    ("F9", Key::F9),
    ("F10", Key::F10),
    ("F11", Key::F11),
    ("F12", Key::F12),
];

/// `{"enabled": true, "maps": {"macos": {"Alt": "MetaLeft"}}}`, keyed by the
/// peer platform lowercased with whitespace removed.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Config {
    enabled: bool,
    maps: HashMap<String, HashMap<String, String>>,
}

/// Returns the event to encode in place of `event`, or `None` to keep it.
pub fn remap_event(keyboard_mode: KeyboardMode, peer: &str, event: &Event) -> Option<Event> {
    // Translate and Legacy modes build text from the local modifier state,
    // which a remap cannot change.
    if keyboard_mode != KeyboardMode::Map {
        return None;
    }
    remap_event_with(&peer_map(peer)?, event)
}

/// `(alt, ctrl, shift, command)` for a mouse event while a mapping applies:
/// the held modifier keys as the peer receives them. `None` keeps the flags
/// computed without the mapping.
pub fn mouse_modifiers(
    keyboard_mode: impl FnOnce() -> String,
    peer: impl FnOnce() -> String,
) -> Option<(bool, bool, bool, bool)> {
    #[allow(unused_mut)]
    let mut held: Vec<Key> = super::MODIFIERS_STATE
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, down)| **down)
        .map(|(key, _)| *key)
        .collect();
    // The Left Ctrl held with AltGr is the fake one, which is never remapped.
    #[cfg(target_os = "windows")]
    if unsafe { super::IS_0X021D_DOWN } {
        held.retain(|key| *key != Key::ControlLeft);
    }
    if held.is_empty() || super::get_keyboard_mode_enum(&keyboard_mode()) != KeyboardMode::Map {
        return None;
    }
    Some(modifier_flags(&peer_map(&peer())?, &held))
}

fn peer_map(peer: &str) -> Option<HashMap<String, String>> {
    let raw = hbb_common::config::LocalConfig::get_option(LOCAL_CONFIG_KEY);
    if raw.is_empty() {
        return None;
    }
    parse_peer_map(&raw, peer)
}

fn parse_peer_map(raw: &str, peer: &str) -> Option<HashMap<String, String>> {
    let mut config: Config = match serde_json::from_str(raw) {
        Ok(config) => config,
        Err(e) => {
            hbb_common::throttled_log!(
                Duration::from_secs(60),
                warn,
                "Failed to parse key mapping config: {}",
                e
            );
            return None;
        }
    };
    if !config.enabled {
        return None;
    }
    let mut peer = peer.to_lowercase();
    peer.retain(|c| !c.is_whitespace());
    config.maps.remove(&peer).filter(|map| !map.is_empty())
}

fn target_key(map: &HashMap<String, String>, key: Key) -> Option<Key> {
    let from = KEYS.iter().find(|(_, k)| *k == key)?.0;
    let to = map.get(from)?;
    KEYS.iter()
        .find(|(id, _)| id == to)
        .map(|(_, k)| *k)
        .filter(|k| *k != key)
}

fn remap_event_with(map: &HashMap<String, String>, event: &Event) -> Option<Event> {
    // Windows sends a fake Left Ctrl with this scan code ahead of AltGr, and
    // keyboard.rs drops it by that scan code.
    #[cfg(target_os = "windows")]
    if event.position_code == 0x021D {
        return None;
    }
    let (key, down) = match event.event_type {
        EventType::KeyPress(key) => (key, true),
        EventType::KeyRelease(key) => (key, false),
        _ => return None,
    };
    let to = target_key(map, key)?;
    let (platform_code, position_code) = local_codes(to)?;
    Some(Event {
        unicode: None,
        event_type: if down {
            EventType::KeyPress(to)
        } else {
            EventType::KeyRelease(to)
        },
        platform_code,
        position_code,
        usb_hid: rdev::usb_hid_keycode_from_key(to).unwrap_or_default(),
        ..*event
    })
}

fn modifier_flags(map: &HashMap<String, String>, held: &[Key]) -> (bool, bool, bool, bool) {
    let (mut alt, mut ctrl, mut shift, mut command) = (false, false, false, false);
    for key in held {
        match target_key(map, *key).unwrap_or(*key) {
            Key::Alt | Key::AltGr => alt = true,
            Key::ControlLeft | Key::ControlRight => ctrl = true,
            Key::ShiftLeft | Key::ShiftRight => shift = true,
            Key::MetaLeft | Key::MetaRight => command = true,
            _ => {}
        }
    }
    (alt, ctrl, shift, command)
}

/// `(platform_code, position_code)` of `key` as the local keyboard reports it.
#[cfg(target_os = "windows")]
fn local_codes(key: Key) -> Option<(u32, u32)> {
    Some((
        rdev::win_code_from_key(key)?,
        rdev::win_scancode_from_key(key)?,
    ))
}

#[cfg(target_os = "macos")]
fn local_codes(key: Key) -> Option<(u32, u32)> {
    let code = rdev::macos_keycode_from_key(key)? as u32;
    Some((code, code))
}

#[cfg(target_os = "linux")]
fn local_codes(key: Key) -> Option<(u32, u32)> {
    // platform_code is a keysym on Linux; none of the mappable keys need it.
    Some((0, rdev::linux_keycode_from_key(key)?))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn local_codes(key: Key) -> Option<(u32, u32)> {
    // Mobile clients encode from usb_hid.
    rdev::usb_hid_keycode_from_key(key).map(|_| (0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    const MAC_LAYOUT: &str =
        r#"{"enabled":true,"maps":{"macos":{"Alt":"MetaLeft","MetaLeft":"Alt"}}}"#;

    fn mac_map() -> HashMap<String, String> {
        parse_peer_map(MAC_LAYOUT, "Mac OS").expect("mac peer has a mapping")
    }

    fn event(event_type: EventType) -> Event {
        let key = match event_type {
            EventType::KeyPress(k) | EventType::KeyRelease(k) => k,
            _ => Key::Unknown(0),
        };
        let (platform_code, position_code) = local_codes(key).unwrap_or_default();
        Event {
            time: SystemTime::now(),
            unicode: None,
            event_type,
            platform_code,
            position_code,
            usb_hid: rdev::usb_hid_keycode_from_key(key).unwrap_or_default(),
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            extra_data: 0,
        }
    }

    #[test]
    fn swaps_alt_and_command_for_mac_peer() {
        let map = mac_map();
        let alt =
            remap_event_with(&map, &event(EventType::KeyPress(Key::Alt))).expect("alt is remapped");
        let meta = event(EventType::KeyPress(Key::MetaLeft));
        assert_eq!(alt.event_type, meta.event_type);
        assert_eq!(alt.platform_code, meta.platform_code);
        assert_eq!(alt.position_code, meta.position_code);
        assert_eq!(alt.usb_hid, meta.usb_hid);

        let released = remap_event_with(&map, &event(EventType::KeyRelease(Key::MetaLeft)))
            .expect("meta is remapped");
        assert_eq!(released.event_type, EventType::KeyRelease(Key::Alt));

        assert_eq!(
            modifier_flags(&map, &[Key::Alt, Key::ShiftLeft]),
            (false, false, true, true)
        );
    }

    #[test]
    fn keeps_keys_without_an_active_mapping() {
        assert!(parse_peer_map(MAC_LAYOUT, "Windows").is_none());
        let disabled = MAC_LAYOUT.replace(r#""enabled":true"#, r#""enabled":false"#);
        assert!(parse_peer_map(&disabled, "Mac OS").is_none());
        assert!(parse_peer_map("not json", "Mac OS").is_none());
        assert!(remap_event_with(&mac_map(), &event(EventType::KeyPress(Key::Tab))).is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn keeps_the_fake_control_sent_with_altgr() {
        let map = parse_peer_map(
            r#"{"enabled":true,"maps":{"macos":{"ControlLeft":"MetaLeft"}}}"#,
            "Mac OS",
        )
        .expect("mac peer has a mapping");
        let mut fake = event(EventType::KeyPress(Key::ControlLeft));
        fake.position_code = 0x021D;
        assert!(remap_event_with(&map, &fake).is_none());
        assert!(remap_event_with(&map, &event(EventType::KeyPress(Key::ControlLeft))).is_some());
    }
}
