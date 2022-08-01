use crate::Key;
use std::{io::prelude::*, sync::mpsc};

enum PyMsg {
    Char(char),
    Str(&'static str),
}

/// The main struct for handling the event emitting
pub(super) struct EnigoPynput {
    tx: mpsc::Sender<(PyMsg, bool)>,
}

impl Default for EnigoPynput {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        start_pynput_service(rx);
        Self { tx }
    }
}
impl EnigoPynput {
    pub(super) fn reset(&mut self) {
        self.tx.send((PyMsg::Char('\0'), true)).ok();
    }

    #[inline]
    pub(super) fn send_pynput(&mut self, key: &Key, is_press: bool) -> bool {
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
        log::info!("send pynput: {:?}", &s);
        return self.tx.send((PyMsg::Str(s), is_press)).is_ok();
    }
}

// impl MouseControllable for EnigoPynput {
//     fn mouse_move_to(&mut self, _x: i32, _y: i32) {
//         unimplemented!()
//     }
//     fn mouse_move_relative(&mut self, _x: i32, _y: i32) {
//         unimplemented!()
//     }
//     fn mouse_down(&mut self, _button: MouseButton) -> crate::ResultType {
//         unimplemented!()
//     }
//     fn mouse_up(&mut self, _button: MouseButton) {
//         unimplemented!()
//     }
//     fn mouse_click(&mut self, _button: MouseButton) {
//         unimplemented!()
//     }
//     fn mouse_scroll_x(&mut self, _length: i32) {
//         unimplemented!()
//     }
//     fn mouse_scroll_y(&mut self, _length: i32) {
//         unimplemented!()
//     }
// }

// impl KeyboardControllable for EnigoPynput {
//     fn get_key_state(&mut self, _key: Key) -> bool {
//         unimplemented!()
//     }

//     fn key_sequence(&mut self, _sequence: &str) {
//         unimplemented!()
//     }
//     fn key_down(&mut self, key: Key) -> crate::ResultType {
//         let _ = self.send_pynput(&key, true);
//         Ok(())
//     }
//     fn key_up(&mut self, key: Key) {
//         let _ = self.send_pynput(&key, false);
//     }
//     fn key_click(&mut self, _key: Key) {
//         unimplemented!()
//     }
// }

static mut PYNPUT_EXIT: bool = false;
static mut PYNPUT_REDAY: bool = false;
static IPC_FILE: &'static str = "/tmp/RustDesk/pynput_service";

fn start_pynput_service(rx: mpsc::Receiver<(PyMsg, bool)>) {
    let mut py = "./pynput_service.py".to_owned();
    if !std::path::Path::new(&py).exists() {
        py = "/usr/share/rustdesk/files/pynput_service.py".to_owned();
        if !std::path::Path::new(&py).exists() {
            py = "/usr/lib/rustdesk/pynput_service.py".to_owned();
            if !std::path::Path::new(&py).exists() {
                log::error!("{} not exits", py);
            }
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
                        "-E",
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
