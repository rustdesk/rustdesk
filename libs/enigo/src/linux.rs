use libc;

use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};

use self::libc::{c_char, c_int, c_void, useconds_t};
use std::{borrow::Cow, ffi::CString, io::prelude::*, ptr, sync::mpsc};

const CURRENT_WINDOW: c_int = 0;
const DEFAULT_DELAY: u64 = 12000;
type Window = c_int;
type Xdo = *const c_void;

#[link(name = "xdo")]
extern "C" {
    fn xdo_free(xdo: Xdo);
    fn xdo_new(display: *const c_char) -> Xdo;

    fn xdo_click_window(xdo: Xdo, window: Window, button: c_int) -> c_int;
    fn xdo_mouse_down(xdo: Xdo, window: Window, button: c_int) -> c_int;
    fn xdo_mouse_up(xdo: Xdo, window: Window, button: c_int) -> c_int;
    fn xdo_move_mouse(xdo: Xdo, x: c_int, y: c_int, screen: c_int) -> c_int;
    fn xdo_move_mouse_relative(xdo: Xdo, x: c_int, y: c_int) -> c_int;

    fn xdo_enter_text_window(
        xdo: Xdo,
        window: Window,
        string: *const c_char,
        delay: useconds_t,
    ) -> c_int;
    fn xdo_send_keysequence_window(
        xdo: Xdo,
        window: Window,
        string: *const c_char,
        delay: useconds_t,
    ) -> c_int;
    fn xdo_send_keysequence_window_down(
        xdo: Xdo,
        window: Window,
        string: *const c_char,
        delay: useconds_t,
    ) -> c_int;
    fn xdo_send_keysequence_window_up(
        xdo: Xdo,
        window: Window,
        string: *const c_char,
        delay: useconds_t,
    ) -> c_int;
    fn xdo_get_input_state(xdo: Xdo) -> u32;
}

fn mousebutton(button: MouseButton) -> c_int {
    match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
        MouseButton::ScrollUp => 4,
        MouseButton::ScrollDown => 5,
        MouseButton::ScrollLeft => 6,
        MouseButton::ScrollRight => 7,
    }
}

