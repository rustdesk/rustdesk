#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::client::get_key_state;
use crate::common::GrabState;
#[cfg(feature = "flutter")]
use crate::flutter::{CUR_SESSION_ID, SESSIONS};
#[cfg(not(any(feature = "flutter", feature = "cli")))]
use crate::ui::CUR_SESSION;
use hbb_common::{log, message_proto::*};
use rdev::{Event, EventType, Key};
#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[cfg(windows)]
static mut IS_ALT_GR: bool = false;

#[cfg(any(target_os = "windows", target_os = "macos"))]
static KEYBOARD_HOOKED: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    static ref TO_RELEASE: Arc<Mutex<HashSet<Key>>> = Arc::new(Mutex::new(HashSet::<Key>::new()));
    static ref MODIFIERS_STATE: Mutex<HashMap<Key, bool>> = {
        let mut m = HashMap::new();
        m.insert(Key::ShiftLeft, false);
        m.insert(Key::ShiftRight, false);
        m.insert(Key::ControlLeft, false);
        m.insert(Key::ControlRight, false);
        m.insert(Key::Alt, false);
        m.insert(Key::AltGr, false);
        m.insert(Key::MetaLeft, false);
        m.insert(Key::MetaRight, false);
        Mutex::new(m)
    };
}

pub mod client {
    use super::*;

    pub fn get_keyboard_mode() -> String {
        #[cfg(not(any(feature = "flutter", feature = "cli")))]
        if let Some(session) = CUR_SESSION.lock().unwrap().as_ref() {
            return session.get_keyboard_mode();
        }
        #[cfg(feature = "flutter")]
        if let Some(session) = SESSIONS
            .read()
            .unwrap()
            .get(&*CUR_SESSION_ID.read().unwrap())
        {
            return session.get_keyboard_mode();
        }
        "legacy".to_string()
    }

    pub fn start_grab_loop() {
        super::start_grab_loop();
    }

    pub fn change_grab_status(state: GrabState) {
        match state {
            GrabState::Ready => {}
            GrabState::Run => {
                #[cfg(windows)]
                update_grab_get_key_name();
                #[cfg(any(target_os = "windows", target_os = "macos"))]
                KEYBOARD_HOOKED.swap(true, Ordering::SeqCst);

                #[cfg(target_os = "linux")]
                rdev::enable_grab();
            }
            GrabState::Wait => {
                release_remote_keys();

                #[cfg(any(target_os = "windows", target_os = "macos"))]
                KEYBOARD_HOOKED.swap(false, Ordering::SeqCst);

                #[cfg(target_os = "linux")]
                rdev::disable_grab();
            }
            GrabState::Exit => {
                #[cfg(target_os = "linux")]
                rdev::exit_grab_listen();
            }
        }
    }

    pub fn process_event(event: &Event, lock_modes: Option<i32>) -> KeyboardMode {
        let keyboard_mode = get_keyboard_mode_enum();

        if is_long_press(&event) {
            return keyboard_mode;
        }

        for key_event in event_to_key_events(&event, keyboard_mode, lock_modes) {
            send_key_event(&key_event);
        }
        keyboard_mode
    }

    pub fn get_modifiers_state(
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) -> (bool, bool, bool, bool) {
        let modifiers_lock = MODIFIERS_STATE.lock().unwrap();
        let ctrl = *modifiers_lock.get(&Key::ControlLeft).unwrap()
            || *modifiers_lock.get(&Key::ControlRight).unwrap()
            || ctrl;
        let shift = *modifiers_lock.get(&Key::ShiftLeft).unwrap()
            || *modifiers_lock.get(&Key::ShiftRight).unwrap()
            || shift;
        let command = *modifiers_lock.get(&Key::MetaLeft).unwrap()
            || *modifiers_lock.get(&Key::MetaRight).unwrap()
            || command;
        let alt = *modifiers_lock.get(&Key::Alt).unwrap()
            || *modifiers_lock.get(&Key::AltGr).unwrap()
            || alt;

        (alt, ctrl, shift, command)
    }

