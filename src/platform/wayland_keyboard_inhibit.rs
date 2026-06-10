//! Keyboard shortcuts inhibition for Wayland clients.
//!
//! While a remote session window has keyboard focus, ask the compositor to
//! stop processing its own keyboard shortcuts (Alt+Tab, Super, ...) so they
//! are delivered to the session window and forwarded to the remote peer.
//! Uses the keyboard-shortcuts-inhibit-unstable-v1 Wayland protocol:
//! <https://wayland.app/protocols/keyboard-shortcuts-inhibit-unstable-v1>
//!
//! Every failure path is a no-op: on X11, on compositors without the
//! protocol, or on any Wayland error the behavior stays as before.
//!
//! Threading: the public functions may be called from any thread (they are
//! invoked from a Flutter FFI worker). All GTK/GDK and Wayland work is
//! marshalled to the GTK main thread via `glib::MainContext::invoke`.
//!
//! Protocol safety: creating a second inhibitor for the same (surface, seat)
//! pair is the fatal protocol error `already_inhibited` on the shared GTK
//! Wayland connection and would take down the whole UI. A singleton
//! inhibitor plus surface-id comparison prevents that.

use std::{
    ffi::{c_char, c_void},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, MutexGuard, Once, OnceLock,
    },
};

use gtk::{gdk, glib, glib::translate::ToGlibPtr, prelude::*};
use hbb_common::log;
use wayland_client::{
    backend::{Backend, ObjectId},
    protocol::{
        wl_registry::{self, WlRegistry},
        wl_seat::WlSeat,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};
use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1::{self, ZwpKeyboardShortcutsInhibitManagerV1},
    zwp_keyboard_shortcuts_inhibitor_v1::{self, ZwpKeyboardShortcutsInhibitorV1},
};

// GTK3 gdk-wayland accessors. They live in libgdk-3, already loaded by the
// `gtk` crate, but are resolved at runtime via `dlsym` rather than hard-linked
// with an `extern` import block. A hard import makes these symbols a load-time
// requirement of the whole cdylib: on a GTK3 build without the Wayland backend
// (where the symbols are absent) the dynamic loader could fail to start
// RustDesk at all with an undefined-symbol error -- turning the intended
// "no-op off Wayland" into a hard crash that affects X11 users too. Resolving
// lazily lets a missing symbol degrade to "inhibition unavailable" instead.
extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

// RTLD_DEFAULT on glibc: search the global symbol scope, which includes the
// already-loaded libgdk-3.
const RTLD_DEFAULT: *mut c_void = std::ptr::null_mut();

struct GdkWaylandSyms {
    display_get_wl_display: unsafe extern "C" fn(*mut gdk::ffi::GdkDisplay) -> *mut c_void,
    window_get_wl_surface: unsafe extern "C" fn(*mut gdk::ffi::GdkWindow) -> *mut c_void,
    seat_get_wl_seat: unsafe extern "C" fn(*mut gdk::ffi::GdkSeat) -> *mut c_void,
}

/// Resolve the gdk-wayland accessors once, caching the result. Returns `None`
/// (also cached) when any symbol is missing, e.g. on a GTK3 build without the
/// Wayland backend. All call sites treat `None` as "Wayland inhibition
/// unavailable" and no-op.
fn gdk_wayland_syms() -> Option<&'static GdkWaylandSyms> {
    static SYMS: OnceLock<Option<GdkWaylandSyms>> = OnceLock::new();
    SYMS.get_or_init(|| unsafe {
        let display = dlsym(
            RTLD_DEFAULT,
            b"gdk_wayland_display_get_wl_display\0".as_ptr() as *const c_char,
        );
        let window = dlsym(
            RTLD_DEFAULT,
            b"gdk_wayland_window_get_wl_surface\0".as_ptr() as *const c_char,
        );
        let seat = dlsym(
            RTLD_DEFAULT,
            b"gdk_wayland_seat_get_wl_seat\0".as_ptr() as *const c_char,
        );
        if display.is_null() || window.is_null() || seat.is_null() {
            log::warn!(
                "shortcuts inhibit: gdk-wayland symbols missing from libgdk-3; \
                 keyboard shortcuts inhibition unavailable"
            );
            return None;
        }
        // SAFETY: each non-null dlsym result points at the named gdk-wayland
        // function, whose signature matches the target pointer type exactly.
        Some(GdkWaylandSyms {
            display_get_wl_display: std::mem::transmute(display),
            window_get_wl_surface: std::mem::transmute(window),
            seat_get_wl_seat: std::mem::transmute(seat),
        })
    })
    .as_ref()
}

