use super::*;
#[cfg(target_os = "macos")]
use dispatch::Queue;
use enigo::{Enigo, Key, KeyboardControllable, MouseButton, MouseControllable};
use hbb_common::{config::COMPRESS_LEVEL, protobuf::EnumOrUnknown};
use std::{
    convert::TryFrom,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

#[derive(Default)]
struct StateCursor {
    hcursor: u64,
    cursor_data: Arc<Message>,
    cached_cursor_data: HashMap<u64, Arc<Message>>,
}

impl super::service::Reset for StateCursor {
    fn reset(&mut self) {
        *self = Default::default();
        crate::platform::reset_input_cache();
        fix_key_down_timeout(true);
        #[cfg(target_os = "linux")]
        ENIGO.lock().unwrap().reset();
    }
}

#[derive(Default)]
struct StatePos {
    cursor_pos: (i32, i32),
}

impl super::service::Reset for StatePos {
    fn reset(&mut self) {
        self.cursor_pos = (0, 0);
    }
}

#[derive(Default)]
struct Input {
    conn: i32,
    time: i64,
}

const KEY_CHAR_START: u64 = 9999;

#[derive(Clone, Default)]
pub struct MouseCursorSub {
    inner: ConnInner,
    cached: HashMap<u64, Arc<Message>>,
}

impl From<ConnInner> for MouseCursorSub {
    fn from(inner: ConnInner) -> Self {
        Self {
            inner,
            cached: HashMap::new(),
        }
    }
}

impl Subscriber for MouseCursorSub {
    #[inline]
    fn id(&self) -> i32 {
        self.inner.id()
    }

    #[inline]
    fn send(&mut self, msg: Arc<Message>) {
        if let Some(message::Union::CursorData(cd)) = &msg.union {
            if let Some(msg) = self.cached.get(&cd.id) {
                self.inner.send(msg.clone());
            } else {
                self.inner.send(msg.clone());
                let mut tmp = Message::new();
                // only send id out, require client side cache also
                tmp.set_cursor_id(cd.id);
                self.cached.insert(cd.id, Arc::new(tmp));
            }
        } else {
            self.inner.send(msg);
        }
    }
}

pub const NAME_CURSOR: &'static str = "mouse_cursor";
pub const NAME_POS: &'static str = "mouse_pos";
pub type MouseCursorService = ServiceTmpl<MouseCursorSub>;

pub fn new_cursor() -> MouseCursorService {
    let sp = MouseCursorService::new(NAME_CURSOR, true);
    sp.repeat::<StateCursor, _>(33, run_cursor);
    sp
}

pub fn new_pos() -> GenericService {
    let sp = GenericService::new(NAME_POS, false);
    sp.repeat::<StatePos, _>(33, run_pos);
    sp
}

fn run_pos(sp: GenericService, state: &mut StatePos) -> ResultType<()> {
    if let Some((x, y)) = crate::get_cursor_pos() {
        if state.cursor_pos.0 != x || state.cursor_pos.1 != y {
            state.cursor_pos = (x, y);
            let mut msg_out = Message::new();
            msg_out.set_cursor_position(CursorPosition {
                x,
                y,
                ..Default::default()
            });
            let exclude = {
                let now = crate::get_time();
                let lock = LATEST_INPUT.lock().unwrap();
                if now - lock.time < 300 {
                    lock.conn
                } else {
                    0
                }
            };
            sp.send_without(msg_out, exclude);
        }
    }

    sp.snapshot(|sps| {
        let mut msg_out = Message::new();
        msg_out.set_cursor_position(CursorPosition {
            x: state.cursor_pos.0,
            y: state.cursor_pos.1,
            ..Default::default()
        });
        sps.send(msg_out);
        Ok(())
    })?;
    Ok(())
}

fn run_cursor(sp: MouseCursorService, state: &mut StateCursor) -> ResultType<()> {
    if let Some(hcursor) = crate::get_cursor()? {
        if hcursor != state.hcursor {
            let msg;
            if let Some(cached) = state.cached_cursor_data.get(&hcursor) {
                super::log::trace!("Cursor data cached, hcursor: {}", hcursor);
                msg = cached.clone();
            } else {
                let mut data = crate::get_cursor_data(hcursor)?;
                data.colors = hbb_common::compress::compress(&data.colors[..], COMPRESS_LEVEL);
                let mut tmp = Message::new();
                tmp.set_cursor_data(data);
                msg = Arc::new(tmp);
                state.cached_cursor_data.insert(hcursor, msg.clone());
                super::log::trace!("Cursor data updated, hcursor: {}", hcursor);
            }
            state.hcursor = hcursor;
            sp.send_shared(msg.clone());
            state.cursor_data = msg;
        }
    }
    sp.snapshot(|sps| {
        sps.send_shared(state.cursor_data.clone());
        Ok(())
    })?;
    Ok(())
}

lazy_static::lazy_static! {
    static ref ENIGO: Arc<Mutex<Enigo>> = {
        #[cfg(target_os = "linux")]
        {
            if crate::platform::is_root() {
                std::env::set_var("PYNPUT_USERNAME", crate::platform::linux::get_active_username());
                std::env::set_var("PYNPUT_USERID", crate::platform::linux::get_active_userid());
            }
        }
        Arc::new(Mutex::new(Enigo::new()))
    };
    static ref KEYS_DOWN: Arc<Mutex<HashMap<u64, Instant>>> = Default::default();
    static ref LATEST_INPUT: Arc<Mutex<Input>> = Default::default();
}
static EXITING: AtomicBool = AtomicBool::new(false);

// mac key input must be run in main thread, otherwise crash on >= osx 10.15
#[cfg(target_os = "macos")]
lazy_static::lazy_static! {
    static ref QUEUE: Queue = Queue::main();
    static ref IS_SERVER: bool =  std::env::args().nth(1) == Some("--server".to_owned());
}

#[cfg(target_os = "linux")]
pub async fn set_uinput() -> ResultType<()> {
    // Keyboard and mouse both open /dev/uinput
    // TODO: Make sure there's no race
    let keyboard = self::uinput::client::UInputKeyboard::new().await?;
    log::info!("UInput keyboard created");
    let mouse = self::uinput::client::UInputMouse::new().await?;
    log::info!("UInput mouse created");

    let mut en = ENIGO.lock().unwrap();
    en.set_uinput_keyboard(Some(Box::new(keyboard)));
    en.set_uinput_mouse(Some(Box::new(mouse)));
    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn set_uinput_resolution(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
    self::uinput::client::set_resolution(minx, maxx, miny, maxy).await
}

pub fn is_left_up(evt: &MouseEvent) -> bool {
    let buttons = evt.mask >> 3;
    let evt_type = evt.mask & 0x7;
    return buttons == 1 && evt_type == 2;
}

#[cfg(windows)]
pub fn mouse_move_relative(x: i32, y: i32) {
    crate::platform::windows::try_change_desktop();
    let mut en = ENIGO.lock().unwrap();
    en.mouse_move_relative(x, y);
}

#[cfg(not(target_os = "macos"))]
fn modifier_sleep() {
    // sleep for a while, this is only for keying in rdp in peer so far
    #[cfg(windows)]
    std::thread::sleep(std::time::Duration::from_nanos(1));
}

#[inline]
fn get_modifier_state(key: Key, en: &mut Enigo) -> bool {
    // https://github.com/rustdesk/rustdesk/issues/332
    // on Linux, if RightAlt is down, RightAlt status is false, Alt status is true
    // but on Windows, both are true
    let x = en.get_key_state(key.clone());
    match key {
        Key::Shift => x || en.get_key_state(Key::RightShift),
        Key::Control => x || en.get_key_state(Key::RightControl),
        Key::Alt => x || en.get_key_state(Key::RightAlt),
        Key::Meta => x || en.get_key_state(Key::RWin),
        Key::RightShift => x || en.get_key_state(Key::Shift),
        Key::RightControl => x || en.get_key_state(Key::Control),
        Key::RightAlt => x || en.get_key_state(Key::Alt),
        Key::RWin => x || en.get_key_state(Key::Meta),
        _ => x,
    }
}

pub fn handle_mouse(evt: &MouseEvent, conn: i32) {
    #[cfg(target_os = "macos")]
    if !*IS_SERVER {
        // having GUI, run main GUI thread, otherwise crash
        let evt = evt.clone();
        QUEUE.exec_async(move || handle_mouse_(&evt, conn));
        return;
    }
    handle_mouse_(evt, conn);
}

pub fn fix_key_down_timeout_loop() {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(1_000));
        fix_key_down_timeout(false);
    });
    if let Err(err) = ctrlc::set_handler(move || {
        fix_key_down_timeout_at_exit();
        std::process::exit(0); // will call atexit on posix, but not on Windows
    }) {
        log::error!("Failed to set Ctrl-C handler: {}", err);
    }
}

