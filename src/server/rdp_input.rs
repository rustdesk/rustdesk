use enigo::{Key, KeyboardControllable, MouseButton, MouseControllable};
use hbb_common::ResultType;
use dbus::{blocking::SyncConnection, Path};
use scrap::wayland::pipewire::{PwStreamInfo, get_portal};
use scrap::wayland::remote_desktop_portal::OrgFreedesktopPortalRemoteDesktop as remote_desktop_portal;
use std::collections::HashMap;
use std::sync::Arc;

pub mod client {
    use super::*;

pub struct RdpInputKeyboard {
        conn: Arc<SyncConnection>,
        session: Path<'static>,
    }

    impl RdpInputKeyboard {
        pub fn new( conn: Arc<SyncConnection>, session: Path<'static>) -> ResultType<Self> {
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
            true 
        }

        fn key_sequence(&mut self, _: &str) {
            // no api for this
        }

        fn key_down(&mut self, key: Key) -> enigo::ResultType {
            let p = get_portal(&self.conn);
            match key {
                Key::Raw(key) => {
                    let key = get_raw_evdev_keycode(key);
                    remote_desktop_portal::notify_keyboard_keycode(&p,self.session.clone(), HashMap::new(), key, 1)?; 
                },
                _ => {}
            }
            Ok(())
        }
        fn key_up(&mut self, key: Key) {
            let p = get_portal(&self.conn);
            match key {
                Key::Raw(key) => {
                    let key = get_raw_evdev_keycode(key);
                    let _ = remote_desktop_portal::notify_keyboard_keycode(&p,self.session.clone(), HashMap::new(), key, 0); 
                },
                _ => {}
            }
        }
        fn key_click(&mut self, key: Key) {
            let p = get_portal(&self.conn);
            match key {
                Key::Raw(key) => {
                    let key = get_raw_evdev_keycode(key);
                    let _ = remote_desktop_portal::notify_keyboard_keycode(&p,self.session.clone(), HashMap::new(), key, 1); 
                    let _ = remote_desktop_portal::notify_keyboard_keycode(&p,self.session.clone(), HashMap::new(), key, 0); 
                },
                _ => {}
            }
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
            let p = get_portal(&self.conn);
            let _ = remote_desktop_portal::notify_pointer_motion_absolute(
                &p,
                self.session.clone(),
                HashMap::new(),
                self.stream.path.clone() as u32,
                x as f64,
                y as f64,
            );
        }
        fn mouse_move_relative(&mut self, x: i32, y: i32) {
            let p = get_portal(&self.conn);
            let _ = remote_desktop_portal::notify_pointer_motion(
                &p,
                self.session.clone(),
                HashMap::new(),
                x as f64,
                y as f64,
            );
        }
        fn mouse_down(&mut self, button: MouseButton) -> enigo::ResultType {
            Ok(())
        }
        fn mouse_up(&mut self, button: MouseButton) {
        }
        fn mouse_click(&mut self, button: MouseButton) {
        }
        fn mouse_scroll_x(&mut self, length: i32) {
        }
        fn mouse_scroll_y(&mut self, length: i32) {
        }
    }

    fn get_raw_evdev_keycode(key: u16) -> i32 {
        let mut key = key as i32 - 8; // 8 is the offset between xkb and evdev
        if key == 126 {key = 125;} // fix for right_meta key
        key
    }
}