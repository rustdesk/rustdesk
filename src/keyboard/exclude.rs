//! Per-peer keyboard shortcut exclude list.
//!
//! Peer option `KEYBOARD_EXCLUDE_SHORTCUTS_OPTION` holds a comma-separated list of chords (`Ctrl+Shift`, `Win+D`,
//! `Alt+Tab`, ...). While a session window grabs the keyboard, every key press goes to the remote PC; a chord on the
//! focused session's list goes to the local PC instead:
//!
//! - A chord with a main key (`Win+D`) is diverted on the press of the main key: the held modifiers are released on
//!   the remote PC, and the held modifiers plus the main key are re-injected locally with `rdev::simulate_code()`.
//!   Every press until all modifiers are released stays local.
//! - A modifier-only chord (`Alt+Shift`, `Win`) is diverted on the first modifier release, when no other key was pressed
//!   since the first modifier went down: the modifiers are pressed locally and the release is re-injected, so the
//!   local system sees the complete chord; the remote PC gets `MASK_KEY` before the releases, so its own action for the
//!   chord (input language switch, Start menu) does not fire.

use super::client;
use rdev::{Event, EventType, Key};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::SystemTime,
};

pub const KEYBOARD_EXCLUDE_SHORTCUTS_OPTION: &str = "keyboard-exclude-shortcuts";

const MOD_CTRL: u8 = 1;
const MOD_ALT: u8 = 2;
const MOD_SHIFT: u8 = 4;
const MOD_WIN: u8 = 8;

/// `F24`: VK `0x87`, set-1 scan code `0x76`.
const MASK_KEY: Key = Key::F24;
const MASK_VK: u32 = 0x87;
const MASK_SCAN: u32 = 0x76;

/// Scan code of the fake left `Ctrl` that Windows sends with `AltGr`.
const FAKE_CTRL_SCAN: u32 = 0x021D;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Chord {
    mods: u8,
    key: Option<Key>,
}

