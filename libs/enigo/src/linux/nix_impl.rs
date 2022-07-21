use super::{pynput::EnigoPynput, xdo::EnigoXdo};
use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};

/// The main struct for handling the event emitting
// #[derive(Default)]
pub struct Enigo {
    xdo: EnigoXdo,
    pynput: EnigoPynput,
    is_x11: bool,
    uinput_keyboard: Option<Box<dyn KeyboardControllable + Send>>,
    uinput_mouse: Option<Box<dyn MouseControllable + Send>>,
}

impl Enigo {
    /// Get delay of xdo implementation.
    pub fn delay(&self) -> u64 {
        self.xdo.delay()
    }
    /// Set delay of xdo implemetation.
    pub fn set_delay(&mut self, delay: u64) {
        self.xdo.set_delay(delay)
    }
    /// Reset pynput.
    pub fn reset(&mut self) {
        self.pynput.reset();
    }
    /// Set uinput keyboard.
    pub fn set_uinput_keyboard(
        &mut self,
        uinput_keyboard: Option<Box<dyn KeyboardControllable + Send>>,
    ) {
        self.uinput_keyboard = uinput_keyboard
    }
    /// Set uinput mouse.
    pub fn set_uinput_mouse(&mut self, uinput_mouse: Option<Box<dyn MouseControllable + Send>>) {
        self.uinput_mouse = uinput_mouse
    }
}

impl Default for Enigo {
    fn default() -> Self {
        Self {
            is_x11: "x11" == hbb_common::platform::linux::get_display_server(),
            uinput_keyboard: None,
            uinput_mouse: None,
            xdo: EnigoXdo::default(),
            pynput: EnigoPynput::default(),
        }
    }
}

impl MouseControllable for Enigo {
    fn mouse_move_to(&mut self, x: i32, y: i32) {
        if self.is_x11 {
            self.xdo.mouse_move_to(x, y);
        } else {
            if let Some(mouse) = &mut self.uinput_mouse {
                mouse.mouse_move_to(x, y)
            }
        }
    }
    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        if self.is_x11 {
            self.xdo.mouse_move_relative(x, y);
        } else {
            if let Some(mouse) = &mut self.uinput_mouse {
                mouse.mouse_move_relative(x, y)
            }
        }
    }
    fn mouse_down(&mut self, button: MouseButton) -> crate::ResultType {
        if self.is_x11 {
            self.xdo.mouse_down(button)
        } else {
            if let Some(mouse) = &mut self.uinput_mouse {
                mouse.mouse_down(button)
            } else {
                Ok(())
            }
        }
    }
    fn mouse_up(&mut self, button: MouseButton) {
        if self.is_x11 {
            self.xdo.mouse_up(button)
        } else {
            if let Some(mouse) = &mut self.uinput_mouse {
                mouse.mouse_up(button)
            }
        }
    }
    fn mouse_click(&mut self, button: MouseButton) {
        if self.is_x11 {
            self.xdo.mouse_click(button)
        } else {
            if let Some(mouse) = &mut self.uinput_mouse {
                mouse.mouse_click(button)
            }
        }
    }
    fn mouse_scroll_x(&mut self, length: i32) {
        if self.is_x11 {
            self.xdo.mouse_scroll_x(length)
        } else {
            if let Some(mouse) = &mut self.uinput_mouse {
                mouse.mouse_scroll_x(length)
            }
        }
    }
    fn mouse_scroll_y(&mut self, length: i32) {
        if self.is_x11 {
            self.xdo.mouse_scroll_y(length)
        } else {
            if let Some(mouse) = &mut self.uinput_mouse {
                mouse.mouse_scroll_y(length)
            }
        }
    }
}

impl KeyboardControllable for Enigo {
    fn get_key_state(&mut self, key: Key) -> bool {
        if self.is_x11 {
            self.xdo.get_key_state(key)
        } else {
            if let Some(keyboard) = &mut self.uinput_keyboard {
                keyboard.get_key_state(key)
            } else {
                false
            }
        }
    }

    fn key_sequence(&mut self, sequence: &str) {
        if self.is_x11 {
            self.xdo.key_sequence(sequence)
        } else {
            if let Some(keyboard) = &mut self.uinput_keyboard {
                keyboard.key_sequence(sequence)
            }
        }
    }

    fn key_down(&mut self, key: Key) -> crate::ResultType {
        if self.is_x11 {
            if self.pynput.send_pynput(&key, true) {
                return Ok(());
            }
            self.xdo.key_down(key)
        } else {
            if let Some(keyboard) = &mut self.uinput_keyboard {
                keyboard.key_down(key)
            } else {
                Ok(())
            }
        }
    }
    fn key_up(&mut self, key: Key) {
        if self.is_x11 {
            if self.pynput.send_pynput(&key, false) {
                return;
            }
            self.xdo.key_up(key)
        } else {
            if let Some(keyboard) = &mut self.uinput_keyboard {
                keyboard.key_up(key)
            }
        }
    }
    fn key_click(&mut self, key: Key) {
        if self.is_x11 {
            self.xdo.key_click(key)
        } else {
            if let Some(keyboard) = &mut self.uinput_keyboard {
                keyboard.key_click(key)
            }
        }
    }
}
