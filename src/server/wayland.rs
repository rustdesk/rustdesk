use super::*;
use hbb_common::{allow_err, anyhow, platform::linux::DISTRO};
use scrap::{
    is_cursor_embedded, set_map_err,
    wayland::pipewire::{fill_displays, try_fix_logical_size},
    Capturer, Display, Frame, TraitCapturer,
};
use std::collections::HashMap;
use std::io;

use crate::{
    client::{
        SCRAP_OTHER_VERSION_OR_X11_REQUIRED, SCRAP_UBUNTU_HIGHER_REQUIRED,
        SCRAP_X11_REQUIRED, SCRAP_XDP_PORTAL_UNAVAILABLE,
    },
    platform::linux::is_x11,
};

lazy_static::lazy_static! {
    static ref CAP_DISPLAY_INFO: RwLock<HashMap<usize, u64>> = RwLock::new(HashMap::new());
    static ref PIPEWIRE_INITIALIZED: RwLock<bool> = RwLock::new(false);
    static ref LOG_SCRAP_COUNT: Mutex<u32> = Mutex::new(0);
    static ref ACTIVE_DISPLAY_COUNT: RwLock<usize> = RwLock::new(0);
}

pub fn init() {
    set_map_err(map_err_scrap);
}

pub(super) fn increment_active_display_count() -> usize {
    let mut count = ACTIVE_DISPLAY_COUNT.write().unwrap();
    *count += 1;
    *count
}

pub(super) fn decrement_active_display_count() -> usize {
    let mut count = ACTIVE_DISPLAY_COUNT.write().unwrap();
    if *count > 0 {
        *count -= 1;
    }
    *count
}

fn map_err_scrap(err: String) -> io::Error {
    // to-do: Handle error better, do not restart server
    if err.starts_with("Did not receive a reply") {
        log::error!("Fatal pipewire error, {}", &err);
        std::process::exit(-1);
    }

    if DISTRO.name.to_uppercase() == "Ubuntu".to_uppercase() {
        if DISTRO.version_id < "21".to_owned() {
            io::Error::new(io::ErrorKind::Other, SCRAP_UBUNTU_HIGHER_REQUIRED)
        } else {
            try_log(&err);
            io::Error::new(io::ErrorKind::Other, err)
        }
    } else {
        try_log(&err);
        let err_lower = err.to_ascii_lowercase();
        if err_lower.contains("org.freedesktop.portal")
            || err_lower.contains("dbus")
            || err_lower.contains("d-bus")
        {
            // The portal D-Bus interface is unreachable. This typically means
            // xdg-desktop-portal has crashed... for more info, see: Issue #12897
            io::Error::new(io::ErrorKind::Other, SCRAP_XDP_PORTAL_UNAVAILABLE)
        } else if err_lower.contains("pipewire") {
            io::Error::new(io::ErrorKind::Other, SCRAP_OTHER_VERSION_OR_X11_REQUIRED)
        } else {
            io::Error::new(io::ErrorKind::Other, SCRAP_X11_REQUIRED)
        }
    }
}

fn try_log(err: &String) {
    let mut lock_count = LOG_SCRAP_COUNT.lock().unwrap();
    if *lock_count >= 1000000 {
        return;
    }
    if *lock_count % 10000 == 0 {
        log::error!("Failed scrap {}", err);
    }
    *lock_count += 1;
}

struct CapturerPtr(*mut Capturer);

impl Clone for CapturerPtr {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl TraitCapturer for CapturerPtr {
    fn frame<'a>(&'a mut self, timeout: std::time::Duration) -> std::io::Result<Frame<'a>> {
        unsafe { (*self.0).frame(timeout) }
    }
}

struct CapDisplayInfo {
    rects: Vec<((i32, i32), usize, usize)>,
    displays: Vec<DisplayInfo>,
    num: usize,
    primary: usize,
    current: usize,
    capturer: CapturerPtr,
}