const INHIBIT_MANAGER_INTERFACE: &str = "zwp_keyboard_shortcuts_inhibit_manager_v1";

// Highest version of the manager interface this code knows how to use. The
// protocol is unstable-v1 and only defines version 1; bind to at most this so
// a newer compositor advertisement does not request unsupported requests.
const INHIBIT_MANAGER_MAX_VERSION: u32 = 1;

#[derive(Default)]
struct State {
    manager: Option<ZwpKeyboardShortcutsInhibitManagerV1>,
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<State>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == INHIBIT_MANAGER_INTERFACE {
                let manager = registry.bind::<ZwpKeyboardShortcutsInhibitManagerV1, _, _>(
                    name,
                    version.min(INHIBIT_MANAGER_MAX_VERSION),
                    qh,
                    (),
                );
                state.manager = Some(manager);
            }
        }
    }
}

impl Dispatch<ZwpKeyboardShortcutsInhibitManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpKeyboardShortcutsInhibitManagerV1,
        _: zwp_keyboard_shortcuts_inhibit_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        // The manager has no events.
    }
}

impl Dispatch<ZwpKeyboardShortcutsInhibitorV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpKeyboardShortcutsInhibitorV1,
        event: zwp_keyboard_shortcuts_inhibitor_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        match event {
            zwp_keyboard_shortcuts_inhibitor_v1::Event::Active => {
                log::debug!("keyboard shortcuts inhibitor active");
            }
            zwp_keyboard_shortcuts_inhibitor_v1::Event::Inactive => {
                // The compositor can deactivate the inhibitor itself
                // (e.g. Super+Esc on GNOME). Nothing to do: the next focus
                // enter re-creates it.
                log::debug!("keyboard shortcuts inhibitor deactivated by compositor");
            }
            _ => {}
        }
    }
}

struct WaylandGlobals {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
    manager: ZwpKeyboardShortcutsInhibitManagerV1,
}

// SAFETY: all fields are only ever accessed from the GTK main thread; the
// Mutex exists to satisfy `static` requirements.
unsafe impl Send for WaylandGlobals {}

struct ActiveInhibitor {
    inhibitor: ZwpKeyboardShortcutsInhibitorV1,
    surface_id: ObjectId,
}

unsafe impl Send for ActiveInhibitor {}

static INIT_TRIED: AtomicBool = AtomicBool::new(false);
static GLOBALS: Mutex<Option<WaylandGlobals>> = Mutex::new(None);
static INHIBITOR: Mutex<Option<ActiveInhibitor>> = Mutex::new(None);

/// Lock a static mutex tolerating poisoning. Both lock holders run only on the
/// GTK main thread and every failure path here is a no-op, so a panic that
/// poisoned the mutex must not turn every later call into a host-crashing
/// `unwrap`. Recover the guard instead and carry on.
fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Called when a remote session window gains keyboard focus.
pub fn inhibit_keyboard_shortcuts() {
    glib::MainContext::default().invoke(do_inhibit);
}

/// Called when a remote session window loses keyboard focus or closes.
pub fn release_keyboard_shortcuts() {
    glib::MainContext::default().invoke(do_release);
}

// Threading and locking invariant (applies to do_inhibit and do_release):
// both run ONLY on the GTK main thread, dispatched through
// `glib::MainContext::default().invoke`. Because they never execute
// concurrently, the two static mutexes are always taken in the same order
// (GLOBALS, then INHIBITOR) and there is no real deadlock risk; the
// consistent order is kept so future edits do not introduce one. This
// single-thread invariant is also what makes the `unsafe impl Send` on the
// Wayland objects sound.

