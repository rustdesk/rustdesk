use crate::client::get_key_state;
use crate::common::GrabState;
#[cfg(feature = "flutter")]
use crate::flutter::FlutterHandler;
use crate::ui_session_interface::Session;
use hbb_common::{log, message_proto::*};
use rdev::{Event, EventType, Key};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::SystemTime;

static mut IS_ALT_GR: bool = false;
pub static KEYBOARD_HOOKED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "flutter")]
lazy_static::lazy_static! {
    pub static ref CUR_SESSION: Arc<Mutex<Option<Session<FlutterHandler>>>> = Default::default();
    pub static ref GRAB_SENDER: Arc<Mutex<Option<mpsc::Sender<GrabState>>>> = Default::default();
}

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
        if let Some(handler) = CUR_SESSION.lock().unwrap().as_ref() {
            handler.get_keyboard_mode()
        } else {
            "legacy".to_string()
        }
    }

    pub fn save_keyboard_mode(value: String) {
        release_remote_keys();
        if let Some(handler) = CUR_SESSION.lock().unwrap().as_mut() {
            handler.save_keyboard_mode(value);
        }
    }

    pub fn start_grab_loop() {
        let (sender, receiver) = mpsc::channel::<GrabState>();
        unsafe {
            grab_loop(receiver);
            *GRAB_SENDER.lock().unwrap() = Some(sender);
        }
        change_grab_status(GrabState::Ready);
    }

    pub fn change_grab_status(state: GrabState) {
        if GrabState::Wait == state {
            release_remote_keys();
        }
        unsafe {
            if let Some(sender) = &*GRAB_SENDER.lock().unwrap() {
                log::info!("grab state: {:?}", state);
                sender.send(state);
            }
        }
    }

    pub fn process_event(event: Event) {
        if is_long_press(&event) {
            return;
        }
        let key_event = event_to_key_event(&event);
        log::info!("key event: {:?}", key_event);
        send_key_event(&key_event);
    }

    pub fn get_modifiers_state(
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) -> (bool, bool, bool, bool) {
        get_modifiers_state(alt, ctrl, shift, command)
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

    pub fn lock_screen() {
        let mut key_event = KeyEvent::new();
        key_event.set_control_key(ControlKey::LockScreen);
        key_event.down = true;
        key_event.mode = KeyboardMode::Legacy.into();
        send_key_event(&key_event);
    }

    pub fn ctrl_alt_del() {
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
        send_key_event(&key_event);
    }
}

pub fn grab_loop(recv: mpsc::Receiver<GrabState>) {
    thread::spawn(move || loop {
        if let Some(state) = recv.recv().ok() {
            match state {
                GrabState::Ready => {
                    #[cfg(any(target_os = "windows", target_os = "macos"))]
                    std::thread::spawn(move || {
                        let func = move |event: Event| match event.event_type {
                            EventType::KeyPress(key) | EventType::KeyRelease(key) => {
                                // fix #2211：CAPS LOCK don't work
                                if key == Key::CapsLock || key == Key::NumLock {
                                    return Some(event);
                                }
                                if KEYBOARD_HOOKED.load(Ordering::SeqCst) {
                                    keyboard::client::process_event(event);
                                    return None;
                                } else {
                                    return Some(event);
                                }
                            }
                            _ => Some(event),
                        };
                        if let Err(error) = rdev::grab(func) {
                            log::error!("rdev Error: {:?}", error)
                        }
                    });

                    #[cfg(target_os = "linux")]
                    rdev::start_grab_listen(move |event: Event| match event.event_type {
                        EventType::KeyPress(key) | EventType::KeyRelease(key) => {
                            if let Key::Unknown(keycode) = key {
                                log::error!("rdev get unknown key, keycode is : {:?}", keycode);
                            } else {
                                crate::keyboard::client::process_event(event);
                            }
                            None
                        }
                        _ => Some(event),
                    });
                }
                GrabState::Run => {
                    #[cfg(any(target_os = "windows", target_os = "macos"))]
                    KEYBOARD_HOOKED.swap(true, Ordering::SeqCst);

                    #[cfg(target_os = "linux")]
                    rdev::enable_grab().ok();
                }
                GrabState::Wait => {
                    #[cfg(any(target_os = "windows", target_os = "macos"))]
                    KEYBOARD_HOOKED.swap(false, Ordering::SeqCst);

                    #[cfg(target_os = "linux")]
                    rdev::disable_grab().ok();
                }
                GrabState::Exit => {
                    #[cfg(target_os = "linux")]
                    rdev::exit_grab_listen().ok();
                }
            }
        }
    });
}

