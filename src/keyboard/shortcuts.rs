//! Keyboard shortcuts for triggering session actions locally.

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

const LOCAL_CONFIG_KEY: &str = "keyboard-shortcuts";

lazy_static::lazy_static! {
    static ref CACHE: RwLock<Arc<Bindings>> = RwLock::new(Arc::new(Bindings::default()));
}

/// Registry of all valid action ids that may appear in `Binding.action`.
/// Source-of-truth lives on the Flutter side (`flutter/lib/consts.dart`,
/// `kShortcutAction*`); these mirror that vocabulary so Rust code can reach
/// for them without re-stringifying.
#[allow(dead_code)]
pub mod action_id {
    pub const SEND_CTRL_ALT_DEL: &str    = "send_ctrl_alt_del";
    pub const TOGGLE_FULLSCREEN: &str    = "toggle_fullscreen";
    pub const SWITCH_DISPLAY_NEXT: &str  = "switch_display_next";
    pub const SWITCH_DISPLAY_PREV: &str  = "switch_display_prev";
    pub const SCREENSHOT: &str           = "screenshot";
    pub const INSERT_LOCK: &str          = "insert_lock";
    pub const REFRESH: &str              = "refresh";
    pub const TOGGLE_AUDIO: &str         = "toggle_audio";
    pub const TOGGLE_BLOCK_INPUT: &str   = "toggle_block_input";
    pub const TOGGLE_RECORDING: &str     = "toggle_recording";
    pub const TOGGLE_PRIVACY_MODE: &str  = "toggle_privacy_mode";
    pub const VIEW_MODE_1_TO_1: &str     = "view_mode_1_to_1";
    pub const VIEW_MODE_SHRINK: &str     = "view_mode_shrink";
    pub const VIEW_MODE_STRETCH: &str    = "view_mode_stretch";
    pub const SWITCH_SIDES: &str         = "switch_sides";
    // switch_tab_1 .. switch_tab_9 are generated below.
}

pub fn switch_tab_action_id(n: u8) -> Option<&'static str> {
    match n {
        1 => Some("switch_tab_1"),
        2 => Some("switch_tab_2"),
        3 => Some("switch_tab_3"),
        4 => Some("switch_tab_4"),
        5 => Some("switch_tab_5"),
        6 => Some("switch_tab_6"),
        7 => Some("switch_tab_7"),
        8 => Some("switch_tab_8"),
        9 => Some("switch_tab_9"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Primary,
    Alt,
    Shift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub action: String,
    pub mods: Vec<Modifier>,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Bindings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bindings: Vec<Binding>,
}

pub fn default_bindings() -> Vec<Binding> {
    let prefix = || vec![Modifier::Primary, Modifier::Alt, Modifier::Shift];
    let mut v = vec![
        Binding { action: action_id::SEND_CTRL_ALT_DEL.into(),    mods: prefix(), key: "delete".into() },
        Binding { action: action_id::TOGGLE_FULLSCREEN.into(),    mods: prefix(), key: "enter".into() },
        Binding { action: action_id::SWITCH_DISPLAY_NEXT.into(),  mods: prefix(), key: "arrow_right".into() },
        Binding { action: action_id::SWITCH_DISPLAY_PREV.into(),  mods: prefix(), key: "arrow_left".into() },
        Binding { action: action_id::SCREENSHOT.into(),           mods: prefix(), key: "p".into() },
    ];
    for n in 1..=9u8 {
        if let Some(action) = switch_tab_action_id(n) {
            v.push(Binding {
                action: action.into(),
                mods: prefix(),
                key: format!("digit{n}"),
            });
        }
    }
    v
}

/// Match a normalized (key, modifiers) pair against the given bindings.
/// Returns the matched action ID, or None.
pub fn match_normalized<'a>(key: &str, mods: &[Modifier], b: &'a Bindings) -> Option<&'a str> {
    if !b.enabled {
        return None;
    }
    for binding in &b.bindings {
        if binding.key == key && mods_equal(&binding.mods, mods) {
            return Some(binding.action.as_str());
        }
    }
    None
}

pub fn normalize_modifiers(alt: bool, ctrl: bool, shift: bool, command: bool) -> Vec<Modifier> {
    let mut v = Vec::new();
    let primary = if cfg!(target_os = "macos") { command } else { ctrl };
    if primary { v.push(Modifier::Primary); }
    if alt     { v.push(Modifier::Alt); }
    if shift   { v.push(Modifier::Shift); }
    v
}