/// Mark gtk-rs as initialized. Flutter initializes GTK through its C embedder
/// (`gtk_init`), so the gtk-rs crate's own init flag stays unset and every
/// `assert_initialized_main_thread!`-guarded call (e.g. `gdk::Display::default`,
/// `gtk::Window::list_toplevels`) would panic. We only reach this on the GTK
/// main thread with GTK already up, so flipping the flag is sound.
fn ensure_gtk_rs_initialized() {
    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        gtk::set_initialized();
    });
    // The `unsafe impl Send` on the Wayland objects and the lock-order
    // reasoning both rely on do_inhibit/do_release running only on the GTK
    // main thread. `glib::MainContext::invoke` is supposed to guarantee that;
    // assert it in debug builds so a wrong assumption surfaces loudly instead
    // of becoming silent undefined behavior.
    debug_assert!(
        gtk::is_initialized_main_thread(),
        "wayland shortcuts inhibit must run on the GTK main thread"
    );
}

/// The default GDK display, but only when it is a Wayland display. Calling
/// the `gdk_wayland_*` accessors on an X11 display crashes, so this gate is
/// mandatory before any of them.
fn wayland_display() -> Option<gdk::Display> {
    let display = gdk::Display::default()?;
    display
        .type_()
        .name()
        .contains("Wayland")
        .then_some(display)
}

/// The `wl_surface` object id of the currently active (focused) toplevel, if
/// any. Must only be called on a Wayland display (see `wayland_display`).
fn active_toplevel_surface_id() -> Option<ObjectId> {
    let syms = gdk_wayland_syms()?;
    let window = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Window>().ok())
        .find(|w| w.is_active())?;
    let gdk_window = window.window()?;
    let surface_ptr = unsafe { (syms.window_get_wl_surface)(gdk_window.to_glib_none().0) };
    if surface_ptr.is_null() {
        return None;
    }
    unsafe { ObjectId::from_ptr(WlSurface::interface(), surface_ptr as *mut _) }.ok()
}

/// Inhibit shortcuts for the currently focused session window. Runs on the
/// GTK main thread.
fn do_inhibit() {
    ensure_gtk_rs_initialized();
    let Some(display) = wayland_display() else {
        return;
    };
    let Some(surface_id) = active_toplevel_surface_id() else {
        log::debug!("shortcuts inhibit: no active toplevel surface, skipping");
        return;
    };

    let mut globals_guard = lock_or_recover(&GLOBALS);
    if !INIT_TRIED.load(Ordering::SeqCst) {
        match init_globals(&display) {
            InitOutcome::Ready(globals) => {
                *globals_guard = Some(globals);
                INIT_TRIED.store(true, Ordering::SeqCst);
            }
            InitOutcome::ManagerAbsent => {
                // The compositor does not offer the protocol: a permanent
                // no-op. Mark it so we stop trying.
                INIT_TRIED.store(true, Ordering::SeqCst);
                log::warn!(
                    "{} not offered by the compositor; \
                     system shortcuts will stay local",
                    INHIBIT_MANAGER_INTERFACE
                );
            }
            InitOutcome::TransientError => {
                // Setup failed for a recoverable reason (e.g. a roundtrip
                // error). Leave INIT_TRIED unset so the next focus enter
                // retries instead of disabling the feature for the whole
                // process lifetime.
            }
        }
    }
    let Some(globals) = globals_guard.as_mut() else {
        return;
    };

    let Some(syms) = gdk_wayland_syms() else {
        return;
    };
    let Some(seat) = display.default_seat() else {
        return;
    };
    let seat_ptr = unsafe { (syms.seat_get_wl_seat)(seat.to_glib_none().0) };
    if seat_ptr.is_null() {
        log::debug!("shortcuts inhibit: null wl_seat, skipping");
        return;
    }

    let mut inhibitor_guard = lock_or_recover(&INHIBITOR);
    if let Some(active) = inhibitor_guard.as_ref() {
        if active.surface_id == surface_id {
            // Already inhibiting this surface (e.g. tab switch inside the
            // same window). Creating a second inhibitor would be the fatal
            // `already_inhibited` protocol error.
            drain_events(globals);
            return;
        }
        // Focus moved to a different session window: re-create.
        active.inhibitor.destroy();
        *inhibitor_guard = None;
    }

    let Ok(surface) = WlSurface::from_id(&globals.conn, surface_id.clone()) else {
        log::debug!("shortcuts inhibit: failed to wrap wl_surface from object id");
        return;
    };
    let Ok(seat_id) = (unsafe { ObjectId::from_ptr(WlSeat::interface(), seat_ptr as *mut _) })
    else {
        log::debug!("shortcuts inhibit: failed to build wl_seat object id from pointer");
        return;
    };
    let Ok(seat_proxy) = WlSeat::from_id(&globals.conn, seat_id) else {
        log::debug!("shortcuts inhibit: failed to wrap wl_seat from object id");
        return;
    };

    let qh = globals.queue.handle();
    let inhibitor = globals
        .manager
        .inhibit_shortcuts(&surface, &seat_proxy, &qh, ());
    let _ = globals.conn.flush();
    *inhibitor_guard = Some(ActiveInhibitor {
        inhibitor,
        surface_id,
    });
    log::debug!("keyboard shortcuts inhibitor created");
    drain_events(globals);
}