    pub fn legacy_modifiers(
        key_event: &mut KeyEvent,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        if alt
            && !crate::is_control_key(&key_event, &ControlKey::Alt)
            && !crate::is_control_key(&key_event, &ControlKey::RAlt)
        {
            key_event.modifiers.push(ControlKey::Alt.into());
        }
        if shift
            && !crate::is_control_key(&key_event, &ControlKey::Shift)
            && !crate::is_control_key(&key_event, &ControlKey::RShift)
        {
            key_event.modifiers.push(ControlKey::Shift.into());
        }
        if ctrl
            && !crate::is_control_key(&key_event, &ControlKey::Control)
            && !crate::is_control_key(&key_event, &ControlKey::RControl)
        {
            key_event.modifiers.push(ControlKey::Control.into());
        }
        if command
            && !crate::is_control_key(&key_event, &ControlKey::Meta)
            && !crate::is_control_key(&key_event, &ControlKey::RWin)
        {
            key_event.modifiers.push(ControlKey::Meta.into());
        }
    }

    pub fn event_lock_screen() -> KeyEvent {
        let mut key_event = KeyEvent::new();
        key_event.set_control_key(ControlKey::LockScreen);
        key_event.down = true;
        key_event.mode = KeyboardMode::Legacy.into();
        key_event
    }

    #[inline]
    pub fn lock_screen() {
        send_key_event(&event_lock_screen());
    }

    pub fn event_ctrl_alt_del() -> KeyEvent {
        let mut key_event = KeyEvent::new();
        if get_peer_platform() == "Windows" {
            key_event.set_control_key(ControlKey::CtrlAltDel);
            key_event.down = true;
        } else {
            key_event.set_control_key(ControlKey::Delete);
            legacy_modifiers(&mut key_event, true, true, false, false);
            key_event.press = true;
        }
        key_event.mode = KeyboardMode::Legacy.into();
        key_event
    }

    #[inline]
    pub fn ctrl_alt_del() {
        send_key_event(&event_ctrl_alt_del());
    }
}

#[cfg(windows)]
pub fn update_grab_get_key_name() {
    match get_keyboard_mode_enum() {
        KeyboardMode::Map => rdev::set_get_key_unicode(false),
        KeyboardMode::Translate => rdev::set_get_key_unicode(true),
        _ => {}
    };
}

#[cfg(target_os = "windows")]
static mut IS_0X021D_DOWN: bool = false;

pub fn start_grab_loop() {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    std::thread::spawn(move || {
        let try_handle_keyboard = move |event: Event, key: Key, is_press: bool| -> Option<Event> {
            // fix #2211：CAPS LOCK don't work
            if key == Key::CapsLock || key == Key::NumLock {
                return Some(event);
            }

            let mut _keyboard_mode = KeyboardMode::Map;
            let _scan_code = event.scan_code;
            let res = if KEYBOARD_HOOKED.load(Ordering::SeqCst) {
                _keyboard_mode = client::process_event(&event, None);
                if is_press {
                    None
                } else {
                    Some(event)
                }
            } else {
                Some(event)
            };

            #[cfg(target_os = "windows")]
            match _scan_code {
                0x1D | 0x021D => rdev::set_modifier(Key::ControlLeft, is_press),
                0xE01D => rdev::set_modifier(Key::ControlRight, is_press),
                0x2A => rdev::set_modifier(Key::ShiftLeft, is_press),
                0x36 => rdev::set_modifier(Key::ShiftRight, is_press),
                0x38 => rdev::set_modifier(Key::Alt, is_press),
                // Right Alt
                0xE038 => rdev::set_modifier(Key::AltGr, is_press),
                0xE05B => rdev::set_modifier(Key::MetaLeft, is_press),
                0xE05C => rdev::set_modifier(Key::MetaRight, is_press),
                _ => {}
            }

            #[cfg(target_os = "windows")]
            unsafe {
                // AltGr
                if _scan_code == 0x021D {
                    IS_0X021D_DOWN = is_press;
                }
            }

            return res;
        };
        let func = move |event: Event| match event.event_type {
            EventType::KeyPress(key) => try_handle_keyboard(event, key, true),
            EventType::KeyRelease(key) => try_handle_keyboard(event, key, false),
            _ => Some(event),
        };
        if let Err(error) = rdev::grab(func) {
            log::error!("rdev Error: {:?}", error)
        }
        #[cfg(target_os = "windows")]
        rdev::set_event_popup(false);
    });

    #[cfg(target_os = "linux")]
    if let Err(err) = rdev::start_grab_listen(move |event: Event| match event.event_type {
        EventType::KeyPress(key) | EventType::KeyRelease(key) => {
            if let Key::Unknown(keycode) = key {
                log::error!("rdev get unknown key, keycode is : {:?}", keycode);
            } else {
                client::process_event(&event, None);
            }
            None
        }
        _ => Some(event),
    }) {
        log::error!("Failed to init rdev grab thread: {:?}", err);
    };
}