/// Set the uinput absolute-pointer range to the whole logical desktop so the compositor maps
/// injected coordinates 1:1 instead of stretching a single-monitor range across all outputs. The
/// PipeWire path does this inline in `check_init`; the DRM path bypasses check_init so it must do it
/// too, otherwise on a multi-monitor host the injected pointer lands on the wrong output — and the
/// hardware cursor, which lives on whichever CRTC the pointer is over, never appears on the captured
/// CRTC (the "cursor not visible" symptom). Reads the layout from the Wayland outputs, so it is
/// independent of the capture backend.
///
/// This is the DRM path's single copy of what `check_init` does inline for PipeWire, and it does the
/// same three things, for the same reasons:
///
/// - drops the cached Wayland layout first, because it can predate compositor changes made while no
///   session was active (rustdesk#15601), and on the hotplug path it is stale by definition;
/// - bounds the IPC wait, because `uinput::client::set_resolution` reads its reply with no timeout of
///   its own, so a hung uinput socket would otherwise block every video-service start on this branch
///   and wedge the hotplug worker inside `rt.block_on`, leaving `UINPUT_REFRESH_BUSY` latched true so
///   that every later hotplug refresh is silently skipped for the process lifetime;
/// - records the applied rect and snapshots the per-display layout baseline, which is what arms the
///   #15601 drift remap. Without it the remap never activates on the DRM path at all.
///
/// It stays a separate copy rather than being folded into `check_init` because `check_init` ships in
/// every Linux build and this feature must not change the drm-off one by so much as a line.
#[cfg(feature = "drm")]
pub(super) async fn update_uinput_resolution() {
    if !crate::input_service::wayland_use_uinput() {
        return;
    }
    scrap::wayland::display::clear_wayland_displays_cache();
    let Some(rect) = scrap::wayland::display::get_desktop_rect_for_uinput() else {
        log::warn!("Failed to get desktop rect for uinput");
        return;
    };
    // Re-snapshot the baseline on every call: this runs at session init and after every hotplug, and
    // the baseline is what the client's coordinates are measured against.
    let snapshot_layout = || {
        super::display_service::set_wayland_layout_baseline(
            scrap::wayland::display::get_display_rects_for_uinput(),
        );
    };
    // Reprogram the device only when the range actually changes. A display stuck in a rebuild loop
    // calls this about once a second, and reapplying an identical range is an IPC roundtrip plus a
    // uinput device reconfiguration under a user who may be at the console.
    if super::display_service::wayland_uinput_rect() == Some(rect) {
        snapshot_layout();
        return;
    }
    let (minx, maxx, miny, maxy) = rect;
    log::info!("update mouse resolution: ({minx}, {maxx}), ({miny}, {maxy})");
    match timeout(
        3_000,
        input_service::update_mouse_resolution(minx, maxx, miny, maxy),
    )
    .await
    {
        // Record the rect only after a successful apply, so a transient failure is retried on the
        // next call instead of being remembered as applied.
        Ok(Ok(())) => {
            super::display_service::set_wayland_uinput_rect(rect);
            snapshot_layout();
        }
        Ok(Err(err)) => log::error!("Failed to update mouse resolution: {}", err),
        Err(err) => log::error!("Failed to update mouse resolution: {}", err),
    }
}

#[tokio::main(flavor = "current_thread")]
pub(super) async fn ensure_inited() -> ResultType<()> {
    // DRM/KMS capture (opt-in): the root service owns the reader and the capturer self-inits over
    // IPC, so there is no PipeWire recorder to initialize here. But we still must set the uinput
    // desktop rect (check_init does this on the PipeWire path, and the DRM path skips check_init).
    #[cfg(feature = "drm")]
    if super::drm_capturer::is_available_cached() {
        update_uinput_resolution().await;
        return Ok(());
    }
    check_init().await
}

pub(super) fn is_inited() -> Option<Message> {
    if is_x11() {
        None
    } else {
        #[cfg(feature = "drm")]
        if super::drm_capturer::is_available_cached() {
            return None;
        }
        if CAP_DISPLAY_INFO.read().unwrap().is_empty() {
            let mut msg_out = Message::new();
            let res = MessageBox {
                msgtype: "nook-nocancel-hasclose".to_owned(),
                title: "Wayland".to_owned(),
                text: "Please Select the screen to be shared(Operate on the peer side).".to_owned(),
                link: "".to_owned(),
                ..Default::default()
            };
            msg_out.set_message_box(res);
            Some(msg_out)
        } else {
            None
        }
    }
}