/// Friendly name ⇄ `rdev::Key`. The first name of a key is its canonical name.
const KEY_NAMES: &[(&str, Key)] = &[
    ("A", Key::KeyA), ("B", Key::KeyB), ("C", Key::KeyC), ("D", Key::KeyD), ("E", Key::KeyE),
    ("F", Key::KeyF), ("G", Key::KeyG), ("H", Key::KeyH), ("I", Key::KeyI), ("J", Key::KeyJ),
    ("K", Key::KeyK), ("L", Key::KeyL), ("M", Key::KeyM), ("N", Key::KeyN), ("O", Key::KeyO),
    ("P", Key::KeyP), ("Q", Key::KeyQ), ("R", Key::KeyR), ("S", Key::KeyS), ("T", Key::KeyT),
    ("U", Key::KeyU), ("V", Key::KeyV), ("W", Key::KeyW), ("X", Key::KeyX), ("Y", Key::KeyY),
    ("Z", Key::KeyZ),
    ("0", Key::Num0), ("1", Key::Num1), ("2", Key::Num2), ("3", Key::Num3), ("4", Key::Num4),
    ("5", Key::Num5), ("6", Key::Num6), ("7", Key::Num7), ("8", Key::Num8), ("9", Key::Num9),
    ("F1", Key::F1), ("F2", Key::F2), ("F3", Key::F3), ("F4", Key::F4), ("F5", Key::F5),
    ("F6", Key::F6), ("F7", Key::F7), ("F8", Key::F8), ("F9", Key::F9), ("F10", Key::F10),
    ("F11", Key::F11), ("F12", Key::F12), ("F13", Key::F13), ("F14", Key::F14), ("F15", Key::F15),
    ("F16", Key::F16), ("F17", Key::F17), ("F18", Key::F18), ("F19", Key::F19), ("F20", Key::F20),
    ("F21", Key::F21), ("F22", Key::F22), ("F23", Key::F23),
    ("Esc", Key::Escape), ("Escape", Key::Escape),
    ("Tab", Key::Tab),
    ("Space", Key::Space),
    ("Enter", Key::Return), ("Return", Key::Return),
    ("Backspace", Key::Backspace),
    ("Delete", Key::Delete), ("Del", Key::Delete),
    ("Insert", Key::Insert), ("Ins", Key::Insert),
    ("Home", Key::Home), ("End", Key::End),
    ("PageUp", Key::PageUp), ("PgUp", Key::PageUp),
    ("PageDown", Key::PageDown), ("PgDn", Key::PageDown),
    ("Left", Key::LeftArrow), ("Right", Key::RightArrow), ("Up", Key::UpArrow), ("Down", Key::DownArrow),
    ("PrintScreen", Key::PrintScreen), ("PrtSc", Key::PrintScreen),
    ("Pause", Key::Pause), ("ScrollLock", Key::ScrollLock),
    ("Apps", Key::Apps), ("Menu", Key::Apps),
    ("`", Key::BackQuote), ("-", Key::Minus), ("=", Key::Equal),
    ("[", Key::LeftBracket), ("]", Key::RightBracket), ("\\", Key::BackSlash),
    (";", Key::SemiColon), ("'", Key::Quote), (",", Key::Comma), (".", Key::Dot), ("/", Key::Slash),
    ("Num0", Key::Kp0), ("Num1", Key::Kp1), ("Num2", Key::Kp2), ("Num3", Key::Kp3), ("Num4", Key::Kp4),
    ("Num5", Key::Kp5), ("Num6", Key::Kp6), ("Num7", Key::Kp7), ("Num8", Key::Kp8), ("Num9", Key::Kp9),
    ("Num+", Key::KpPlus), ("Num-", Key::KpMinus), ("Num*", Key::KpMultiply), ("Num/", Key::KpDivide),
    ("NumEnter", Key::KpReturn), ("NumDel", Key::KpDecimal),
    ("VolumeUp", Key::VolumeUp), ("VolumeDown", Key::VolumeDown), ("VolumeMute", Key::VolumeMute),
];

fn mod_bit(key: Key) -> u8 {
    match key {
        Key::ControlLeft | Key::ControlRight => MOD_CTRL,
        Key::Alt | Key::AltGr => MOD_ALT,
        Key::ShiftLeft | Key::ShiftRight => MOD_SHIFT,
        Key::MetaLeft | Key::MetaRight => MOD_WIN,
        _ => 0,
    }
}

impl Chord {
    /// Parses `Ctrl+Alt+Shift+Win+<key>` (any order, case-insensitive); returns `None` for an unknown token, a second
    /// main key or an empty chord.
    pub fn parse(s: &str) -> Option<Chord> {
        let mut mods = 0u8;
        let mut key = None;
        // `+` as a key name: "Ctrl++" splits into ["Ctrl", "", ""].
        let tokens: Vec<&str> = s.split('+').map(|t| t.trim()).collect();
        let mut i = 0;
        while i < tokens.len() {
            let t = tokens[i];
            let lower = t.to_lowercase();
            let bit = match lower.as_str() {
                "ctrl" | "control" => MOD_CTRL,
                "alt" => MOD_ALT,
                "shift" => MOD_SHIFT,
                "win" | "windows" | "meta" | "super" | "cmd" => MOD_WIN,
                _ => 0,
            };
            if bit != 0 {
                mods |= bit;
            } else if t.is_empty() {
                // A trailing "+" key.
                if i + 2 == tokens.len() && tokens[i + 1].is_empty() {
                    if key.is_some() {
                        return None;
                    }
                    key = Some(Key::Equal);
                    i += 1;
                } else {
                    return None;
                }
            } else {
                let k = KEY_NAMES
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(t))
                    .map(|(_, k)| *k)?;
                if key.is_some() {
                    return None;
                }
                key = Some(k);
            }
            i += 1;
        }
        if mods == 0 && key.is_none() {
            return None;
        }
        Some(Chord { mods, key })
    }

    /// Canonical `Ctrl+Alt+Shift+Win+<key>` form.
    pub fn to_canonical(&self) -> String {
        let mut parts: Vec<&str> = vec![];
        if self.mods & MOD_CTRL != 0 {
            parts.push("Ctrl");
        }
        if self.mods & MOD_ALT != 0 {
            parts.push("Alt");
        }
        if self.mods & MOD_SHIFT != 0 {
            parts.push("Shift");
        }
        if self.mods & MOD_WIN != 0 {
            parts.push("Win");
        }
        if let Some(k) = self.key {
            if let Some((n, _)) = KEY_NAMES.iter().find(|(_, kk)| *kk == k) {
                parts.push(n);
            }
        }
        parts.join("+")
    }
}

