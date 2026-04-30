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
    pub const SWITCH_DISPLAY_ALL: &str   = "switch_display_all";
    pub const SCREENSHOT: &str           = "screenshot";
    pub const INSERT_LOCK: &str          = "insert_lock";
    pub const REFRESH: &str              = "refresh";
    pub const TOGGLE_BLOCK_INPUT: &str   = "toggle_block_input";
    pub const TOGGLE_RECORDING: &str     = "toggle_recording";
    pub const SWITCH_SIDES: &str         = "switch_sides";
    pub const CLOSE_TAB: &str            = "close_tab";
    pub const TOGGLE_TOOLBAR: &str       = "toggle_toolbar";
    pub const RESTART_REMOTE: &str       = "restart_remote";
    pub const RESET_CANVAS: &str         = "reset_canvas";
    pub const TOGGLE_MUTE: &str          = "toggle_mute";
    pub const PIN_TOOLBAR: &str          = "pin_toolbar";
    pub const VIEW_MODE_ORIGINAL: &str   = "view_mode_original";
    pub const VIEW_MODE_ADAPTIVE: &str   = "view_mode_adaptive";
    pub const TOGGLE_CHAT: &str               = "toggle_chat";
    pub const TOGGLE_QUALITY_MONITOR: &str    = "toggle_quality_monitor";
    pub const TOGGLE_SHOW_REMOTE_CURSOR: &str = "toggle_show_remote_cursor";
    pub const TOGGLE_SHOW_MY_CURSOR: &str     = "toggle_show_my_cursor";
    pub const TOGGLE_DISABLE_CLIPBOARD: &str  = "toggle_disable_clipboard";
    pub const PRIVACY_MODE_1: &str            = "privacy_mode_1";
    pub const PRIVACY_MODE_2: &str            = "privacy_mode_2";
    pub const KEYBOARD_MODE_MAP: &str         = "keyboard_mode_map";
    pub const KEYBOARD_MODE_TRANSLATE: &str   = "keyboard_mode_translate";
    pub const KEYBOARD_MODE_LEGACY: &str      = "keyboard_mode_legacy";
    pub const CODEC_AUTO: &str                = "codec_auto";
    pub const CODEC_VP8: &str                 = "codec_vp8";
    pub const CODEC_VP9: &str                 = "codec_vp9";
    pub const CODEC_AV1: &str                 = "codec_av1";
    pub const CODEC_H264: &str                = "codec_h264";
    pub const CODEC_H265: &str                = "codec_h265";
    pub const PLUG_OUT_ALL_VIRTUAL_DISPLAYS: &str = "plug_out_all_virtual_displays";
    pub const TOGGLE_RELATIVE_MOUSE_MODE: &str = "toggle_relative_mouse_mode";
    pub const TOGGLE_FOLLOW_REMOTE_CURSOR: &str = "toggle_follow_remote_cursor";
    pub const TOGGLE_FOLLOW_REMOTE_WINDOW: &str = "toggle_follow_remote_window";
    pub const TOGGLE_ZOOM_CURSOR: &str        = "toggle_zoom_cursor";
    pub const TOGGLE_REVERSE_MOUSE_WHEEL: &str = "toggle_reverse_mouse_wheel";
    pub const TOGGLE_SWAP_LEFT_RIGHT_MOUSE: &str = "toggle_swap_left_right_mouse";
    pub const TOGGLE_LOCK_AFTER_SESSION_END: &str = "toggle_lock_after_session_end";
    pub const TOGGLE_TRUE_COLOR: &str         = "toggle_true_color";
    pub const TOGGLE_SWAP_CTRL_CMD: &str      = "toggle_swap_ctrl_cmd";
    pub const TOGGLE_ENABLE_FILE_COPY_PASTE: &str = "toggle_enable_file_copy_paste";
    pub const VIEW_MODE_CUSTOM: &str          = "view_mode_custom";
    pub const IMAGE_QUALITY_BEST: &str        = "image_quality_best";
    pub const IMAGE_QUALITY_BALANCED: &str    = "image_quality_balanced";
    pub const IMAGE_QUALITY_LOW: &str         = "image_quality_low";
    pub const SEND_CLIPBOARD_KEYSTROKES: &str = "send_clipboard_keystrokes";
    pub const TOGGLE_INPUT_SOURCE: &str       = "toggle_input_source";
    pub const SWITCH_TAB_NEXT: &str           = "switch_tab_next";
    pub const SWITCH_TAB_PREV: &str           = "switch_tab_prev";
    pub const TOGGLE_VOICE_CALL: &str         = "toggle_voice_call";
    pub const TOGGLE_VIEW_ONLY: &str          = "toggle_view_only";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Primary,
    Ctrl,
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
    /// Persistent companion to `enabled`: when true, the matcher returns early
    /// and every keystroke flows through to the remote (i.e. all bindings are
    /// suspended). Stored alongside `enabled` and `bindings` so a single
    /// reload refreshes both flags.
    #[serde(default)]
    pub pass_through: bool,
    #[serde(default)]
    pub bindings: Vec<Binding>,
}