pub(super) async fn check_init() -> ResultType<()> {
    if !is_x11() {
        if CAP_DISPLAY_INFO.read().unwrap().is_empty() {
            if crate::input_service::wayland_use_uinput() {
                // The cached layout may predate compositor changes made while no session
                // was active, https://github.com/rustdesk/rustdesk/issues/15601
                scrap::wayland::display::clear_wayland_displays_cache();
                if let Some((minx, maxx, miny, maxy)) =
                    scrap::wayland::display::get_desktop_rect_for_uinput()
                {
                    log::info!(
                        "update mouse resolution: ({}, {}), ({}, {})",
                        minx,
                        maxx,
                        miny,
                        maxy
                    );
                    // Bound the IPC wait like the periodic refresh does, so a hung
                    // response can't stall session init.
                    match timeout(
                        3_000,
                        input_service::update_mouse_resolution(minx, maxx, miny, maxy),
                    )
                    .await
                    {
                        Ok(Ok(())) => {
                            super::display_service::set_wayland_uinput_rect((
                                minx, maxx, miny, maxy,
                            ));
                            // Snapshot the per-display layout the client's coordinates
                            // will be based on, so the mouse path can correct them if
                            // the compositor moves a monitor mid-session.
                            super::display_service::set_wayland_layout_baseline(
                                scrap::wayland::display::get_display_rects_for_uinput(),
                            );
                        }
                        Ok(Err(err)) => log::error!("Failed to update mouse resolution: {}", err),
                        Err(err) => log::error!("Failed to update mouse resolution: {}", err),
                    }
                } else {
                    log::warn!("Failed to get desktop rect for uinput");
                }
            }

            let mut lock = CAP_DISPLAY_INFO.write().unwrap();
            if lock.is_empty() {
                // Check if PipeWire is already initialized to prevent duplicate recorder creation
                if *PIPEWIRE_INITIALIZED.read().unwrap() {
                    log::warn!("wayland_diag: Preventing duplicate PipeWire initialization");
                    return Ok(());
                }

                let mut all = Display::all()?;
                log::debug!("Initializing displays with fill_displays()");
                {
                    let temp_mouse_move_handle = input_service::TemporaryMouseMoveHandle::new();
                    let move_mouse_to = |x, y| temp_mouse_move_handle.move_mouse_to(x, y);
                    fill_displays(move_mouse_to, crate::get_cursor_pos, &mut all)?;
                }
                log::debug!("Attempting to fix logical size with try_fix_logical_size()");
                try_fix_logical_size(&mut all);
                *PIPEWIRE_INITIALIZED.write().unwrap() = true;
                let num = all.len();
                let primary = super::display_service::get_primary_2(&all);
                let mut displays = super::display_service::update_sync_displays(&all);
                for display in displays.iter_mut() {
                    display.cursor_embedded = is_cursor_embedded();
                }

                let mut rects: Vec<((i32, i32), usize, usize)> = Vec::new();
                for d in &all {
                    rects.push((d.origin(), d.width(), d.height()));
                }

                log::debug!(
                    "#displays={}, primary={}, rects: {:?}, cpus={}/{}",
                    num,
                    primary,
                    rects,
                    num_cpus::get_physical(),
                    num_cpus::get()
                );

                // Create individual CapDisplayInfo for each display with its own capturer
                for (idx, display) in all.into_iter().enumerate() {
                    let capturer =
                        Box::into_raw(Box::new(Capturer::new(display).with_context(|| {
                            format!("Failed to create capturer for display {}", idx)
                        })?));
                    let capturer = CapturerPtr(capturer);

                    let cap_display_info = Box::into_raw(Box::new(CapDisplayInfo {
                        rects: rects.clone(),
                        displays: displays.clone(),
                        num,
                        primary,
                        current: idx,
                        capturer,
                    }));

                    lock.insert(idx, cap_display_info as u64);
                }
            }
        }
    }
    Ok(())
}

pub(super) async fn get_displays_and_primary() -> ResultType<(Vec<DisplayInfo>, usize)> {
    #[cfg(feature = "drm")]
    if super::drm_capturer::is_available_cached() {
        // This function runs once per login (update_get_sync_displays_on_login is its only
        // caller), and login is the moment the client is PROMISED a display list -- so refresh
        // that list over a live `_drm` handshake first. The service wakes sleeping displays and
        // answers with the settled truth, which is what makes an unattended box with an idled,
        // DISABLED panel connectable at all: the cached list would either omit the panel (probed
        // while asleep) or advertise a display with no scanout behind it (probed while awake), and
        // either way the wake then firing inside the capture handshake would change the list the
        // client had already been given. Properly async, so the executor is never blocked; on any
        // failure the cache serves as before.
        super::drm_capturer::refresh_displays_for_login().await;
        if let Some(displays) = super::drm_capturer::get_display_infos() {
            // DRM connector order is not the compositor's primary; resolve the real primary from
            // the compositor layout (matched by normalized connector name), not a hardcoded index 0.
            return Ok((displays, super::drm_capturer::get_primary_index()));
        }
    }
    check_init().await?;
    // Keep one read guard so clear/reinitialization cannot split these across cache snapshots.
    let cap_map = CAP_DISPLAY_INFO.read().unwrap();
    if let Some(addr) = cap_map.values().next() {
        let cap_display_info: *const CapDisplayInfo = *addr as _;
        unsafe {
            let cap_display_info = &*cap_display_info;
            Ok((cap_display_info.displays.clone(), cap_display_info.primary))
        }
    } else {
        bail!("Failed to get capturer display info");
    }
}

