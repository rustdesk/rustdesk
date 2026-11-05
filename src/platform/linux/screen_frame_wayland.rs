use super::Style;
use gtk::{cairo, gdk, glib, prelude::*};
use std::{
    cell::RefCell,
    ffi::OsStr,
    process::{Child, Command, Stdio},
    time::Duration,
};

const HELPER_ARG: &str = "--screen-frame-wayland";

thread_local! {
    static HELPER: RefCell<Option<(Child, [u8; 5])>> = RefCell::new(None);
}

pub(crate) fn is_available() -> bool {
    if std::env::var_os("DISPLAY").map_or(true, |display| display.is_empty()) {
        return false;
    }
    // DISPLAY can outlive XWayland. Probe a short-lived XCB connection instead of
    // opening GDK's X11 backend in the UI process, whose I/O errors can exit it.
    match scrap::x11::Server::default() {
        Ok(_) => true,
        Err(err) => {
            hbb_common::log::trace!("XWayland is unavailable for screen frames: {err:?}");
            false
        }
    }
}

pub(super) fn update(visible: bool) {
    if let Err(err) = update_helper(visible) {
        hbb_common::throttled_log!(
            Duration::from_secs(5),
            warn,
            "Failed to update Wayland screen frame: {err}"
        );
    }
}

fn update_helper(visible: bool) -> std::io::Result<()> {
    HELPER.with(|current| {
        let mut current = current.borrow_mut();
        if !visible {
            return stop_helper(&mut current);
        }
        // Wayland streams have no stable display mapping, so use the configured
        // frame color for every output rather than implying a viewed/unviewed state.
        let style = Style::load();
        let style = [
            style.color[0],
            style.color[1],
            style.color[2],
            style.width,
            style.transparency,
        ];
        if let Some((child, previous)) = current.as_mut() {
            // Retain an exited child until hide: a lost XWayland connection must
            // stop drawing, not trigger a respawn loop on subsequent CM updates.
            if let Some(status) = child.try_wait()? {
                hbb_common::throttled_log!(
                    Duration::from_secs(5), warn,
                    "Wayland screen frame helper exited ({status}); borders remain disabled until hidden"
                );
                return Ok(());
            }
            if *previous == style {
                return Ok(());
            }
        }
        stop_helper(&mut current)?;
        // GDK's X11 I/O error handler calls _exit(1). A separate thread/display in
        // CM cannot contain that failure; only the helper connects to XWayland.
        let child = Command::new(std::env::current_exe()?)
            .arg(HELPER_ARG)
            .args(style.map(|value| value.to_string()))
            .env("GDK_BACKEND", "x11")
            .stdin(Stdio::piped())
            .spawn()?;
        *current = Some((child, style));
        Ok(())
    })
}

fn stop_helper(current: &mut Option<(Child, [u8; 5])>) -> std::io::Result<()> {
    if let Some((child, _)) = current.as_mut() {
        if child.try_wait()?.is_none() {
            child.kill()?;
            child.wait()?;
        }
    }
    *current = None;
    Ok(())
}

pub(crate) fn dispatch() {
    let mut args = std::env::args_os();
    if args.nth(1).as_deref() != Some(OsStr::new(HELPER_ARG)) {
        return;
    }
    let mut style = [0; 5];
    for value in &mut style {
        let Some(parsed) = args.next().and_then(|arg| arg.to_str()?.parse::<u8>().ok()) else {
            eprintln!("Invalid Wayland screen frame style");
            std::process::exit(1);
        };
        *value = parsed;
    }
    style[3] = style[3].clamp(2, 20);
    style[4] = style[4].min(80);
    if let Err(err) = gtk::init() {
        eprintln!("Screen frames on Wayland require an available XWayland display: {err}");
        std::process::exit(1);
    }
    let Some(screen) =
        gdk::Screen::default().filter(|screen| screen.display().type_().name() == "GdkX11Display")
    else {
        std::process::exit(1);
    };
    screen.connect_monitors_changed(move |screen| queue_rebuild(screen, style));
    screen.connect_size_changed(move |screen| queue_rebuild(screen, style));
    screen.connect_composited_changed(move |screen| queue_rebuild(screen, style));
    // The pipe belongs to CM. EOF also removes borders when CM exits abruptly,
    // without a timer or a second IPC protocol to track the parent's lifetime.
    glib::source::unix_fd_add_local(
        0,
        glib::IOCondition::HUP | glib::IOCondition::ERR | glib::IOCondition::NVAL,
        |_, _| {
            gtk::main_quit();
            glib::ControlFlow::Break
        },
    );
    rebuild(&screen, style);
    gtk::main();
    std::process::exit(0);
}

fn queue_rebuild(screen: &gdk::Screen, style: [u8; 5]) {
    let screen = screen.clone();
    glib::idle_add_local_once(move || rebuild(&screen, style));
}

fn rebuild(screen: &gdk::Screen, style: [u8; 5]) {
    super::WINDOWS.with(|windows| {
        for window in windows.borrow_mut().drain(..) {
            unsafe { window.destroy() };
        }
    });
    let display = screen.display();
    let mut windows = Vec::new();
    for index in 0..display.n_monitors() {
        let Some(monitor) = display.monitor(index) else {
            continue;
        };
        // Read geometry from the same XWayland display used by the windows. Native
        // Wayland output coordinates can use a different scale and cannot be mixed in.
        let rect = monitor.geometry();
        let (x, y, width, height) = (rect.x(), rect.y(), rect.width(), rect.height());
        if width <= 0 || height <= 0 {
            continue;
        }
        let scale = monitor.scale_factor().max(1);
        let thickness = ((i32::from(style[3]) + scale - 1) / scale)
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
            let window = create_window(screen, style, left, top, w, h);
            window.input_shape_combine_region(Some(&cairo::Region::create()));
            window.show_all();
            if let Some(native) = window.window() {
                native.set_opacity(1.0 - f64::from(style[4]) / 100.0);
            } else {
                hbb_common::throttled_log!(
                    std::time::Duration::from_secs(5),
                    warn,
                    "No native window is available for Wayland screen frame transparency"
                );
            }
            windows.push(window);
        }
    }
    super::WINDOWS.with(|current| *current.borrow_mut() = windows);
}

fn create_window(
    screen: &gdk::Screen,
    style: [u8; 5],
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> gtk::Window {
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
    // set_default_size alone leaves empty popups at GTK's 200x200 natural size.
    window.set_size_request(width, height);
    window.move_(x, y);
    window.connect_draw(move |_, context| {
        context.set_source_rgb(
            f64::from(style[0]) / 255.0,
            f64::from(style[1]) / 255.0,
            f64::from(style[2]) / 255.0,
        );
        if let Err(err) = context.paint() {
            hbb_common::throttled_log!(
                Duration::from_secs(5),
                warn,
                "Failed to draw Wayland screen frame: {err}"
            );
        }
        glib::Propagation::Stop
    });
    window
}
