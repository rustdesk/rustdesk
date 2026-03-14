use crate::ipc::{self, new_listener, Connection, Data, DataKeyboard, DataMouse};
use enigo::{Key, KeyboardControllable, MouseButton, MouseControllable};
use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AttributeSet, EventType, InputEvent,
};
use hbb_common::{
    allow_err, bail, log,
    tokio::{self, runtime::Runtime},
    ResultType,
};

static IPC_CONN_TIMEOUT: u64 = 1000;
static IPC_REQUEST_TIMEOUT: u64 = 1000;
static IPC_POSTFIX_KEYBOARD: &str = "_uinput_keyboard";
static IPC_POSTFIX_MOUSE: &str = "_uinput_mouse";
static IPC_POSTFIX_CONTROL: &str = "_uinput_control";

pub mod client {
    use super::*;

    pub struct UInputKeyboard {
        conn: Connection,
        rt: Runtime,
    }

    impl UInputKeyboard {
        pub async fn new() -> ResultType<Self> {
            let conn = ipc::connect(IPC_CONN_TIMEOUT, IPC_POSTFIX_KEYBOARD).await?;
            let rt = Runtime::new()?;
            Ok(Self { conn, rt })
        }

        fn send(&mut self, data: Data) -> ResultType<()> {
            self.rt.block_on(self.conn.send(&data))
        }

        fn send_get_key_state(&mut self, data: Data) -> ResultType<bool> {
            self.rt.block_on(self.conn.send(&data))?;

            match self
                .rt
                .block_on(self.conn.next_timeout(IPC_REQUEST_TIMEOUT))
            {
                Ok(Some(Data::KeyboardResponse(ipc::DataKeyboardResponse::GetKeyState(state)))) => {
                    Ok(state)
                }
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
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
            self
        }

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
            // Sequence events are normally handled in the --server process before reaching here.
            // Forward via IPC as a fallback — input_text_wayland can still handle ASCII chars
            // via keysym/uinput, though non-ASCII will be skipped (no clipboard in --service).
            log::debug!(
                "UInputKeyboard::key_sequence called (len={})",
                sequence.len()
            );
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
        rt: Runtime,
    }

    impl UInputMouse {
        pub async fn new() -> ResultType<Self> {
            let conn = ipc::connect(IPC_CONN_TIMEOUT, IPC_POSTFIX_MOUSE).await?;
            let rt = Runtime::new()?;
            Ok(Self { conn, rt })
        }

        fn send(&mut self, data: Data) -> ResultType<()> {
            self.rt.block_on(self.conn.send(&data))
        }

        pub fn send_refresh(&mut self) -> ResultType<()> {
            self.send(Data::Mouse(DataMouse::Refresh))
        }
    }

    impl MouseControllable for UInputMouse {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
            self
        }

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
    use scrap::wayland::{
        pipewire::RDP_SESSION_INFO, remote_desktop_portal::OrgFreedesktopPortalRemoteDesktop,
    };
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
            (enigo::Key::Snapshot, evdev::Key::KEY_SYSRQ),
            (enigo::Key::Insert, evdev::Key::KEY_INSERT),
            (enigo::Key::Help, evdev::Key::KEY_HELP),
            (enigo::Key::Sleep, evdev::Key::KEY_SLEEP),
            // (enigo::Key::Separator, evdev::Key::KEY_SEPARATOR),
            (enigo::Key::Scroll, evdev::Key::KEY_SCROLLLOCK),
            (enigo::Key::NumLock, evdev::Key::KEY_NUMLOCK),
            (enigo::Key::RWin, evdev::Key::KEY_RIGHTMETA),
            (enigo::Key::Apps, evdev::Key::KEY_COMPOSE),    // it's a little strange that the key is mapped to KEY_COMPOSE, not KEY_MENU
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