pub fn fix_key_down_timeout_at_exit() {
    if EXITING.load(Ordering::SeqCst) {
        return;
    }
    EXITING.store(true, Ordering::SeqCst);
    fix_key_down_timeout(true);
    log::info!("fix_key_down_timeout_at_exit");
}

#[inline]
fn get_layout(key: u32) -> Key {
    Key::Layout(std::char::from_u32(key).unwrap_or('\0'))
}

fn fix_key_down_timeout(force: bool) {
    if KEYS_DOWN.lock().unwrap().is_empty() {
        return;
    }
    let cloned = (*KEYS_DOWN.lock().unwrap()).clone();
    for (key, value) in cloned.into_iter() {
        if force || value.elapsed().as_millis() >= 360_000 {
            KEYS_DOWN.lock().unwrap().remove(&key);
            let key = if key < KEY_CHAR_START {
                if let Some(key) = KEY_MAP.get(&(key as _)) {
                    Some(*key)
                } else {
                    None
                }
            } else {
                Some(get_layout((key - KEY_CHAR_START) as _))
            };
            if let Some(key) = key {
                let func = move || {
                    let mut en = ENIGO.lock().unwrap();
                    en.key_up(key);
                    log::debug!("Fixed {:?} timeout", key);
                };
                #[cfg(target_os = "macos")]
                QUEUE.exec_async(func);
                #[cfg(not(target_os = "macos"))]
                func();
            }
        }
    }
}

// e.g. current state of ctrl is down, but ctrl not in modifier, we should change ctrl to up, to make modifier state sync between remote and local
#[inline]
fn fix_modifier(
    modifiers: &[EnumOrUnknown<ControlKey>],
    key0: ControlKey,
    key1: Key,
    en: &mut Enigo,
) {
    if get_modifier_state(key1, en) && !modifiers.contains(&EnumOrUnknown::new(key0)) {
        #[cfg(windows)]
        if key0 == ControlKey::Control && get_modifier_state(Key::Alt, en) {
            // AltGr case
            return;
        }
        en.key_up(key1);
        log::debug!("Fixed {:?}", key1);
    }
}

fn fix_modifiers(modifiers: &[EnumOrUnknown<ControlKey>], en: &mut Enigo, ck: i32) {
    if ck != ControlKey::Shift.value() {
        fix_modifier(modifiers, ControlKey::Shift, Key::Shift, en);
    }
    if ck != ControlKey::RShift.value() {
        fix_modifier(modifiers, ControlKey::Shift, Key::RightShift, en);
    }
    if ck != ControlKey::Alt.value() {
        fix_modifier(modifiers, ControlKey::Alt, Key::Alt, en);
    }
    if ck != ControlKey::RAlt.value() {
        fix_modifier(modifiers, ControlKey::Alt, Key::RightAlt, en);
    }
    if ck != ControlKey::Control.value() {
        fix_modifier(modifiers, ControlKey::Control, Key::Control, en);
    }
    if ck != ControlKey::RControl.value() {
        fix_modifier(modifiers, ControlKey::Control, Key::RightControl, en);
    }
    if ck != ControlKey::Meta.value() {
        fix_modifier(modifiers, ControlKey::Meta, Key::Meta, en);
    }
    if ck != ControlKey::RWin.value() {
        fix_modifier(modifiers, ControlKey::Meta, Key::RWin, en);
    }
}