pub fn default_bindings() -> Vec<Binding> {
    let prefix = || vec![Modifier::Primary, Modifier::Alt, Modifier::Shift];
    // Defaults align with AnyDesk's M/S/I/C/Delete/Arrow/Digit conventions
    // where applicable; "P" for screenshot also matches AnyDesk.
    vec![
        Binding { action: action_id::SEND_CTRL_ALT_DEL.into(),         mods: prefix(), key: "delete".into() },
        Binding { action: action_id::TOGGLE_FULLSCREEN.into(),         mods: prefix(), key: "enter".into() },
        Binding { action: action_id::SWITCH_DISPLAY_NEXT.into(),       mods: prefix(), key: "arrow_right".into() },
        Binding { action: action_id::SWITCH_DISPLAY_PREV.into(),       mods: prefix(), key: "arrow_left".into() },
        Binding { action: action_id::SCREENSHOT.into(),                mods: prefix(), key: "p".into() },
        Binding { action: action_id::TOGGLE_SHOW_REMOTE_CURSOR.into(), mods: prefix(), key: "m".into() },
        Binding { action: action_id::TOGGLE_MUTE.into(),               mods: prefix(), key: "s".into() },
        Binding { action: action_id::TOGGLE_BLOCK_INPUT.into(),        mods: prefix(), key: "i".into() },
        Binding { action: action_id::TOGGLE_CHAT.into(),               mods: prefix(), key: "c".into() },
    ]
}

/// Match a normalized (key, modifiers) pair against the given bindings.
/// Returns the matched action ID, or None when the matcher is off
/// (`enabled == false`), suspended (`pass_through == true`), or no binding
/// fires for this combo.
///
/// Defense-in-depth: bindings with an empty modifier list are skipped here
/// even though the recording dialog refuses to save them. A hand-edited
/// config (or a future writer-side bug) that lets an empty-mods binding
/// through would otherwise turn that key's every press into a swallowed
/// shortcut, breaking normal typing in the remote session — a much worse
/// failure than the binding simply not firing.
pub fn match_normalized<'a>(key: &str, mods: &[Modifier], b: &'a Bindings) -> Option<&'a str> {
    if !b.enabled || b.pass_through {
        return None;
    }
    for binding in &b.bindings {
        if binding.mods.is_empty() {
            continue;
        }
        if binding.key == key && mods_equal(&binding.mods, mods) {
            return Some(binding.action.as_str());
        }
    }
    None
}