enum PyMsg {
    Char(char),
    Str(&'static str),
}

/// The main struct for handling the event emitting
pub struct Enigo {
    xdo: Xdo,
    delay: u64,
    tx: mpsc::Sender<(PyMsg, bool)>,
}
// This is safe, we have a unique pointer.
// TODO: use Unique<c_char> once stable.
unsafe impl Send for Enigo {}

impl Default for Enigo {
    /// Create a new Enigo instance
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        start_pynput_service(rx);
        Self {
            xdo: unsafe { xdo_new(ptr::null()) },
            delay: DEFAULT_DELAY,
            tx,
        }
    }
}
impl Enigo {
    /// Get the delay per keypress.
    /// Default value is 12000.
    /// This is Linux-specific.
    pub fn delay(&self) -> u64 {
        self.delay
    }
    /// Set the delay per keypress.
    /// This is Linux-specific.
    pub fn set_delay(&mut self, delay: u64) {
        self.delay = delay;
    }
    ///
    pub fn reset(&mut self) {
        self.tx.send((PyMsg::Char('\0'), true)).ok();
    }
    #[inline]
    fn send_pynput(&mut self, key: &Key, is_press: bool) -> bool {
        if unsafe { PYNPUT_EXIT || !PYNPUT_REDAY } {
            return false;
        }
        if let Key::Layout(c) = key {
            return self.tx.send((PyMsg::Char(*c), is_press)).is_ok();
        }
        if let Key::Raw(_) = key {
            return false;
        }
        #[allow(deprecated)]
        let s = match key {
            Key::Alt => "Alt_L",
            Key::Backspace => "BackSpace",
            Key::CapsLock => "Caps_Lock",
            Key::Control => "Control_L",
            Key::Delete => "Delete",
            Key::DownArrow => "Down",
            Key::End => "End",
            Key::Escape => "Escape",
            Key::F1 => "F1",
            Key::F10 => "F10",
            Key::F11 => "F11",
            Key::F12 => "F12",
            Key::F2 => "F2",
            Key::F3 => "F3",
            Key::F4 => "F4",
            Key::F5 => "F5",
            Key::F6 => "F6",
            Key::F7 => "F7",
            Key::F8 => "F8",
            Key::F9 => "F9",
            Key::Home => "Home",
            Key::LeftArrow => "Left",
            Key::Option => "Option",
            Key::PageDown => "Page_Down",
            Key::PageUp => "Page_Up",
            Key::Return => "Return",
            Key::RightArrow => "Right",
            Key::Shift => "Shift_L",
            Key::Space => "space",
            Key::Tab => "Tab",
            Key::UpArrow => "Up",
            Key::Numpad0 => "0",
            Key::Numpad1 => "1",
            Key::Numpad2 => "2",
            Key::Numpad3 => "3",
            Key::Numpad4 => "4",
            Key::Numpad5 => "5",
            Key::Numpad6 => "6",
            Key::Numpad7 => "7",
            Key::Numpad8 => "8",
            Key::Numpad9 => "9",
            Key::Decimal => "KP_Decimal",
            Key::Cancel => "Cancel",
            Key::Clear => "Clear",
            Key::Pause => "Pause",
            Key::Kana => "Kana",
            Key::Hangul => "Hangul",
            Key::Hanja => "Hanja",
            Key::Kanji => "Kanji",
            Key::Select => "Select",
            Key::Print => "Print",
            Key::Execute => "Execute",
            Key::Snapshot => "3270_PrintScreen",
            Key::Insert => "Insert",
            Key::Help => "Help",
            Key::Separator => "KP_Separator",
            Key::Scroll => "Scroll_Lock",
            Key::NumLock => "Num_Lock",
            Key::RWin => "Super_R",
            Key::Apps => "Menu",
            Key::Multiply => "KP_Multiply",
            Key::Add => "KP_Add",
            Key::Subtract => "KP_Subtract",
            Key::Divide => "KP_Divide",
            Key::Equals => "KP_Equal",
            Key::NumpadEnter => "KP_Enter",
            Key::RightShift => "Shift_R",
            Key::RightControl => "Control_R",
            Key::RightAlt => "Mode_switch",
            Key::Command | Key::Super | Key::Windows | Key::Meta => "Super_L",
            _ => {
                return true;
            }
        };
        return self.tx.send((PyMsg::Str(s), is_press)).is_ok();
    }
}
impl Drop for Enigo {
    fn drop(&mut self) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            xdo_free(self.xdo);
        }
    }
}
impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            xdo_move_mouse(self.xdo, x as c_int, y as c_int, 0);
        }
    }
    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            xdo_move_mouse_relative(self.xdo, x as c_int, y as c_int);
        }
    }
    fn mouse_down(&mut self, button: MouseButton) -> crate::ResultType {
        if self.xdo.is_null() {
            return Ok(());
        }
        unsafe {
            xdo_mouse_down(self.xdo, CURRENT_WINDOW, mousebutton(button));
        }
        Ok(())
    }
    fn mouse_up(&mut self, button: MouseButton) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            xdo_mouse_up(self.xdo, CURRENT_WINDOW, mousebutton(button));
        }
    }
    fn mouse_click(&mut self, button: MouseButton) {
        if self.xdo.is_null() {
            return;
        }
        unsafe {
            xdo_click_window(self.xdo, CURRENT_WINDOW, mousebutton(button));
        }
    }
    fn mouse_scroll_x(&mut self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = MouseButton::ScrollLeft;
        } else {
            button = MouseButton::ScrollRight;
        }

        if length < 0 {
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }
    fn mouse_scroll_y(&mut self, length: i32) {
        let button;
        let mut length = length;

        if length < 0 {
            button = MouseButton::ScrollUp;
        } else {
            button = MouseButton::ScrollDown;
        }

        if length < 0 {
            length = -length;
        }

        for _ in 0..length {
            self.mouse_click(button);
        }
    }
}
fn keysequence<'a>(key: Key) -> Cow<'a, str> {
    if let Key::Layout(c) = key {
        return Cow::Owned(format!("U{:X}", c as u32));
    }
    if let Key::Raw(k) = key {
        return Cow::Owned(format!("{}", k as u16));
    }
    #[allow(deprecated)]
    // I mean duh, we still need to support deprecated keys until they're removed
    // https://www.rubydoc.info/gems/xdo/XDo/Keyboard
    // https://gitlab.com/cunidev/gestures/-/wikis/xdotool-list-of-key-codes
    Cow::Borrowed(match key {
        Key::Alt => "Alt",
        Key::Backspace => "BackSpace",
        Key::CapsLock => "Caps_Lock",
        Key::Control => "Control",
        Key::Delete => "Delete",
        Key::DownArrow => "Down",
        Key::End => "End",
        Key::Escape => "Escape",
        Key::F1 => "F1",
        Key::F10 => "F10",
        Key::F11 => "F11",
        Key::F12 => "F12",
        Key::F2 => "F2",
        Key::F3 => "F3",
        Key::F4 => "F4",
        Key::F5 => "F5",
        Key::F6 => "F6",
        Key::F7 => "F7",
        Key::F8 => "F8",
        Key::F9 => "F9",
        Key::Home => "Home",
        //Key::Layout(_) => unreachable!(),
        Key::LeftArrow => "Left",
        Key::Option => "Option",
        Key::PageDown => "Page_Down",
        Key::PageUp => "Page_Up",
        //Key::Raw(_) => unreachable!(),
        Key::Return => "Return",
        Key::RightArrow => "Right",
        Key::Shift => "Shift",
        Key::Space => "space",
        Key::Tab => "Tab",
        Key::UpArrow => "Up",
        Key::Numpad0 => "U30", //"KP_0",
        Key::Numpad1 => "U31", //"KP_1",
        Key::Numpad2 => "U32", //"KP_2",
        Key::Numpad3 => "U33", //"KP_3",
        Key::Numpad4 => "U34", //"KP_4",
        Key::Numpad5 => "U35", //"KP_5",
        Key::Numpad6 => "U36", //"KP_6",
        Key::Numpad7 => "U37", //"KP_7",
        Key::Numpad8 => "U38", //"KP_8",
        Key::Numpad9 => "U39", //"KP_9",
        Key::Decimal => "U2E", //"KP_Decimal",
        Key::Cancel => "Cancel",
        Key::Clear => "Clear",
        Key::Pause => "Pause",
        Key::Kana => "Kana",
        Key::Hangul => "Hangul",
        Key::Junja => "",
        Key::Final => "",
        Key::Hanja => "Hanja",
        Key::Kanji => "Kanji",
        Key::Convert => "",
        Key::Select => "Select",
        Key::Print => "Print",
        Key::Execute => "Execute",
        Key::Snapshot => "3270_PrintScreen",
        Key::Insert => "Insert",
        Key::Help => "Help",
        Key::Sleep => "",
        Key::Separator => "KP_Separator",
        Key::VolumeUp => "",
        Key::VolumeDown => "",
        Key::Mute => "",
        Key::Scroll => "Scroll_Lock",
        Key::NumLock => "Num_Lock",
        Key::RWin => "Super_R",
        Key::Apps => "Menu",
        Key::Multiply => "KP_Multiply",
        Key::Add => "KP_Add",
        Key::Subtract => "KP_Subtract",
        Key::Divide => "KP_Divide",
        Key::Equals => "KP_Equal",
        Key::NumpadEnter => "KP_Enter",
        Key::RightShift => "Shift_R",
        Key::RightControl => "Control_R",
        Key::RightAlt => "Alt_R",

        Key::Command | Key::Super | Key::Windows | Key::Meta => "Super",

        _ => "",
    })
}
impl KeyboardControllable for Enigo {
    fn get_key_state(&mut self, key: Key) -> bool {
        if self.xdo.is_null() {
            return false;
        }
        /*
        // modifier keys mask
        pub const ShiftMask: c_uint = 0x01;
        pub const LockMask: c_uint = 0x02;
        pub const ControlMask: c_uint = 0x04;
        pub const Mod1Mask: c_uint = 0x08;
        pub const Mod2Mask: c_uint = 0x10;
        pub const Mod3Mask: c_uint = 0x20;
        pub const Mod4Mask: c_uint = 0x40;
        pub const Mod5Mask: c_uint = 0x80;
        */
        let mod_shift = 1 << 0;
        let mod_lock = 1 << 1;
        let mod_control = 1 << 2;
        let mod_alt = 1 << 3;
        let mod_numlock = 1 << 4;
        let mod_meta = 1 << 6;
        let mask = unsafe { xdo_get_input_state(self.xdo) };
        match key {
            Key::Shift => mask & mod_shift != 0,
            Key::CapsLock => mask & mod_lock != 0,
            Key::Control => mask & mod_control != 0,
            Key::Alt => mask & mod_alt != 0,
            Key::NumLock => mask & mod_numlock != 0,
            Key::Meta => mask & mod_meta != 0,
            _ => false,
        }
    }

