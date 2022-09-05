use super::{xdo::EnigoXdo};
use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};
use std::io::Read;

/// The main struct for handling the event emitting
// #[derive(Default)]
pub struct Enigo {
    xdo: EnigoXdo,
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

fn get_led_state(key: Key) -> bool{
    let led_file = match key{
        Key::CapsLock => {
            "/sys/class/leds/input1::capslock/brightness"
        }
        Key::NumLock => {
            "/sys/class/leds/input1::numlock/brightness"
        }
        _ => {
            return false;
        }
    };

    let status = if let Ok(mut file) = std::fs::File::open(&led_file) {
        let mut content = String::new();
        file.read_to_string(&mut content).ok();
        let status = content.trim_end().to_string().parse::<i32>().unwrap_or(0);
        status
    }else{
        0
    };
    status == 1
}

impl KeyboardControllable for Enigo {
    fn get_key_state(&mut self, key: Key) -> bool {
        if self.is_x11 {
            self.xdo.get_key_state(key)
        } else {
            if let Some(keyboard) = &mut self.uinput_keyboard {
                keyboard.get_key_state(key)
            } else {
                get_led_state(key)
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
            self.xdo.key_up(key)
        } else {
            if let Some(keyboard) = &mut self.uinput_keyboard {
                keyboard.key_up(key)
            }
        }
    }
    fn key_click(&mut self, key: Key) {
        self.key_down(key).ok();
        self.key_up(key);
    }
}
