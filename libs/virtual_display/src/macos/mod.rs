extern "C" {
    fn RustDeskVirtualDisplaySupported() -> bool;
    fn RustDeskVirtualDisplayMask() -> u32;
    fn RustDeskOwnsVirtualDisplay(display_id: u32) -> bool;
    fn RustDeskConfigureVirtualDisplay(
        display_id: u32,
        width: u32,
        height: u32,
        scale: u32,
    ) -> bool;
    fn RustDeskVirtualDisplayMode(
        display_id: u32,
        width: *mut u32,
        height: *mut u32,
        scale: *mut u32,
    ) -> bool;
    fn RustDeskResizeVirtualDisplay(display_id: u32, width: u32, height: u32) -> bool;
    fn RustDeskToggleVirtualDisplay(index: i32, on: bool) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub scale: u32,
}

pub fn is_supported() -> bool {
    unsafe { RustDeskVirtualDisplaySupported() }
}

pub fn active_mask() -> u32 {
    unsafe { RustDeskVirtualDisplayMask() }
}

pub fn owns_display(display_id: u32) -> bool {
    unsafe { RustDeskOwnsVirtualDisplay(display_id) }
}

pub fn display_mode(display_id: u32) -> Option<DisplayMode> {
    let (mut width, mut height, mut scale) = (0, 0, 0);
    if unsafe { RustDeskVirtualDisplayMode(display_id, &mut width, &mut height, &mut scale) } {
        Some(DisplayMode {
            width,
            height,
            scale,
        })
    } else {
        None
    }
}

// Mode changes may block while CoreGraphics updates the display; use a worker thread.
pub fn configure(display_id: u32, width: u32, height: u32, scale: u32) -> bool {
    unsafe { RustDeskConfigureVirtualDisplay(display_id, width, height, scale) }
}

// Width and height are logical pixels; the current native scale is preserved.
pub fn resize(display_id: u32, width: u32, height: u32) -> bool {
    unsafe { RustDeskResizeVirtualDisplay(display_id, width, height) }
}

pub fn toggle(index: i32, on: bool) -> bool {
    unsafe { RustDeskToggleVirtualDisplay(index, on) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_api_rejects_unowned_displays_and_invalid_indices() {
        let _ = is_supported();
        assert_eq!(active_mask(), 0);
        assert!(!owns_display(0));
        assert_eq!(display_mode(0), None);
        assert!(!configure(0, 1920, 1080, 1));
        assert!(!resize(0, 1920, 1080));
        for index in [-1, 0, 5] {
            assert!(!toggle(index, true));
        }
    }
}