    fn key_sequence(&mut self, sequence: &str) {
        if self.xdo.is_null() {
            return;
        }
        if let Ok(string) = CString::new(sequence) {
            unsafe {
                xdo_enter_text_window(
                    self.xdo,
                    CURRENT_WINDOW,
                    string.as_ptr(),
                    self.delay as useconds_t,
                );
            }
        }
    }
    fn key_down(&mut self, key: Key) -> crate::ResultType {
        if self.xdo.is_null() {
            return Ok(());
        }
        if self.send_pynput(&key, true) {
            return Ok(());
        }
        let string = CString::new(&*keysequence(key))?;
        unsafe {
            xdo_send_keysequence_window_down(
                self.xdo,
                CURRENT_WINDOW,
                string.as_ptr(),
                self.delay as useconds_t,
            );
        }
        Ok(())
    }
    fn key_up(&mut self, key: Key) {
        if self.xdo.is_null() {
            return;
        }
        if self.send_pynput(&key, false) {
            return;
        }
        if let Ok(string) = CString::new(&*keysequence(key)) {
            unsafe {
                xdo_send_keysequence_window_up(
                    self.xdo,
                    CURRENT_WINDOW,
                    string.as_ptr(),
                    self.delay as useconds_t,
                );
            }
        }
    }
    fn key_click(&mut self, key: Key) {
        if self.xdo.is_null() {
            return;
        }
        if let Ok(string) = CString::new(&*keysequence(key)) {
            unsafe {
                xdo_send_keysequence_window(
                    self.xdo,
                    CURRENT_WINDOW,
                    string.as_ptr(),
                    self.delay as useconds_t,
                );
            }
        }
    }
}

