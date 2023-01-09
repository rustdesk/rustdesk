use super::xdo::EnigoXdo;
use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};
use std::io::Read;
use tfc::{traits::*, Context as TFC_Context, Key as TFC_Key};

pub type CustomKeyboard = Box<dyn KeyboardControllable + Send>;
pub type CustomMouce = Box<dyn MouseControllable + Send>;

/// The main struct for handling the event emitting
// #[derive(Default)]
pub struct Enigo {
    xdo: EnigoXdo,
    is_x11: bool,
    tfc: Option<TFC_Context>,
    custom_keyboard: Option<CustomKeyboard>,
    cutsom_mouse: Option<CustomMouce>,
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
    /// Set custom keyboard.
    pub fn set_custom_keyboard(&mut self, custom_keyboard: CustomKeyboard) {
        self.custom_keyboard = Some(custom_keyboard)
    }
    /// Set custom mouse.
    pub fn set_custom_mouse(&mut self, custom_mouse: CustomMouce) {
        self.cutsom_mouse = Some(custom_mouse)
    }
    /// Get custom keyboard.
    pub fn get_custom_keyboard(&mut self) -> &mut Option<CustomKeyboard> {
        &mut self.custom_keyboard
    }
    /// Get custom mouse.
    pub fn get_custom_mouse(&mut self) -> &mut Option<CustomMouce> {
        &mut self.cutsom_mouse
    }

    fn tfc_key_down_or_up(&mut self, key: Key, down: bool, up: bool) -> bool {
        match &mut self.tfc {
            None => false,
            Some(tfc) => {
                if let Key::Layout(chr) = key {
                    if down {
                        if let Err(_) = tfc.unicode_char_down(chr) {
                            return false;
                        }
                    }
                    if up {
                        if let Err(_) = tfc.unicode_char_up(chr) {
                            return false;
                        }
                    }
                    return true;
                }
                let key = match convert_to_tfc_key(key) {
                    Some(key) => key,
                    None => {
                        return false;
                    }
                };

                if down {
                    if let Err(_) = tfc.key_down(key) {
                        return false;
                    }
                };
                if up {
                    if let Err(_) = tfc.key_up(key) {
                        return false;
                    }
                };
                return true;
            }
        }
    }
}

impl Default for Enigo {
    fn default() -> Self {
        let is_x11 = "x11" == hbb_common::platform::linux::get_display_server();
        Self {
            is_x11,
            tfc: if is_x11 {
                match TFC_Context::new() {
                    Ok(ctx) => Some(ctx),
                    Err(..) => {
                        println!("kbd context error");
                        None
                    }
                }
            } else {
                None
            },
            custom_keyboard: None,
            cutsom_mouse: None,
            xdo: EnigoXdo::default(),
        }
    }
}

impl MouseControllable for Enigo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn mouse_move_to(&mut self, x: i32, y: i32) {
        if self.is_x11 {
            self.xdo.mouse_move_to(x, y);
        } else {
            if let Some(mouse) = &mut self.cutsom_mouse {
                mouse.mouse_move_to(x, y)
            }
        }
    }
    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        if self.is_x11 {
            self.xdo.mouse_move_relative(x, y);
        } else {
            if let Some(mouse) = &mut self.cutsom_mouse {
                mouse.mouse_move_relative(x, y)
            }
        }
    }
    fn mouse_down(&mut self, button: MouseButton) -> crate::ResultType {
        if self.is_x11 {
            self.xdo.mouse_down(button)
        } else {
            if let Some(mouse) = &mut self.cutsom_mouse {
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
            if let Some(mouse) = &mut self.cutsom_mouse {
                mouse.mouse_up(button)
            }
        }
    }
    fn mouse_click(&mut self, button: MouseButton) {
        if self.is_x11 {
            self.xdo.mouse_click(button)
        } else {
            if let Some(mouse) = &mut self.cutsom_mouse {
                mouse.mouse_click(button)
            }
        }
    }
    fn mouse_scroll_x(&mut self, length: i32) {
        if self.is_x11 {
            self.xdo.mouse_scroll_x(length)
        } else {
            if let Some(mouse) = &mut self.cutsom_mouse {
                mouse.mouse_scroll_x(length)
            }
        }
    }
    fn mouse_scroll_y(&mut self, length: i32) {
        if self.is_x11 {
            self.xdo.mouse_scroll_y(length)
        } else {
            if let Some(mouse) = &mut self.cutsom_mouse {
                mouse.mouse_scroll_y(length)
            }
        }
    }
}

fn get_led_state(key: Key) -> bool {
    let led_file = match key {
        Key::CapsLock => "/sys/class/leds/input1::capslock/brightness",
        Key::NumLock => "/sys/class/leds/input1::numlock/brightness",
        _ => {
            return false;
        }
    };

    let status = if let Ok(mut file) = std::fs::File::open(&led_file) {
        let mut content = String::new();
        file.read_to_string(&mut content).ok();
        let status = content.trim_end().to_string().parse::<i32>().unwrap_or(0);
        status
    } else {
        0
    };
    status == 1
}