pub fn normalize_modifiers(alt: bool, ctrl: bool, shift: bool, command: bool) -> Vec<Modifier> {
    // iOS shares Apple's keyboard semantics with macOS — recording dialog
    // already treats iOS as `_isMac`, so the matcher must too.
    //
    // AltGr conflation: `get_modifiers_state` ORs Alt and AltGr, so an
    // AltGr+key press satisfies `Modifier::Alt`. Theoretical collision only;
    // fix at `get_modifiers_state` if a real bug surfaces.
    let mut v = Vec::new();
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        if command { v.push(Modifier::Primary); }
        if ctrl    { v.push(Modifier::Ctrl); }
    } else {
        if ctrl    { v.push(Modifier::Primary); }
    }
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
        Key::Backspace => "backspace".into(),
        Key::Tab => "tab".into(),
        Key::Space => "space".into(),
        Key::Home => "home".into(),
        Key::End => "end".into(),
        Key::PageUp => "page_up".into(),
        Key::PageDown => "page_down".into(),
        Key::Insert => "insert".into(),
        // Numpad Enter (`KpReturn`) shares the "enter" name with the main
        // Return key — matches the Web matcher (`NumpadEnter` -> "enter") and
        // matches user expectation that the two physical Enters are
        // interchangeable for shortcuts.
        Key::Return | Key::KpReturn => "enter".into(),
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
        Key::Num0 => "digit0".into(),
        Key::Num1 => "digit1".into(),
        Key::Num2 => "digit2".into(),
        Key::Num3 => "digit3".into(),
        Key::Num4 => "digit4".into(),
        Key::Num5 => "digit5".into(),
        Key::Num6 => "digit6".into(),
        Key::Num7 => "digit7".into(),
        Key::Num8 => "digit8".into(),
        Key::Num9 => "digit9".into(),
        Key::F1 => "f1".into(),
        Key::F2 => "f2".into(),
        Key::F3 => "f3".into(),
        Key::F4 => "f4".into(),
        Key::F5 => "f5".into(),
        Key::F6 => "f6".into(),
        Key::F7 => "f7".into(),
        Key::F8 => "f8".into(),
        Key::F9 => "f9".into(),
        Key::F10 => "f10".into(),
        Key::F11 => "f11".into(),
        Key::F12 => "f12".into(),
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
///
/// ── Two known minor warts. DO NOT add global state to "fix" either: ──
///
/// 1. Orphan KeyRelease forwarded to peer.
///    When a shortcut matches we eat the KeyPress, but the matching
///    KeyRelease (whose `event_type` returns None from `event_to_key_name`)
///    still flows through to the peer. The remote sees a release for a
///    press it never received. Every input server we forward to ignores
///    releases for unpressed keys, so user-visible impact is nil — the
///    pre-existing hard-coded screenshot-shortcut path had the same shape
///    for years without a single bug report.
///
/// 2. OS auto-repeat re-dispatches a held shortcut.
///    rdev does not expose an `is_repeat` flag, so a held combo
///    (Cmd+Alt+Shift+P) would dispatch every ~30-50ms while the keys are
///    down — toggle actions oscillate, screenshot fires many times. In
///    practice the OS initial auto-repeat delay is ~250ms and a normal
///    shortcut press is 50-100ms, so the user has to *deliberately* hold
///    the combo to hit this. The Web side gets a free fix via the
///    browser's `KeyboardEvent.repeat`; on native we accept the wart.
///
/// The "fix" for either would be a process-global `HashSet<rdev::Key>` (or
/// equivalent) with paired insert-on-press / remove-on-release logic in
/// both `process_event*` paths plus a clear-on-leave hook. The cost:
///
///   * Lock contention on the hot keystroke path.
///   * Three input sources (rdev grab, Flutter raw key, Flutter USB HID)
///     all converge to `rdev::Key`, so correctness depends on
///     `rdev::key_from_code` / `rdev::usb_hid_key_from_code` /
///     `rdev::get_win_key` agreeing on the same physical key — the project
///     already has scattered swap_modifier_key / ControlLeft↔MetaLeft
///     fixups for places where they historically *didn't* agree. Any new
///     mismatch silently leaks the set; "shortcut stopped responding"
///     after a stuck entry is a worse failure mode than "shortcut fired
///     twice."
///   * Leak risk on focus loss / disconnect, requiring a clear hook the
///     callers must remember to invoke.
///   * Two new code paths to keep in lockstep with two existing keyboard
///     pipelines.
///
/// For two warts whose user-visible impact is nil-to-marginal, that
/// trade-off goes the wrong way. Leave it. If a real user bug shows up
/// here, revisit then with concrete repro — not pre-emptively.
pub fn match_event(event: &rdev::Event) -> Option<String> {
    let bindings = current();
    if !bindings.enabled || bindings.pass_through {
        return None;
    }
    // Note: `match_normalized` re-checks both flags below — this short-circuit
    // is just to avoid the `event_to_key_name` + `get_modifiers_state` work
    // in the common bypass case.
    let key_name = event_to_key_name(event)?;
    let (alt, ctrl, shift, command) =
        crate::keyboard::client::get_modifiers_state(false, false, false, false);
    let mods = normalize_modifiers(alt, ctrl, shift, command);
    match_normalized(&key_name, &mods, &bindings).map(str::to_owned)
}