static mut PYNPUT_EXIT: bool = false;
static mut PYNPUT_REDAY: bool = false;
static IPC_FILE: &'static str = "/tmp/RustDesk/pynput_service";

fn start_pynput_service(rx: mpsc::Receiver<(PyMsg, bool)>) {
    let mut py = "./pynput_service.py".to_owned();
    if !std::path::Path::new(&py).exists() {
        py = "/usr/share/rustdesk/files/pynput_service.py".to_owned();
        if !std::path::Path::new(&py).exists() {
            log::error!("{} not exits", py);
        }
    }
    log::info!("pynput service: {}", py);
    std::thread::spawn(move || {
        let username = std::env::var("PYNPUT_USERNAME").unwrap_or("".to_owned());
        let userid = std::env::var("PYNPUT_USERID").unwrap_or("".to_owned());
        let status = if username.is_empty() {
            std::process::Command::new("python3")
                .arg(&py)
                .arg(IPC_FILE)
                .status()
                .map(|x| x.success())
        } else {
            let mut status = Ok(true);
            for i in 0..100 {
                if i % 10 == 0 {
                    log::info!("#{} try to start pynput server", i);
                }
                status = std::process::Command::new("sudo")
                    .args(vec![
                        &format!("XDG_RUNTIME_DIR=/run/user/{}", userid) as &str,
                        "-u",
                        &username,
                        "python3",
                        &py,
                        IPC_FILE,
                    ])
                    .status()
                    .map(|x| x.success());
                match status {
                    Ok(true) => break,
                    _ => {}
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            status
        };
        log::info!(
            "pynput server exit with username/id {}/{}: {:?}",
            username,
            userid,
            status
        );
        unsafe {
            PYNPUT_EXIT = true;
        }
    });
    std::thread::spawn(move || {
        for i in 0..300 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let mut conn = match std::os::unix::net::UnixStream::connect(IPC_FILE) {
                Ok(conn) => conn,
                Err(err) => {
                    if i % 15 == 0 {
                        log::warn!("Failed to connect to {}: {}", IPC_FILE, err);
                    }
                    continue;
                }
            };
            if let Err(err) = conn.set_nonblocking(true) {
                log::error!("Failed to set ipc nonblocking: {}", err);
                return;
            }
            log::info!("Conntected to pynput server");
            let d = std::time::Duration::from_millis(30);
            unsafe {
                PYNPUT_REDAY = true;
            }
            let mut buf = [0u8; 1024];
            loop {
                if unsafe { PYNPUT_EXIT } {
                    break;
                }
                match rx.recv_timeout(d) {
                    Ok((msg, is_press)) => {
                        let msg = match msg {
                            PyMsg::Char(chr) => {
                                format!("{}{}", if is_press { 'p' } else { 'r' }, chr)
                            }
                            PyMsg::Str(s) => format!("{}{}", if is_press { 'p' } else { 'r' }, s),
                        };
                        let n = msg.len();
                        buf[0] = n as _;
                        buf[1..(n + 1)].copy_from_slice(msg.as_bytes());
                        if let Err(err) = conn.write_all(&buf[..n + 1]) {
                            log::error!("Failed to write to ipc: {}", err);
                            break;
                        }
                    }
                    Err(err) => match err {
                        mpsc::RecvTimeoutError::Disconnected => {
                            log::error!("pynput sender disconnecte");
                            break;
                        }
                        _ => {}
                    },
                }
            }
            unsafe {
                PYNPUT_REDAY = false;
            }
            break;
        }
    });
}