fn handle_mouse_(evt: &MouseEvent, conn: i32) {
    if EXITING.load(Ordering::SeqCst) {
        return;
    }
    #[cfg(windows)]
    crate::platform::windows::try_change_desktop();
    let buttons = evt.mask >> 3;
    let evt_type = evt.mask & 0x7;
    if evt_type == 0 {
        let time = crate::get_time();
        *LATEST_INPUT.lock().unwrap() = Input { time, conn };
    }
    let mut en = ENIGO.lock().unwrap();
    #[cfg(not(target_os = "macos"))]
    let mut to_release = Vec::new();
    if evt_type == 1 {
        fix_modifiers(&evt.modifiers[..], &mut en, 0);
        #[cfg(target_os = "macos")]
        en.reset_flag();
        for ref ck in evt.modifiers.iter() {
            if let Some(key) = KEY_MAP.get(&ck.value()) {
                #[cfg(target_os = "macos")]
                en.add_flag(key);
                #[cfg(not(target_os = "macos"))]
                if key != &Key::CapsLock && key != &Key::NumLock {
                    if !get_modifier_state(key.clone(), &mut en) {
                        en.key_down(key.clone()).ok();
                        modifier_sleep();
                        to_release.push(key);
                    }
                }
            }
        }
    }
    match evt_type {
        0 => {
            en.mouse_move_to(evt.x, evt.y);
        }
        1 => match buttons {
            1 => {
                allow_err!(en.mouse_down(MouseButton::Left));
            }
            2 => {
                allow_err!(en.mouse_down(MouseButton::Right));
            }
            4 => {
                allow_err!(en.mouse_down(MouseButton::Middle));
            }
            _ => {}
        },
        2 => match buttons {
            1 => {
                en.mouse_up(MouseButton::Left);
            }
            2 => {
                en.mouse_up(MouseButton::Right);
            }
            4 => {
                en.mouse_up(MouseButton::Middle);
            }
            _ => {}
        },
        3 => {
            #[allow(unused_mut)]
            let mut x = evt.x;
            #[allow(unused_mut)]
            let mut y = evt.y;
            #[cfg(not(windows))]
            {
                x = -x;
                y = -y;
            }
            if x != 0 {
                en.mouse_scroll_x(x);
            }
            if y != 0 {
                en.mouse_scroll_y(y);
            }
        }
        _ => {}
    }
    #[cfg(not(target_os = "macos"))]
    for key in to_release {
        en.key_up(key.clone());
    }
}

pub fn is_enter(evt: &KeyEvent) -> bool {
    if let Some(key_event::Union::ControlKey(ck)) = evt.union {
        if ck.value() == ControlKey::Return.value() || ck.value() == ControlKey::NumpadEnter.value()
        {
            return true;
        }
    }
    return false;
}

pub async fn lock_screen() {
    cfg_if::cfg_if! {
    if #[cfg(target_os = "linux")] {
        // xdg_screensaver lock not work on Linux from our service somehow
        // loginctl lock-session also not work, they both work run rustdesk from cmd
        std::thread::spawn(|| {
            let mut key_event = KeyEvent::new();
            key_event.down = true;
            key_event.set_chr('l' as _);
            key_event.modifiers.push(ControlKey::Meta.into());
            handle_key(&key_event);
            key_event.down = false;
            handle_key(&key_event);
        });
    } else if #[cfg(target_os = "macos")] {
        // CGSession -suspend not real lock screen, it is user switch
        std::thread::spawn(|| {
            let mut key_event = KeyEvent::new();
            key_event.down = true;
            key_event.set_chr('q' as _);
            key_event.modifiers.push(ControlKey::Meta.into());
            key_event.modifiers.push(ControlKey::Control.into());
            handle_key(&key_event);
            key_event.down = false;
            handle_key(&key_event);
        });
    } else {
    crate::platform::lock_screen();
    }
    }
    super::video_service::switch_to_primary().await;
}

lazy_static::lazy_static! {
    static ref KEY_MAP: HashMap<i32, Key> =
    [
        (ControlKey::Alt, Key::Alt),
        (ControlKey::Backspace, Key::Backspace),
        (ControlKey::CapsLock, Key::CapsLock),
        (ControlKey::Control, Key::Control),
        (ControlKey::Delete, Key::Delete),
        (ControlKey::DownArrow, Key::DownArrow),
        (ControlKey::End, Key::End),
        (ControlKey::Escape, Key::Escape),
        (ControlKey::F1, Key::F1),
        (ControlKey::F10, Key::F10),
        (ControlKey::F11, Key::F11),
        (ControlKey::F12, Key::F12),
        (ControlKey::F2, Key::F2),
        (ControlKey::F3, Key::F3),
        (ControlKey::F4, Key::F4),
        (ControlKey::F5, Key::F5),
        (ControlKey::F6, Key::F6),
        (ControlKey::F7, Key::F7),
        (ControlKey::F8, Key::F8),
        (ControlKey::F9, Key::F9),
        (ControlKey::Home, Key::Home),
        (ControlKey::LeftArrow, Key::LeftArrow),
        (ControlKey::Meta, Key::Meta),
        (ControlKey::Option, Key::Option),
        (ControlKey::PageDown, Key::PageDown),
        (ControlKey::PageUp, Key::PageUp),
        (ControlKey::Return, Key::Return),
        (ControlKey::RightArrow, Key::RightArrow),
        (ControlKey::Shift, Key::Shift),
        (ControlKey::Space, Key::Space),
        (ControlKey::Tab, Key::Tab),
        (ControlKey::UpArrow, Key::UpArrow),
        (ControlKey::Numpad0, Key::Numpad0),
        (ControlKey::Numpad1, Key::Numpad1),
        (ControlKey::Numpad2, Key::Numpad2),
        (ControlKey::Numpad3, Key::Numpad3),
        (ControlKey::Numpad4, Key::Numpad4),
        (ControlKey::Numpad5, Key::Numpad5),
        (ControlKey::Numpad6, Key::Numpad6),
        (ControlKey::Numpad7, Key::Numpad7),
        (ControlKey::Numpad8, Key::Numpad8),
        (ControlKey::Numpad9, Key::Numpad9),
        (ControlKey::Cancel, Key::Cancel),
        (ControlKey::Clear, Key::Clear),
        (ControlKey::Menu, Key::Alt),
        (ControlKey::Pause, Key::Pause),
        (ControlKey::Kana, Key::Kana),
        (ControlKey::Hangul, Key::Hangul),
        (ControlKey::Junja, Key::Junja),
        (ControlKey::Final, Key::Final),
        (ControlKey::Hanja, Key::Hanja),
        (ControlKey::Kanji, Key::Kanji),
        (ControlKey::Convert, Key::Convert),
        (ControlKey::Select, Key::Select),
        (ControlKey::Print, Key::Print),
        (ControlKey::Execute, Key::Execute),
        (ControlKey::Snapshot, Key::Snapshot),
        (ControlKey::Insert, Key::Insert),
        (ControlKey::Help, Key::Help),
        (ControlKey::Sleep, Key::Sleep),
        (ControlKey::Separator, Key::Separator),
        (ControlKey::Scroll, Key::Scroll),
        (ControlKey::NumLock, Key::NumLock),
        (ControlKey::RWin, Key::RWin),
        (ControlKey::Apps, Key::Apps),
        (ControlKey::Multiply, Key::Multiply),
        (ControlKey::Add, Key::Add),
        (ControlKey::Subtract, Key::Subtract),
        (ControlKey::Decimal, Key::Decimal),
        (ControlKey::Divide, Key::Divide),
        (ControlKey::Equals, Key::Equals),
        (ControlKey::NumpadEnter, Key::NumpadEnter),
        (ControlKey::RAlt, Key::RightAlt),
        (ControlKey::RControl, Key::RightControl),
        (ControlKey::RShift, Key::RightShift),
    ].iter().map(|(a, b)| (a.value(), b.clone())).collect();
    static ref NUMPAD_KEY_MAP: HashMap<i32, bool> =
    [
        (ControlKey::Home, true),
        (ControlKey::UpArrow, true),
        (ControlKey::PageUp, true),
        (ControlKey::LeftArrow, true),
        (ControlKey::RightArrow, true),
        (ControlKey::End, true),
        (ControlKey::DownArrow, true),
        (ControlKey::PageDown, true),
        (ControlKey::Insert, true),
        (ControlKey::Delete, true),
    ].iter().map(|(a, b)| (a.value(), b.clone())).collect();
}