/// Returns the canonical form of chord `s`, or "" when `s` is not a valid chord.
pub fn normalize_chord(s: &str) -> String {
    Chord::parse(s).map(|c| c.to_canonical()).unwrap_or_default()
}

/// Parses a comma-separated chord list; invalid entries are dropped, duplicates are kept once.
pub fn parse_list(s: &str) -> Vec<Chord> {
    let mut res: Vec<Chord> = vec![];
    for c in s.split(',').filter_map(|t| Chord::parse(t)) {
        if !res.contains(&c) {
            res.push(c);
        }
    }
    res
}

fn list_to_string(list: &[Chord]) -> String {
    list.iter()
        .map(|c| c.to_canonical())
        .collect::<Vec<_>>()
        .join(",")
}

lazy_static::lazy_static! {
    /// Peer id → exclude list. Filled from the peer option on first use and by `set_exclude_list()`.
    static ref EXCLUDE_LISTS: Mutex<HashMap<String, Vec<Chord>>> = Mutex::new(HashMap::new());
    static ref STATE: Mutex<HookState> = Mutex::new(HookState::default());
    static ref RECORDED: Mutex<String> = Mutex::new(String::new());
    static ref RECORDING_SINCE: Mutex<Option<std::time::Instant>> = Mutex::new(None);
}

/// Stores `value` as the exclude list of the peer of `session` (peer option and cache) and returns the canonical form.
#[cfg(feature = "flutter")]
pub fn set_exclude_list(session: &crate::flutter::FlutterSession, value: &str) -> String {
    let list = parse_list(value);
    let canonical = list_to_string(&list);
    session.set_option(
        KEYBOARD_EXCLUDE_SHORTCUTS_OPTION.to_owned(),
        canonical.clone(),
    );
    let peer_id = session.lc.read().unwrap().get_id().to_owned();
    EXCLUDE_LISTS.lock().unwrap().insert(peer_id, list);
    canonical
}

#[cfg(feature = "flutter")]
fn cur_exclude_list() -> Vec<Chord> {
    let Some(session) = crate::flutter::get_cur_session() else {
        return vec![];
    };
    if !session.is_default() {
        return vec![];
    }
    let peer_id = session.lc.read().unwrap().get_id().to_owned();
    let mut lists = EXCLUDE_LISTS.lock().unwrap();
    if let Some(list) = lists.get(&peer_id) {
        return list.clone();
    }
    let list = parse_list(&session.get_option(KEYBOARD_EXCLUDE_SHORTCUTS_OPTION.to_owned()));
    lists.insert(peer_id, list.clone());
    list
}

#[derive(Default)]
struct HookState {
    /// Physically held modifiers, with their press event.
    held: HashMap<Key, Event>,
    /// Modifiers pressed since all modifiers were up.
    seq_mods: u8,
    /// A non-modifier key was pressed since all modifiers were up.
    seq_other: bool,
    /// A modifier was released since all modifiers were up.
    seq_released: bool,
    /// A chord is diverted to the local PC until all modifiers are released.
    diverted: bool,
    /// Events re-injected by this module, expected back in the hook: `(position_code, is_press, injected at)`.
    expected: VecDeque<(u32, bool, std::time::Instant)>,
    /// Scan codes of presses swallowed while recording; their releases are swallowed too.
    recorded_presses: std::collections::HashSet<u32>,
}

