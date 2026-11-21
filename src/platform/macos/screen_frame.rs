use crate::platform::screen_frame::Style;
use cocoa::{
    base::{id, nil, NO, YES},
    foundation::{NSPoint, NSRect, NSSize, NSString},
};
use dispatch::Queue;
use objc::{class, msg_send, sel, sel_impl};
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

const CG_DISPLAY_BEGIN_CONFIGURATION_FLAG: u32 = 1 << 0;
const CG_OVERLAY_WINDOW_LEVEL_KEY: i32 = 15;
const NS_WINDOW_SHARING_READ_ONLY: usize = 1;

static REQUESTED_VISIBLE: AtomicBool = AtomicBool::new(false);
static DISPLAY_CALLBACK_REGISTERED: AtomicBool = AtomicBool::new(false);
static UPDATE_QUEUED: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    static ref WINDOWS: Mutex<Vec<ScreenFrameWindow>> = Mutex::new(Vec::new());
}

type DisplayReconfigurationCallback = extern "C" fn(u32, u32, *mut c_void);

struct ScreenFrameWindow {
    panel: usize,
    window_id: u32,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGDisplayRegisterReconfigurationCallback(
        callback: DisplayReconfigurationCallback,
        user_info: *mut c_void,
    ) -> i32;
    fn CGWindowLevelForKey(key: i32) -> i32;
}

pub(crate) fn set_visible(visible: bool) {
    if visible {
        register_display_callback();
    }
    if REQUESTED_VISIBLE.swap(visible, Ordering::AcqRel) != visible {
        if unsafe { hbb_common::libc::pthread_main_np() } != 0 {
            apply_requested_state();
        } else {
            Queue::main().exec_sync(apply_requested_state);
        }
    } else if visible {
        // Remaining sessions may view a different set of displays after a disconnect.
        refresh_colors();
    }
}

pub(crate) fn window_ids() -> Vec<u32> {
    WINDOWS
        .lock()
        .unwrap()
        .iter()
        .map(|window| window.window_id)
        .collect()
}

pub(crate) fn refresh() {
    if REQUESTED_VISIBLE.load(Ordering::Acquire) {
        queue_update();
    }
}

pub(crate) fn refresh_colors() {
    // Keep the same windows so color changes do not invalidate the SCK exclusion IDs.
    Queue::main().exec_async(|| objc::rc::autoreleasepool(|| unsafe {
        for window in WINDOWS.lock().unwrap().iter() {
            let panel = window.panel as id;
            let screen: id = msg_send![panel, screen];
            let style = style_for_screen(screen);
            let alpha = 1.0 - f64::from(style.transparency) / 100.0;
            let color: id = msg_send![class!(NSColor), colorWithSRGBRed: f64::from(style.color[0]) / 255.0
                green: f64::from(style.color[1]) / 255.0 blue: f64::from(style.color[2]) / 255.0 alpha: alpha];
            let _: () = msg_send![panel, setBackgroundColor: color];
        }
    }));
}

fn register_display_callback() {
    if DISPLAY_CALLBACK_REGISTERED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let result = unsafe {
        CGDisplayRegisterReconfigurationCallback(display_reconfigured, std::ptr::null_mut())
    };
    if result != 0 {
        DISPLAY_CALLBACK_REGISTERED.store(false, Ordering::Release);
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to monitor display changes for screen frame: {result}"
        );
    }
}

extern "C" fn display_reconfigured(_display: u32, flags: u32, _user_info: *mut c_void) {
    if flags & CG_DISPLAY_BEGIN_CONFIGURATION_FLAG == 0 && REQUESTED_VISIBLE.load(Ordering::Acquire)
    {
        queue_update();
    }
}

fn queue_update() {
    if UPDATE_QUEUED.swap(true, Ordering::AcqRel) {
        return;
    }
    Queue::main().exec_async(apply_requested_state);
}

fn apply_requested_state() {
    UPDATE_QUEUED.store(false, Ordering::Release);
    objc::rc::autoreleasepool(|| unsafe {
        destroy_windows();
        if REQUESTED_VISIBLE.load(Ordering::Acquire) {
            create_windows();
        }
    });
    crate::ui_cm_interface::screen_frame_window_ids_changed(window_ids());
}