        static ref KEY_MAP_LAYOUT: HashMap<char, (evdev::Key, bool)> = HashMap::from(
            [
                ('a', (evdev::Key::KEY_A, false)),
                ('b', (evdev::Key::KEY_B, false)),
                ('c', (evdev::Key::KEY_C, false)),
                ('d', (evdev::Key::KEY_D, false)),
                ('e', (evdev::Key::KEY_E, false)),
                ('f', (evdev::Key::KEY_F, false)),
                ('g', (evdev::Key::KEY_G, false)),
                ('h', (evdev::Key::KEY_H, false)),
                ('i', (evdev::Key::KEY_I, false)),
                ('j', (evdev::Key::KEY_J, false)),
                ('k', (evdev::Key::KEY_K, false)),
                ('l', (evdev::Key::KEY_L, false)),
                ('m', (evdev::Key::KEY_M, false)),
                ('n', (evdev::Key::KEY_N, false)),
                ('o', (evdev::Key::KEY_O, false)),
                ('p', (evdev::Key::KEY_P, false)),
                ('q', (evdev::Key::KEY_Q, false)),
                ('r', (evdev::Key::KEY_R, false)),
                ('s', (evdev::Key::KEY_S, false)),
                ('t', (evdev::Key::KEY_T, false)),
                ('u', (evdev::Key::KEY_U, false)),
                ('v', (evdev::Key::KEY_V, false)),
                ('w', (evdev::Key::KEY_W, false)),
                ('x', (evdev::Key::KEY_X, false)),
                ('y', (evdev::Key::KEY_Y, false)),
                ('z', (evdev::Key::KEY_Z, false)),
                ('0', (evdev::Key::KEY_0, false)),
                ('1', (evdev::Key::KEY_1, false)),
                ('2', (evdev::Key::KEY_2, false)),
                ('3', (evdev::Key::KEY_3, false)),
                ('4', (evdev::Key::KEY_4, false)),
                ('5', (evdev::Key::KEY_5, false)),
                ('6', (evdev::Key::KEY_6, false)),
                ('7', (evdev::Key::KEY_7, false)),
                ('8', (evdev::Key::KEY_8, false)),
                ('9', (evdev::Key::KEY_9, false)),
                ('`', (evdev::Key::KEY_GRAVE, false)),
                ('-', (evdev::Key::KEY_MINUS, false)),
                ('=', (evdev::Key::KEY_EQUAL, false)),
                ('[', (evdev::Key::KEY_LEFTBRACE, false)),
                (']', (evdev::Key::KEY_RIGHTBRACE, false)),
                ('\\', (evdev::Key::KEY_BACKSLASH, false)),
                (',', (evdev::Key::KEY_COMMA, false)),
                ('.', (evdev::Key::KEY_DOT, false)),
                ('/', (evdev::Key::KEY_SLASH, false)),
                (';', (evdev::Key::KEY_SEMICOLON, false)),
                ('\'', (evdev::Key::KEY_APOSTROPHE, false)),
                // Space is intentionally in both KEY_MAP_LAYOUT (char-to-evdev for text input)
                // and KEY_MAP (Key::Space for key events). Both maps serve different lookup paths.
                (' ', (evdev::Key::KEY_SPACE, false)),

                // Shift + key
                ('A', (evdev::Key::KEY_A, true)),
                ('B', (evdev::Key::KEY_B, true)),
                ('C', (evdev::Key::KEY_C, true)),
                ('D', (evdev::Key::KEY_D, true)),
                ('E', (evdev::Key::KEY_E, true)),
                ('F', (evdev::Key::KEY_F, true)),
                ('G', (evdev::Key::KEY_G, true)),
                ('H', (evdev::Key::KEY_H, true)),
                ('I', (evdev::Key::KEY_I, true)),
                ('J', (evdev::Key::KEY_J, true)),
                ('K', (evdev::Key::KEY_K, true)),
                ('L', (evdev::Key::KEY_L, true)),
                ('M', (evdev::Key::KEY_M, true)),
                ('N', (evdev::Key::KEY_N, true)),
                ('O', (evdev::Key::KEY_O, true)),
                ('P', (evdev::Key::KEY_P, true)),
                ('Q', (evdev::Key::KEY_Q, true)),
                ('R', (evdev::Key::KEY_R, true)),
                ('S', (evdev::Key::KEY_S, true)),
                ('T', (evdev::Key::KEY_T, true)),
                ('U', (evdev::Key::KEY_U, true)),
                ('V', (evdev::Key::KEY_V, true)),
                ('W', (evdev::Key::KEY_W, true)),
                ('X', (evdev::Key::KEY_X, true)),
                ('Y', (evdev::Key::KEY_Y, true)),
                ('Z', (evdev::Key::KEY_Z, true)),
                (')', (evdev::Key::KEY_0, true)),
                ('!', (evdev::Key::KEY_1, true)),
                ('@', (evdev::Key::KEY_2, true)),
                ('#', (evdev::Key::KEY_3, true)),
                ('$', (evdev::Key::KEY_4, true)),
                ('%', (evdev::Key::KEY_5, true)),
                ('^', (evdev::Key::KEY_6, true)),
                ('&', (evdev::Key::KEY_7, true)),
                ('*', (evdev::Key::KEY_8, true)),
                ('(', (evdev::Key::KEY_9, true)),
                ('~', (evdev::Key::KEY_GRAVE, true)),
                ('_', (evdev::Key::KEY_MINUS, true)),
                ('+', (evdev::Key::KEY_EQUAL, true)),
                ('{', (evdev::Key::KEY_LEFTBRACE, true)),
                ('}', (evdev::Key::KEY_RIGHTBRACE, true)),
                ('|', (evdev::Key::KEY_BACKSLASH, true)),
                ('<', (evdev::Key::KEY_COMMA, true)),
                ('>', (evdev::Key::KEY_DOT, true)),
                ('?', (evdev::Key::KEY_SLASH, true)),
                (':', (evdev::Key::KEY_SEMICOLON, true)),
                ('"', (evdev::Key::KEY_APOSTROPHE, true)),
            ]);

