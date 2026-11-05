use crate::platform::screen_frame::Style;
use gtk::{cairo, gdk, glib, prelude::*};
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Once,
    },
};

static VISIBLE: AtomicBool = AtomicBool::new(false);
static UPDATE_QUEUED: AtomicBool = AtomicBool::new(false);
static DISPLAY_SIGNALS: Once = Once::new();

thread_local! {
    static WINDOWS: RefCell<Vec<gtk::Window>> = RefCell::new(Vec::new());
}

pub(crate) fn set_visible(visible: bool) {
    if VISIBLE.swap(visible, Ordering::AcqRel) != visible {
        queue_update();
    } else if visible {
        refresh_colors();
    }
}

pub(crate) fn refresh() {
    if VISIBLE.load(Ordering::Acquire) {
        queue_update();
    }
}

pub(crate) fn refresh_colors() {
    if !VISIBLE.load(Ordering::Acquire) {
        return;
    }
    glib::idle_add_once(|| {
        let empty = WINDOWS.with(|windows| {
            let windows = windows.borrow();
            for window in windows.iter() {
                window.queue_draw();
            }
            windows.is_empty()
        });
        if empty {
            refresh();
        }
    });
}

fn queue_update() {
    if !UPDATE_QUEUED.swap(true, Ordering::AcqRel) {
        // CM notifications arrive on IPC threads. Use the existing GTK main loop;
        // invoking a MainContext directly can instead run on the calling thread.
        glib::idle_add_once(|| {
            UPDATE_QUEUED.store(false, Ordering::Release);
            apply_state();
        });
    }
}

fn apply_state() {
    WINDOWS.with(|windows| {
        for window in windows.borrow_mut().drain(..) {
            unsafe { window.destroy() };
        }
    });
    if !VISIBLE.load(Ordering::Acquire) {
        return;
    }
    // Flutter initializes GTK from C++; initialize the Rust bindings on that same UI thread.
    if let Err(err) = gtk::init() {
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            warn,
            "Failed to initialize screen frame GTK bindings: {err}"
        );
        return;
    }
    let Some(screen) = gdk::Screen::default() else {
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            warn,
            "No GTK screen is available for screen frames"
        );
        return;
    };
    // Wayland does not offer the X11 global positioning used below. Do not put
    // misleading frames around XWayland's virtual screen in a Wayland session.
    if !super::is_x11_or_headless() || screen.display().type_().name() != "GdkX11Display" {
        return;
    }
    DISPLAY_SIGNALS.call_once(|| {
        // The default screen lives with GTK; these handlers remain for subsequent sessions.
        screen.connect_monitors_changed(|_| refresh());
        screen.connect_size_changed(|_| refresh());
        screen.connect_composited_changed(|_| refresh());
    });
    let server = match scrap::x11::Server::default() {
        Ok(server) => server,
        Err(err) => {
            hbb_common::throttled_log!(
                std::time::Duration::from_secs(5),
                warn,
                "Failed to enumerate X11 screen frame displays: {err:?}"
            );
            return;
        }
    };
    let scale = screen
        .root_window()
        .map_or(1, |window| window.scale_factor())
        .max(1);
    let mut windows = Vec::new();
    // Use the capture backend's RandR monitor names, not GTK product names or
    // enumeration indices, so per-session subscriptions match the physical display.
    for display in scrap::x11::Server::displays(server) {
        let name = display.name();
        let rect = display.rect();
        let (x, y) = (i32::from(rect.x) / scale, i32::from(rect.y) / scale);
        let (width, height) = (i32::from(rect.w) / scale, i32::from(rect.h) / scale);
        if width <= 0 || height <= 0 {
            continue;
        }
        let style = Style::for_display(&name);
        let thickness = ((i32::from(style.width) + scale - 1) / scale)
            .min(width / 2)
            .min(height / 2);
        // Leave the corners to the horizontal strips so compositor opacity is applied once.
        for (left, top, w, h) in [
            (x, y, width, thickness),
            (x, y + height - thickness, width, thickness),
            (x, y + thickness, thickness, height - 2 * thickness),
            (
                x + width - thickness,
                y + thickness,
                thickness,
                height - 2 * thickness,
            ),
        ] {
            if w <= 0 || h <= 0 {
                continue;
            }
            let window = create_window(&screen, &name, left, top, w, h);
            // An empty input region passes both local and injected mouse input through.
            window.input_shape_combine_region(Some(&cairo::Region::create()));
            window.show_all();
            // GTK widget opacity leaves these empty popups opaque. Set the
            // realized X11 window's compositor opacity instead.
            if let Some(native) = window.window() {
                native.set_opacity(1.0 - f64::from(style.transparency) / 100.0);
            } else {
                hbb_common::throttled_log!(
                    std::time::Duration::from_secs(5),
                    warn,
                    "No native window is available for screen frame transparency"
                );
            }
            windows.push(window);
        }
    }
    WINDOWS.with(|current| *current.borrow_mut() = windows);
}

fn create_window(
    screen: &gdk::Screen,
    name: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> gtk::Window {
    // Override-redirect popups keep the WM from decorating or repositioning these
    // narrow windows. X11 capture includes them; do not hide them to fake exclusion.
    let window = gtk::Window::new(gtk::WindowType::Popup);
    window.set_screen(screen);
    window.set_title("RustDesk Screen Frame");
    window.set_decorated(false);
    window.set_accept_focus(false);
    window.set_focus_on_map(false);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);
    window.set_keep_above(true);
    window.set_resizable(false);
    window.set_app_paintable(true);
    // Empty, non-resizable GTK windows otherwise keep a 200x200 logical-pixel
    // natural size; a default size alone cannot make a thin border.
    window.set_size_request(width, height);
    window.move_(x, y);
    let name = name.to_owned();
    window.connect_draw(move |_, context| {
        let [r, g, b] = Style::for_display(&name).color;
        context.set_source_rgb(
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
        );
        if let Err(err) = context.paint() {
            hbb_common::throttled_log!(
                std::time::Duration::from_secs(5),
                warn,
                "Failed to draw X11 screen frame: {err}"
            );
        }
        glib::Propagation::Stop
    });
    window
}
