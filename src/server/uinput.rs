use crate::ipc::{self, new_listener, Connection, Data, DataKeyboard, DataMouse};
use enigo::{Key, KeyboardControllable, MouseButton, MouseControllable};
use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AttributeSet, EventType, InputEvent,
};
use hbb_common::{allow_err, bail, log, tokio::{self, runtime::Runtime}, ResultType};

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

            match self.rt.block_on(self.conn.next_timeout(IPC_REQUEST_TIMEOUT)) {
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
            DataKeyboard::KeyDown(enigo::Key::Raw(code)) => {
                let down_event = InputEvent::new(EventType::KEY, *code - 8, 1);
                allow_err!(keyboard.emit(&[down_event]));
            }
            DataKeyboard::KeyUp(enigo::Key::Raw(code)) => {
                let down_event = InputEvent::new(EventType::KEY, *code - 8, 0);
                allow_err!(keyboard.emit(&[down_event]));
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
            DataMouse::Refresh => {
                // unreachable!()
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
                                            mouse = match mouce::Mouse::new_uinput(rng_x, rng_y) {
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