        // ((minx, maxx), (miny, maxy))
        static ref RESOLUTION: Mutex<((i32, i32), (i32, i32))> = Mutex::new(((0, 0), (0, 0)));
    }

    /// Input text on Wayland using layout-independent methods.
    /// ASCII chars (0x20-0x7E): Portal keysym or uinput fallback
    /// Non-ASCII chars: skipped — this runs in the --service (root) process where clipboard
    /// operations are unreliable (typically no user session environment).
    /// Non-ASCII input is normally handled by the --server process via input_text_via_clipboard_server.
    fn input_text_wayland(text: &str, keyboard: &mut VirtualDevice) {
        let portal_info = {
            let session_info = RDP_SESSION_INFO.lock().unwrap();
            session_info
                .as_ref()
                .map(|info| (info.conn.clone(), info.session.clone()))
        };

        for c in text.chars() {
            let keysym = char_to_keysym(c);
            if can_input_via_keysym(c, keysym) {
                // Try Portal first — down+up on the same channel
                if let Some((ref conn, ref session)) = portal_info {
                    let portal = scrap::wayland::pipewire::get_portal(conn);
                    if portal
                        .notify_keyboard_keysym(session, HashMap::new(), keysym, 1)
                        .is_ok()
                    {
                        if let Err(e) =
                            portal.notify_keyboard_keysym(session, HashMap::new(), keysym, 0)
                        {
                            log::warn!(
                                "input_text_wayland: portal key-up failed for keysym {:#x}: {:?}",
                                keysym,
                                e
                            );
                        }
                        continue;
                    }
                }
                // Portal unavailable or failed, fallback to uinput (down+up together)
                let key = enigo::Key::Layout(c);
                if let Ok((evdev_key, is_shift)) = map_key(&key) {
                    let mut shift_pressed = false;
                    if is_shift {
                        let shift_down =
                            InputEvent::new(EventType::KEY, evdev::Key::KEY_LEFTSHIFT.code(), 1);
                        if keyboard.emit(&[shift_down]).is_ok() {
                            shift_pressed = true;
                        } else {
                            log::warn!("input_text_wayland: failed to press Shift for '{}'", c);
                        }
                    }
                    let key_down = InputEvent::new(EventType::KEY, evdev_key.code(), 1);
                    let key_up = InputEvent::new(EventType::KEY, evdev_key.code(), 0);
                    allow_err!(keyboard.emit(&[key_down, key_up]));
                    if shift_pressed {
                        let shift_up =
                            InputEvent::new(EventType::KEY, evdev::Key::KEY_LEFTSHIFT.code(), 0);
                        allow_err!(keyboard.emit(&[shift_up]));
                    }
                }
            } else {
                log::debug!("Skipping non-ASCII character in uinput service (no clipboard access)");
            }
        }
    }