impl KeyboardControllable for Enigo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_key_state(&mut self, key: Key) -> bool {
        if self.is_x11 {
            self.xdo.get_key_state(key)
        } else {
            if let Some(keyboard) = &mut self.custom_keyboard {
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
            if let Some(keyboard) = &mut self.custom_keyboard {
                keyboard.key_sequence(sequence)
            }
        }
    }

    fn key_down(&mut self, key: Key) -> crate::ResultType {
        if self.is_x11 {
            let has_down = self.tfc_key_down_or_up(key, true, false);
            if !has_down {
                self.xdo.key_down(key)
            } else {
                Ok(())
            }
        } else {
            if let Some(keyboard) = &mut self.custom_keyboard {
                keyboard.key_down(key)
            } else {
                Ok(())
            }
        }
    }
    fn key_up(&mut self, key: Key) {
        if self.is_x11 {
            let has_down = self.tfc_key_down_or_up(key, false, true);
            if !has_down {
                self.xdo.key_up(key)
            }
        } else {
            if let Some(keyboard) = &mut self.custom_keyboard {
                keyboard.key_up(key)
            }
        }
    }
    fn key_click(&mut self, key: Key) {
        self.key_down(key).ok();
        self.key_up(key);
    }
}

fn convert_to_tfc_key(key: Key) -> Option<TFC_Key> {
    let key = match key {
        Key::Alt => TFC_Key::Alt,
        Key::Backspace => TFC_Key::DeleteOrBackspace,
        Key::CapsLock => TFC_Key::CapsLock,
        Key::Control => TFC_Key::Control,
        Key::Delete => TFC_Key::ForwardDelete,
        Key::DownArrow => TFC_Key::DownArrow,
        Key::End => TFC_Key::End,
        Key::Escape => TFC_Key::Escape,
        Key::F1 => TFC_Key::F1,
        Key::F10 => TFC_Key::F10,
        Key::F11 => TFC_Key::F11,
        Key::F12 => TFC_Key::F12,
        Key::F2 => TFC_Key::F2,
        Key::F3 => TFC_Key::F3,
        Key::F4 => TFC_Key::F4,
        Key::F5 => TFC_Key::F5,
        Key::F6 => TFC_Key::F6,
        Key::F7 => TFC_Key::F7,
        Key::F8 => TFC_Key::F8,
        Key::F9 => TFC_Key::F9,
        Key::Home => TFC_Key::Home,
        Key::LeftArrow => TFC_Key::LeftArrow,
        Key::PageDown => TFC_Key::PageDown,
        Key::PageUp => TFC_Key::PageUp,
        Key::Return => TFC_Key::ReturnOrEnter,
        Key::RightArrow => TFC_Key::RightArrow,
        Key::Shift => TFC_Key::Shift,
        Key::Space => TFC_Key::Space,
        Key::Tab => TFC_Key::Tab,
        Key::UpArrow => TFC_Key::UpArrow,
        Key::Numpad0 => TFC_Key::N0,
        Key::Numpad1 => TFC_Key::N1,
        Key::Numpad2 => TFC_Key::N2,
        Key::Numpad3 => TFC_Key::N3,
        Key::Numpad4 => TFC_Key::N4,
        Key::Numpad5 => TFC_Key::N5,
        Key::Numpad6 => TFC_Key::N6,
        Key::Numpad7 => TFC_Key::N7,
        Key::Numpad8 => TFC_Key::N8,
        Key::Numpad9 => TFC_Key::N9,
        Key::Decimal => TFC_Key::NumpadDecimal,
        Key::Clear => TFC_Key::NumpadClear,
        Key::Pause => TFC_Key::PlayPause,
        Key::Print => TFC_Key::Print,
        Key::Snapshot => TFC_Key::PrintScreen,
        Key::Insert => TFC_Key::Insert,
        Key::Scroll => TFC_Key::ScrollLock,
        Key::NumLock => TFC_Key::NumLock,
        Key::RWin => TFC_Key::Meta,
        Key::Apps => TFC_Key::Apps,
        Key::Multiply => TFC_Key::NumpadMultiply,
        Key::Add => TFC_Key::NumpadPlus,
        Key::Subtract => TFC_Key::NumpadMinus,
        Key::Divide => TFC_Key::NumpadDivide,
        Key::Equals => TFC_Key::NumpadEquals,
        Key::NumpadEnter => TFC_Key::NumpadEnter,
        Key::RightShift => TFC_Key::RightShift,
        Key::RightControl => TFC_Key::RightControl,
        Key::RightAlt => TFC_Key::RightAlt,
        Key::Command | Key::Super | Key::Windows | Key::Meta => TFC_Key::Meta,
        _ => {
            return None;
        }
    };
    Some(key)
}