pub fn clear() {
    if is_x11() {
        return;
    }
    // The DRM path augments its geometry from the compositor's Wayland outputs (logical origin +
    // scale), which scrap caches process-wide. The PipeWire path clears that cache on session close,
    // but the DRM path opens no PipeWire session, so without this it would keep matching DRM outputs
    // against STALE geometry after a monitor hotplug/rotation/scale change. Invalidate it on teardown
    // so the next session re-reads fresh geometry (lazily, on the next enumeration) and self-heals.
    #[cfg(feature = "drm")]
    if super::drm_capturer::is_available_cached() {
        scrap::wayland::display::clear_wayland_displays_cache();
    }
    // NOTE: intentionally do NOT reset the DRM probe cache here. `clear()` runs on every capturer
    // teardown (which happens on each video-service restart), and re-probing `_drm` from the async
    // enumeration path blocks the executor long enough to trip "deadline has elapsed" and spiral
    // into a restart loop. DRM availability is fixed at service start, so the cache stays valid.
    let mut write_lock = CAP_DISPLAY_INFO.write().unwrap();
    for (_, addr) in write_lock.iter() {
        let cap_display_info: *mut CapDisplayInfo = *addr as _;
        unsafe {
            let _box_capturer = Box::from_raw((*cap_display_info).capturer.0);
            let _box_cap_display_info = Box::from_raw(cap_display_info);
        }
    }
    write_lock.clear();

    // Reset PipeWire initialization flag to allow recreation on next init
    *PIPEWIRE_INITIALIZED.write().unwrap() = false;
}

/// Initialize the PipeWire/portal capture path from the plain (sync) video thread, so a DRM display
/// that cannot be captured can fall through to PipeWire for THAT display. `ensure_inited` short-circuits
/// to the DRM branch whenever DRM is globally available, so it never runs `check_init`; this helper
/// drives the same async portal ScreenCast init directly (mirroring `ensure_inited`'s pattern). Needed
/// because `is_available()` is a GLOBAL verdict — it stays true for the still-working DRM outputs — so
/// without a per-display fallback a single failed/demoted DRM display would restart-loop the video
/// service instead of degrading to PipeWire only for itself.
#[cfg(feature = "drm")]
#[tokio::main(flavor = "current_thread")]
async fn ensure_pipewire_inited() -> ResultType<()> {
    check_init().await
}