    /// Send a single key down or up event for a Layout character.
    /// Used by KeyDown/KeyUp to maintain correct press/release semantics.
    /// `down`: true for key press, false for key release.
    fn input_char_wayland_key_event(chr: char, down: bool, keyboard: &mut VirtualDevice) {
        let keysym = char_to_keysym(chr);
        let portal_state: u32 = if down { 1 } else { 0 };

        if can_input_via_keysym(chr, keysym) {
            let portal_info = {
                let session_info = RDP_SESSION_INFO.lock().unwrap();
                session_info
                    .as_ref()
                    .map(|info| (info.conn.clone(), info.session.clone()))
            };
            if let Some((ref conn, ref session)) = portal_info {
                let portal = scrap::wayland::pipewire::get_portal(conn);
                if portal
                    .notify_keyboard_keysym(session, HashMap::new(), keysym, portal_state)
                    .is_ok()
                {
                    return;
                }
            }
            // Portal unavailable or failed, fallback to uinput
            let key = enigo::Key::Layout(chr);
            if let Ok((evdev_key, is_shift)) = map_key(&key) {
                if down {
                    // Press: Shift↓ (if needed) → Key↓
                    if is_shift {
                        let shift_down =
                            InputEvent::new(EventType::KEY, evdev::Key::KEY_LEFTSHIFT.code(), 1);
                        if let Err(e) = keyboard.emit(&[shift_down]) {
                            log::warn!("input_char_wayland_key_event: failed to press Shift for '{}': {:?}", chr, e);
                        }
                    }
                    let key_down = InputEvent::new(EventType::KEY, evdev_key.code(), 1);
                    allow_err!(keyboard.emit(&[key_down]));
                } else {
                    // Release: Key↑ → Shift↑ (if needed)
                    let key_up = InputEvent::new(EventType::KEY, evdev_key.code(), 0);
                    allow_err!(keyboard.emit(&[key_up]));
                    if is_shift {
                        let shift_up =
                            InputEvent::new(EventType::KEY, evdev::Key::KEY_LEFTSHIFT.code(), 0);
                        if let Err(e) = keyboard.emit(&[shift_up]) {
                            log::warn!("input_char_wayland_key_event: failed to release Shift for '{}': {:?}", chr, e);
                        }
                    }
                }
            }
        } else {
            // Non-ASCII: no reliable down/up semantics available.
            // Clipboard paste is atomic and handled elsewhere.
            log::debug!(
                "Skipping non-ASCII character key {} in uinput service",
                if down { "down" } else { "up" }
            );
        }
    }

    /// Check if character can be input via keysym (ASCII printable with valid keysym).
    #[inline]
    pub(crate) fn can_input_via_keysym(c: char, keysym: i32) -> bool {
        // ASCII printable: 0x20 (space) to 0x7E (tilde)
        (c as u32 >= 0x20 && c as u32 <= 0x7E) && keysym != 0
    }