/// Match `event` against the cached bindings; if it matched, push a
/// `shortcut_triggered` Flutter session event and return `true` so the caller
/// can `return` early. Returns `false` when no shortcut fired (caller should
/// continue with normal key handling).
///
/// `session_id`:
/// * `Some(&id)` — Flutter FFI path: dispatch to the exact session whose key
///   event we're processing. No dependence on the global focus tracker.
/// * `None` — rdev grab loop: the loop is process-wide and has no way to know
///   which Flutter session id the keystroke was meant for, so route to the
///   globally-current session via `flutter::get_cur_session_id()`.
#[cfg(feature = "flutter")]
pub fn try_dispatch(session_id: Option<&hbb_common::SessionID>, event: &rdev::Event) -> bool {
    let Some(action_id) = match_event(event) else {
        return false;
    };
    let resolved;
    let sid = match session_id {
        Some(id) => id,
        None => {
            resolved = crate::flutter::get_cur_session_id();
            &resolved
        }
    };
    crate::flutter::push_session_event(sid, "shortcut_triggered", vec![("action", &action_id)]);
    true
}

fn mods_bits(m: &[Modifier]) -> u8 {
    let mut bits = 0u8;
    for x in m {
        bits |= match x {
            Modifier::Primary => 1,
            Modifier::Alt     => 2,
            Modifier::Shift   => 4,
            // macOS users can bind shortcuts that use Control independently
            // of Command. On Win/Linux this variant should never appear in a
            // saved binding (`normalize_modifiers` collapses Ctrl into
            // Primary), but we still give it a distinct bit so a hand-edited
            // config can't accidentally collide with another modifier.
            Modifier::Ctrl    => 8,
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

    fn make_press(k: rdev::Key) -> rdev::Event {
        rdev::Event {
            time: std::time::SystemTime::now(),
            unicode: None,
            platform_code: 0,
            position_code: 0,
            event_type: rdev::EventType::KeyPress(k),
            usb_hid: 0,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            extra_data: 0,
        }
    }

    #[test]
    fn event_to_key_name_handles_f_keys() {
        use rdev::Key;
        assert_eq!(event_to_key_name(&make_press(Key::F1)), Some("f1".into()));
        assert_eq!(event_to_key_name(&make_press(Key::F5)), Some("f5".into()));
        assert_eq!(event_to_key_name(&make_press(Key::F12)), Some("f12".into()));
    }

    /// Cross-language parity for default bindings. The fixture file is the
    /// shared source of truth — Dart has a mirror test against the same file
    /// (`kDefaultShortcutBindings matches fixture` in
    /// `flutter/test/keyboard_shortcuts_test.dart`). Any drift on either
    /// side breaks one of the two tests.
    #[test]
    fn default_bindings_match_fixture_json() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../flutter/test/fixtures/default_keyboard_shortcuts.json"
        ))
        .expect("fixture is valid JSON");
        let actual: serde_json::Value =
            serde_json::to_value(default_bindings()).expect("serialize defaults");
        assert_eq!(
            fixture, actual,
            "default_bindings() drifted from \
             flutter/test/fixtures/default_keyboard_shortcuts.json — update \
             shortcuts.rs, the fixture, and Dart kDefaultShortcutBindings together"
        );
    }

    #[test]
    fn event_to_key_name_treats_numpad_enter_as_enter() {
        use rdev::{Event, EventType, Key};
        let make = |k: Key| Event {
            time: std::time::SystemTime::now(),
            unicode: None,
            platform_code: 0,
            position_code: 0,
            event_type: EventType::KeyPress(k),
            usb_hid: 0,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            extra_data: 0,
        };
        assert_eq!(event_to_key_name(&make(Key::Return)), Some("enter".into()));
        assert_eq!(event_to_key_name(&make(Key::KpReturn)), Some("enter".into()));
    }

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
        assert!(actions.contains(&action_id::TOGGLE_SHOW_REMOTE_CURSOR));
        assert!(actions.contains(&action_id::TOGGLE_MUTE));
        assert!(actions.contains(&action_id::TOGGLE_BLOCK_INPUT));
        assert!(actions.contains(&action_id::TOGGLE_CHAT));
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
    fn match_returns_none_when_pass_through() {
        let bindings = Bindings {
            enabled: true,
            pass_through: true,
            bindings: default_bindings(),
        };
        let result = match_normalized(
            "p",
            &[Modifier::Primary, Modifier::Alt, Modifier::Shift],
            &bindings,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn match_returns_none_when_disabled() {
        let bindings = Bindings { enabled: false, pass_through: false, bindings: default_bindings() };
        let result = match_for_test("p", &[Modifier::Primary, Modifier::Alt, Modifier::Shift], &bindings);
        assert_eq!(result, None);
    }

    #[test]
    fn match_screenshot_when_enabled() {
        let bindings = Bindings { enabled: true, pass_through: false, bindings: default_bindings() };
        let result = match_for_test("p", &[Modifier::Primary, Modifier::Alt, Modifier::Shift], &bindings);
        assert_eq!(result, Some(action_id::SCREENSHOT));
    }

    #[test]
    fn match_returns_none_when_modifiers_partial() {
        let bindings = Bindings { enabled: true, pass_through: false, bindings: default_bindings() };
        // missing Shift
        let result = match_for_test("p", &[Modifier::Primary, Modifier::Alt], &bindings);
        assert_eq!(result, None);
    }

    #[test]
    fn match_does_not_fire_on_extra_unbound_keys() {
        let bindings = Bindings { enabled: true, pass_through: false, bindings: default_bindings() };
        let result = match_for_test("z", &[Modifier::Primary, Modifier::Alt, Modifier::Shift], &bindings);
        assert_eq!(result, None);
    }

    #[test]
    fn match_handles_duplicate_modifiers_in_input() {
        // A user-edited config could contain duplicate modifiers; the matcher must
        // treat the modifier list as a set, not a multiset.
        let bindings = Bindings {
            enabled: true,
            pass_through: false,
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
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            // On Apple platforms Ctrl is NOT primary
            assert!(!mods.contains(&Modifier::Primary));
            assert!(mods.contains(&Modifier::Ctrl));
        } else {
            assert!(mods.contains(&Modifier::Primary));
        }
        assert!(mods.contains(&Modifier::Alt));
        assert!(mods.contains(&Modifier::Shift));
    }

    #[test]
    fn modifier_normalization_command_is_primary_on_apple() {
        let mods = normalize_modifiers(true, false, true, /*command=*/true);
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            assert!(mods.contains(&Modifier::Primary));
        } else {
            // On Win/Linux Command/Meta is NOT primary
            assert!(!mods.contains(&Modifier::Primary));
        }
    }

    #[test]
    fn match_refuses_zero_modifier_bindings() {
        // Defense-in-depth: a hand-edited config with empty `mods` must NOT
        // turn every plain "P" press into a screenshot shortcut, which would
        // swallow all typing in the remote session. The recording dialog
        // already refuses to save such bindings, but the matcher must hold
        // the line independently.
        let bindings = Bindings {
            enabled: true,
            pass_through: false,
            bindings: vec![Binding {
                action: "screenshot".into(),
                mods: vec![],
                key: "p".into(),
            }],
        };
        assert_eq!(match_normalized("p", &[], &bindings), None);
        // Even with extra modifiers held by the user, a zero-mod binding
        // still doesn't match (no shape of held modifiers can equal the
        // empty saved set after the empty-check skips the entry).
        assert_eq!(
            match_normalized("p", &[Modifier::Primary], &bindings),
            None,
        );
    }

    /// Cross-language parity for the full set of shortcut-bindable key
    /// names (not just the defaults). The fixture lists every name the
    /// matcher accepts; this test verifies the (rdev::Key → name) round-trip
    /// covers exactly that set. Dart has a mirror test against the same
    /// fixture (`logicalKeyName covers the supported-keys fixture` in
    /// `flutter/test/keyboard_shortcuts_test.dart`).
    ///
    /// Adding a key requires updates in three places: the fixture, this
    /// table, and the Dart `logicalKeyName` — that's the price of the
    /// parity guarantee. Drift on any side breaks one of the two tests.
    #[test]
    fn supported_keys_match_fixture() {
        use rdev::Key;
        use std::collections::BTreeSet;

        let table: &[(&str, Key)] = &[
            ("a", Key::KeyA), ("b", Key::KeyB), ("c", Key::KeyC),
            ("d", Key::KeyD), ("e", Key::KeyE), ("f", Key::KeyF),
            ("g", Key::KeyG), ("h", Key::KeyH), ("i", Key::KeyI),
            ("j", Key::KeyJ), ("k", Key::KeyK), ("l", Key::KeyL),
            ("m", Key::KeyM), ("n", Key::KeyN), ("o", Key::KeyO),
            ("p", Key::KeyP), ("q", Key::KeyQ), ("r", Key::KeyR),
            ("s", Key::KeyS), ("t", Key::KeyT), ("u", Key::KeyU),
            ("v", Key::KeyV), ("w", Key::KeyW), ("x", Key::KeyX),
            ("y", Key::KeyY), ("z", Key::KeyZ),
            ("digit0", Key::Num0), ("digit1", Key::Num1),
            ("digit2", Key::Num2), ("digit3", Key::Num3),
            ("digit4", Key::Num4), ("digit5", Key::Num5),
            ("digit6", Key::Num6), ("digit7", Key::Num7),
            ("digit8", Key::Num8), ("digit9", Key::Num9),
            ("f1", Key::F1), ("f2", Key::F2), ("f3", Key::F3),
            ("f4", Key::F4), ("f5", Key::F5), ("f6", Key::F6),
            ("f7", Key::F7), ("f8", Key::F8), ("f9", Key::F9),
            ("f10", Key::F10), ("f11", Key::F11), ("f12", Key::F12),
            ("delete", Key::Delete),
            ("backspace", Key::Backspace),
            ("tab", Key::Tab),
            ("space", Key::Space),
            ("enter", Key::Return),
            ("enter", Key::KpReturn),
            ("arrow_left", Key::LeftArrow),
            ("arrow_right", Key::RightArrow),
            ("arrow_up", Key::UpArrow),
            ("arrow_down", Key::DownArrow),
            ("home", Key::Home),
            ("end", Key::End),
            ("page_up", Key::PageUp),
            ("page_down", Key::PageDown),
            ("insert", Key::Insert),
        ];

        // Round-trip: every entry in the table must map through
        // event_to_key_name to its declared name.
        for (name, key) in table {
            assert_eq!(
                event_to_key_name(&make_press(*key)).as_deref(),
                Some(*name),
                "rdev::Key::{:?} should map to {:?}",
                key, name,
            );
        }

        // The set of names produced by the table must equal the fixture.
        let actual: BTreeSet<&str> = table.iter().map(|(n, _)| *n).collect();
        let fixture_raw: Vec<String> = serde_json::from_str(include_str!(
            "../../flutter/test/fixtures/supported_shortcut_keys.json"
        ))
        .expect("fixture is valid JSON");
        let expected: BTreeSet<&str> =
            fixture_raw.iter().map(String::as_str).collect();
        assert_eq!(
            actual, expected,
            "event_to_key_name vocabulary drifted from \
             flutter/test/fixtures/supported_shortcut_keys.json — update \
             shortcuts.rs, the fixture, and Dart logicalKeyName together"
        );
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
