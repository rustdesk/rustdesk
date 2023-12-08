use enigo::{Key, KeyboardControllable, MouseButton, MouseControllable};
use hbb_common::ResultType;
use dbus::{blocking::SyncConnection, Path};
use scrap::wayland::pipewire::get_portal;
use scrap::wayland::remote_desktop_portal::OrgFreedesktopPortalRemoteDesktop as remote_desktop_portal;
use std::collections::HashMap;

pub mod client {
    use super::*;

    pub struct RdpInputKeyboard {
        conn: SyncConnection,
        session: Path<'static>,
    }

    impl RdpInputKeyboard {
        pub fn new( conn: SyncConnection, session: Path<'static>) -> ResultType<Self> {
                // let conn = get_portal(&conn);
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

        fn get_key_state(&mut self, key: Key) -> bool {
            true // todo
        }

        fn key_sequence(&mut self, sequence: &str) {
        }

        fn key_down(&mut self, key: Key) -> enigo::ResultType {
            //ex: simulate meta
            // let p = get_portal(&self.conn);
            // remote_desktop_portal::notify_keyboard_keycode(&p,self.session.clone(), HashMap::new(), 125, 1)?; 
            // remote_desktop_portal::notify_keyboard_keycode(&p,self.session.clone(), HashMap::new(), 125, 0)?; 
            Ok(())
        }
        fn key_up(&mut self, key: Key) {
        }
        fn key_click(&mut self, key: Key) {
        }
    }


    pub struct RdpInputMouse {
    }

    impl RdpInputMouse {
        pub async fn new() -> ResultType<Self> {
                Ok(Self {})
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
        }
        fn mouse_move_relative(&mut self, x: i32, y: i32) {
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
}