/// Map an rdev::Event to a string key name, matching the storage schema.
/// Returns None for events we don't intercept (modifier-only presses, releases, etc.).
pub fn event_to_key_name(event: &rdev::Event) -> Option<String> {
    use rdev::{EventType, Key};
    let key = match event.event_type {
        EventType::KeyPress(k) => k,
        _ => return None,
    };
    Some(match key {
        Key::Delete => "delete".into(),
        Key::Return => "enter".into(),
        Key::LeftArrow => "arrow_left".into(),
        Key::RightArrow => "arrow_right".into(),
        Key::UpArrow => "arrow_up".into(),
        Key::DownArrow => "arrow_down".into(),
        Key::KeyA => "a".into(),
        Key::KeyB => "b".into(),
        Key::KeyC => "c".into(),
        Key::KeyD => "d".into(),
        Key::KeyE => "e".into(),
        Key::KeyF => "f".into(),
        Key::KeyG => "g".into(),
        Key::KeyH => "h".into(),
        Key::KeyI => "i".into(),
        Key::KeyJ => "j".into(),
        Key::KeyK => "k".into(),
        Key::KeyL => "l".into(),
        Key::KeyM => "m".into(),
        Key::KeyN => "n".into(),
        Key::KeyO => "o".into(),
        Key::KeyP => "p".into(),
        Key::KeyQ => "q".into(),
        Key::KeyR => "r".into(),
        Key::KeyS => "s".into(),
        Key::KeyT => "t".into(),
        Key::KeyU => "u".into(),
        Key::KeyV => "v".into(),
        Key::KeyW => "w".into(),
        Key::KeyX => "x".into(),
        Key::KeyY => "y".into(),
        Key::KeyZ => "z".into(),
        Key::Num1 => "digit1".into(),
        Key::Num2 => "digit2".into(),
        Key::Num3 => "digit3".into(),
        Key::Num4 => "digit4".into(),
        Key::Num5 => "digit5".into(),
        Key::Num6 => "digit6".into(),
        Key::Num7 => "digit7".into(),
        Key::Num8 => "digit8".into(),
        Key::Num9 => "digit9".into(),
        _ => return None,
    })
}

/// Read keyboard-shortcut bindings from `LocalConfig` and refresh the cache.
///
/// Empty or invalid JSON falls back to `Bindings::default()` (disabled, no
/// bindings). Call this once at startup and again whenever the config is
/// written.
pub fn reload_from_config() {
    let raw = hbb_common::config::LocalConfig::get_option(LOCAL_CONFIG_KEY);
    let parsed = if raw.is_empty() {
        Bindings::default()
    } else {
        serde_json::from_str(&raw).unwrap_or_default()
    };
    if let Ok(mut w) = CACHE.write() {
        *w = Arc::new(parsed);
    }
}

/// Snapshot of the currently cached bindings. Cheap (one atomic increment) —
/// safe to call on every keystroke.
pub fn current() -> Arc<Bindings> {
    CACHE
        .read()
        .map(|b| Arc::clone(&b))
        .unwrap_or_else(|_| Arc::new(Bindings::default()))
}

/// Match an `rdev::Event` against the cached bindings. Returns the matched
/// action id, or `None` if no binding fires. The Flutter side ignores unknown
/// action ids (logged as "no handler"), so no whitelist check is needed here.
pub fn match_event(event: &rdev::Event) -> Option<String> {
    let bindings = current();
    if !bindings.enabled {
        return None;
    }
    let key_name = event_to_key_name(event)?;
    let (alt, ctrl, shift, command) =
        crate::keyboard::client::get_modifiers_state(false, false, false, false);
    let mods = normalize_modifiers(alt, ctrl, shift, command);
    match_normalized(&key_name, &mods, &bindings).map(str::to_owned)
}

fn mods_bits(m: &[Modifier]) -> u8 {
    let mut bits = 0u8;
    for x in m {
        bits |= match x {
            Modifier::Primary => 1,
            Modifier::Alt     => 2,
            Modifier::Shift   => 4,
        };
    }
    bits
}