pub fn handle_key(evt: &KeyEvent) {
    #[cfg(target_os = "macos")]
    if !*IS_SERVER {
        // having GUI, run main GUI thread, otherwise crash
        let evt = evt.clone();
        QUEUE.exec_async(move || handle_key_(&evt));
        return;
    }
    handle_key_(evt);
}

fn handle_key_(evt: &KeyEvent) {
    if EXITING.load(Ordering::SeqCst) {
        return;
    }
    #[cfg(windows)]
    crate::platform::windows::try_change_desktop();
    let mut en = ENIGO.lock().unwrap();
    // disable numlock if press home etc when numlock is on,
    // because we will get numpad value (7,8,9 etc) if not
    #[cfg(windows)]
    let mut disable_numlock = false;
    #[cfg(target_os = "macos")]
    en.reset_flag();
    #[cfg(not(target_os = "macos"))]
    let mut to_release = Vec::new();
    #[cfg(not(target_os = "macos"))]
    let mut has_cap = false;
    #[cfg(windows)]
    let mut has_numlock = false;
    if evt.down {
        let ck = if let Some(key_event::Union::ControlKey(ck)) = evt.union {
            ck.value()
        } else {
            -1
        };
        fix_modifiers(&evt.modifiers[..], &mut en, ck);
        for ref ck in evt.modifiers.iter() {
            if let Some(key) = KEY_MAP.get(&ck.value()) {
                #[cfg(target_os = "linux")]
                if key == &Key::Alt && !get_modifier_state(key.clone(), &mut en) {
                    // for AltGr on Linux
                    if KEYS_DOWN
                        .lock()
                        .unwrap()
                        .get(&(ControlKey::RAlt.value() as _))
                        .is_some()
                    {
                        continue;
                    }
                }
                #[cfg(target_os = "macos")]
                en.add_flag(key);
                #[cfg(not(target_os = "macos"))]
                {
                    if key == &Key::CapsLock {
                        has_cap = true;
                    } else if key == &Key::NumLock {
                        #[cfg(windows)]
                        {
                            has_numlock = true;
                        }
                    } else {
                        if !get_modifier_state(key.clone(), &mut en) {
                            en.key_down(key.clone()).ok();
                            modifier_sleep();
                            to_release.push(key);
                        }
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    if has_cap != en.get_key_state(Key::CapsLock) {
        en.key_down(Key::CapsLock).ok();
        en.key_up(Key::CapsLock);
    }
    #[cfg(windows)]
    if crate::common::valid_for_numlock(evt) {
        if has_numlock != en.get_key_state(Key::NumLock) {
            en.key_down(Key::NumLock).ok();
            en.key_up(Key::NumLock);
        }
    }
    match evt.union {
        Some(key_event::Union::ControlKey(ck)) => {
            if let Some(key) = KEY_MAP.get(&ck.value()) {
                #[cfg(windows)]
                if let Some(_) = NUMPAD_KEY_MAP.get(&ck.value()) {
                    disable_numlock = en.get_key_state(Key::NumLock);
                    if disable_numlock {
                        en.key_down(Key::NumLock).ok();
                        en.key_up(Key::NumLock);
                    }
                }
                if evt.down {
                    allow_err!(en.key_down(key.clone()));
                    KEYS_DOWN
                        .lock()
                        .unwrap()
                        .insert(ck.value() as _, Instant::now());
                } else {
                    en.key_up(key.clone());
                    KEYS_DOWN.lock().unwrap().remove(&(ck.value() as _));
                }
            } else if ck.value() == ControlKey::CtrlAltDel.value() {
                // have to spawn new thread because send_sas is tokio_main, the caller can not be tokio_main.
                std::thread::spawn(|| {
                    allow_err!(send_sas());
                });
            } else if ck.value() == ControlKey::LockScreen.value() {
                lock_screen_2();
            }
        }
        Some(key_event::Union::Chr(chr)) => {
            if evt.down {
                if en.key_down(get_layout(chr)).is_ok() {
                    KEYS_DOWN
                        .lock()
                        .unwrap()
                        .insert(chr as u64 + KEY_CHAR_START, Instant::now());
                } else {
                    if let Ok(chr) = char::try_from(chr) {
                        let mut x = chr.to_string();
                        if get_modifier_state(Key::Shift, &mut en)
                            || get_modifier_state(Key::CapsLock, &mut en)
                        {
                            x = x.to_uppercase();
                        }
                        en.key_sequence(&x);
                    }
                }
            } else {
                en.key_up(get_layout(chr));
                KEYS_DOWN
                    .lock()
                    .unwrap()
                    .remove(&(chr as u64 + KEY_CHAR_START));
            }
        }
        Some(key_event::Union::Unicode(chr)) => {
            if let Ok(chr) = char::try_from(chr) {
                en.key_sequence(&chr.to_string());
            }
        }
        Some(key_event::Union::Seq(ref seq)) => {
            en.key_sequence(&seq);
        }
        _ => {}
    }
    #[cfg(not(target_os = "macos"))]
    for key in to_release {
        en.key_up(key.clone());
    }
    #[cfg(windows)]
    if disable_numlock {
        en.key_down(Key::NumLock).ok();
        en.key_up(Key::NumLock);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn lock_screen_2() {
    lock_screen().await;
}

#[tokio::main(flavor = "current_thread")]
async fn send_sas() -> ResultType<()> {
    let mut stream = crate::ipc::connect(1000, crate::POSTFIX_SERVICE).await?;
    timeout(1000, stream.send(&crate::ipc::Data::SAS)).await??;
    Ok(())
}

#[cfg(target_os = "linux")]
pub mod uinput {
    use crate::ipc::{self, new_listener, Connection, Data, DataKeyboard, DataMouse};
    use enigo::{Key, KeyboardControllable, MouseButton, MouseControllable};
    use evdev::{
        uinput::{VirtualDevice, VirtualDeviceBuilder},
        AttributeSet, EventType, InputEvent,
    };
    use hbb_common::{allow_err, bail, log, tokio, ResultType};

    static IPC_CONN_TIMEOUT: u64 = 1000;
    static IPC_REQUEST_TIMEOUT: u64 = 1000;
    static IPC_POSTFIX_KEYBOARD: &str = "_uinput_keyboard";
    static IPC_POSTFIX_MOUSE: &str = "_uinput_mouse";
    static IPC_POSTFIX_CONTROL: &str = "_uinput_control";

    pub mod client {
        use super::*;

        pub struct UInputKeyboard {
            conn: Connection,
        }

        impl UInputKeyboard {
            pub async fn new() -> ResultType<Self> {
                let conn = ipc::connect(IPC_CONN_TIMEOUT, IPC_POSTFIX_KEYBOARD).await?;
                Ok(Self { conn })
            }

            #[tokio::main(flavor = "current_thread")]
            async fn send(&mut self, data: Data) -> ResultType<()> {
                self.conn.send(&data).await
            }

            #[tokio::main(flavor = "current_thread")]
            async fn send_get_key_state(&mut self, data: Data) -> ResultType<bool> {
                self.conn.send(&data).await?;

                match self.conn.next_timeout(IPC_REQUEST_TIMEOUT).await {
                    Ok(Some(Data::KeyboardResponse(ipc::DataKeyboardResponse::GetKeyState(
                        state,
                    )))) => Ok(state),
                    Ok(Some(resp)) => {
                        // FATAL error!!!
                        bail!(
                            "FATAL error, wait keyboard result other response: {:?}",
                            &resp
                        );
                    }
                    Ok(None) => {
                        // FATAL error!!!
                        // Maybe wait later
                        bail!("FATAL error, wait keyboard result, receive None",);
                    }
                    Err(e) => {
                        // FATAL error!!!
                        bail!(
                            "FATAL error, wait keyboard result timeout {}, {}",
                            &e,
                            IPC_REQUEST_TIMEOUT
                        );
                    }
                }
            }
        }

        impl KeyboardControllable for UInputKeyboard {
            fn get_key_state(&mut self, key: Key) -> bool {
                match self.send_get_key_state(Data::Keyboard(DataKeyboard::GetKeyState(key))) {
                    Ok(state) => state,
                    Err(e) => {
                        // unreachable!()
                        log::error!("Failed to get key state {}", &e);
                        false
                    }
                }
            }

            fn key_sequence(&mut self, sequence: &str) {
                allow_err!(self.send(Data::Keyboard(DataKeyboard::Sequence(sequence.to_string()))));
            }

            // TODO: handle error???
            fn key_down(&mut self, key: Key) -> enigo::ResultType {
                allow_err!(self.send(Data::Keyboard(DataKeyboard::KeyDown(key))));
                Ok(())
            }
            fn key_up(&mut self, key: Key) {
                allow_err!(self.send(Data::Keyboard(DataKeyboard::KeyUp(key))));
            }
            fn key_click(&mut self, key: Key) {
                allow_err!(self.send(Data::Keyboard(DataKeyboard::KeyClick(key))));
            }
        }

        pub struct UInputMouse {
            conn: Connection,
        }

        impl UInputMouse {
            pub async fn new() -> ResultType<Self> {
                let conn = ipc::connect(IPC_CONN_TIMEOUT, IPC_POSTFIX_MOUSE).await?;
                Ok(Self { conn })
            }

            #[tokio::main(flavor = "current_thread")]
            async fn send(&mut self, data: Data) -> ResultType<()> {
                self.conn.send(&data).await
            }
        }

        impl MouseControllable for UInputMouse {
            fn mouse_move_to(&mut self, x: i32, y: i32) {
                allow_err!(self.send(Data::Mouse(DataMouse::MoveTo(x, y))));
            }
            fn mouse_move_relative(&mut self, x: i32, y: i32) {
                allow_err!(self.send(Data::Mouse(DataMouse::MoveRelative(x, y))));
            }
            // TODO: handle error???
            fn mouse_down(&mut self, button: MouseButton) -> enigo::ResultType {
                allow_err!(self.send(Data::Mouse(DataMouse::Down(button))));
                Ok(())
            }
            fn mouse_up(&mut self, button: MouseButton) {
                allow_err!(self.send(Data::Mouse(DataMouse::Up(button))));
            }
            fn mouse_click(&mut self, button: MouseButton) {
                allow_err!(self.send(Data::Mouse(DataMouse::Click(button))));
            }
            fn mouse_scroll_x(&mut self, length: i32) {
                allow_err!(self.send(Data::Mouse(DataMouse::ScrollX(length))));
            }
            fn mouse_scroll_y(&mut self, length: i32) {
                allow_err!(self.send(Data::Mouse(DataMouse::ScrollY(length))));
            }
        }

        pub async fn set_resolution(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
            let mut conn = ipc::connect(IPC_CONN_TIMEOUT, IPC_POSTFIX_CONTROL).await?;
            conn.send(&Data::Control(ipc::DataControl::Resolution {
                minx,
                maxx,
                miny,
                maxy,
            }))
            .await?;
            let _ = conn.next().await?;
            Ok(())
        }
    }

    pub mod service {
        use super::*;
        use hbb_common::lazy_static;
        use mouce::MouseActions;
        use std::{collections::HashMap, sync::Mutex};

        lazy_static::lazy_static! {
        static ref KEY_MAP: HashMap<enigo::Key, evdev::Key> = HashMap::from(
            [
                (enigo::Key::Alt, evdev::Key::KEY_LEFTALT),
                (enigo::Key::Backspace, evdev::Key::KEY_BACKSPACE),
                (enigo::Key::CapsLock, evdev::Key::KEY_CAPSLOCK),
                (enigo::Key::Control, evdev::Key::KEY_LEFTCTRL),
                (enigo::Key::Delete, evdev::Key::KEY_DELETE),
                (enigo::Key::DownArrow, evdev::Key::KEY_DOWN),
                (enigo::Key::End, evdev::Key::KEY_END),
                (enigo::Key::Escape, evdev::Key::KEY_ESC),
                (enigo::Key::F1, evdev::Key::KEY_F1),
                (enigo::Key::F10, evdev::Key::KEY_F10),
                (enigo::Key::F11, evdev::Key::KEY_F11),
                (enigo::Key::F12, evdev::Key::KEY_F12),
                (enigo::Key::F2, evdev::Key::KEY_F2),
                (enigo::Key::F3, evdev::Key::KEY_F3),
                (enigo::Key::F4, evdev::Key::KEY_F4),
                (enigo::Key::F5, evdev::Key::KEY_F5),
                (enigo::Key::F6, evdev::Key::KEY_F6),
                (enigo::Key::F7, evdev::Key::KEY_F7),
                (enigo::Key::F8, evdev::Key::KEY_F8),
                (enigo::Key::F9, evdev::Key::KEY_F9),
                (enigo::Key::Home, evdev::Key::KEY_HOME),
                (enigo::Key::LeftArrow, evdev::Key::KEY_LEFT),
                (enigo::Key::Meta, evdev::Key::KEY_LEFTMETA),
                (enigo::Key::Option, evdev::Key::KEY_OPTION),
                (enigo::Key::PageDown, evdev::Key::KEY_PAGEDOWN),
                (enigo::Key::PageUp, evdev::Key::KEY_PAGEUP),
                (enigo::Key::Return, evdev::Key::KEY_ENTER),
                (enigo::Key::RightArrow, evdev::Key::KEY_RIGHT),
                (enigo::Key::Shift, evdev::Key::KEY_LEFTSHIFT),
                (enigo::Key::Space, evdev::Key::KEY_SPACE),
                (enigo::Key::Tab, evdev::Key::KEY_TAB),
                (enigo::Key::UpArrow, evdev::Key::KEY_UP),
                (enigo::Key::Numpad0, evdev::Key::KEY_KP0),  // check if correct?
                (enigo::Key::Numpad1, evdev::Key::KEY_KP1),
                (enigo::Key::Numpad2, evdev::Key::KEY_KP2),
                (enigo::Key::Numpad3, evdev::Key::KEY_KP3),
                (enigo::Key::Numpad4, evdev::Key::KEY_KP4),
                (enigo::Key::Numpad5, evdev::Key::KEY_KP5),
                (enigo::Key::Numpad6, evdev::Key::KEY_KP6),
                (enigo::Key::Numpad7, evdev::Key::KEY_KP7),
                (enigo::Key::Numpad8, evdev::Key::KEY_KP8),
                (enigo::Key::Numpad9, evdev::Key::KEY_KP9),
                (enigo::Key::Cancel, evdev::Key::KEY_CANCEL),
                (enigo::Key::Clear, evdev::Key::KEY_CLEAR),
                (enigo::Key::Alt, evdev::Key::KEY_LEFTALT),
                (enigo::Key::Pause, evdev::Key::KEY_PAUSE),
                (enigo::Key::Kana, evdev::Key::KEY_KATAKANA),  // check if correct?
                (enigo::Key::Hangul, evdev::Key::KEY_HANGEUL),  // check if correct?
                // (enigo::Key::Junja, evdev::Key::KEY_JUNJA),     // map?
                // (enigo::Key::Final, evdev::Key::KEY_FINAL),     // map?
                (enigo::Key::Hanja, evdev::Key::KEY_HANJA),
                // (enigo::Key::Kanji, evdev::Key::KEY_KANJI),      // map?
                // (enigo::Key::Convert, evdev::Key::KEY_CONVERT),
                (enigo::Key::Select, evdev::Key::KEY_SELECT),
                (enigo::Key::Print, evdev::Key::KEY_PRINT),
                // (enigo::Key::Execute, evdev::Key::KEY_EXECUTE),
                // (enigo::Key::Snapshot, evdev::Key::KEY_SNAPSHOT),
                (enigo::Key::Insert, evdev::Key::KEY_INSERT),
                (enigo::Key::Help, evdev::Key::KEY_HELP),
                (enigo::Key::Sleep, evdev::Key::KEY_SLEEP),
                // (enigo::Key::Separator, evdev::Key::KEY_SEPARATOR),
                (enigo::Key::Scroll, evdev::Key::KEY_SCROLLLOCK),
                (enigo::Key::NumLock, evdev::Key::KEY_NUMLOCK),
                (enigo::Key::RWin, evdev::Key::KEY_RIGHTMETA),
                (enigo::Key::Apps, evdev::Key::KEY_CONTEXT_MENU),
                (enigo::Key::Multiply, evdev::Key::KEY_KPASTERISK),
                (enigo::Key::Add, evdev::Key::KEY_KPPLUS),
                (enigo::Key::Subtract, evdev::Key::KEY_KPMINUS),
                (enigo::Key::Decimal, evdev::Key::KEY_KPCOMMA),   // KEY_KPDOT and KEY_KPCOMMA are exchanged?
                (enigo::Key::Divide, evdev::Key::KEY_KPSLASH),
                (enigo::Key::Equals, evdev::Key::KEY_KPEQUAL),
                (enigo::Key::NumpadEnter, evdev::Key::KEY_KPENTER),
                (enigo::Key::RightAlt, evdev::Key::KEY_RIGHTALT),
                (enigo::Key::RightControl, evdev::Key::KEY_RIGHTCTRL),
                (enigo::Key::RightShift, evdev::Key::KEY_RIGHTSHIFT),
            ]);

            static ref KEY_MAP_LAYOUT: HashMap<char, evdev::Key> = HashMap::from(
                [
                    ('a', evdev::Key::KEY_A),
                    ('b', evdev::Key::KEY_B),
                    ('c', evdev::Key::KEY_C),
                    ('d', evdev::Key::KEY_D),
                    ('e', evdev::Key::KEY_E),
                    ('f', evdev::Key::KEY_F),
                    ('g', evdev::Key::KEY_G),
                    ('h', evdev::Key::KEY_H),
                    ('i', evdev::Key::KEY_I),
                    ('j', evdev::Key::KEY_J),
                    ('k', evdev::Key::KEY_K),
                    ('l', evdev::Key::KEY_L),
                    ('m', evdev::Key::KEY_M),
                    ('n', evdev::Key::KEY_N),
                    ('o', evdev::Key::KEY_O),
                    ('p', evdev::Key::KEY_P),
                    ('q', evdev::Key::KEY_Q),
                    ('r', evdev::Key::KEY_R),
                    ('s', evdev::Key::KEY_S),
                    ('t', evdev::Key::KEY_T),
                    ('u', evdev::Key::KEY_U),
                    ('v', evdev::Key::KEY_V),
                    ('w', evdev::Key::KEY_W),
                    ('x', evdev::Key::KEY_X),
                    ('y', evdev::Key::KEY_Y),
                    ('z', evdev::Key::KEY_Z),
                    ('0', evdev::Key::KEY_0),
                    ('1', evdev::Key::KEY_1),
                    ('2', evdev::Key::KEY_2),
                    ('3', evdev::Key::KEY_3),
                    ('4', evdev::Key::KEY_4),
                    ('5', evdev::Key::KEY_5),
                    ('6', evdev::Key::KEY_6),
                    ('7', evdev::Key::KEY_7),
                    ('8', evdev::Key::KEY_8),
                    ('9', evdev::Key::KEY_9),
                    ('`', evdev::Key::KEY_GRAVE),
                    ('-', evdev::Key::KEY_MINUS),
                    ('=', evdev::Key::KEY_EQUAL),
                    ('[', evdev::Key::KEY_LEFTBRACE),
                    (']', evdev::Key::KEY_RIGHTBRACE),
                    ('\\', evdev::Key::KEY_BACKSLASH),
                    (',', evdev::Key::KEY_COMMA),
                    ('.', evdev::Key::KEY_DOT),
                    ('/', evdev::Key::KEY_SLASH),
                    (';', evdev::Key::KEY_SEMICOLON),
                    ('\'', evdev::Key::KEY_APOSTROPHE),
                ]);

            // ((minx, maxx), (miny, maxy))
            static ref RESOLUTION: Mutex<((i32, i32), (i32, i32))> = Mutex::new(((0, 0), (0, 0)));
        }

        fn create_uinput_keyboard() -> ResultType<VirtualDevice> {
            // TODO: ensure keys here
            let mut keys = AttributeSet::<evdev::Key>::new();
            for i in evdev::Key::KEY_ESC.code()..(evdev::Key::BTN_TRIGGER_HAPPY40.code() + 1) {
                let key = evdev::Key::new(i);
                if !format!("{:?}", &key).contains("unknown key") {
                    keys.insert(key);
                }
            }
            let mut leds = AttributeSet::<evdev::LedType>::new();
            leds.insert(evdev::LedType::LED_NUML);
            leds.insert(evdev::LedType::LED_CAPSL);
            leds.insert(evdev::LedType::LED_SCROLLL);
            let mut miscs = AttributeSet::<evdev::MiscType>::new();
            miscs.insert(evdev::MiscType::MSC_SCAN);
            let keyboard = VirtualDeviceBuilder::new()?
                .name("RustDesk UInput Keyboard")
                .with_keys(&keys)?
                .with_leds(&leds)?
                .with_miscs(&miscs)?
                .build()?;
            Ok(keyboard)
        }

        fn map_key(key: &enigo::Key) -> ResultType<evdev::Key> {
            if let Some(k) = KEY_MAP.get(&key) {
                log::trace!("mapkey {:?}, get {:?}", &key, &k);
                return Ok(k.clone());
            } else {
                match key {
                    enigo::Key::Layout(c) => {
                        if let Some(k) = KEY_MAP_LAYOUT.get(&c) {
                            log::trace!("mapkey {:?}, get {:?}", &key, k);
                            return Ok(k.clone());
                        }
                    }
                    // enigo::Key::Raw(c) => {
                    //     let k = evdev::Key::new(c);
                    //     if !format!("{:?}", &k).contains("unknown key") {
                    //         return Ok(k.clone());
                    //     }
                    // }
                    _ => {}
                }
            }
            bail!("Failed to map key {:?}", &key);
        }

        async fn ipc_send_data(stream: &mut Connection, data: &Data) {
            allow_err!(stream.send(data).await);
        }

        async fn handle_keyboard(
            stream: &mut Connection,
            keyboard: &mut VirtualDevice,
            data: &DataKeyboard,
        ) {
            log::trace!("handle_keyboard {:?}", &data);
            match data {
                DataKeyboard::Sequence(_seq) => {
                    // ignore
                }
                DataKeyboard::KeyDown(key) => {
                    if let Ok(k) = map_key(key) {
                        let down_event = InputEvent::new(EventType::KEY, k.code(), 1);
                        allow_err!(keyboard.emit(&[down_event]));
                    }
                }
                DataKeyboard::KeyUp(key) => {
                    if let Ok(k) = map_key(key) {
                        let up_event = InputEvent::new(EventType::KEY, k.code(), 0);
                        allow_err!(keyboard.emit(&[up_event]));
                    }
                }
                DataKeyboard::KeyClick(key) => {
                    if let Ok(k) = map_key(key) {
                        let down_event = InputEvent::new(EventType::KEY, k.code(), 1);
                        let up_event = InputEvent::new(EventType::KEY, k.code(), 0);
                        allow_err!(keyboard.emit(&[down_event, up_event]));
                    }
                }
                DataKeyboard::GetKeyState(key) => {
                    let key_state = if enigo::Key::CapsLock == *key {
                        match keyboard.get_led_state() {
                            Ok(leds) => leds.contains(evdev::LedType::LED_CAPSL),
                            Err(_e) => {
                                // log::debug!("Failed to get led state {}", &_e);
                                false
                            }
                        }
                    } else {
                        match keyboard.get_key_state() {
                            Ok(keys) => match key {
                                enigo::Key::Shift => {
                                    keys.contains(evdev::Key::KEY_LEFTSHIFT)
                                        || keys.contains(evdev::Key::KEY_RIGHTSHIFT)
                                }
                                enigo::Key::Control => {
                                    keys.contains(evdev::Key::KEY_LEFTCTRL)
                                        || keys.contains(evdev::Key::KEY_RIGHTCTRL)
                                }
                                enigo::Key::Alt => {
                                    keys.contains(evdev::Key::KEY_LEFTALT)
                                        || keys.contains(evdev::Key::KEY_RIGHTALT)
                                }
                                enigo::Key::NumLock => keys.contains(evdev::Key::KEY_NUMLOCK),
                                enigo::Key::Meta => {
                                    keys.contains(evdev::Key::KEY_LEFTMETA)
                                        || keys.contains(evdev::Key::KEY_RIGHTMETA)
                                }
                                _ => false,
                            },
                            Err(_e) => {
                                // log::debug!("Failed to get key state: {}", &_e);
                                false
                            }
                        }
                    };
                    ipc_send_data(
                        stream,
                        &Data::KeyboardResponse(ipc::DataKeyboardResponse::GetKeyState(key_state)),
                    )
                    .await;
                }
            }
        }

        fn handle_mouse(mouse: &mut mouce::nix::UInputMouseManager, data: &DataMouse) {
            log::trace!("handle_mouse {:?}", &data);
            match data {
                DataMouse::MoveTo(x, y) => {
                    allow_err!(mouse.move_to(*x as _, *y as _))
                }
                DataMouse::MoveRelative(x, y) => {
                    allow_err!(mouse.move_relative(*x, *y))
                }
                DataMouse::Down(button) => {
                    let btn = match button {
                        enigo::MouseButton::Left => mouce::common::MouseButton::Left,
                        enigo::MouseButton::Middle => mouce::common::MouseButton::Middle,
                        enigo::MouseButton::Right => mouce::common::MouseButton::Right,
                        _ => {
                            return;
                        }
                    };
                    allow_err!(mouse.press_button(&btn))
                }
                DataMouse::Up(button) => {
                    let btn = match button {
                        enigo::MouseButton::Left => mouce::common::MouseButton::Left,
                        enigo::MouseButton::Middle => mouce::common::MouseButton::Middle,
                        enigo::MouseButton::Right => mouce::common::MouseButton::Right,
                        _ => {
                            return;
                        }
                    };
                    allow_err!(mouse.release_button(&btn))
                }
                DataMouse::Click(button) => {
                    let btn = match button {
                        enigo::MouseButton::Left => mouce::common::MouseButton::Left,
                        enigo::MouseButton::Middle => mouce::common::MouseButton::Middle,
                        enigo::MouseButton::Right => mouce::common::MouseButton::Right,
                        _ => {
                            return;
                        }
                    };
                    allow_err!(mouse.click_button(&btn))
                }
                DataMouse::ScrollX(_length) => {
                    // TODO: not supported for now
                }
                DataMouse::ScrollY(length) => {
                    let mut length = *length;

                    let scroll = if length < 0 {
                        mouce::common::ScrollDirection::Up
                    } else {
                        mouce::common::ScrollDirection::Down
                    };

                    if length < 0 {
                        length = -length;
                    }

                    for _ in 0..length {
                        allow_err!(mouse.scroll_wheel(&scroll))
                    }
                }
            }
        }

        fn spawn_keyboard_handler(mut stream: Connection) {
            tokio::spawn(async move {
                let mut keyboard = match create_uinput_keyboard() {
                    Ok(keyboard) => keyboard,
                    Err(e) => {
                        log::error!("Failed to create keyboard {}", e);
                        return;
                    }
                };
                loop {
                    tokio::select! {
                        res = stream.next() => {
                            match res {
                                Err(err) => {
                                    log::info!("UInput keyboard ipc connection closed: {}", err);
                                    break;
                                }
                                Ok(Some(data)) => {
                                    match data {
                                        Data::Keyboard(data) => {
                                            handle_keyboard(&mut stream, &mut keyboard, &data).await;
                                        }
                                        _ => {
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            });
        }

        fn spawn_mouse_handler(mut stream: ipc::Connection) {
            let resolution = RESOLUTION.lock().unwrap();
            if resolution.0 .0 == resolution.0 .1 || resolution.1 .0 == resolution.1 .1 {
                return;
            }
            let rng_x = resolution.0.clone();
            let rng_y = resolution.1.clone();
            tokio::spawn(async move {
                log::info!(
                    "Create uinput mouce with rng_x: ({}, {}), rng_y: ({}, {})",
                    rng_x.0,
                    rng_x.1,
                    rng_y.0,
                    rng_y.1
                );
                let mut mouse = match mouce::Mouse::new_uinput(rng_x, rng_y) {
                    Ok(mouse) => mouse,
                    Err(e) => {
                        log::error!("Failed to create mouse, {}", e);
                        return;
                    }
                };
                loop {
                    tokio::select! {
                        res = stream.next() => {
                            match res {
                                Err(err) => {
                                    log::info!("UInput mouse ipc connection closed: {}", err);
                                    break;
                                }
                                Ok(Some(data)) => {
                                    match data {
                                        Data::Mouse(data) => {
                                            handle_mouse(&mut mouse, &data);
                                        }
                                        _ => {
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            });
        }

        fn spawn_controller_handler(mut stream: ipc::Connection) {
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        res = stream.next() => {
                            match res {
                                Err(_err) => {
                                    // log::info!("UInput controller ipc connection closed: {}", err);
                                    break;
                                }
                                Ok(Some(data)) => {
                                    match data {
                                        Data::Control(data) => match data {
                                            ipc::DataControl::Resolution{
                                                minx,
                                                maxx,
                                                miny,
                                                maxy,
                                            } => {
                                                *RESOLUTION.lock().unwrap() = ((minx, maxx), (miny, maxy));
                                                allow_err!(stream.send(&Data::Empty).await);
                                            }
                                        }
                                        _ => {
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            });
        }

        /// Start uinput service.
        async fn start_service<F: FnOnce(ipc::Connection) + Copy>(postfix: &str, handler: F) {
            match new_listener(postfix).await {
                Ok(mut incoming) => {
                    while let Some(result) = incoming.next().await {
                        match result {
                            Ok(stream) => {
                                log::debug!("Got new connection of uinput ipc {}", postfix);
                                handler(Connection::new(stream));
                            }
                            Err(err) => {
                                log::error!("Couldn't get uinput mouse client: {:?}", err);
                            }
                        }
                    }
                }
                Err(err) => {
                    log::error!("Failed to start uinput mouse ipc service: {}", err);
                }
            }
        }

        /// Start uinput keyboard service.
        #[tokio::main(flavor = "current_thread")]
        pub async fn start_service_keyboard() {
            log::info!("start uinput keyboard service");
            start_service(IPC_POSTFIX_KEYBOARD, spawn_keyboard_handler).await;
        }

        /// Start uinput mouse service.
        #[tokio::main(flavor = "current_thread")]
        pub async fn start_service_mouse() {
            log::info!("start uinput mouse service");
            start_service(IPC_POSTFIX_MOUSE, spawn_mouse_handler).await;
        }

        /// Start uinput mouse service.
        #[tokio::main(flavor = "current_thread")]
        pub async fn start_service_control() {
            log::info!("start uinput control service");
            start_service(IPC_POSTFIX_CONTROL, spawn_controller_handler).await;
        }

        pub fn stop_service_keyboard() {
            log::info!("stop uinput keyboard service");
        }
        pub fn stop_service_mouse() {
            log::info!("stop uinput mouse service");
        }
        pub fn stop_service_control() {
            log::info!("stop uinput control service");
        }
    }
}
