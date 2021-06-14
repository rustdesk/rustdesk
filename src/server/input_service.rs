use super::*;
#[cfg(target_os = "macos")]
use dispatch::Queue;
use enigo::{Enigo, KeyboardControllable, MouseButton, MouseControllable};
use hbb_common::config::COMPRESS_LEVEL;
use std::convert::TryFrom;

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

static mut LATEST_INPUT: Input = Input { conn: 0, time: 0 };

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
        if let Some(message::Union::cursor_data(cd)) = &msg.union {
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
            let exclude = unsafe {
                if crate::get_time() - LATEST_INPUT.time < 300 {
                    LATEST_INPUT.conn
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
    static ref ENIGO: Arc<Mutex<Enigo>> = Arc::new(Mutex::new(Enigo::new()));
}

// mac key input must be run in main thread, otherwise crash on >= osx 10.15
#[cfg(target_os = "macos")]
lazy_static::lazy_static! {
    static ref QUEUE: Queue = Queue::main();
    static ref IS_SERVER: bool =  std::env::args().nth(1) == Some("--server".to_owned());
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

#[cfg(not(target_os = "macos"))]
#[inline]
fn get_modifier_state(key: enigo::Key, en: &mut Enigo) -> bool {
    let x = en.get_key_state(key.clone());
    match key {
        enigo::Key::Shift => x || en.get_key_state(enigo::Key::RightShift),
        enigo::Key::Control => x || en.get_key_state(enigo::Key::RightControl),
        enigo::Key::Alt => x || en.get_key_state(enigo::Key::RightAlt),
        enigo::Key::Meta => x || en.get_key_state(enigo::Key::RWin),
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

fn handle_mouse_(evt: &MouseEvent, conn: i32) {
    #[cfg(windows)]
    crate::platform::windows::try_change_desktop();
    let buttons = evt.mask >> 3;
    let evt_type = evt.mask & 0x7;
    if evt_type == 0 {
        unsafe {
            let time = crate::get_time();
            LATEST_INPUT = Input { time, conn };
        }
    }
    let mut en = ENIGO.lock().unwrap();
    #[cfg(not(target_os = "macos"))]
    let mut to_release = Vec::new();
    #[cfg(target_os = "macos")]
    en.reset_flag();
    for ref ck in evt.modifiers.iter() {
        if let Some(key) = KEY_MAP.get(&ck.value()) {
            if evt_type == 1 || evt_type == 2 {
                #[cfg(target_os = "macos")]
                en.add_flag(key);
                #[cfg(not(target_os = "macos"))]
                if key != &enigo::Key::CapsLock && key != &enigo::Key::NumLock {
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
    if let Some(key_event::Union::control_key(ck)) = evt.union {
        if ck.value() == ControlKey::Return.value() || ck.value() == ControlKey::NumpadEnter.value()
        {
            return true;
        }
    }
    return false;
}

lazy_static::lazy_static! {
    static ref KEY_MAP: HashMap<i32, enigo::Key> =
    [
        (ControlKey::Alt, enigo::Key::Alt),
        (ControlKey::Backspace, enigo::Key::Backspace),
        (ControlKey::CapsLock, enigo::Key::CapsLock),
        (ControlKey::Control, enigo::Key::Control),
        (ControlKey::Delete, enigo::Key::Delete),
        (ControlKey::DownArrow, enigo::Key::DownArrow),
        (ControlKey::End, enigo::Key::End),
        (ControlKey::Escape, enigo::Key::Escape),
        (ControlKey::F1, enigo::Key::F1),
        (ControlKey::F10, enigo::Key::F10),
        (ControlKey::F11, enigo::Key::F11),
        (ControlKey::F12, enigo::Key::F12),
        (ControlKey::F2, enigo::Key::F2),
        (ControlKey::F3, enigo::Key::F3),
        (ControlKey::F4, enigo::Key::F4),
        (ControlKey::F5, enigo::Key::F5),
        (ControlKey::F6, enigo::Key::F6),
        (ControlKey::F7, enigo::Key::F7),
        (ControlKey::F8, enigo::Key::F8),
        (ControlKey::F9, enigo::Key::F9),
        (ControlKey::Home, enigo::Key::Home),
        (ControlKey::LeftArrow, enigo::Key::LeftArrow),
        (ControlKey::Meta, enigo::Key::Meta),
        (ControlKey::Option, enigo::Key::Option),
        (ControlKey::PageDown, enigo::Key::PageDown),
        (ControlKey::PageUp, enigo::Key::PageUp),
        (ControlKey::Return, enigo::Key::Return),
        (ControlKey::RightArrow, enigo::Key::RightArrow),
        (ControlKey::Shift, enigo::Key::Shift),
        (ControlKey::Space, enigo::Key::Space),
        (ControlKey::Tab, enigo::Key::Tab),
        (ControlKey::UpArrow, enigo::Key::UpArrow),
        (ControlKey::Numpad0, enigo::Key::Numpad0),
        (ControlKey::Numpad1, enigo::Key::Numpad1),
        (ControlKey::Numpad2, enigo::Key::Numpad2),
        (ControlKey::Numpad3, enigo::Key::Numpad3),
        (ControlKey::Numpad4, enigo::Key::Numpad4),
        (ControlKey::Numpad5, enigo::Key::Numpad5),
        (ControlKey::Numpad6, enigo::Key::Numpad6),
        (ControlKey::Numpad7, enigo::Key::Numpad7),
        (ControlKey::Numpad8, enigo::Key::Numpad8),
        (ControlKey::Numpad9, enigo::Key::Numpad9),
        (ControlKey::Cancel, enigo::Key::Cancel),
        (ControlKey::Clear, enigo::Key::Clear),
        (ControlKey::Menu, enigo::Key::Alt),
        (ControlKey::Pause, enigo::Key::Pause),
        (ControlKey::Kana, enigo::Key::Kana),
        (ControlKey::Hangul, enigo::Key::Hangul),
        (ControlKey::Junja, enigo::Key::Junja),
        (ControlKey::Final, enigo::Key::Final),
        (ControlKey::Hanja, enigo::Key::Hanja),
        (ControlKey::Kanji, enigo::Key::Kanji),
        (ControlKey::Convert, enigo::Key::Convert),
        (ControlKey::Select, enigo::Key::Select),
        (ControlKey::Print, enigo::Key::Print),
        (ControlKey::Execute, enigo::Key::Execute),
        (ControlKey::Snapshot, enigo::Key::Snapshot),
        (ControlKey::Insert, enigo::Key::Insert),
        (ControlKey::Help, enigo::Key::Help),
        (ControlKey::Sleep, enigo::Key::Sleep),
        (ControlKey::Separator, enigo::Key::Separator),
        (ControlKey::Scroll, enigo::Key::Scroll),
        (ControlKey::NumLock, enigo::Key::NumLock),
        (ControlKey::RWin, enigo::Key::RWin),
        (ControlKey::Apps, enigo::Key::Apps),
        (ControlKey::Multiply, enigo::Key::Multiply),
        (ControlKey::Add, enigo::Key::Add),
        (ControlKey::Subtract, enigo::Key::Subtract),
        (ControlKey::Decimal, enigo::Key::Decimal),
        (ControlKey::Divide, enigo::Key::Divide),
        (ControlKey::Equals, enigo::Key::Equals),
        (ControlKey::NumpadEnter, enigo::Key::NumpadEnter),
        (ControlKey::RAlt, enigo::Key::RightAlt),
        (ControlKey::RWin, enigo::Key::RWin),
        (ControlKey::RControl, enigo::Key::RightControl),
        (ControlKey::RShift, enigo::Key::RightShift),
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
    for ref ck in evt.modifiers.iter() {
        if let Some(key) = KEY_MAP.get(&ck.value()) {
            #[cfg(target_os = "macos")]
            en.add_flag(key);
            #[cfg(not(target_os = "macos"))]
            {
                if key == &enigo::Key::CapsLock {
                    has_cap = true;
                } else if key == &enigo::Key::NumLock {
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
    #[cfg(not(target_os = "macos"))]
    if crate::common::valid_for_capslock(evt) {
        if has_cap != en.get_key_state(enigo::Key::CapsLock) {
            en.key_down(enigo::Key::CapsLock).ok();
            en.key_up(enigo::Key::CapsLock);
        }
    }
    #[cfg(windows)]
    if crate::common::valid_for_numlock(evt) {
        if has_numlock != en.get_key_state(enigo::Key::NumLock) {
            en.key_down(enigo::Key::NumLock).ok();
            en.key_up(enigo::Key::NumLock);
        }
    }
    match evt.union {
        Some(key_event::Union::control_key(ck)) => {
            if let Some(key) = KEY_MAP.get(&ck.value()) {
                #[cfg(windows)]
                if let Some(_) = NUMPAD_KEY_MAP.get(&ck.value()) {
                    disable_numlock = en.get_key_state(enigo::Key::NumLock);
                    if disable_numlock {
                        en.key_down(enigo::Key::NumLock).ok();
                        en.key_up(enigo::Key::NumLock);
                    }
                }
                if evt.down {
                    allow_err!(en.key_down(key.clone()));
                } else {
                    en.key_up(key.clone());
                }
            } else if ck.value() == ControlKey::CtrlAltDel.value() {
                // have to spawn new thread because send_sas is tokio_main, the caller can not be tokio_main.
                std::thread::spawn(|| {
                    allow_err!(send_sas());
                });
            } else if ck.value() == ControlKey::LockScreen.value() {
                crate::platform::lock_screen();
                super::video_service::switch_to_primary();
            }
        }
        Some(key_event::Union::chr(chr)) => {
            if evt.down {
                allow_err!(en.key_down(enigo::Key::Layout(chr as u8 as _)));
            } else {
                en.key_up(enigo::Key::Layout(chr as u8 as _));
            }
        }
        Some(key_event::Union::unicode(chr)) => {
            if let Ok(chr) = char::try_from(chr) {
                en.key_sequence(&chr.to_string());
            }
        }
        Some(key_event::Union::seq(ref seq)) => {
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
        en.key_down(enigo::Key::NumLock).ok();
        en.key_up(enigo::Key::NumLock);
    }
}

#[tokio::main(basic_scheduler)]
async fn send_sas() -> ResultType<()> {
    let mut stream = crate::ipc::connect(1000, crate::POSTFIX_SERVICE).await?;
    timeout(1000, stream.send(&crate::ipc::Data::SAS)).await??;
    Ok(())
}
