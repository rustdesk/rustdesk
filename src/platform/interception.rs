use hbb_common::{log, ResultType, anyhow::anyhow};
use std::sync::{Arc, Mutex};
use interception::{Stroke, MouseState, MouseFlags, Filter};
pub use interception::Interception;

pub struct InterceptionContext(pub Interception);

unsafe impl Send for InterceptionContext {}

impl std::ops::Deref for InterceptionContext {
    type Target = Interception;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Keyboard,
    Mouse,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

pub fn find_first_device(ctx: &Interception, kind: DeviceKind) -> Option<i32> {
    // Interception uses device IDs 1-10 for Keyboard, 11-20 for Mouse.
    let range = match kind {
        DeviceKind::Keyboard => 1..11,
        DeviceKind::Mouse => 11..21,
    };

    let mut buffer = [0u8; 512];
    for i in range {
        // We need to provide a buffer for the hardware ID
        let len = ctx.get_hardware_id(i, &mut buffer);
        if len > 0 {
             let hw_id = String::from_utf8_lossy(&buffer[..len as usize]);
             log::info!("Found {:?} device at {}: {}", kind, i, hw_id);
             return Some(i);
        }
    }
    None
}

pub fn mouse_click(ctx: &Interception, mouse_dev: i32, btn: MouseButton) {
     mouse_down(ctx, mouse_dev, btn);
     mouse_up(ctx, mouse_dev, btn);
}

pub fn mouse_down(ctx: &Interception, mouse_dev: i32, btn: MouseButton) {
    let mut flags = MouseFlags::empty();
    let state = match btn {
        MouseButton::Left => MouseState::LEFT_BUTTON_DOWN,
        MouseButton::Right => MouseState::RIGHT_BUTTON_DOWN,
        MouseButton::Middle => MouseState::MIDDLE_BUTTON_DOWN,
    };
    
    let stroke = Stroke::Mouse {
        state,
        flags,
        rolling: 0,
        x: 0,
        y: 0,
        information: 0,
    };
    
    ctx.send(mouse_dev, &[stroke]);
}

pub fn mouse_up(ctx: &Interception, mouse_dev: i32, btn: MouseButton) {
    let mut flags = MouseFlags::empty();
    let state = match btn {
        MouseButton::Left => MouseState::LEFT_BUTTON_UP,
        MouseButton::Right => MouseState::RIGHT_BUTTON_UP,
        MouseButton::Middle => MouseState::MIDDLE_BUTTON_UP,
    };

    let stroke = Stroke::Mouse {
        state,
        flags,
        rolling: 0,
        x: 0,
        y: 0,
        information: 0,
    };
    
    ctx.send(mouse_dev, &[stroke]);
}

use winapi::um::winuser::{GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN};

pub fn mouse_move(ctx: &Interception, mouse_dev: i32, x: i32, y: i32) {
    let mut flags = MouseFlags::MOVE_ABSOLUTE;
    // Align with Enigo's logic: scale pixels to 0..65535 range based on virtual screen bounds.
    let (left, top, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    
    // Check for zero width/height to avoid division by zero which shouldn't happen on a valid desktop
    if width == 0 || height == 0 {
        return;
    }

    let abs_x = ((x - left) * 65535 / width) as i32;
    let abs_y = ((y - top) * 65535 / height) as i32;

    let stroke = Stroke::Mouse {
        state: MouseState::empty(),
        flags,
        rolling: 0,
        x: abs_x,
        y: abs_y,
        information: 0,
    };
    ctx.send(mouse_dev, &[stroke]);
}

pub fn mouse_move_relative(ctx: &Interception, mouse_dev: i32, dx: i32, dy: i32) {
    let flags = MouseFlags::MOVE_RELATIVE;
    let stroke = Stroke::Mouse {
        state: MouseState::empty(),
        flags,
        rolling: 0,
        x: dx,
        y: dy,
        information: 0,
    };
    ctx.send(mouse_dev, &[stroke]);
}

pub fn mouse_scroll(ctx: &Interception, mouse_dev: i32, amount: i32) {
    let stroke = Stroke::Mouse {
        state: MouseState::WHEEL,
        flags: MouseFlags::empty(),
        rolling: amount as i16,
        x: 0,
        y: 0,
        information: 0,
    };
    ctx.send(mouse_dev, &[stroke]);
}