    /// Convert a Unicode character to X11 keysym.
    pub(crate) fn char_to_keysym(c: char) -> i32 {
        let codepoint = c as u32;
        if codepoint == 0 {
            // Null character has no keysym
            0
        } else if (0x20..=0x7E).contains(&codepoint) {
            // ASCII printable (0x20-0x7E): keysym == Unicode codepoint
            codepoint as i32
        } else if (0xA0..=0xFF).contains(&codepoint) {
            // Latin-1 supplement (0xA0-0xFF): keysym == Unicode codepoint (per X11 keysym spec)
            codepoint as i32
        } else {
            // Everything else (control chars 0x01-0x1F, DEL 0x7F, and all other non-ASCII Unicode):
            // keysym = 0x01000000 | codepoint (X11 Unicode keysym encoding)
            (0x0100_0000 | codepoint) as i32
        }
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

    pub fn map_key(key: &enigo::Key) -> ResultType<(evdev::Key, bool)> {
        if let Some(k) = KEY_MAP.get(&key) {
            log::trace!("mapkey matched in KEY_MAP, evdev={:?}", &k);
            return Ok((k.clone(), false));
        } else {
            match key {
                enigo::Key::Layout(c) => {
                    if let Some((k, is_shift)) = KEY_MAP_LAYOUT.get(&c) {
                        log::trace!("mapkey Layout matched, evdev={:?}", k);
                        return Ok((k.clone(), is_shift.clone()));
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
        let data_desc = match data {
            DataKeyboard::Sequence(seq) => format!("Sequence(len={})", seq.len()),
            DataKeyboard::KeyDown(Key::Layout(_))
            | DataKeyboard::KeyUp(Key::Layout(_))
            | DataKeyboard::KeyClick(Key::Layout(_)) => "Layout(<redacted>)".to_string(),
            _ => format!("{:?}", data),
        };
        log::trace!("handle_keyboard received: {}", data_desc);
        match data {
            DataKeyboard::Sequence(seq) => {
                // Normally handled by --server process (input_text_via_clipboard_server).
                // Fallback: input_text_wayland handles ASCII via keysym/uinput;
                // non-ASCII will be skipped (no clipboard access in --service process).
                if !seq.is_empty() {
                    input_text_wayland(seq, keyboard);
                }
            }
            DataKeyboard::KeyDown(enigo::Key::Raw(code)) => {
                if *code < 8 {
                    log::error!("Invalid Raw keycode {} (must be >= 8 due to XKB offset), skipping", code);
                } else {
                    let down_event = InputEvent::new(EventType::KEY, *code - 8, 1);
                    allow_err!(keyboard.emit(&[down_event]));
                }
            }
            DataKeyboard::KeyUp(enigo::Key::Raw(code)) => {
                if *code < 8 {
                    log::error!("Invalid Raw keycode {} (must be >= 8 due to XKB offset), skipping", code);
                } else {
                    let up_event = InputEvent::new(EventType::KEY, *code - 8, 0);
                    allow_err!(keyboard.emit(&[up_event]));
                }
            }
            DataKeyboard::KeyDown(key) => {
                if let Key::Layout(chr) = key {
                    input_char_wayland_key_event(*chr, true, keyboard);
                } else {
                    if let Ok((k, _is_shift)) = map_key(key) {
                        let down_event = InputEvent::new(EventType::KEY, k.code(), 1);
                        allow_err!(keyboard.emit(&[down_event]));
                    }
                }
            }
            DataKeyboard::KeyUp(key) => {
                if let Key::Layout(chr) = key {
                    input_char_wayland_key_event(*chr, false, keyboard);
                } else {
                    if let Ok((k, _)) = map_key(key) {
                        let up_event = InputEvent::new(EventType::KEY, k.code(), 0);
                        allow_err!(keyboard.emit(&[up_event]));
                    }
                }
            }
            DataKeyboard::KeyClick(key) => {
                if let Key::Layout(chr) = key {
                    input_text_wayland(&chr.to_string(), keyboard);
                } else {
                    if let Ok((k, _is_shift)) = map_key(key) {
                        let down_event = InputEvent::new(EventType::KEY, k.code(), 1);
                        let up_event = InputEvent::new(EventType::KEY, k.code(), 0);
                        allow_err!(keyboard.emit(&[down_event, up_event]));
                    }
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
                } else if enigo::Key::NumLock == *key {
                    match keyboard.get_led_state() {
                        Ok(leds) => leds.contains(evdev::LedType::LED_NUML),
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

    fn handle_mouse(mouse: &mut mouce::UInputMouseManager, data: &DataMouse) {
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
                    enigo::MouseButton::Left => mouce::MouseButton::Left,
                    enigo::MouseButton::Middle => mouce::MouseButton::Middle,
                    enigo::MouseButton::Right => mouce::MouseButton::Right,
                    _ => {
                        return;
                    }
                };
                allow_err!(mouse.press_button(&btn))
            }
            DataMouse::Up(button) => {
                let btn = match button {
                    enigo::MouseButton::Left => mouce::MouseButton::Left,
                    enigo::MouseButton::Middle => mouce::MouseButton::Middle,
                    enigo::MouseButton::Right => mouce::MouseButton::Right,
                    _ => {
                        return;
                    }
                };
                allow_err!(mouse.release_button(&btn))
            }
            DataMouse::Click(button) => {
                let btn = match button {
                    enigo::MouseButton::Left => mouce::MouseButton::Left,
                    enigo::MouseButton::Middle => mouce::MouseButton::Middle,
                    enigo::MouseButton::Right => mouce::MouseButton::Right,
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
                    mouce::ScrollDirection::Up
                } else {
                    mouce::ScrollDirection::Down
                };

                if length < 0 {
                    length = -length;
                }

                for _ in 0..length {
                    allow_err!(mouse.scroll_wheel(&scroll))
                }
            }
            DataMouse::Refresh => {
                // unreachable!()
            }
        }
    }

    fn spawn_keyboard_handler(mut stream: Connection) {
        log::debug!("spawn_keyboard_handler: new keyboard handler connection");
        tokio::spawn(async move {
            let mut keyboard = match create_uinput_keyboard() {
                Ok(keyboard) => {
                    log::debug!("UInput keyboard device created successfully");
                    keyboard
                }
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
                                        log::warn!("Unexpected data type in keyboard handler");
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
            let mut mouse = match mouce::UInputMouseManager::new(rng_x, rng_y) {
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
                                        if let DataMouse::Refresh = data {
                                            let resolution = RESOLUTION.lock().unwrap();
                                            let rng_x = resolution.0.clone();
                                            let rng_y = resolution.1.clone();
                                            log::info!(
                                                "Refresh uinput mouce with rng_x: ({}, {}), rng_y: ({}, {})",
                                                rng_x.0,
                                                rng_x.1,
                                                rng_y.0,
                                                rng_y.1
                                            );
                                            mouse = match mouce::UInputMouseManager::new(rng_x, rng_y) {
                                                Ok(mouse) => mouse,
                                                Err(e) => {
                                                    log::error!("Failed to create mouse, {}", e);
                                                    return;
                                                }
                                            }
                                        } else {
                                            handle_mouse(&mut mouse, &data);
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

// https://github.com/emrebicer/mouce
mod mouce {
    use std::{
        fs::File,
        io::{Error, ErrorKind, Result},
        mem::size_of,
        os::{
            raw::{c_char, c_int, c_long, c_uint, c_ulong, c_ushort},
            unix::{fs::OpenOptionsExt, io::AsRawFd},
        },
        thread,
        time::Duration,
    };

    pub const O_NONBLOCK: c_int = 2048;

    /// ioctl and uinput definitions
    const UI_ABS_SETUP: c_ulong = 1075598596;
    const UI_SET_EVBIT: c_ulong = 1074025828;
    const UI_SET_KEYBIT: c_ulong = 1074025829;
    const UI_SET_RELBIT: c_ulong = 1074025830;
    const UI_SET_ABSBIT: c_ulong = 1074025831;
    const UI_DEV_SETUP: c_ulong = 1079792899;
    const UI_DEV_CREATE: c_ulong = 21761;
    const UI_DEV_DESTROY: c_uint = 21762;

    pub const EV_KEY: c_int = 0x01;
    pub const EV_REL: c_int = 0x02;
    pub const EV_ABS: c_int = 0x03;
    pub const REL_X: c_uint = 0x00;
    pub const REL_Y: c_uint = 0x01;
    pub const ABS_X: c_uint = 0x00;
    pub const ABS_Y: c_uint = 0x01;
    pub const REL_WHEEL: c_uint = 0x08;
    pub const REL_HWHEEL: c_uint = 0x06;
    pub const BTN_LEFT: c_int = 0x110;
    pub const BTN_RIGHT: c_int = 0x111;
    pub const BTN_MIDDLE: c_int = 0x112;
    pub const BTN_SIDE: c_int = 0x113;
    pub const BTN_EXTRA: c_int = 0x114;
    pub const BTN_FORWARD: c_int = 0x115;
    pub const BTN_BACK: c_int = 0x116;
    pub const BTN_TASK: c_int = 0x117;
    const SYN_REPORT: c_int = 0x00;
    const EV_SYN: c_int = 0x00;
    const BUS_USB: c_ushort = 0x03;

    /// uinput types
    #[repr(C)]
    struct UInputSetup {
        id: InputId,
        name: [c_char; UINPUT_MAX_NAME_SIZE],
        ff_effects_max: c_ulong,
    }

    #[repr(C)]
    struct InputId {
        bustype: c_ushort,
        vendor: c_ushort,
        product: c_ushort,
        version: c_ushort,
    }

    #[repr(C)]
    pub struct InputEvent {
        pub time: TimeVal,
        pub r#type: c_ushort,
        pub code: c_ushort,
        pub value: c_int,
    }

    #[repr(C)]
    pub struct TimeVal {
        pub tv_sec: c_ulong,
        pub tv_usec: c_ulong,
    }

    #[repr(C)]
    pub struct UinputAbsSetup {
        pub code: c_ushort,
        pub absinfo: InputAbsinfo,
    }

    #[repr(C)]
    pub struct InputAbsinfo {
        pub value: c_int,
        pub minimum: c_int,
        pub maximum: c_int,
        pub fuzz: c_int,
        pub flat: c_int,
        pub resolution: c_int,
    }

    extern "C" {
        fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
        fn write(fd: c_int, buf: *mut InputEvent, count: usize) -> c_long;
    }

    #[derive(Debug, Copy, Clone)]
    pub enum MouseButton {
        Left,
        Middle,
        Side,
        Extra,
        Right,
        Back,
        Forward,
        Task,
    }

    #[derive(Debug, Copy, Clone)]
    pub enum ScrollDirection {
        Up,
        Down,
        Right,
        Left,
    }

    const UINPUT_MAX_NAME_SIZE: usize = 80;

    pub struct UInputMouseManager {
        uinput_file: File,
    }

    impl UInputMouseManager {
        pub fn new(rng_x: (i32, i32), rng_y: (i32, i32)) -> Result<Self> {
            let manager = UInputMouseManager {
                uinput_file: File::options()
                    .write(true)
                    .custom_flags(O_NONBLOCK)
                    .open("/dev/uinput")?,
            };
            let fd = manager.uinput_file.as_raw_fd();
            unsafe {
                // For press events (also needed for mouse movement)
                ioctl(fd, UI_SET_EVBIT, EV_KEY);
                ioctl(fd, UI_SET_KEYBIT, BTN_LEFT);
                ioctl(fd, UI_SET_KEYBIT, BTN_RIGHT);
                ioctl(fd, UI_SET_KEYBIT, BTN_MIDDLE);

                // For mouse movement
                ioctl(fd, UI_SET_EVBIT, EV_ABS);
                ioctl(fd, UI_SET_ABSBIT, ABS_X);
                ioctl(
                    fd,
                    UI_ABS_SETUP,
                    &UinputAbsSetup {
                        code: ABS_X as _,
                        absinfo: InputAbsinfo {
                            value: 0,
                            minimum: rng_x.0,
                            maximum: rng_x.1,
                            fuzz: 0,
                            flat: 0,
                            resolution: 0,
                        },
                    },
                );
                ioctl(fd, UI_SET_ABSBIT, ABS_Y);
                ioctl(
                    fd,
                    UI_ABS_SETUP,
                    &UinputAbsSetup {
                        code: ABS_Y as _,
                        absinfo: InputAbsinfo {
                            value: 0,
                            minimum: rng_y.0,
                            maximum: rng_y.1,
                            fuzz: 0,
                            flat: 0,
                            resolution: 0,
                        },
                    },
                );

                ioctl(fd, UI_SET_EVBIT, EV_REL);
                ioctl(fd, UI_SET_RELBIT, REL_X);
                ioctl(fd, UI_SET_RELBIT, REL_Y);
                ioctl(fd, UI_SET_RELBIT, REL_WHEEL);
                ioctl(fd, UI_SET_RELBIT, REL_HWHEEL);
            }

            let mut usetup = UInputSetup {
                id: InputId {
                    bustype: BUS_USB,
                    // Random vendor and product
                    vendor: 0x2222,
                    product: 0x3333,
                    version: 0,
                },
                name: [0; UINPUT_MAX_NAME_SIZE],
                ff_effects_max: 0,
            };

            let mut device_bytes: Vec<c_char> = "mouce-library-fake-mouse"
                .chars()
                .map(|ch| ch as c_char)
                .collect();

            // Fill the rest of the name buffer with empty chars
            for _ in 0..UINPUT_MAX_NAME_SIZE - device_bytes.len() {
                device_bytes.push('\0' as c_char);
            }

            usetup.name.copy_from_slice(&device_bytes);

            unsafe {
                ioctl(fd, UI_DEV_SETUP, &usetup);
                ioctl(fd, UI_DEV_CREATE);
            }

            // On UI_DEV_CREATE the kernel will create the device node for this
            // device. We are inserting a pause here so that userspace has time
            // to detect, initialize the new device, and can start listening to
            // the event, otherwise it will not notice the event we are about to send.
            thread::sleep(Duration::from_millis(300));

            Ok(manager)
        }

        /// Write the given event to the uinput file
        fn emit(&self, r#type: c_int, code: c_int, value: c_int) -> Result<()> {
            let mut event = InputEvent {
                time: TimeVal {
                    tv_sec: 0,
                    tv_usec: 0,
                },
                r#type: r#type as c_ushort,
                code: code as c_ushort,
                value,
            };
            let fd = self.uinput_file.as_raw_fd();

            unsafe {
                let count = size_of::<InputEvent>();
                let written_bytes = write(fd, &mut event, count);
                if written_bytes == -1 || written_bytes != count as c_long {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!("failed while trying to write to a file"),
                    ));
                }
            }

            Ok(())
        }

        /// Syncronize the device
        fn syncronize(&self) -> Result<()> {
            self.emit(EV_SYN, SYN_REPORT, 0)?;
            // Give uinput some time to update the mouse location,
            // otherwise it fails to move the mouse on release mode
            // A delay of 1 milliseconds seems to be enough for it
            thread::sleep(Duration::from_millis(1));
            Ok(())
        }

        /// Move the mouse relative to the current position
        fn move_relative_(&self, x: i32, y: i32) -> Result<()> {
            // uinput does not move the mouse in pixels but uses `units`. I couldn't
            // find information regarding to this uinput `unit`, but according to
            // my findings 1 unit corresponds to exactly 2 pixels.
            //
            // To achieve the expected behavior; divide the parameters by 2
            //
            // This seems like there is a bug in this crate, but the
            // behavior is the same on other projects that make use of
            // uinput. e.g. `ydotool`. When you try to move your mouse,
            // it will move 2x further pixels
            self.emit(EV_REL, REL_X as c_int, (x as f32 / 2.).ceil() as c_int)?;
            self.emit(EV_REL, REL_Y as c_int, (y as f32 / 2.).ceil() as c_int)?;
            self.syncronize()
        }

        fn map_btn(button: &MouseButton) -> c_int {
            match button {
                MouseButton::Left => BTN_LEFT,
                MouseButton::Right => BTN_RIGHT,
                MouseButton::Middle => BTN_MIDDLE,
                MouseButton::Side => BTN_SIDE,
                MouseButton::Extra => BTN_EXTRA,
                MouseButton::Forward => BTN_FORWARD,
                MouseButton::Back => BTN_BACK,
                MouseButton::Task => BTN_TASK,
            }
        }

        pub fn move_to(&self, x: usize, y: usize) -> Result<()> {
            // // For some reason, absolute mouse move events are not working on uinput
            // // (as I understand those events are intended for touch events)
            // //
            // // As a work around solution; first set the mouse to top left, then
            // // call relative move function to simulate an absolute move event
            //self.move_relative(i32::MIN, i32::MIN)?;
            //self.move_relative(x as i32, y as i32)

            self.emit(EV_ABS, ABS_X as c_int, x as c_int)?;
            self.emit(EV_ABS, ABS_Y as c_int, y as c_int)?;
            self.syncronize()
        }

        pub fn move_relative(&self, x_offset: i32, y_offset: i32) -> Result<()> {
            self.move_relative_(x_offset, y_offset)
        }

        pub fn press_button(&self, button: &MouseButton) -> Result<()> {
            self.emit(EV_KEY, Self::map_btn(button), 1)?;
            self.syncronize()
        }

        pub fn release_button(&self, button: &MouseButton) -> Result<()> {
            self.emit(EV_KEY, Self::map_btn(button), 0)?;
            self.syncronize()
        }

        pub fn click_button(&self, button: &MouseButton) -> Result<()> {
            self.press_button(button)?;
            self.release_button(button)
        }

        pub fn scroll_wheel(&self, direction: &ScrollDirection) -> Result<()> {
            let (code, scroll_value) = match direction {
                ScrollDirection::Up => (REL_WHEEL, 1),
                ScrollDirection::Down => (REL_WHEEL, -1),
                ScrollDirection::Left => (REL_HWHEEL, -1),
                ScrollDirection::Right => (REL_HWHEEL, 1),
            };
            self.emit(EV_REL, code as c_int, scroll_value)?;
            self.syncronize()
        }
    }

    impl Drop for UInputMouseManager {
        fn drop(&mut self) {
            let fd = self.uinput_file.as_raw_fd();
            unsafe {
                // Destroy the device, the file is closed automatically by the File module
                ioctl(fd, UI_DEV_DESTROY as c_ulong);
            }
        }
    }
}