/// While true, the keyboard hook records the next chord into `RECORDED` and swallows every key event.
static RECORDING: AtomicBool = AtomicBool::new(false);

/// Recording MUST end by itself after this long, whatever the UI does: while it lasts, no key reaches any window.
const RECORDING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// An injected event not seen back in the hook within this time is dropped from `HookState::expected`.
const EXPECTED_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

/// Starts recording a chord; `take_recorded_chord()` returns it once pressed.
pub fn start_recording() {
    RECORDED.lock().unwrap().clear();
    *RECORDING_SINCE.lock().unwrap() = Some(std::time::Instant::now());
    RECORDING.store(true, Ordering::SeqCst);
    hbb_common::log::info!("keyboard exclude: recording started");
}

/// Whether recording is on; ends it once `RECORDING_TIMEOUT` has passed.
fn recording_active() -> bool {
    if !RECORDING.load(Ordering::SeqCst) {
        return false;
    }
    let expired = RECORDING_SINCE
        .lock()
        .unwrap()
        .map_or(true, |t| t.elapsed() >= RECORDING_TIMEOUT);
    if expired {
        RECORDING.store(false, Ordering::SeqCst);
        hbb_common::log::warn!("keyboard exclude: recording timed out");
        return false;
    }
    true
}

pub fn stop_recording() {
    RECORDING.store(false, Ordering::SeqCst);
}

/// The recorded chord in canonical form, or "" while none was recorded; clears it.
pub fn take_recorded_chord() -> String {
    std::mem::take(&mut *RECORDED.lock().unwrap())
}

impl HookState {
    fn held_bits(&self) -> u8 {
        self.held.keys().fold(0, |b, k| b | mod_bit(*k))
    }
}

fn make_event(key: Key, platform_code: u32, position_code: u32, is_press: bool) -> Event {
    Event {
        time: SystemTime::now(),
        unicode: None,
        event_type: if is_press {
            EventType::KeyPress(key)
        } else {
            EventType::KeyRelease(key)
        },
        platform_code,
        position_code,
        usb_hid: 0,
        #[cfg(target_os = "windows")]
        extra_data: 0,
        #[cfg(target_os = "macos")]
        extra_data: 0,
    }
}