pub fn is_long_press(event: &Event) -> bool {
    let mut keys = MODIFIERS_STATE.lock().unwrap();
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
    let to_release = TO_RELEASE.lock().unwrap();
    let keys = to_release.iter().map(|&key| key).collect::<Vec<Key>>();
    drop(to_release);
    for key in keys {
        let event_type = EventType::KeyRelease(key);
        let event = event_type_to_event(event_type);
        log::info!("release key: {:?}", key);
        client::process_event(event);
    }
}

pub fn get_keyboard_mode_enum() -> KeyboardMode {
    match client::get_keyboard_mode().as_str() {
        "map" => KeyboardMode::Map,
        "translate" => KeyboardMode::Translate,
        _ => KeyboardMode::Legacy,
    }
}

pub fn add_numlock_capslock_state(key_event: &mut KeyEvent) {
    if get_key_state(enigo::Key::CapsLock) {
        key_event.modifiers.push(ControlKey::CapsLock.into());
    }
    if get_key_state(enigo::Key::NumLock) {
        key_event.modifiers.push(ControlKey::NumLock.into());
    }
}

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
    let alt =
        *modifiers_lock.get(&Key::Alt).unwrap() || *modifiers_lock.get(&Key::AltGr).unwrap() || alt;

    (alt, ctrl, shift, command)
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

pub fn event_to_key_event(event: &Event) -> KeyEvent {
    let mut key_event = KeyEvent::new();
    update_modifiers_state(event);

    let mut to_release = TO_RELEASE.lock().unwrap();
    match event.event_type {
        EventType::KeyPress(key) => {
            to_release.insert(key);
        }
        EventType::KeyRelease(key) => {
            to_release.remove(&key);
        }
        _ => {}
    }
    drop(to_release);

    let keyboard_mode = get_keyboard_mode_enum();
    key_event.mode = keyboard_mode.into();
    match keyboard_mode {
        KeyboardMode::Map => {
            map_keyboard_mode(event, &mut key_event);
        }
        KeyboardMode::Translate => {
            translate_keyboard_mode(event, &mut key_event);
        }
        _ => {
            legacy_keyboard_mode(event, &mut key_event);
        }
    };
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    add_numlock_capslock_state(&mut key_event);

    return key_event;
}

pub fn event_type_to_event(event_type: EventType) -> Event {
    Event {
        event_type,
        time: SystemTime::now(),
        name: None,
        code: 0,
        scan_code: 0,
    }
}

#[cfg(feature = "flutter")]
pub fn send_key_event(key_event: &KeyEvent) {
    if let Some(handler) = CUR_SESSION.lock().unwrap().as_ref() {
        log::info!("Sending key even {:?}", key_event);
        handler.send_key_event(key_event);
    }
}

pub fn get_peer_platform() -> String {
    if let Some(handler) = CUR_SESSION.lock().unwrap().as_ref() {
        handler.peer_platform()
    } else {
        log::error!("get peer platform error");
        "Windows".to_string()
    }
}

pub fn legacy_keyboard_mode(event: &Event, key_event: &mut KeyEvent) {
    // legacy mode(0): Generate characters locally, look for keycode on other side.
    let (mut key, down_or_up) = match event.event_type {
        EventType::KeyPress(key) => (key, true),
        EventType::KeyRelease(key) => (key, false),
        _ => {
            return;
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
            if event.scan_code & 0x200 != 0 {
                unsafe {
                    IS_ALT_GR = true;
                }
                return;
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
                return;
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
            return;
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
        let mut chr = match event.name {
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
                return;
            }
            key_event.set_chr(chr as _);
        } else {
            log::error!("Unknown key {:?}", &event);
            return;
        }
    }
    let (alt, ctrl, shift, command) = get_modifiers_state(alt, ctrl, shift, command);
    client::legacy_modifiers(key_event, alt, ctrl, shift, command);

    if down_or_up == true {
        key_event.down = true;
    }
}

pub fn map_keyboard_mode(event: &Event, key_event: &mut KeyEvent) {
    let peer = get_peer_platform();

    let key = match event.event_type {
        EventType::KeyPress(key) => {
            key_event.down = true;
            key
        }
        EventType::KeyRelease(key) => {
            key_event.down = false;
            key
        }
        _ => return,
    };
    let keycode: u32 = match peer.as_str() {
        "Windows" => rdev::win_keycode_from_key(key).unwrap_or_default().into(),
        "MacOS" => rdev::macos_keycode_from_key(key).unwrap_or_default().into(),
        _ => rdev::linux_keycode_from_key(key).unwrap_or_default().into(),
    };
    key_event.set_chr(keycode);
}

pub fn translate_keyboard_mode(event: &Event, key_event: &mut KeyEvent) {}