fn mods_equal(a: &[Modifier], b: &[Modifier]) -> bool {
    mods_bits(a) == mods_bits(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_round_trip_json() {
        let json = r#"{
            "enabled": true,
            "bindings": [
                {"action": "send_ctrl_alt_del", "mods": ["primary","alt","shift"], "key": "delete"},
                {"action": "toggle_fullscreen",  "mods": ["primary","alt","shift"], "key": "enter"}
            ]
        }"#;
        let parsed: Bindings = serde_json::from_str(json).expect("parse");
        assert!(parsed.enabled);
        assert_eq!(parsed.bindings.len(), 2);
        assert_eq!(parsed.bindings[0].action, "send_ctrl_alt_del");
        assert_eq!(parsed.bindings[0].key, "delete");

        let serialized = serde_json::to_string(&parsed).expect("serialize");
        let reparsed: Bindings = serde_json::from_str(&serialized).expect("reparse");
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn defaults_match_design_doc() {
        let defaults = default_bindings();
        let actions: Vec<&str> = defaults.iter().map(|b| b.action.as_str()).collect();
        assert!(actions.contains(&action_id::SEND_CTRL_ALT_DEL));
        assert!(actions.contains(&action_id::TOGGLE_FULLSCREEN));
        assert!(actions.contains(&action_id::SWITCH_DISPLAY_NEXT));
        assert!(actions.contains(&action_id::SWITCH_DISPLAY_PREV));
        assert!(actions.contains(&action_id::SCREENSHOT));
        assert!(actions.contains(&"switch_tab_1"));
        assert!(actions.contains(&"switch_tab_9"));
        // every default binding includes the three-modifier prefix
        for b in &defaults {
            assert!(b.mods.contains(&Modifier::Primary));
            assert!(b.mods.contains(&Modifier::Alt));
            assert!(b.mods.contains(&Modifier::Shift));
        }
    }

    fn match_for_test<'a>(key: &str, mods: &[Modifier], b: &'a Bindings) -> Option<&'a str> {
        match_normalized(key, mods, b)
    }

    #[test]
    fn match_returns_none_when_disabled() {
        let bindings = Bindings { enabled: false, bindings: default_bindings() };
        let result = match_for_test("p", &[Modifier::Primary, Modifier::Alt, Modifier::Shift], &bindings);
        assert_eq!(result, None);
    }

    #[test]
    fn match_screenshot_when_enabled() {
        let bindings = Bindings { enabled: true, bindings: default_bindings() };
        let result = match_for_test("p", &[Modifier::Primary, Modifier::Alt, Modifier::Shift], &bindings);
        assert_eq!(result, Some(action_id::SCREENSHOT));
    }

    #[test]
    fn match_returns_none_when_modifiers_partial() {
        let bindings = Bindings { enabled: true, bindings: default_bindings() };
        // missing Shift
        let result = match_for_test("p", &[Modifier::Primary, Modifier::Alt], &bindings);
        assert_eq!(result, None);
    }

    #[test]
    fn match_does_not_fire_on_extra_unbound_keys() {
        let bindings = Bindings { enabled: true, bindings: default_bindings() };
        let result = match_for_test("z", &[Modifier::Primary, Modifier::Alt, Modifier::Shift], &bindings);
        assert_eq!(result, None);
    }

    #[test]
    fn match_handles_duplicate_modifiers_in_input() {
        // A user-edited config could contain duplicate modifiers; the matcher must
        // treat the modifier list as a set, not a multiset.
        let bindings = Bindings {
            enabled: true,
            bindings: vec![Binding {
                action: "x".into(),
                mods: vec![Modifier::Primary, Modifier::Alt],
                key: "a".into(),
            }],
        };
        // Caller passes Primary twice — must not match a binding with Primary+Alt.
        assert_eq!(
            match_normalized("a", &[Modifier::Primary, Modifier::Primary], &bindings),
            None,
        );
        // Caller passes Primary+Alt with one duplicate — should still match.
        assert_eq!(
            match_normalized("a", &[Modifier::Primary, Modifier::Alt, Modifier::Alt], &bindings),
            Some("x"),
        );
    }

    #[test]
    fn modifier_normalization_primary_resolves_per_os() {
        // On Win/Linux: pressing Ctrl satisfies Primary
        let mods = normalize_modifiers(/*alt=*/true, /*ctrl=*/true, /*shift=*/true, /*command=*/false);
        if cfg!(target_os = "macos") {
            // On macOS Ctrl is NOT primary
            assert!(!mods.contains(&Modifier::Primary));
        } else {
            assert!(mods.contains(&Modifier::Primary));
        }
        assert!(mods.contains(&Modifier::Alt));
        assert!(mods.contains(&Modifier::Shift));
    }

    #[test]
    fn modifier_normalization_command_is_primary_on_mac() {
        let mods = normalize_modifiers(true, false, true, /*command=*/true);
        if cfg!(target_os = "macos") {
            assert!(mods.contains(&Modifier::Primary));
        } else {
            // On Win/Linux Command/Meta is NOT primary
            assert!(!mods.contains(&Modifier::Primary));
        }
    }

    #[test]
    fn reload_handles_missing_and_invalid_json() {
        // empty (no value set) → defaults
        hbb_common::config::LocalConfig::set_option(LOCAL_CONFIG_KEY.into(), String::new());
        reload_from_config();
        let b = current();
        assert!(!b.enabled);
        assert!(b.bindings.is_empty());

        // invalid JSON → defaults (no panic)
        hbb_common::config::LocalConfig::set_option(LOCAL_CONFIG_KEY.into(), "not json".into());
        reload_from_config();
        let b = current();
        assert!(!b.enabled);
    }
}