/// Whether the system reports virtual key `vk` down (`GetAsyncKeyState`).
#[cfg(target_os = "windows")]
fn is_key_down(vk: u32) -> bool {
    unsafe { (winapi::um::winuser::GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

fn send_remote(mode: &str, event: &Event) {
    client::process_event(mode, event, None);
}

fn send_remote_mask(mode: &str) {
    send_remote(mode, &make_event(MASK_KEY, MASK_VK, MASK_SCAN, true));
    send_remote(mode, &make_event(MASK_KEY, MASK_VK, MASK_SCAN, false));
}

/// Releases every held modifier on the remote PC, masked when `Alt` or `Win` is among them.
fn release_remote_mods(mode: &str, state: &HookState) {
    release_remote_held(mode, &state.held.values().cloned().collect::<Vec<_>>());
}

/// Releases the modifiers of `held` (their press events) on the remote PC, masked when `Alt` or `Win` is among them.
fn release_remote_held(mode: &str, held: &[Event]) {
    let bits = held.iter().fold(0u8, |b, e| match e.event_type {
        EventType::KeyPress(k) => b | mod_bit(k),
        _ => b,
    });
    if bits & (MOD_ALT | MOD_WIN) != 0 {
        send_remote_mask(mode);
    }
    for ev in held {
        let mut ev = ev.clone();
        if let EventType::KeyPress(k) = ev.event_type {
            ev.event_type = EventType::KeyRelease(k);
        }
        send_remote(mode, &ev);
    }
}

#[cfg(target_os = "windows")]
fn inject_local(state: &mut HookState, position_code: u32, is_press: bool) {
    state
        .expected
        .push_back((position_code, is_press, std::time::Instant::now()));
    // Windows can deliver the injected event to this hook before `SendInput` returns (a nested call on this thread):
    // `on_hook_event()` then passes it to the local PC through `INJECTING`, and it is not expected any more.
    INJECTING.with(|i| i.set(true));
    REENTERED.with(|r| r.set(0));
    let res = rdev::simulate_code(None, Some(position_code), is_press);
    INJECTING.with(|i| i.set(false));
    if res.is_err() || REENTERED.with(|r| r.get()) > 0 {
        state.expected.pop_back();
    }
    if res.is_err() {
        hbb_common::log::error!(
            "keyboard exclude: failed to inject scan code 0x{:x}",
            position_code
        );
    }
}

thread_local! {
    /// True while `inject_local()` is inside `SendInput`.
    static INJECTING: std::cell::Cell<bool> = std::cell::Cell::new(false);
    /// Nested hook calls during the current `inject_local()`.
    static REENTERED: std::cell::Cell<u32> = std::cell::Cell::new(0);
}

/// Whether `event` is a `VK_PACKET` event: a `SendInput` event with `KEYEVENTF_UNICODE` (AutoHotkey types characters
/// missing from the current layout this way), whose scan code field holds a UTF-16 code unit, not a key position.
/// rdev's `get_code()` replaces the virtual key `VK_PACKET` with that scan code field, so the event carries the code
/// unit in both `platform_code` and `position_code`; for a key, the virtual key and the scan code differ (letters
/// 0x41-0x5A / 0x10-0x32, digits 0x30-0x39 / 0x02-0x0B, extended keys carry the 0xE0 prefix in `position_code`).
fn is_packet_event(event: &Event) -> bool {
    event.platform_code == event.position_code && event.platform_code != 0
}

thread_local! {
    /// High surrogate of a character split over two `VK_PACKET` events.
    static PENDING_HIGH_SURROGATE: std::cell::Cell<Option<u16>> = std::cell::Cell::new(None);
}

/// Sends the character of a grabbed `VK_PACKET` key-down to the remote PC as text (`KeyEvent.seq`, legacy mode,
/// which the controlled side types with `process_seq()` whatever the session's keyboard mode); swallows the event
/// locally. Key-ups carry the same character and are only swallowed.
fn send_packet_char(event: &Event, is_press: bool) -> Option<Event> {
    if !is_press {
        return None;
    }
    let unit = (event.position_code & 0xFFFF) as u16;
    let text = if (0xD800..0xDC00).contains(&unit) {
        PENDING_HIGH_SURROGATE.with(|p| p.set(Some(unit)));
        return None;
    } else if (0xDC00..0xE000).contains(&unit) {
        match PENDING_HIGH_SURROGATE.with(|p| p.take()) {
            Some(high) => String::from_utf16_lossy(&[high, unit]),
            None => return None,
        }
    } else {
        String::from_utf16_lossy(&[unit])
    };
    let mut key_event = base::message_proto::KeyEvent::new();
    key_event.set_seq(text);
    key_event.down = true;
    key_event.press = true;
    key_event.mode = base::message_proto::KeyboardMode::Legacy.into();
    hbb_common::log::info!("[kbd] unicode packet {:?} sent as text", key_event.seq());
    super::send_key_event(&key_event);
    None
}

/// Whether some program on this PC has registered `mods` + virtual key `vk` as a global hotkey (`RegisterHotKey`
/// fails with `ERROR_HOTKEY_ALREADY_REGISTERED`). Such a hotkey fires only when the keys reach this PC's input
/// system, which the grabbed session prevents; AutoHotkey registers its plain hotkeys this way.
#[cfg(target_os = "windows")]
fn registered_hotkey(mods: u8, vk: u32) -> bool {
    use winapi::um::winuser::{RegisterHotKey, UnregisterHotKey};
    const ID: i32 = 0xBFF0;
    let mut m: u32 = 0x4000; // MOD_NOREPEAT
    if mods & MOD_ALT != 0 {
        m |= 0x0001;
    }
    if mods & MOD_CTRL != 0 {
        m |= 0x0002;
    }
    if mods & MOD_SHIFT != 0 {
        m |= 0x0004;
    }
    if mods & MOD_WIN != 0 {
        m |= 0x0008;
    }
    unsafe {
        if RegisterHotKey(std::ptr::null_mut(), ID, m, vk) != 0 {
            UnregisterHotKey(std::ptr::null_mut(), ID);
            false
        } else {
            std::io::Error::last_os_error().raw_os_error() == Some(1409)
        }
    }
}

/// Decides a keyboard hook event. Returns `Some(res)` when this module handled the event (`res` is the hook result:
/// `None` swallows it locally), `None` to leave it to the default path.
#[cfg(all(feature = "flutter", target_os = "windows"))]
pub fn on_hook_event(event: &Event, key: Key, is_press: bool, hooked: bool) -> Option<Option<Event>> {
    // While this thread waits in `CallNextHookEx` (`probe_next_hooks()`), Windows can deliver another keyboard event to
    // it (e.g. one a later hook injects): that nested call MUST take the default path and MUST NOT wait for `STATE`,
    // which would hang the hook thread for good (Windows then stops calling the hook: no key reaches the session).
    if INJECTING.with(|i| i.get()) {
        REENTERED.with(|r| r.set(r.get() + 1));
        return Some(Some(event.clone()));
    }
    if IN_PROBE.with(|p| p.get()) {
        return None;
    }
    if hooked && is_packet_event(event) {
        return Some(send_packet_char(event, is_press));
    }
    let Ok(mut state) = STATE.try_lock() else {
        hbb_common::log::warn!("keyboard exclude: hook state busy, default path");
        return None;
    };
    while state
        .expected
        .front()
        .is_some_and(|(_, _, t)| t.elapsed() >= EXPECTED_TIMEOUT)
    {
        let (pos, press, _) = state.expected.pop_front().unwrap();
        hbb_common::log::warn!(
            "keyboard exclude: injected event 0x{:x} press={} never came back",
            pos,
            press
        );
    }
    let front = state.expected.front().copied();
    if let Some((pos, press, _)) = front {
        if pos == event.position_code && press == is_press {
            state.expected.pop_front();
            return Some(Some(event.clone()));
        }
    }

    let bit = if event.position_code == FAKE_CTRL_SCAN {
        0
    } else {
        mod_bit(key)
    };
    let held_bits_before = state.held_bits();
    if is_press {
        if bit != 0 {
            if !state.held.contains_key(&key) {
                state.held.insert(key, event.clone());
            }
            state.seq_mods |= bit;
        } else {
            state.seq_other = true;
        }
    }
    let first_release = !is_press && bit != 0 && !state.seq_released;
    if !is_press && bit != 0 {
        state.held.remove(&key);
        state.seq_released = true;
    }
    let reset = |state: &mut HookState| {
        if state.held.is_empty() {
            state.seq_mods = 0;
            state.seq_other = false;
            state.seq_released = false;
            state.diverted = false;
        }
    };

    // Recording: every press is swallowed (so no other keyboard hook or window sees it), and so is the release of
    // every press swallowed here.
    let recording = recording_active();
    // After recording, the release of a press swallowed while recording is handled like any release (default path:
    // sent to the remote PC when the keyboard is grabbed, passed locally): a swallowed release could leave a key down.
    if !recording && !is_press {
        state.recorded_presses.remove(&event.position_code);
    }
    if recording {
        if !is_press {
            state.recorded_presses.remove(&event.position_code);
        }
        // The fake left `Ctrl` of `AltGr` (`bit == 0`, `mod_bit(key) != 0`) and keys without a name in `KEY_NAMES`
        // complete no chord.
        let chord = if is_press
            && mod_bit(key) == 0
            && KEY_NAMES.iter().any(|(_, k)| *k == key)
        {
            Some(Chord {
                mods: held_bits_before,
                key: Some(key),
            })
        } else if first_release && !state.seq_other && held_bits_before == state.seq_mods {
            Some(Chord {
                mods: state.seq_mods,
                key: None,
            })
        } else {
            None
        };
        if is_press {
            state.recorded_presses.insert(event.position_code);
        }
        if let Some(chord) = chord {
            *RECORDED.lock().unwrap() = chord.to_canonical();
            RECORDING.store(false, Ordering::SeqCst);
            hbb_common::log::info!("keyboard exclude: recorded {}", chord.to_canonical());
        }
        reset(&mut *state);
        return Some(None);
    }

    if !hooked {
        reset(&mut *state);
        return None;
    }

    // A diverted chord's modifiers were injected locally, so the system key state reports them down exactly while
    // they are physically held. A `held` entry the system reports up (its release was never seen here, e.g. another
    // hook swallowed it) is dropped; with none left, the divert ends and this event is processed normally.
    if state.diverted && is_press {
        state.held.retain(|_, e| is_key_down(e.platform_code));
        reset(&mut *state);
        if !state.diverted {
            hbb_common::log::info!("keyboard exclude: divert ended, no diverted modifier is down");
        }
    }
    if state.diverted {
        // Keys another program injects while the chord is held (a hotkey typing its text) go to the remote PC.
        if is_press && event.extra_data != 0 {
            return None;
        }
        if is_press {
            return Some(Some(event.clone()));
        }
        reset(&mut *state);
        if !state.diverted {
            hbb_common::log::info!("keyboard exclude: divert ended");
        }
        return None;
    }

    // Main-key chord: decided on the press of a non-modifier key.
    if is_press && bit == 0 {
        let chord = Chord {
            mods: held_bits_before,
            key: Some(key),
        };
        let hotkey = held_bits_before & (MOD_CTRL | MOD_ALT) != 0
            && held_bits_before & MOD_WIN == 0
            && registered_hotkey(held_bits_before, event.platform_code);
        if hotkey {
            hbb_common::log::info!(
                "keyboard exclude: {} is a registered hotkey on this PC",
                chord.to_canonical()
            );
        }
        if !hotkey && !cur_exclude_list().contains(&chord) {
            let probe = ProbeInput::from_state(&state);
            drop(state);
            return probe_next_hooks(probe, event, true);
        }
        let mode = super::get_keyboard_mode();
        release_remote_mods(&mode, &state);
        let held: Vec<u32> = state.held.values().map(|e| e.position_code).collect();
        for pos in held {
            inject_local(&mut state, pos, true);
        }
        inject_local(&mut state, event.position_code, true);
        state.diverted = true;
        hbb_common::log::info!("keyboard exclude: diverted {}", chord.to_canonical());
        return Some(None);
    }

    // Modifier-only chord: decided on the first modifier release of a sequence without another key.
    if first_release && !state.seq_other && held_bits_before == state.seq_mods {
        let chord = Chord {
            mods: state.seq_mods,
            key: None,
        };
        if !cur_exclude_list().contains(&chord) {
            reset(&mut *state);
            return None;
        }
        let mode = super::get_keyboard_mode();
        send_remote_mask(&mode);
        send_remote(&mode, event);
        let held: Vec<u32> = state.held.values().map(|e| e.position_code).collect();
        for pos in held {
            inject_local(&mut state, pos, true);
        }
        inject_local(&mut state, event.position_code, true);
        inject_local(&mut state, event.position_code, false);
        state.diverted = true;
        hbb_common::log::info!("keyboard exclude: diverted {}", chord.to_canonical());
        reset(&mut *state);
        return Some(None);
    }

    reset(&mut *state);
    if is_press {
        // A modifier press: the hooks behind this one track modifier state from it.
        let probe = ProbeInput::from_state(&state);
        drop(state);
        return probe_next_hooks(probe, event, false);
    }
    None
}

thread_local! {
    /// True while this thread is inside `CallNextHookEx` in `probe_next_hooks()`.
    static IN_PROBE: std::cell::Cell<bool> = std::cell::Cell::new(false);
}

/// What `probe_next_hooks()` needs from `HookState`, taken before the `STATE` guard is dropped.
struct ProbeInput {
    held: Vec<Event>,
    alt: bool,
    ctrl: bool,
}

impl ProbeInput {
    fn from_state(state: &HookState) -> Self {
        ProbeInput {
            held: state.held.values().cloned().collect(),
            alt: state.held.keys().any(|k| matches!(k, Key::Alt | Key::AltGr)),
            ctrl: state
                .held
                .keys()
                .any(|k| matches!(k, Key::ControlLeft | Key::ControlRight)),
        }
    }
}

/// Hands a press that the grabbed session would take to the keyboard hooks installed before this one
/// (`CallNextHookEx`, e.g. AutoHotkey), before deciding.
///
/// - A hook swallowed it (a hotkey fired): the press MUST NOT reach the remote PC; for a non-modifier key the
///   modifiers held for the hotkey are released there (masked), so the text the hotkey types arrives unmodified. The
///   hotkey's own output is injected input, which passes this hook like any key and reaches the remote PC.
/// - No hook swallowed it: the default path (sent to the remote PC, swallowed locally).
///
/// The press is handed on here, inside this hook procedure, with the `KBDLLHOOKSTRUCT` rebuilt from `event`; the
/// return value MUST then be `Some(None)` or the default path's `None` for a press, both of which make rdev return
/// without calling the next hook a second time.
#[cfg(target_os = "windows")]
fn probe_next_hooks(probe: ProbeInput, event: &Event, is_main_key: bool) -> Option<Option<Event>> {
    use winapi::um::winuser::{CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN};
    let alt = probe.alt;
    let ctrl = probe.ctrl;
    let pos = event.position_code;
    let mut flags = 0u32;
    if (pos >> 8) == 0xE0 {
        flags |= 0x01; // LLKHF_EXTENDED
    }
    if alt {
        flags |= 0x20; // LLKHF_ALTDOWN
    }
    let mut data = KBDLLHOOKSTRUCT {
        vkCode: event.platform_code,
        scanCode: pos & 0xFF,
        flags,
        time: 0,
        dwExtraInfo: event.extra_data,
    };
    let msg = if alt && !ctrl { WM_SYSKEYDOWN } else { WM_KEYDOWN };
    IN_PROBE.with(|p| p.set(true));
    let suppressed = unsafe {
        CallNextHookEx(
            std::ptr::null_mut(),
            HC_ACTION,
            msg as usize,
            &mut data as *mut KBDLLHOOKSTRUCT as isize,
        )
    } != 0;
    IN_PROBE.with(|p| p.set(false));
    if !suppressed {
        return None;
    }
    hbb_common::log::info!(
        "[kbd] vk=0x{:x} pos=0x{:x}: swallowed by a later keyboard hook (hotkey), not sent",
        event.platform_code,
        pos
    );
    if is_main_key {
        release_remote_held(&super::get_keyboard_mode(), &probe.held);
    }
    Some(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_normalize() {
        assert_eq!(normalize_chord("win+d"), "Win+D");
        assert_eq!(normalize_chord("Shift + Alt"), "Alt+Shift");
        assert_eq!(normalize_chord("ctrl+shift+esc"), "Ctrl+Shift+Esc");
        assert_eq!(normalize_chord("Win"), "Win");
        assert_eq!(normalize_chord("Ctrl++"), "Ctrl+=");
        assert_eq!(normalize_chord("Ctrl+Q+W"), "");
        assert_eq!(normalize_chord("Foo"), "");
        assert_eq!(normalize_chord(""), "");
        assert_eq!(
            list_to_string(&parse_list("Win+D, alt+shift,Win+D,bogus")),
            "Win+D,Alt+Shift"
        );
    }
}
