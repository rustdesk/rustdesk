use super::*;
#[cfg(target_os = "linux")]
use crate::common::IS_X11;
#[cfg(target_os = "macos")]
use dispatch::Queue;
use enigo::{Enigo, Key, KeyboardControllable, MouseButton, MouseControllable};
use hbb_common::{config::COMPRESS_LEVEL, get_time, protobuf::EnumOrUnknown};
use rdev::{self, EventType, Key as RdevKey, RawKey};
#[cfg(target_os = "macos")]
use rdev::{CGEventSourceStateID, CGEventTapLocation, VirtualInput};
use std::time::Duration;
use std::{
    convert::TryFrom,
    ops::Sub,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{self, Instant},
};

const INVALID_CURSOR_POS: i32 = i32::MIN;

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

struct StatePos {
    cursor_pos: (i32, i32),
}

impl Default for StatePos {
    fn default() -> Self {
        Self {
            cursor_pos: (INVALID_CURSOR_POS, INVALID_CURSOR_POS),
        }
    }
}

impl super::service::Reset for StatePos {
    fn reset(&mut self) {
        self.cursor_pos = (INVALID_CURSOR_POS, INVALID_CURSOR_POS);
    }
}

impl StatePos {
    #[inline]
    fn is_valid(&self) -> bool {
        self.cursor_pos.0 != INVALID_CURSOR_POS
    }

    #[inline]
    fn is_moved(&self, x: i32, y: i32) -> bool {
        self.is_valid() && (self.cursor_pos.0 != x || self.cursor_pos.1 != y)
    }
}