/// Release the active inhibitor on focus leave. Runs on the GTK main thread.
fn do_release() {
    ensure_gtk_rs_initialized();
    let mut globals_guard = lock_or_recover(&GLOBALS);
    let mut inhibitor_guard = lock_or_recover(&INHIBITOR);
    // If the inhibited surface is still the active toplevel, this leave is
    // spurious: the documented enter-before-leave race, where focus moving
    // between two session windows enqueues inhibit(new) before release(old).
    // Keep the live inhibitor. `active_toplevel_surface_id` touches
    // `gdk_wayland_*`, which is only safe on Wayland; an inhibitor only
    // exists on Wayland, so reaching the call below is safe.
    let keep = match inhibitor_guard.as_ref() {
        Some(active) => Some(&active.surface_id) == active_toplevel_surface_id().as_ref(),
        None => return,
    };
    if keep {
        return;
    }
    if let Some(active) = inhibitor_guard.take() {
        active.inhibitor.destroy();
        if let Some(globals) = globals_guard.as_mut() {
            let _ = globals.conn.flush();
            drain_events(globals);
        }
        log::debug!("keyboard shortcuts inhibitor released");
    }
}

/// Result of a one-time setup attempt. Distinguishes a permanent no-op (the
/// compositor does not implement the protocol) from a recoverable failure that
/// is worth retrying on a later focus enter.
enum InitOutcome {
    Ready(WaylandGlobals),
    ManagerAbsent,
    TransientError,
}

/// One-time setup on the GTK main thread: wrap the existing GTK Wayland
/// connection (guest mode) and bind the inhibit manager from the registry
/// on a dedicated event queue, leaving GTK's own queue untouched.
fn init_globals(display: &gdk::Display) -> InitOutcome {
    let Some(syms) = gdk_wayland_syms() else {
        return InitOutcome::TransientError;
    };
    let wl_display = unsafe { (syms.display_get_wl_display)(display.to_glib_none().0) };
    if wl_display.is_null() {
        return InitOutcome::TransientError;
    }
    let backend = unsafe { Backend::from_foreign_display(wl_display as *mut _) };
    let conn = Connection::from_backend(backend);
    let mut queue = conn.new_event_queue::<State>();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());
    let mut state = State::default();
    if let Err(err) = queue.roundtrip(&mut state) {
        log::warn!("shortcuts inhibit: registry roundtrip failed: {err}");
        return InitOutcome::TransientError;
    }
    let Some(manager) = state.manager.take() else {
        return InitOutcome::ManagerAbsent;
    };
    InitOutcome::Ready(WaylandGlobals {
        conn,
        queue,
        state,
        manager,
    })
}

/// Process pending events on our queue (inhibitor active/inactive) so they
/// do not accumulate.
fn drain_events(globals: &mut WaylandGlobals) {
    let _ = globals.queue.dispatch_pending(&mut globals.state);
}
