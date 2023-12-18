use crate::uinput::service::map_key;
use dbus::{blocking::SyncConnection, Path};
use enigo::{Key, KeyboardControllable, MouseButton, MouseControllable};
use hbb_common::ResultType;
use scrap::wayland::pipewire::{get_portal, PwStreamInfo};
use scrap::wayland::remote_desktop_portal::OrgFreedesktopPortalRemoteDesktop as remote_desktop_portal;
use std::collections::HashMap;
use std::sync::Arc;

pub mod client {
    use super::*;

    const EVDEV_MOUSE_LEFT: i32 = 272;
    const EVDEV_MOUSE_RIGHT: i32 = 273;
    const EVDEV_MOUSE_MIDDLE: i32 = 274;

    const PRESSED_DOWN_STATE: u32 = 1;
    const PRESSED_UP_STATE: u32 = 0;

    pub struct RdpInputKeyboard {
        conn: Arc<SyncConnection>,
        session: Path<'static>,
    }

    impl RdpInputKeyboard {
        pub fn new(conn: Arc<SyncConnection>, session: Path<'static>) -> ResultType<Self> {
            Ok(Self { conn, session })
        }
    }

    impl KeyboardControllable for RdpInputKeyboard {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn get_key_state(&mut self, _: Key) -> bool {
            // no api for this
            false
        }

        fn key_sequence(&mut self, s: &str) {
            for c in s.chars() {
                let key = Key::Layout(c);
                let _ = handle_key(true, key, self.conn.clone(), &self.session);
                let _ = handle_key(false, key, self.conn.clone(), &self.session);
            }
        }

        fn key_down(&mut self, key: Key) -> enigo::ResultType {
            handle_key(true, key, self.conn.clone(), &self.session)?;
            Ok(())
        }
        fn key_up(&mut self, key: Key) {
            let _ = handle_key(false, key, self.conn.clone(), &self.session);
        }
        fn key_click(&mut self, key: Key) {
            let _ = handle_key(true, key, self.conn.clone(), &self.session);
            let _ = handle_key(false, key, self.conn.clone(), &self.session);
        }
    }

    pub struct RdpInputMouse {
        conn: Arc<SyncConnection>,
        session: Path<'static>,
        stream: PwStreamInfo,
    }

    impl RdpInputMouse {
        pub fn new(
            conn: Arc<SyncConnection>,
            session: Path<'static>,
            stream: PwStreamInfo,
        ) -> ResultType<Self> {
            Ok(Self {
                conn,
                session,
                stream,
            })
        }
    }

    impl MouseControllable for RdpInputMouse {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn mouse_move_to(&mut self, x: i32, y: i32) {
            let portal = get_portal(&self.conn);
            let _ = remote_desktop_portal::notify_pointer_motion_absolute(
                &portal,
                &self.session,
                HashMap::new(),
                self.stream.path as u32,
                x as f64,
                y as f64,
            );
        }
        fn mouse_move_relative(&mut self, x: i32, y: i32) {
            let portal = get_portal(&self.conn);
            let _ = remote_desktop_portal::notify_pointer_motion(
                &portal,
                &self.session,
                HashMap::new(),
                x as f64,
                y as f64,
            );
        }
        fn mouse_down(&mut self, button: MouseButton) -> enigo::ResultType {
            handle_mouse(true, button, self.conn.clone(), &self.session);
            Ok(())
        }
        fn mouse_up(&mut self, button: MouseButton) {
            handle_mouse(false, button, self.conn.clone(), &self.session);
        }
        fn mouse_click(&mut self, button: MouseButton) {
            handle_mouse(true, button, self.conn.clone(), &self.session);
            handle_mouse(false, button, self.conn.clone(), &self.session);
        }
        fn mouse_scroll_x(&mut self, length: i32) {
            let portal = get_portal(&self.conn);
            let _ = remote_desktop_portal::notify_pointer_axis(
                &portal,
                &self.session,
                HashMap::new(),
                length as f64,
                0 as f64,
            );
        }
        fn mouse_scroll_y(&mut self, length: i32) {
            let portal = get_portal(&self.conn);
            let _ = remote_desktop_portal::notify_pointer_axis(
                &portal,
                &self.session,
                HashMap::new(),
                0 as f64,
                length as f64,
            );
        }
    }

    fn get_raw_evdev_keycode(key: u16) -> i32 {
        // 8 is the offset between xkb and evdev
        let mut key = key as i32 - 8;
        // fix for right_meta key
        if key == 126 {
            key = 125;
        }
        key
    }

    fn handle_key(
        down: bool,
        key: Key,
        conn: Arc<SyncConnection>,
        session: &Path<'static>,
    ) -> ResultType<()> {
        let state: u32 = if down {
            PRESSED_DOWN_STATE
        } else {
            PRESSED_UP_STATE
        };
        let portal = get_portal(&conn);
        match key {
            Key::Raw(key) => {
                let key = get_raw_evdev_keycode(key);
                remote_desktop_portal::notify_keyboard_keycode(
                    &portal,
                    &session,
                    HashMap::new(),
                    key,
                    state,
                )?;
            }
            _ => {
                if let Ok((key, is_shift)) = map_key(&key) {
                    if is_shift {
                        remote_desktop_portal::notify_keyboard_keycode(
                            &portal,
                            &session,
                            HashMap::new(),
                            evdev::Key::KEY_LEFTSHIFT.code() as i32,
                            state,
                        )?;
                    }
                    remote_desktop_portal::notify_keyboard_keycode(
                        &portal,
                        &session,
                        HashMap::new(),
                        key.code() as i32,
                        state,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn handle_mouse(
        down: bool,
        button: MouseButton,
        conn: Arc<SyncConnection>,
        session: &Path<'static>,
    ) {
        let portal = get_portal(&conn);
        let but_key = match button {
            MouseButton::Left => EVDEV_MOUSE_LEFT,
            MouseButton::Right => EVDEV_MOUSE_RIGHT,
            MouseButton::Middle => EVDEV_MOUSE_MIDDLE,
            _ => {
                return;
            }
        };
        let state: u32 = if down {
            PRESSED_DOWN_STATE
        } else {
            PRESSED_UP_STATE
        };
        let _ = remote_desktop_portal::notify_pointer_button(
            &portal,
            &session,
            HashMap::new(),
            but_key,
            state,
        );
    }
}