pub fn is_long_press(event: &Event) -> bool {
    let keys = MODIFIERS_STATE.lock().unwrap();
    match event.event_type {
        EventType::KeyPress(k) => {
            if let Some(&state) = keys.get(&k) {
                if state == true {
                    return true;
                }
            }
        }
        _ => {}
    };
    return false;
}

pub fn release_remote_keys() {
    // todo!: client quit suddenly, how to release keys?
    let to_release = TO_RELEASE.lock().unwrap().clone();
    TO_RELEASE.lock().unwrap().clear();
    for key in to_release {
        let event_type = EventType::KeyRelease(key);
        let event = event_type_to_event(event_type);
        // to-do: BUG
        // Release events should be sent to the corresponding sessions, instead of current session.
        client::process_event(&event, None);
    }
}

pub fn get_keyboard_mode_enum() -> KeyboardMode {
    match client::get_keyboard_mode().as_str() {
        "map" => KeyboardMode::Map,
        "translate" => KeyboardMode::Translate,
        _ => KeyboardMode::Legacy,
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn add_numlock_capslock_with_lock_modes(key_event: &mut KeyEvent, lock_modes: i32) {
    const CAPS_LOCK: i32 = 1;
    const NUM_LOCK: i32 = 2;
    // const SCROLL_LOCK: i32 = 3;
    if lock_modes & (1 << CAPS_LOCK) != 0 {
        key_event.modifiers.push(ControlKey::CapsLock.into());
    }
    if lock_modes & (1 << NUM_LOCK) != 0 {
        key_event.modifiers.push(ControlKey::NumLock.into());
    }
    // if lock_modes & (1 << SCROLL_LOCK) != 0 {
    //     key_event.modifiers.push(ControlKey::ScrollLock.into());
    // }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn add_numlock_capslock_status(key_event: &mut KeyEvent) {
    if get_key_state(enigo::Key::CapsLock) {
        key_event.modifiers.push(ControlKey::CapsLock.into());
    }
    if get_key_state(enigo::Key::NumLock) {
        key_event.modifiers.push(ControlKey::NumLock.into());
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn convert_numpad_keys(key: Key) -> Key {
    if get_key_state(enigo::Key::NumLock) {
        return key;
    }
    match key {
        Key::Kp0 => Key::Insert,
        Key::KpDecimal => Key::Delete,
        Key::Kp1 => Key::End,
        Key::Kp2 => Key::DownArrow,
        Key::Kp3 => Key::PageDown,
        Key::Kp4 => Key::LeftArrow,
        Key::Kp5 => Key::Clear,
        Key::Kp6 => Key::RightArrow,
        Key::Kp7 => Key::Home,
        Key::Kp8 => Key::UpArrow,
        Key::Kp9 => Key::PageUp,
        _ => key,
    }
}

fn update_modifiers_state(event: &Event) {
    // for mouse
    let mut keys = MODIFIERS_STATE.lock().unwrap();
    match event.event_type {
        EventType::KeyPress(k) => {
            if keys.contains_key(&k) {
                keys.insert(k, true);
            }
        }
        EventType::KeyRelease(k) => {
            if keys.contains_key(&k) {
                keys.insert(k, false);
            }
        }
        _ => {}
    };
}

pub fn event_to_key_events(
    event: &Event,
    keyboard_mode: KeyboardMode,
    lock_modes: Option<i32>,
) -> Vec<KeyEvent> {
    let mut key_event = KeyEvent::new();
    update_modifiers_state(event);

    match event.event_type {
        EventType::KeyPress(key) => {
            TO_RELEASE.lock().unwrap().insert(key);
        }
        EventType::KeyRelease(key) => {
            TO_RELEASE.lock().unwrap().remove(&key);
        }
        _ => {}
    }

    key_event.mode = keyboard_mode.into();
    let mut key_events = match keyboard_mode {
        KeyboardMode::Map => match map_keyboard_mode(event, key_event) {
            Some(event) => [event].to_vec(),
            None => Vec::new(),
        },
        KeyboardMode::Translate => translate_keyboard_mode(event, key_event),
        _ => {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                legacy_keyboard_mode(event, key_event)
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                Vec::new()
            }
        }
    };

    if keyboard_mode != KeyboardMode::Translate {
        for key_event in &mut key_events {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            if let Some(lock_modes) = lock_modes {
                add_numlock_capslock_with_lock_modes(key_event, lock_modes);
            } else {
                add_numlock_capslock_status(key_event);
            }
        }
    }

    key_events
}

pub fn event_type_to_event(event_type: EventType) -> Event {
    Event {
        event_type,
        time: SystemTime::now(),
        unicode: None,
        code: 0,
        scan_code: 0,
    }
}

pub fn send_key_event(key_event: &KeyEvent) {
    #[cfg(not(any(feature = "flutter", feature = "cli")))]
    if let Some(session) = CUR_SESSION.lock().unwrap().as_ref() {
        session.send_key_event(key_event);
    }
    #[cfg(feature = "flutter")]
    if let Some(session) = SESSIONS
        .read()
        .unwrap()
        .get(&*CUR_SESSION_ID.read().unwrap())
    {
        session.send_key_event(key_event);
    }
}

pub fn get_peer_platform() -> String {
    #[cfg(not(any(feature = "flutter", feature = "cli")))]
    if let Some(session) = CUR_SESSION.lock().unwrap().as_ref() {
        return session.peer_platform();
    }
    #[cfg(feature = "flutter")]
    if let Some(session) = SESSIONS
        .read()
        .unwrap()
        .get(&*CUR_SESSION_ID.read().unwrap())
    {
        return session.peer_platform();
    }
    "Windows".to_string()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn legacy_keyboard_mode(event: &Event, mut key_event: KeyEvent) -> Vec<KeyEvent> {
    let mut events = Vec::new();
    // legacy mode(0): Generate characters locally, look for keycode on other side.
    let (mut key, down_or_up) = match event.event_type {
        EventType::KeyPress(key) => (key, true),
        EventType::KeyRelease(key) => (key, false),
        _ => {
            return events;
        }
    };

    let peer = get_peer_platform();
    let is_win = peer == "Windows";
    if is_win {
        key = convert_numpad_keys(key);
    }

    let alt = get_key_state(enigo::Key::Alt);
    #[cfg(windows)]
    let ctrl = {
        let mut tmp = get_key_state(enigo::Key::Control) || get_key_state(enigo::Key::RightControl);
        unsafe {
            if IS_ALT_GR {
                if alt || key == Key::AltGr {
                    if tmp {
                        tmp = false;
                    }
                } else {
                    IS_ALT_GR = false;
                }
            }
        }
        tmp
    };
    #[cfg(not(windows))]
    let ctrl = get_key_state(enigo::Key::Control) || get_key_state(enigo::Key::RightControl);
    let shift = get_key_state(enigo::Key::Shift) || get_key_state(enigo::Key::RightShift);
    #[cfg(windows)]
    let command = crate::platform::windows::get_win_key_state();
    #[cfg(not(windows))]
    let command = get_key_state(enigo::Key::Meta);
    let control_key = match key {
        Key::Alt => Some(ControlKey::Alt),
        Key::AltGr => Some(ControlKey::RAlt),
        Key::Backspace => Some(ControlKey::Backspace),
        Key::ControlLeft => {
            // when pressing AltGr, an extra VK_LCONTROL with a special
            // scancode with bit 9 set is sent, let's ignore this.
            #[cfg(windows)]
            if (event.scan_code >> 8) == 0xE0 {
                unsafe {
                    IS_ALT_GR = true;
                }
                return events;
            }
            Some(ControlKey::Control)
        }
        Key::ControlRight => Some(ControlKey::RControl),
        Key::DownArrow => Some(ControlKey::DownArrow),
        Key::Escape => Some(ControlKey::Escape),
        Key::F1 => Some(ControlKey::F1),
        Key::F10 => Some(ControlKey::F10),
        Key::F11 => Some(ControlKey::F11),
        Key::F12 => Some(ControlKey::F12),
        Key::F2 => Some(ControlKey::F2),
        Key::F3 => Some(ControlKey::F3),
        Key::F4 => Some(ControlKey::F4),
        Key::F5 => Some(ControlKey::F5),
        Key::F6 => Some(ControlKey::F6),
        Key::F7 => Some(ControlKey::F7),
        Key::F8 => Some(ControlKey::F8),
        Key::F9 => Some(ControlKey::F9),
        Key::LeftArrow => Some(ControlKey::LeftArrow),
        Key::MetaLeft => Some(ControlKey::Meta),
        Key::MetaRight => Some(ControlKey::RWin),
        Key::Return => Some(ControlKey::Return),
        Key::RightArrow => Some(ControlKey::RightArrow),
        Key::ShiftLeft => Some(ControlKey::Shift),
        Key::ShiftRight => Some(ControlKey::RShift),
        Key::Space => Some(ControlKey::Space),
        Key::Tab => Some(ControlKey::Tab),
        Key::UpArrow => Some(ControlKey::UpArrow),
        Key::Delete => {
            if is_win && ctrl && alt {
                client::ctrl_alt_del();
                return events;
            }
            Some(ControlKey::Delete)
        }
        Key::Apps => Some(ControlKey::Apps),
        Key::Cancel => Some(ControlKey::Cancel),
        Key::Clear => Some(ControlKey::Clear),
        Key::Kana => Some(ControlKey::Kana),
        Key::Hangul => Some(ControlKey::Hangul),
        Key::Junja => Some(ControlKey::Junja),
        Key::Final => Some(ControlKey::Final),
        Key::Hanja => Some(ControlKey::Hanja),
        Key::Hanji => Some(ControlKey::Hanja),
        Key::Convert => Some(ControlKey::Convert),
        Key::Print => Some(ControlKey::Print),
        Key::Select => Some(ControlKey::Select),
        Key::Execute => Some(ControlKey::Execute),
        Key::PrintScreen => Some(ControlKey::Snapshot),
        Key::Help => Some(ControlKey::Help),
        Key::Sleep => Some(ControlKey::Sleep),
        Key::Separator => Some(ControlKey::Separator),
        Key::KpReturn => Some(ControlKey::NumpadEnter),
        Key::Kp0 => Some(ControlKey::Numpad0),
        Key::Kp1 => Some(ControlKey::Numpad1),
        Key::Kp2 => Some(ControlKey::Numpad2),
        Key::Kp3 => Some(ControlKey::Numpad3),
        Key::Kp4 => Some(ControlKey::Numpad4),
        Key::Kp5 => Some(ControlKey::Numpad5),
        Key::Kp6 => Some(ControlKey::Numpad6),
        Key::Kp7 => Some(ControlKey::Numpad7),
        Key::Kp8 => Some(ControlKey::Numpad8),
        Key::Kp9 => Some(ControlKey::Numpad9),
        Key::KpDivide => Some(ControlKey::Divide),
        Key::KpMultiply => Some(ControlKey::Multiply),
        Key::KpDecimal => Some(ControlKey::Decimal),
        Key::KpMinus => Some(ControlKey::Subtract),
        Key::KpPlus => Some(ControlKey::Add),
        Key::CapsLock | Key::NumLock | Key::ScrollLock => {
            return events;
        }
        Key::Home => Some(ControlKey::Home),
        Key::End => Some(ControlKey::End),
        Key::Insert => Some(ControlKey::Insert),
        Key::PageUp => Some(ControlKey::PageUp),
        Key::PageDown => Some(ControlKey::PageDown),
        Key::Pause => Some(ControlKey::Pause),
        _ => None,
    };
    if let Some(k) = control_key {
        key_event.set_control_key(k);
    } else {
        let name = event
            .unicode
            .as_ref()
            .and_then(|unicode| unicode.name.clone());
        let mut chr = match &name {
            Some(ref s) => {
                if s.len() <= 2 {
                    // exclude chinese characters
                    s.chars().next().unwrap_or('\0')
                } else {
                    '\0'
                }
            }
            _ => '\0',
        };
        if chr == '·' {
            // special for Chinese
            chr = '`';
        }
        if chr == '\0' {
            chr = match key {
                Key::Num1 => '1',
                Key::Num2 => '2',
                Key::Num3 => '3',
                Key::Num4 => '4',
                Key::Num5 => '5',
                Key::Num6 => '6',
                Key::Num7 => '7',
                Key::Num8 => '8',
                Key::Num9 => '9',
                Key::Num0 => '0',
                Key::KeyA => 'a',
                Key::KeyB => 'b',
                Key::KeyC => 'c',
                Key::KeyD => 'd',
                Key::KeyE => 'e',
                Key::KeyF => 'f',
                Key::KeyG => 'g',
                Key::KeyH => 'h',
                Key::KeyI => 'i',
                Key::KeyJ => 'j',
                Key::KeyK => 'k',
                Key::KeyL => 'l',
                Key::KeyM => 'm',
                Key::KeyN => 'n',
                Key::KeyO => 'o',
                Key::KeyP => 'p',
                Key::KeyQ => 'q',
                Key::KeyR => 'r',
                Key::KeyS => 's',
                Key::KeyT => 't',
                Key::KeyU => 'u',
                Key::KeyV => 'v',
                Key::KeyW => 'w',
                Key::KeyX => 'x',
                Key::KeyY => 'y',
                Key::KeyZ => 'z',
                Key::Comma => ',',
                Key::Dot => '.',
                Key::SemiColon => ';',
                Key::Quote => '\'',
                Key::LeftBracket => '[',
                Key::RightBracket => ']',
                Key::Slash => '/',
                Key::BackSlash => '\\',
                Key::Minus => '-',
                Key::Equal => '=',
                Key::BackQuote => '`',
                _ => '\0',
            }
        }
        if chr != '\0' {
            if chr == 'l' && is_win && command {
                client::lock_screen();
                return events;
            }
            key_event.set_chr(chr as _);
        } else {
            log::error!("Unknown key {:?}", &event);
            return events;
        }
    }
    let (alt, ctrl, shift, command) = client::get_modifiers_state(alt, ctrl, shift, command);
    client::legacy_modifiers(&mut key_event, alt, ctrl, shift, command);

    if down_or_up == true {
        key_event.down = true;
    }
    events.push(key_event);
    events
}

pub fn map_keyboard_mode(event: &Event, mut key_event: KeyEvent) -> Option<KeyEvent> {
    match event.event_type {
        EventType::KeyPress(..) => {
            key_event.down = true;
        }
        EventType::KeyRelease(..) => {
            key_event.down = false;
        }
        _ => return None,
    };

    let mut peer = get_peer_platform().to_lowercase();
    peer.retain(|c| !c.is_whitespace());

    #[cfg(target_os = "windows")]
    let keycode = match peer.as_str() {
        "windows" => {
            // https://github.com/rustdesk/rustdesk/issues/1371
            // Filter scancodes that are greater than 255 and the hight word is not 0xE0.
            if event.scan_code > 255 && (event.scan_code >> 8) != 0xE0 {
                return None;
            }
            event.scan_code
        }
        "macos" => {
            if hbb_common::config::LocalConfig::get_kb_layout_type() == "ISO" {
                rdev::win_scancode_to_macos_iso_code(event.scan_code)?
            } else {
                rdev::win_scancode_to_macos_code(event.scan_code)?
            }
        }
        _ => rdev::win_scancode_to_linux_code(event.scan_code)?,
    };
    #[cfg(target_os = "macos")]
    let keycode = match peer.as_str() {
        "windows" => rdev::macos_code_to_win_scancode(event.code as _)?,
        "macos" => event.code as _,
        _ => rdev::macos_code_to_linux_code(event.code as _)?,
    };
    #[cfg(target_os = "linux")]
    let keycode = match peer.as_str() {
        "windows" => rdev::linux_code_to_win_scancode(event.code as _)?,
        "macos" => {
            if hbb_common::config::LocalConfig::get_kb_layout_type() == "ISO" {
                rdev::linux_code_to_macos_iso_code(event.code as _)?
            } else {
                rdev::linux_code_to_macos_code(event.code as _)?
            }
        }
        _ => event.code as _,
    };
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let keycode = 0;

    key_event.set_chr(keycode);
    Some(key_event)
}

fn try_fill_unicode(event: &Event, key_event: &KeyEvent, events: &mut Vec<KeyEvent>) {
    match &event.unicode {
        Some(unicode_info) => {
            if let Some(name) = unicode_info.name {
                if name.len() > 0 {
                    let mut evt = key_event.clone();
                    evt.set_seq(name);
                    events.push(evt);
                }
            }
        }
        None => {}
    }
}

#[cfg(target_os = "windows")]
fn is_hot_key_modifiers_down() -> bool {
    if rdev::get_modifier(Key::ControlLeft) || rdev::get_modifier(Key::ControlRight) {
        return true;
    }
    if rdev::get_modifier(Key::Alt) || rdev::get_modifier(Key::AltGr) {
        return true;
    }
    if rdev::get_modifier(Key::MetaLeft) || rdev::get_modifier(Key::MetaRight) {
        return true;
    }
    return false;
}

pub fn translate_virtual_keycode(event: &Event, mut key_event: KeyEvent) -> Option<KeyEvent> {
    match event.event_type {
        EventType::KeyPress(..) => {
            key_event.down = true;
        }
        EventType::KeyRelease(..) => {
            key_event.down = false;
        }
        _ => return None,
    };

    let mut peer = get_peer_platform().to_lowercase();
    peer.retain(|c| !c.is_whitespace());

    // #[cfg(target_os = "windows")]
    // let keycode = match peer.as_str() {
    //     "windows" => event.code,
    //     "macos" => {
    //         if hbb_common::config::LocalConfig::get_kb_layout_type() == "ISO" {
    //             rdev::win_scancode_to_macos_iso_code(event.scan_code)?
    //         } else {
    //             rdev::win_scancode_to_macos_code(event.scan_code)?
    //         }
    //     }
    //     _ => rdev::win_scancode_to_linux_code(event.scan_code)?,
    // };

    key_event.set_chr(event.code as _);
    Some(key_event)
}

pub fn translate_keyboard_mode(event: &Event, key_event: KeyEvent) -> Vec<KeyEvent> {
    let mut events: Vec<KeyEvent> = Vec::new();
    if let Some(unicode_info) = &event.unicode {
        if unicode_info.is_dead {
            return events;
        }
    }

    #[cfg(target_os = "windows")]
    unsafe {
        if event.scan_code == 0x021D {
            return events;
        }

        if IS_0X021D_DOWN {
            if event.scan_code == 0xE038 {
                return events;
            }
        }
    }

    #[cfg(target_os = "windows")]
    if unsafe { IS_0X021D_DOWN } || !is_hot_key_modifiers_down() {
        try_fill_unicode(event, &key_event, &mut events);
    }

    #[cfg(target_os = "windows")]
    unsafe {
        if IS_0X021D_DOWN {
            return events;
        }
    }

    #[cfg(not(target_os = "windows"))]
    try_fill_unicode(event, &key_event, &mut events);

    if events.is_empty() {
        if let Some(evt) = translate_virtual_keycode(event, key_event) {
            events.push(evt);
        }
    }
    events
}