unsafe fn create_windows() {
    let screens: id = msg_send![class!(NSScreen), screens];
    if screens == nil {
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to get displays for screen frame"
        );
        return;
    }
    let count: usize = msg_send![screens, count];
    if count == 0 {
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            warn,
            "No display is available for screen frame"
        );
        return;
    }

    // Overlay stays above menus while leaving help and dragging windows in front.
    let level = CGWindowLevelForKey(CG_OVERLAY_WINDOW_LEVEL_KEY) as i64;
    let mut windows = Vec::with_capacity(count.saturating_mul(4));
    for index in 0..count {
        let screen: id = msg_send![screens, objectAtIndex: index];
        if screen == nil {
            continue;
        }
        let style = style_for_screen(screen);
        let frame: NSRect = msg_send![screen, frame];
        if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
            continue;
        }
        // Use points on macOS to keep the apparent width consistent across displays.
        // AppKit handles backing pixels: 4 pt becomes 8 px on a 2x Retina display.
        let thickness = f64::from(style.width)
            .min(frame.size.width / 2.0)
            .min(frame.size.height / 2.0);
        let x = frame.origin.x;
        let y = frame.origin.y;
        let width = frame.size.width;
        let height = frame.size.height;
        // Leave the corners to the horizontal strips so translucent windows do not overlap.
        let rects = [
            NSRect::new(NSPoint::new(x, y), NSSize::new(width, thickness)),
            NSRect::new(
                NSPoint::new(x, y + height - thickness),
                NSSize::new(width, thickness),
            ),
            NSRect::new(
                NSPoint::new(x, y + thickness),
                NSSize::new(thickness, height - 2.0 * thickness),
            ),
            NSRect::new(
                NSPoint::new(x + width - thickness, y + thickness),
                NSSize::new(thickness, height - 2.0 * thickness),
            ),
        ];
        for rect in rects {
            if let Some(window) = create_window(rect, level, &style) {
                windows.push(window);
            }
        }
    }
    WINDOWS.lock().unwrap().extend(windows);
}

unsafe fn create_window(rect: NSRect, level: i64, style: &Style) -> Option<ScreenFrameWindow> {
    let panel: id = msg_send![class!(NSPanel), alloc];
    if panel == nil {
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to allocate screen frame window"
        );
        return None;
    }
    let panel: id = msg_send![panel,
        initWithContentRect: rect
        styleMask: 0usize
        backing: 2usize
        defer: NO
    ];
    if panel == nil {
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to initialize screen frame window"
        );
        return None;
    }

    let color: id = msg_send![class!(NSColor),
        colorWithSRGBRed: f64::from(style.color[0]) / 255.0
        green: f64::from(style.color[1]) / 255.0
        blue: f64::from(style.color[2]) / 255.0
        alpha: 1.0f64 - f64::from(style.transparency) / 100.0
    ];
    let collection_behavior = (1usize << 0) | (1usize << 4) | (1usize << 6) | (1usize << 8);
    let _: () = msg_send![panel, setBackgroundColor: color];
    let _: () = msg_send![panel, setOpaque: NO];
    let _: () = msg_send![panel, setHasShadow: NO];
    let _: () = msg_send![panel, setIgnoresMouseEvents: YES];
    let _: () = msg_send![panel, setHidesOnDeactivate: NO];
    let _: () = msg_send![panel, setCanHide: NO];
    let _: () = msg_send![panel, setSharingType: NS_WINDOW_SHARING_READ_ONLY];
    let _: () = msg_send![panel, setReleasedWhenClosed: NO];
    let _: () = msg_send![panel, setFloatingPanel: YES];
    let _: () = msg_send![panel, setBecomesKeyOnlyIfNeeded: YES];
    let _: () = msg_send![panel, setCollectionBehavior: collection_behavior];
    let _: () = msg_send![panel, setLevel: level];
    let _: () = msg_send![panel, orderFrontRegardless];
    let window_id: isize = msg_send![panel, windowNumber];
    if window_id <= 0 || window_id > u32::MAX as isize {
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to get a valid screen frame window number"
        );
        let _: () = msg_send![panel, orderOut: nil];
        let _: () = msg_send![panel, release];
        return None;
    }
    Some(ScreenFrameWindow {
        panel: panel as usize,
        window_id: window_id as u32,
    })
}

unsafe fn style_for_screen(screen: id) -> Style {
    let description: id = msg_send![screen, deviceDescription];
    let key = NSString::alloc(nil).init_str("NSScreenNumber");
    let number: id = msg_send![description, objectForKey: key];
    let _: () = msg_send![key, release];
    if number == nil {
        return Style::load();
    }
    let display_id: u32 = msg_send![number, unsignedIntValue];
    Style::for_display(&display_id.to_string())
}

unsafe fn destroy_windows() {
    let windows = std::mem::take(&mut *WINDOWS.lock().unwrap());
    for window in windows {
        let window = window.panel as id;
        let _: () = msg_send![window, orderOut: nil];
        let _: () = msg_send![window, release];
    }
}