pub(super) fn get_capturer_for_display(
    display_idx: usize,
) -> ResultType<super::video_service::CapturerInfo> {
    if is_x11() {
        bail!("Do not call this function if not wayland");
    }
    // DRM/KMS capture path: build the capturer straight from the service `_drm` stream, bypassing
    // the PipeWire CAP_DISPLAY_INFO machinery entirely. `is_available()` is a GLOBAL verdict, so a
    // per-display DRM failure (an ungrabbable/demoted CRTC, or — after the phase-2 split — a
    // render-node-absent seat or a convert failure on the unprivileged side) must NOT propagate out
    // and restart-loop this per-display video service. Instead fall THROUGH to PipeWire for just this
    // display; the other DRM outputs keep streaming over DRM.
    // The ONE gate that keeps the probing form on purpose: this runs on the plain video thread,
    // not an async executor, and it is the capture-build path, so a definitive verdict is worth
    // seconds here. It is also what makes a cold cache recoverable at all -- warm_availability
    // gives up after its attempts, so if EVERY gate were cache-only a --server that started
    // before the root service would never see DRM again for the rest of its life.
    #[cfg(feature = "drm")]
    if super::drm_capturer::is_available() {
        match super::drm_capturer::get_capturer_info(display_idx) {
            Ok(info) => return Ok(info),
            Err(e) => {
                log::warn!(
                    "drm capturer for display {} unavailable ({:#}); falling back to PipeWire",
                    display_idx,
                    e
                );
                ensure_pipewire_inited()?;
            }
        }
    }
    let cap_map = CAP_DISPLAY_INFO.read().unwrap();
    // Serve ONLY the exact PipeWire entry for this index. Do NOT fall back to another index's
    // `CapDisplayInfo`: `CapturerPtr` is a bare `*mut Capturer` cloned by raw-pointer copy, so aliasing
    // one entry to two `display_idx` values would let two video-service threads call `frame()` on the
    // same `Recorder` with no lock (data race / UB), and it would also mis-map input against the wrong
    // rect. DRM and PipeWire do not share an index space (the portal often exposes one whole-desktop
    // stream at index 0), so a demoted non-primary DRM index has no PipeWire entry here; that case is
    // handled at the source by dropping the demoted display from the advertised list (see
    // drm_capturer demotion) so the client re-enumerates against a consistent list, rather than being
    // papered over with a shared/mismatched capturer.
    if let Some(addr) = cap_map.get(&display_idx) {
        let cap_display_info: *const CapDisplayInfo = *addr as _;
        unsafe {
            let cap_display_info = &*cap_display_info;
            let rect = cap_display_info.rects[cap_display_info.current];
            // Reaching here with DRM active means get_capturer_info bailed (a demoted display) and
            // we fell through to PipeWire. Serve this stream ONLY if its rect matches the
            // geometry we advertised for this index. The portal typically exposes one whole-desktop
            // stream, so on a multi-monitor host that rect is the FULL desktop while the advertised DRM
            // geometry is a single connector -> serving it would stretch the frame and offset all
            // input. Bail instead; get_display_infos advertised the display offline, so the client
            // re-enumerates against a consistent list. A single-display host matches (whole-desktop ==
            // that display) and is served normally. On a pure-PipeWire host is_available() is false and
            // this guard is skipped, preserving upstream behavior exactly.
            #[cfg(feature = "drm")]
            if super::drm_capturer::is_available_cached() {
                let (advertised, single_display) = match super::drm_capturer::get_display_infos() {
                    Some(list) => (list.get(display_idx).cloned(), list.len() == 1),
                    None => (None, false),
                };
                if let Some(advertised) = advertised {
                    // Compare LOGICAL against LOGICAL. The advertised DRM geometry carries the
                    // PHYSICAL mode plus the compositor scale (augment_with_wayland_geometry sets
                    // x/y/scale and deliberately leaves width/height physical), while the PipeWire
                    // rect is already logical (try_fix_logical_size). Comparing the two raw made a
                    // scaled output disagree with itself -- 2880x1800 against 1440x900 -- and the
                    // guard then rejected a portal stream that really was this display, leaving it
                    // advertised offline instead of degrading to PipeWire.
                    //
                    // The size check itself stays, because on a multi-monitor host it is what
                    // tells a single connector apart from the whole-desktop rect the portal
                    // usually exposes. On a single-display host that rect IS this display by
                    // construction, so only the position has to agree.
                    let scale = if advertised.scale > 0.0 {
                        advertised.scale
                    } else {
                        1.0
                    };
                    let logical_w = (advertised.width as f64 / scale).round() as usize;
                    let logical_h = (advertised.height as f64 / scale).round() as usize;
                    let consistent = advertised.x == rect.0 .0
                        && advertised.y == rect.0 .1
                        && (single_display || (logical_w == rect.1 && logical_h == rect.2));
                    if !consistent {
                        // Report the LOGICAL numbers, the ones actually compared, so the message
                        // does not look like a mismatch of quantities that were never expected to
                        // match on a scaled output.
                        bail!(
                            "drm display {} demoted with no geometry-consistent PipeWire stream (advertised {}x{} logical +{}+{} vs stream {}x{}+{}+{}); advertised offline",
                            display_idx,
                            logical_w,
                            logical_h,
                            advertised.x,
                            advertised.y,
                            rect.1,
                            rect.2,
                            rect.0 .0,
                            rect.0 .1
                        );
                    }
                }
            }
            Ok(super::video_service::CapturerInfo {
                origin: rect.0,
                width: rect.1,
                height: rect.2,
                ndisplay: cap_display_info.num,
                current: cap_display_info.current,
                privacy_mode_id: 0,
                _capturer_privacy_mode_id: 0,
                capturer: Box::new(cap_display_info.capturer.clone()),
            })
        }
    } else {
        bail!(
            "Failed to get capturer display info for display {}",
            display_idx
        );
    }
}

pub fn common_get_error() -> String {
    if DISTRO.name.to_uppercase() == "Ubuntu".to_uppercase() {
        if DISTRO.version_id < "21".to_owned() {
            return "".to_owned();
        }
    } else {
        // to-do: check other distros
    }
    return "".to_owned();
}