#[derive(Default, Clone, Copy)]
struct Input {
    conn: i32,
    time: i64,
    x: i32,
    y: i32,
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

#[inline]
fn update_last_cursor_pos(x: i32, y: i32) {
    let mut lock = LATEST_SYS_CURSOR_POS.lock().unwrap();
    if lock.1 .0 != x || lock.1 .1 != y {
        (lock.0, lock.1) = (Instant::now(), (x, y))
    }
}

fn run_pos(sp: GenericService, state: &mut StatePos) -> ResultType<()> {
    let (_, (x, y)) = *LATEST_SYS_CURSOR_POS.lock().unwrap();
    if x == INVALID_CURSOR_POS || y == INVALID_CURSOR_POS {
        return Ok(());
    }

    if state.is_moved(x, y) {
        let mut msg_out = Message::new();
        msg_out.set_cursor_position(CursorPosition {
            x,
            y,
            ..Default::default()
        });
        let exclude = {
            let now = get_time();
            let lock = LATEST_PEER_INPUT_CURSOR.lock().unwrap();
            if now - lock.time < 300 {
                lock.conn
            } else {
                0
            }
        };
        sp.send_without(msg_out, exclude);
    }
    state.cursor_pos = (x, y);

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

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
enum KeysDown {
    RdevKey(RawKey),
    EnigoKey(u64),
}

lazy_static::lazy_static! {
    static ref ENIGO: Arc<Mutex<Enigo>> = {
        Arc::new(Mutex::new(Enigo::new()))
    };
    static ref KEYS_DOWN: Arc<Mutex<HashMap<KeysDown, Instant>>> = Default::default();
    static ref LATEST_PEER_INPUT_CURSOR: Arc<Mutex<Input>> = Default::default();
    static ref LATEST_SYS_CURSOR_POS: Arc<Mutex<(Instant, (i32, i32))>> = Arc::new(Mutex::new((Instant::now().sub(MOUSE_MOVE_PROTECTION_TIMEOUT), (INVALID_CURSOR_POS, INVALID_CURSOR_POS))));
}
static EXITING: AtomicBool = AtomicBool::new(false);

const MOUSE_MOVE_PROTECTION_TIMEOUT: Duration = Duration::from_millis(1_000);
// Actual diff of (x,y) is (1,1) here. But 5 may be tolerant.
const MOUSE_ACTIVE_DISTANCE: i32 = 5;

static RECORD_CURSOR_POS_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn try_start_record_cursor_pos() {
    if RECORD_CURSOR_POS_RUNNING.load(Ordering::SeqCst) {
        return;
    }

    RECORD_CURSOR_POS_RUNNING.store(true, Ordering::SeqCst);
    thread::spawn(|| {
        let interval = time::Duration::from_millis(33);
        loop {
            if !RECORD_CURSOR_POS_RUNNING.load(Ordering::SeqCst) {
                break;
            }

            let now = time::Instant::now();
            if let Some((x, y)) = crate::get_cursor_pos() {
                update_last_cursor_pos(x, y);
            }
            let elapsed = now.elapsed();
            if elapsed < interval {
                thread::sleep(interval - elapsed);
            }
        }
        update_last_cursor_pos(INVALID_CURSOR_POS, INVALID_CURSOR_POS);
    });
}

pub fn try_stop_record_cursor_pos() {
    let count_lock = CONN_COUNT.lock().unwrap();
    if *count_lock > 0 {
        return;
    }
    RECORD_CURSOR_POS_RUNNING.store(false, Ordering::SeqCst);
}

// mac key input must be run in main thread, otherwise crash on >= osx 10.15
#[cfg(target_os = "macos")]
lazy_static::lazy_static! {
    static ref QUEUE: Queue = Queue::main();
    static ref IS_SERVER: bool =  std::env::args().nth(1) == Some("--server".to_owned());
}

#[cfg(target_os = "macos")]
static mut VIRTUAL_INPUT_MTX: Mutex<()> = Mutex::new(());
#[cfg(target_os = "macos")]
static mut VIRTUAL_INPUT: Option<VirtualInput> = None;

// First call set_uinput() will create keyboard and mouse clients.
// The clients are ipc connections that must live shorter than tokio runtime.
// Thus this function must not be called in a temporary runtime.
#[cfg(target_os = "linux")]
pub async fn setup_uinput(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
    // Keyboard and mouse both open /dev/uinput
    // TODO: Make sure there's no race
    set_uinput_resolution(minx, maxx, miny, maxy).await?;

    let keyboard = super::uinput::client::UInputKeyboard::new().await?;
    log::info!("UInput keyboard created");
    let mouse = super::uinput::client::UInputMouse::new().await?;
    log::info!("UInput mouse created");

    ENIGO
        .lock()
        .unwrap()
        .set_custom_keyboard(Box::new(keyboard));
    ENIGO.lock().unwrap().set_custom_mouse(Box::new(mouse));
    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn update_mouse_resolution(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
    set_uinput_resolution(minx, maxx, miny, maxy).await?;

    std::thread::spawn(|| {
        if let Some(mouse) = ENIGO.lock().unwrap().get_custom_mouse() {
            if let Some(mouse) = mouse
                .as_mut_any()
                .downcast_mut::<super::uinput::client::UInputMouse>()
            {
                allow_err!(mouse.send_refresh());
            } else {
                log::error!("failed downcast uinput mouse");
            }
        }
    });

    Ok(())
}

#[cfg(target_os = "linux")]
async fn set_uinput_resolution(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
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

#[cfg(windows)]
fn modifier_sleep() {
    // sleep for a while, this is only for keying in rdp in peer so far
    std::thread::sleep(std::time::Duration::from_nanos(1));
}

#[inline]
#[cfg(not(target_os = "macos"))]
fn is_pressed(key: &Key, en: &mut Enigo) -> bool {
    get_modifier_state(key.clone(), en)
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
    if !active_mouse_(conn) {
        return;
    }
    let evt_type = evt.mask & 0x7;
    if evt_type == 0 {
        let time = get_time();
        *LATEST_PEER_INPUT_CURSOR.lock().unwrap() = Input {
            time,
            conn,
            x: evt.x,
            y: evt.y,
        };
    }
    #[cfg(target_os = "macos")]
    if !*IS_SERVER {
        // having GUI, run main GUI thread, otherwise crash
        let evt = evt.clone();
        QUEUE.exec_async(move || handle_mouse_(&evt));
        return;
    }
    #[cfg(windows)]
    crate::portable_service::client::handle_mouse(evt);
    #[cfg(not(windows))]
    handle_mouse_(evt);
}

pub fn fix_key_down_timeout_loop() {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(10_000));
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

#[cfg(target_os = "linux")]
pub fn clear_remapped_keycode() {
    ENIGO.lock().unwrap().tfc_clear_remapped();
}

#[inline]
fn record_key_is_control_key(record_key: u64) -> bool {
    record_key < KEY_CHAR_START
}

#[inline]
fn record_key_is_chr(record_key: u64) -> bool {
    record_key < KEY_CHAR_START
}

#[inline]
fn record_key_to_key(record_key: u64) -> Option<Key> {
    if record_key_is_control_key(record_key) {
        control_key_value_to_key(record_key as _)
    } else if record_key_is_chr(record_key) {
        let chr: u32 = (record_key - KEY_CHAR_START) as _;
        Some(char_value_to_key(chr))
    } else {
        None
    }
}

#[inline]
fn release_record_key(record_key: KeysDown) {
    let func = move || match record_key {
        KeysDown::RdevKey(raw_key) => {
            simulate_(&EventType::KeyRelease(RdevKey::RawKey(raw_key)));
        }
        KeysDown::EnigoKey(key) => {
            if let Some(key) = record_key_to_key(key) {
                ENIGO.lock().unwrap().key_up(key);
                log::debug!("Fixed {:?} timeout", key);
            }
        }
    };

    #[cfg(target_os = "macos")]
    QUEUE.exec_async(func);
    #[cfg(not(target_os = "macos"))]
    func();
}

fn fix_key_down_timeout(force: bool) {
    let key_down = KEYS_DOWN.lock().unwrap();
    if key_down.is_empty() {
        return;
    }
    let cloned = (*key_down).clone();
    drop(key_down);

    for (record_key, time) in cloned.into_iter() {
        if force || time.elapsed().as_millis() >= 360_000 {
            record_pressed_key(record_key, false);
            release_record_key(record_key);
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

fn active_mouse_(conn: i32) -> bool {
    // out of time protection
    if LATEST_SYS_CURSOR_POS.lock().unwrap().0.elapsed() > MOUSE_MOVE_PROTECTION_TIMEOUT {
        return true;
    }

    // last conn input may be protected
    if LATEST_PEER_INPUT_CURSOR.lock().unwrap().conn != conn {
        return false;
    }

    let in_active_dist = |a: i32, b: i32| -> bool { (a - b).abs() < MOUSE_ACTIVE_DISTANCE };

    // Check if input is in valid range
    match crate::get_cursor_pos() {
        Some((x, y)) => {
            let (last_in_x, last_in_y) = {
                let lock = LATEST_PEER_INPUT_CURSOR.lock().unwrap();
                (lock.x, lock.y)
            };
            let mut can_active = in_active_dist(last_in_x, x) && in_active_dist(last_in_y, y);
            // The cursor may not have been moved to last input position if system is busy now.
            // While this is not a common case, we check it again after some time later.
            if !can_active {
                // 10 micros may be enough for system to move cursor.
                // We do not care about the situation which system is too slow(more than 10 micros is required).
                std::thread::sleep(std::time::Duration::from_micros(10));
                // Sleep here can also somehow suppress delay accumulation.
                if let Some((x2, y2)) = crate::get_cursor_pos() {
                    can_active = in_active_dist(last_in_x, x2) && in_active_dist(last_in_y, y2);
                }
            }
            if !can_active {
                let mut lock = LATEST_PEER_INPUT_CURSOR.lock().unwrap();
                lock.x = INVALID_CURSOR_POS / 2;
                lock.y = INVALID_CURSOR_POS / 2;
            }
            can_active
        }
        None => true,
    }
}

pub fn handle_mouse_(evt: &MouseEvent) {
    if EXITING.load(Ordering::SeqCst) {
        return;
    }

    #[cfg(windows)]
    crate::platform::windows::try_change_desktop();
    let buttons = evt.mask >> 3;
    let evt_type = evt.mask & 0x7;
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
                        #[cfg(windows)]
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
            0x01 => {
                allow_err!(en.mouse_down(MouseButton::Left));
            }
            0x02 => {
                allow_err!(en.mouse_down(MouseButton::Right));
            }
            0x04 => {
                allow_err!(en.mouse_down(MouseButton::Middle));
            }
            0x08 => {
                allow_err!(en.mouse_down(MouseButton::Back));
            }
            0x10 => {
                allow_err!(en.mouse_down(MouseButton::Forward));
            }
            _ => {}
        },
        2 => match buttons {
            0x01 => {
                en.mouse_up(MouseButton::Left);
            }
            0x02 => {
                en.mouse_up(MouseButton::Right);
            }
            0x04 => {
                en.mouse_up(MouseButton::Middle);
            }
            0x08 => {
                en.mouse_up(MouseButton::Back);
            }
            0x10 => {
                en.mouse_up(MouseButton::Forward);
            }
            _ => {}
        },
        3 | 4 => {
            #[allow(unused_mut)]
            let mut x = evt.x;
            #[allow(unused_mut)]
            let mut y = evt.y;
            #[cfg(not(windows))]
            {
                x = -x;
                y = -y;
            }
            #[cfg(target_os = "macos")]
            {
                // TODO: support track pad on win.
                let is_track_pad = evt
                    .modifiers
                    .contains(&EnumOrUnknown::new(ControlKey::Scroll));

                // fix shift + scroll(down/up)
                if !is_track_pad
                    && evt
                        .modifiers
                        .contains(&EnumOrUnknown::new(ControlKey::Shift))
                {
                    x = y;
                    y = 0;
                }

                if x != 0 {
                    en.mouse_scroll_x(x, is_track_pad);
                }
                if y != 0 {
                    en.mouse_scroll_y(y, is_track_pad);
                }
            }

            #[cfg(not(target_os = "macos"))]
            {
                if x != 0 {
                    en.mouse_scroll_x(x);
                }
                if y != 0 {
                    en.mouse_scroll_y(y);
                }
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

pub fn handle_key(evt: &KeyEvent) {
    #[cfg(target_os = "macos")]
    if !*IS_SERVER {
        // having GUI, run main GUI thread, otherwise crash
        let evt = evt.clone();
        QUEUE.exec_async(move || handle_key_(&evt));
        std::thread::sleep(Duration::from_millis(20));
        return;
    }
    #[cfg(windows)]
    crate::portable_service::client::handle_key(evt);
    #[cfg(not(windows))]
    handle_key_(evt);
    #[cfg(target_os = "macos")]
    std::thread::sleep(Duration::from_millis(20));
}

#[cfg(target_os = "macos")]
#[inline]
fn reset_input() {
    unsafe {
        let _lock = VIRTUAL_INPUT_MTX.lock();
        VIRTUAL_INPUT =
            VirtualInput::new(CGEventSourceStateID::Private, CGEventTapLocation::Session).ok();
    }
}

#[cfg(target_os = "macos")]
pub fn reset_input_ondisconn() {
    if !*IS_SERVER {
        QUEUE.exec_async(reset_input);
    } else {
        reset_input();
    }
}

fn sim_rdev_rawkey_position(code: u32, keydown: bool) {
    #[cfg(target_os = "windows")]
    let rawkey = RawKey::ScanCode(code);
    #[cfg(target_os = "linux")]
    let rawkey = RawKey::LinuxXorgKeycode(code);
    // // to-do: test android
    // #[cfg(target_os = "android")]
    // let rawkey = RawKey::LinuxConsoleKeycode(code);
    #[cfg(target_os = "macos")]
    let rawkey = RawKey::MacVirtualKeycode(code);

    // map mode(1): Send keycode according to the peer platform.
    record_pressed_key(KeysDown::RdevKey(rawkey), keydown);

    let event_type = if keydown {
        EventType::KeyPress(RdevKey::RawKey(rawkey))
    } else {
        EventType::KeyRelease(RdevKey::RawKey(rawkey))
    };
    simulate_(&event_type);
}

#[cfg(target_os = "windows")]
fn sim_rdev_rawkey_virtual(code: u32, keydown: bool) {
    let rawkey = RawKey::WinVirtualKeycode(code);
    record_pressed_key(KeysDown::RdevKey(rawkey), keydown);
    let event_type = if keydown {
        EventType::KeyPress(RdevKey::RawKey(rawkey))
    } else {
        EventType::KeyRelease(RdevKey::RawKey(rawkey))
    };
    simulate_(&event_type);
}

#[cfg(target_os = "macos")]
#[inline]
fn simulate_(event_type: &EventType) {
    unsafe {
        let _lock = VIRTUAL_INPUT_MTX.lock();
        if let Some(virtual_input) = &VIRTUAL_INPUT {
            let _ = virtual_input.simulate(&event_type);
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[inline]
fn simulate_(event_type: &EventType) {
    match rdev::simulate(&event_type) {
        Ok(()) => (),
        Err(_simulate_error) => {
            log::error!("Could not send {:?}", &event_type);
        }
    }
}

fn is_modifier_in_key_event(control_key: ControlKey, key_event: &KeyEvent) -> bool {
    key_event
        .modifiers
        .iter()
        .position(|&m| m == control_key.into())
        .is_some()
}

#[inline]
fn control_key_value_to_key(value: i32) -> Option<Key> {
    KEY_MAP.get(&value).and_then(|k| Some(*k))
}

#[inline]
fn char_value_to_key(value: u32) -> Key {
    Key::Layout(std::char::from_u32(value).unwrap_or('\0'))
}

fn is_not_same_status(client_locking: bool, remote_locking: bool) -> bool {
    client_locking != remote_locking
}

#[cfg(target_os = "windows")]
fn has_numpad_key(key_event: &KeyEvent) -> bool {
    key_event
        .modifiers
        .iter()
        .filter(|&&ck| NUMPAD_KEY_MAP.get(&ck.value()).is_some())
        .count()
        != 0
}

#[cfg(target_os = "windows")]
fn is_rdev_numpad_key(key_event: &KeyEvent) -> bool {
    let code = key_event.chr();
    let key = rdev::get_win_key(code, 0);
    match key {
        RdevKey::Home
        | RdevKey::UpArrow
        | RdevKey::PageUp
        | RdevKey::LeftArrow
        | RdevKey::RightArrow
        | RdevKey::End
        | RdevKey::DownArrow
        | RdevKey::PageDown
        | RdevKey::Insert
        | RdevKey::Delete => true,
        _ => false,
    }
}

#[cfg(target_os = "windows")]
fn is_numlock_disabled(key_event: &KeyEvent) -> bool {
    // disable numlock if press home etc when numlock is on,
    // because we will get numpad value (7,8,9 etc) if not
    match key_event.mode.unwrap() {
        KeyboardMode::Map => is_rdev_numpad_key(key_event),
        _ => has_numpad_key(key_event),
    }
}

fn click_capslock(en: &mut Enigo) {
    #[cfg(not(targe_os = "macos"))]
    en.key_click(enigo::Key::CapsLock);
    #[cfg(target_os = "macos")]
    let _ = en.key_down(enigo::Key::CapsLock);
}

fn click_numlock(_en: &mut Enigo) {
    // without numlock in macos
    #[cfg(not(target_os = "macos"))]
    _en.key_click(enigo::Key::NumLock);
}

fn sync_numlock_capslock_status(key_event: &KeyEvent) {
    let mut en = ENIGO.lock().unwrap();

    let client_caps_locking = is_modifier_in_key_event(ControlKey::CapsLock, key_event);
    let client_num_locking = is_modifier_in_key_event(ControlKey::NumLock, key_event);
    let remote_caps_locking = en.get_key_state(enigo::Key::CapsLock);
    let remote_num_locking = en.get_key_state(enigo::Key::NumLock);

    let need_click_capslock = is_not_same_status(client_caps_locking, remote_caps_locking);
    let need_click_numlock = is_not_same_status(client_num_locking, remote_num_locking);

    #[cfg(not(target_os = "windows"))]
    let disable_numlock = false;
    #[cfg(target_os = "windows")]
    let disable_numlock = is_numlock_disabled(key_event);

    if need_click_capslock {
        click_capslock(&mut en);
    }

    if need_click_numlock && !disable_numlock {
        click_numlock(&mut en);
    }
}

fn map_keyboard_mode(evt: &KeyEvent) {
    #[cfg(windows)]
    crate::platform::windows::try_change_desktop();

    // Wayland
    #[cfg(target_os = "linux")]
    if !*IS_X11 {
        let mut en = ENIGO.lock().unwrap();
        let code = evt.chr() as u16;

        if evt.down {
            en.key_down(enigo::Key::Raw(code)).ok();
        } else {
            en.key_up(enigo::Key::Raw(code));
        }
        return;
    }

    sim_rdev_rawkey_position(evt.chr(), evt.down);
}

#[cfg(target_os = "macos")]
fn add_flags_to_enigo(en: &mut Enigo, key_event: &KeyEvent) {
    // When long-pressed the command key, then press and release
    // the Tab key, there should be CGEventFlagCommand in the flag.
    en.reset_flag();
    for ck in key_event.modifiers.iter() {
        if let Some(key) = KEY_MAP.get(&ck.value()) {
            en.add_flag(key);
        }
    }
}

fn get_control_key_value(key_event: &KeyEvent) -> i32 {
    if let Some(key_event::Union::ControlKey(ck)) = key_event.union {
        ck.value()
    } else {
        -1
    }
}

fn release_unpressed_modifiers(en: &mut Enigo, key_event: &KeyEvent) {
    let ck_value = get_control_key_value(key_event);
    fix_modifiers(&key_event.modifiers[..], en, ck_value);
}

#[cfg(target_os = "linux")]
fn is_altgr_pressed() -> bool {
    let altgr_rawkey = RawKey::LinuxXorgKeycode(ControlKey::RAlt.value() as _);
    KEYS_DOWN
        .lock()
        .unwrap()
        .get(&KeysDown::RdevKey(altgr_rawkey))
        .is_some()
}

#[cfg(not(target_os = "macos"))]
fn press_modifiers(en: &mut Enigo, key_event: &KeyEvent, to_release: &mut Vec<Key>) {
    for ref ck in key_event.modifiers.iter() {
        if let Some(key) = control_key_value_to_key(ck.value()) {
            if !is_pressed(&key, en) {
                #[cfg(target_os = "linux")]
                if key == Key::Alt && is_altgr_pressed() {
                    continue;
                }
                en.key_down(key.clone()).ok();
                to_release.push(key.clone());
                #[cfg(windows)]
                modifier_sleep();
            }
        }
    }
}

fn sync_modifiers(en: &mut Enigo, key_event: &KeyEvent, _to_release: &mut Vec<Key>) {
    #[cfg(target_os = "macos")]
    add_flags_to_enigo(en, key_event);

    if key_event.down {
        release_unpressed_modifiers(en, key_event);
        #[cfg(not(target_os = "macos"))]
        press_modifiers(en, key_event, _to_release);
    }
}

fn process_control_key(en: &mut Enigo, ck: &EnumOrUnknown<ControlKey>, down: bool) {
    if let Some(key) = control_key_value_to_key(ck.value()) {
        if down {
            en.key_down(key).ok();
        } else {
            en.key_up(key);
        }
    }
}

#[inline]
fn need_to_uppercase(en: &mut Enigo) -> bool {
    get_modifier_state(Key::Shift, en) || get_modifier_state(Key::CapsLock, en)
}

fn process_chr(en: &mut Enigo, chr: u32, down: bool) {
    let key = char_value_to_key(chr);

    if down {
        if en.key_down(key).is_ok() {
        } else {
            if let Ok(chr) = char::try_from(chr) {
                let mut s = chr.to_string();
                if need_to_uppercase(en) {
                    s = s.to_uppercase();
                }
                en.key_sequence(&s);
            };
        }
    } else {
        en.key_up(key);
    }
}

fn process_unicode(en: &mut Enigo, chr: u32) {
    if let Ok(chr) = char::try_from(chr) {
        en.key_sequence(&chr.to_string());
    }
}

fn process_seq(en: &mut Enigo, sequence: &str) {
    en.key_sequence(&sequence);
}

#[cfg(not(target_os = "macos"))]
fn release_keys(en: &mut Enigo, to_release: &Vec<Key>) {
    for key in to_release {
        en.key_up(key.clone());
    }
}

fn record_pressed_key(record_key: KeysDown, down: bool) {
    let mut key_down = KEYS_DOWN.lock().unwrap();
    if down {
        key_down.insert(record_key, Instant::now());
    } else {
        key_down.remove(&record_key);
    }
}

fn is_function_key(ck: &EnumOrUnknown<ControlKey>) -> bool {
    let mut res = false;
    if ck.value() == ControlKey::CtrlAltDel.value() {
        // have to spawn new thread because send_sas is tokio_main, the caller can not be tokio_main.
        std::thread::spawn(|| {
            allow_err!(send_sas());
        });
        res = true;
    } else if ck.value() == ControlKey::LockScreen.value() {
        lock_screen_2();
        res = true;
    }
    return res;
}

fn legacy_keyboard_mode(evt: &KeyEvent) {
    #[cfg(windows)]
    crate::platform::windows::try_change_desktop();
    let mut to_release: Vec<Key> = Vec::new();

    let mut en = ENIGO.lock().unwrap();
    sync_modifiers(&mut en, &evt, &mut to_release);

    let down = evt.down;
    match evt.union {
        Some(key_event::Union::ControlKey(ck)) => {
            if is_function_key(&ck) {
                return;
            }
            let record_key = ck.value() as u64;
            record_pressed_key(KeysDown::EnigoKey(record_key), down);
            process_control_key(&mut en, &ck, down)
        }
        Some(key_event::Union::Chr(chr)) => {
            let record_key = chr as u64 + KEY_CHAR_START;
            record_pressed_key(KeysDown::EnigoKey(record_key), down);
            process_chr(&mut en, chr, down)
        }
        Some(key_event::Union::Unicode(chr)) => process_unicode(&mut en, chr),
        Some(key_event::Union::Seq(ref seq)) => process_seq(&mut en, seq),
        _ => {}
    }

    #[cfg(not(target_os = "macos"))]
    release_keys(&mut en, &to_release);
}

#[cfg(target_os = "windows")]
fn translate_process_code(code: u32, down: bool) {
    crate::platform::windows::try_change_desktop();
    match code >> 16 {
        0 => sim_rdev_rawkey_position(code, down),
        vk_code => sim_rdev_rawkey_virtual(vk_code, down),
    };
}

fn translate_keyboard_mode(evt: &KeyEvent) {
    match &evt.union {
        Some(key_event::Union::Seq(seq)) => {
            // Fr -> US
            // client: Shift + & => 1(send to remote)
            // remote: Shift + 1 => !
            //
            // Try to release shift first.
            // remote: Shift + 1 => 1
            let mut en = ENIGO.lock().unwrap();

            #[cfg(target_os = "linux")]
            {
                simulate_(&EventType::KeyRelease(RdevKey::ShiftLeft));
                simulate_(&EventType::KeyRelease(RdevKey::ShiftRight));
                for chr in seq.chars() {
                    en.key_click(Key::Layout(chr));
                }
            }
            #[cfg(not(target_os = "linux"))]
            en.key_sequence(seq);
        }
        Some(key_event::Union::Chr(..)) => {
            #[cfg(target_os = "windows")]
            translate_process_code(evt.chr(), evt.down);
            #[cfg(not(target_os = "windows"))]
            sim_rdev_rawkey_position(evt.chr(), evt.down);
        }
        Some(key_event::Union::Unicode(..)) => {
            // Do not handle unicode for now.
        }
        _ => {
            log::debug!("Unreachable. Unexpected key event {:?}", &evt);
        }
    }
}

pub fn handle_key_(evt: &KeyEvent) {
    if EXITING.load(Ordering::SeqCst) {
        return;
    }

    if evt.down {
        sync_numlock_capslock_status(evt)
    }
    match evt.mode.unwrap() {
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

lazy_static::lazy_static! {
    static ref MODIFIER_MAP: HashMap<i32, Key> = [
        (ControlKey::Alt, Key::Alt),
        (ControlKey::RAlt, Key::RightAlt),
        (ControlKey::Control, Key::Control),
        (ControlKey::RControl, Key::RightControl),
        (ControlKey::Shift, Key::Shift),
        (ControlKey::RShift, Key::RightShift),
        (ControlKey::Meta, Key::Meta),
        (ControlKey::RWin, Key::RWin),
    ].iter().map(|(a, b)| (a.value(), b.clone())).collect();
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
