use super::*;
#[cfg(target_os = "macos")]
use dispatch::Queue;
use enigo::{Enigo, Key, KeyboardControllable, MouseButton, MouseControllable};
use crate::common::IS_X11;
use hbb_common::{config::COMPRESS_LEVEL, protobuf::EnumOrUnknown};
use rdev::{simulate, EventType, Key as RdevKey};
use std::{
    convert::TryFrom,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tfc::{traits::*, Context as TFC_Context, Key as TFC_Key};

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
                data.colors =
                    hbb_common::compress::compress(&data.colors[..], COMPRESS_LEVEL).into();
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
        Arc::new(Mutex::new(Enigo::new()))
    };
    static ref KEYS_DOWN: Arc<Mutex<HashMap<u64, Instant>>> = Default::default();
    static ref LATEST_INPUT: Arc<Mutex<Input>> = Default::default();
    static ref TFC_CONTEXT: Mutex<TFC_Context> = Mutex::new(TFC_Context::new().expect("kbd context error"));
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
    let keyboard = super::uinput::client::UInputKeyboard::new().await?;
    log::info!("UInput keyboard created");
    let mouse = super::uinput::client::UInputMouse::new().await?;
    log::info!("UInput mouse created");

    let mut en = ENIGO.lock().unwrap();
    en.set_uinput_keyboard(Some(Box::new(keyboard)));
    en.set_uinput_mouse(Some(Box::new(mouse)));
    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn set_uinput_resolution(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
    super::uinput::client::set_resolution(minx, maxx, miny, maxy).await
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

            key_event.set_chr('l' as _);
            key_event.modifiers.push(ControlKey::Meta.into());
            key_event.mode = KeyboardMode::Legacy.into();

            key_event.down = true;
            handle_key(&key_event);

            key_event.down = false;
            handle_key(&key_event);
        });
    } else if #[cfg(target_os = "macos")] {
        // CGSession -suspend not real lock screen, it is user switch
        std::thread::spawn(|| {
            let mut key_event = KeyEvent::new();

            key_event.set_chr('q' as _);
            key_event.modifiers.push(ControlKey::Meta.into());
            key_event.modifiers.push(ControlKey::Control.into());
            key_event.mode = KeyboardMode::Legacy.into();

            key_event.down = true;
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

fn rdev_key_down_or_up(key: RdevKey, down_or_up: bool) {
    let event_type = match down_or_up {
        true => EventType::KeyPress(key),
        false => EventType::KeyRelease(key),
    };
    let delay = std::time::Duration::from_millis(20);
    match simulate(&event_type) {
        Ok(()) => (),
        Err(_simulate_error) => {
            log::error!("Could not send {:?}", &event_type);
        }
    }
    // Let ths OS catchup (at least MacOS)
    std::thread::sleep(delay);
}

fn rdev_key_click(key: RdevKey) {
    rdev_key_down_or_up(key, true);
    rdev_key_down_or_up(key, false);
}

fn sync_status(evt: &KeyEvent) -> (bool, bool) {
    let mut en = ENIGO.lock().unwrap();

    // remote caps status
    let caps_locking = evt
        .modifiers
        .iter()
        .position(|&r| r == ControlKey::CapsLock.into())
        .is_some();
    // remote numpad status
    let num_locking = evt
        .modifiers
        .iter()
        .position(|&r| r == ControlKey::NumLock.into())
        .is_some();

    let click_capslock = (caps_locking && !en.get_key_state(enigo::Key::CapsLock))
        || (!caps_locking && en.get_key_state(enigo::Key::CapsLock));
    let click_numlock = (num_locking && !en.get_key_state(enigo::Key::NumLock))
        || (!num_locking && en.get_key_state(enigo::Key::NumLock));
    return (click_capslock, click_numlock);
}

fn map_keyboard_mode(evt: &KeyEvent) {
    // map mode(1): Send keycode according to the peer platform.
    let (click_capslock, click_numlock) = sync_status(evt);

    // Wayland
    if !*IS_X11.lock().unwrap() {
        let mut en = ENIGO.lock().unwrap();
        let code = evt.chr() as u16;

        #[cfg(not(target_os = "macos"))]
        if click_capslock {
            en.key_click(enigo::Key::CapsLock);
        }
        #[cfg(not(target_os = "macos"))]
        if click_numlock {
            en.key_click(enigo::Key::NumLock);
        }
        #[cfg(target_os = "macos")]
        en.key_down(enigo::Key::CapsLock);

        if evt.down {
            en.key_down(enigo::Key::Raw(code)).ok();
        } else {
            en.key_up(enigo::Key::Raw(code));
        }
        return;
    }

    #[cfg(not(target_os = "macos"))]
    if click_capslock {
        rdev_key_click(RdevKey::CapsLock);
    }
    #[cfg(not(target_os = "macos"))]
    if click_numlock {
        rdev_key_click(RdevKey::NumLock);
    }
    #[cfg(target_os = "macos")]
    if evt.down && click_capslock {
        rdev_key_down_or_up(RdevKey::CapsLock, evt.down);
    }

    rdev_key_down_or_up(RdevKey::Unknown(evt.chr()), evt.down);
    return;
}

fn tfc_key_down_or_up(key: Key, down: bool, up: bool) {
    if let Key::Layout(chr) = key {
        log::info!("tfc_key_down_or_up :{:?}", chr);
        if down {
            if let Err(_) = TFC_CONTEXT.lock().unwrap().unicode_char_down(chr) {
                log::error!("Failed to press char {:?}", chr);
            };
        }
        if up {
            if let Err(_) = TFC_CONTEXT.lock().unwrap().unicode_char_down(chr) {
                log::error!("Failed to press char {:?}", chr);
            };
        }
        return;
    }

    let key = match key {
        Key::Alt => TFC_Key::Alt,
        Key::Backspace => TFC_Key::DeleteOrBackspace,
        Key::CapsLock => TFC_Key::CapsLock,
        Key::Control => TFC_Key::Control,
        Key::Delete => TFC_Key::ForwardDelete,
        Key::DownArrow => TFC_Key::DownArrow,
        Key::End => TFC_Key::End,
        Key::Escape => TFC_Key::Escape,
        Key::F1 => TFC_Key::F1,
        Key::F10 => TFC_Key::F10,
        Key::F11 => TFC_Key::F11,
        Key::F12 => TFC_Key::F12,
        Key::F2 => TFC_Key::F2,
        Key::F3 => TFC_Key::F3,
        Key::F4 => TFC_Key::F4,
        Key::F5 => TFC_Key::F5,
        Key::F6 => TFC_Key::F6,
        Key::F7 => TFC_Key::F7,
        Key::F8 => TFC_Key::F8,
        Key::F9 => TFC_Key::F9,
        Key::Home => TFC_Key::Home,
        Key::LeftArrow => TFC_Key::LeftArrow,
        Key::PageDown => TFC_Key::PageDown,
        Key::PageUp => TFC_Key::PageUp,
        Key::Return => TFC_Key::ReturnOrEnter,
        Key::RightArrow => TFC_Key::RightArrow,
        Key::Shift => TFC_Key::Shift,
        Key::Space => TFC_Key::Space,
        Key::Tab => TFC_Key::Tab,
        Key::UpArrow => TFC_Key::UpArrow,
        Key::Numpad0 => TFC_Key::N0,
        Key::Numpad1 => TFC_Key::N1,
        Key::Numpad2 => TFC_Key::N2,
        Key::Numpad3 => TFC_Key::N3,
        Key::Numpad4 => TFC_Key::N4,
        Key::Numpad5 => TFC_Key::N5,
        Key::Numpad6 => TFC_Key::N6,
        Key::Numpad7 => TFC_Key::N7,
        Key::Numpad8 => TFC_Key::N8,
        Key::Numpad9 => TFC_Key::N9,
        Key::Decimal => TFC_Key::NumpadDecimal,
        Key::Clear => TFC_Key::NumpadClear,
        Key::Pause => TFC_Key::PlayPause,
        Key::Print => TFC_Key::Print,
        Key::Snapshot => TFC_Key::PrintScreen,
        Key::Insert => TFC_Key::Insert,
        Key::Scroll => TFC_Key::ScrollLock,
        Key::NumLock => TFC_Key::NumLock,
        Key::RWin => TFC_Key::Meta,
        Key::Apps => TFC_Key::Apps,
        Key::Multiply => TFC_Key::NumpadMultiply,
        Key::Add => TFC_Key::NumpadPlus,
        Key::Subtract => TFC_Key::NumpadMinus,
        Key::Divide => TFC_Key::NumpadDivide,
        Key::Equals => TFC_Key::NumpadEquals,
        Key::NumpadEnter => TFC_Key::NumpadEnter,
        Key::RightShift => TFC_Key::RightShift,
        Key::RightControl => TFC_Key::RightControl,
        Key::RightAlt => TFC_Key::RightAlt,
        Key::Command | Key::Super | Key::Windows | Key::Meta => TFC_Key::Meta,
        _ => {
            return;
        }
    };

    log::info!("tfc_key_down_or_up: {:?}", key);
    if down {
        if let Err(_) = TFC_CONTEXT.lock().unwrap().key_down(key) {
            log::error!("Failed to press char {:?}", key);
        };
    }
    if up {
        if let Err(_) = TFC_CONTEXT.lock().unwrap().key_up(key) {
            log::error!("Failed to press char {:?}", key);
        };
    }
}

fn legacy_keyboard_mode(evt: &KeyEvent) {
    let (click_capslock, click_numlock) = sync_status(evt);

    #[cfg(windows)]
    crate::platform::windows::try_change_desktop();
    let mut en = ENIGO.lock().unwrap();
    if click_capslock {
        if *IS_X11.lock().unwrap() {
            tfc_key_down_or_up(Key::CapsLock, true, true);
        } else {
            en.key_click(Key::CapsLock);
        }
    }
    if click_numlock {
        if *IS_X11.lock().unwrap() {
            tfc_key_down_or_up(Key::NumLock, true, true);
        } else {
            en.key_click(Key::NumLock);
        }
    }
    // disable numlock if press home etc when numlock is on,
    // because we will get numpad value (7,8,9 etc) if not
    #[cfg(windows)]
    let mut disable_numlock = false;
    #[cfg(target_os = "macos")]
    en.reset_flag();
    // When long-pressed the command key, then press and release
    // the Tab key, there should be CGEventFlagCommand in the flag.
    #[cfg(target_os = "macos")]
    for ck in evt.modifiers.iter() {
        if let Some(key) = KEY_MAP.get(&ck.value()) {
            en.add_flag(key);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let mut to_release = Vec::new();
    
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
                #[cfg(not(target_os = "macos"))]
                if !get_modifier_state(key.clone(), &mut en) {
                    if *IS_X11.lock().unwrap() {
                        tfc_key_down_or_up(key.clone(), true, false);
                    } else {
                        en.key_down(key.clone()).ok();
                    }
                    modifier_sleep();
                    to_release.push(key);
                }
            }
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
                    if *IS_X11.lock().unwrap() {
                        tfc_key_down_or_up(key.clone(), true, false);
                    } else {
                        en.key_down(key.clone()).ok();
                    }
                    KEYS_DOWN
                        .lock()
                        .unwrap()
                        .insert(ck.value() as _, Instant::now());
                } else {
                    if *IS_X11.lock().unwrap() {
                        tfc_key_down_or_up(key.clone(), false, true);
                    } else {
                        en.key_up(key.clone());
                    }
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
                if *IS_X11.lock().unwrap() {
                    tfc_key_down_or_up(get_layout(chr), true, false);
                } else {
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
                }
                KEYS_DOWN
                    .lock()
                    .unwrap()
                    .insert(chr as u64 + KEY_CHAR_START, Instant::now());
            } else {
                if *IS_X11.lock().unwrap() {
                    tfc_key_down_or_up(get_layout(chr), false, true);
                } else {
                    en.key_up(get_layout(chr));
                }
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
        if *IS_X11.lock().unwrap() {
            tfc_key_down_or_up(key.clone(), false, true);
        } else {
            en.key_up(key.clone());
        }
    }
}

fn translate_keyboard_mode(evt: &KeyEvent) {
    let chr = char::from_u32(evt.chr()).unwrap_or_default();
    // down(true)->press && press(false)-> release
    if evt.down && !evt.press {
        TFC_CONTEXT
            .lock()
            .unwrap()
            .unicode_char(chr)
            .expect("unicode_char_down error");
    }
}

fn handle_key_(evt: &KeyEvent) {
    if EXITING.load(Ordering::SeqCst) {
        return;
    }

    match evt.mode.unwrap() {
        KeyboardMode::Legacy => {
            legacy_keyboard_mode(evt);
        }
        KeyboardMode::Map => {
            map_keyboard_mode(evt);
        }
        KeyboardMode::Translate => {
            translate_keyboard_mode(evt);
        }
        _ => {
            legacy_keyboard_mode(evt);
        }
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

#[cfg(test)]
mod test {
    use super::*;
    use rdev::{listen, Event, EventType, Key};
    use std::sync::mpsc;

    #[test]
    fn test_handle_key() {
        // listen
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            std::env::set_var("KEYBOARD_ONLY", "y");
            let func = move |event: Event| {
                tx.send(event).ok();
            };
            if let Err(error) = listen(func) {
                println!("Error: {:?}", error);
            }
        });
        // set key/char base on char
        let mut evt = KeyEvent::new();
        evt.set_chr(66);
        evt.mode = KeyboardMode::Legacy.into();

        evt.modifiers.push(ControlKey::CapsLock.into());

        // press
        evt.down = true;
        handle_key(&evt);
        if let Ok(listen_evt) = rx.recv() {
            assert_eq!(listen_evt.event_type, EventType::KeyPress(Key::Num1))
        }
        // release
        evt.down = false;
        handle_key(&evt);
        if let Ok(listen_evt) = rx.recv() {
            assert_eq!(listen_evt.event_type, EventType::KeyRelease(Key::Num1))
        }
    }
    #[test]
    fn test_get_key_state() {
        let mut en = ENIGO.lock().unwrap();
        println!(
            "[*] test_get_key_state: {:?}",
            en.get_key_state(enigo::Key::NumLock)
        );
    }
}
