use super::*;
use crate::common::SimpleCallOnReturn;
#[cfg(target_os = "linux")]
use crate::platform::linux::is_x11;
#[cfg(windows)]
use crate::virtual_display_manager;
#[cfg(windows)]
use hbb_common::get_version_number;
use hbb_common::protobuf::MessageField;
use scrap::Display;
use std::sync::atomic::{AtomicBool, Ordering};

// https://github.com/rustdesk/rustdesk/discussions/6042, avoiding dbus call

pub const NAME: &'static str = "display";

#[cfg(windows)]
const DUMMY_DISPLAY_SIDE_MAX_SIZE: usize = 1024;

struct ChangedResolution {
    original: (i32, i32),
    changed: (i32, i32),
}

lazy_static::lazy_static! {
    static ref IS_CAPTURER_MAGNIFIER_SUPPORTED: bool = is_capturer_mag_supported();
    static ref CHANGED_RESOLUTIONS: Arc<RwLock<HashMap<String, ChangedResolution>>> = Default::default();
    static ref SYNC_DISPLAYS: Arc<Mutex<SyncDisplaysInfo>> = Default::default();
}

#[cfg(target_os = "linux")]
lazy_static::lazy_static! {
    static ref WAYLAND_UINPUT_RECT: Mutex<WaylandUinputRect> = Default::default();
    static ref WAYLAND_LAYOUT: Mutex<WaylandLayout> = Default::default();
}

#[cfg(target_os = "linux")]
const WAYLAND_LAYOUT_CHECK_INTERVAL: Duration = Duration::from_millis(1500);

#[cfg(target_os = "linux")]
#[derive(Default)]
struct WaylandUinputRect {
    rect: Option<(i32, i32, i32, i32)>,
    last_check: Option<std::time::Instant>,
}

// Per-display layout used to correct injected coordinates when the compositor moves a
// monitor mid-session. The client keeps sending coordinates offset by the layout it was
// told at session init (`baseline`); we remap them onto the current layout (`live`).
// https://github.com/rustdesk/rustdesk/issues/15601
#[cfg(target_os = "linux")]
#[derive(Default)]
struct WaylandLayout {
    baseline: Vec<scrap::wayland::display::DisplayRect>,
    live: Vec<scrap::wayland::display::DisplayRect>,
    // What the live capturers were built against. Separate from `baseline` because a session
    // init resets that one, and the generation detector needs a memory that a reset cannot
    // erase: two inits straddling a rotation would otherwise leave nothing to compare against.
    seen: Vec<scrap::wayland::display::DisplayRect>,
    // A capturer recorded a build layout other than `seen`, tagged with the generation it was
    // built at: the poll observed the live layout between that capturer's snapshot read and its
    // record, so one of the two is stale and the next poll owes an edge whatever it sees. Only
    // while that generation is current: the record can also land between the poll consuming an
    // edge and the bump it promotes (or after the bump, with a snapshot from before it), and that
    // capturer rebuilds on its own, so a second promotion would tear the fresh ones down again.
    // Consumed by `observe`, which the poll runs right after `edge`; a session init's baseline
    // reset leaves it alone.
    unseen_build: Option<u64>,
}

#[cfg(target_os = "linux")]
impl WaylandLayout {
    // Replace the per-session input baseline. Before the first poll the outgoing baseline is
    // the only record of the layout the capturers were built against, so it seeds `seen`.
    fn reset_baseline(&mut self, baseline: Vec<scrap::wayland::display::DisplayRect>) {
        if self.seen.is_empty() {
            let previous = std::mem::take(&mut self.baseline);
            self.seen = previous;
        }
        self.baseline = baseline;
        self.live.clear();
    }

    // An EDGE (live vs the layout the capturers were built against), not a level: comparing
    // against the baseline latches true for the whole session. With nothing observed yet the
    // baseline is that record, and a missing snapshot at init makes the first success the edge,
    // or transform=0 sticks.
    fn edge(
        &self,
        live: &[scrap::wayland::display::DisplayRect],
        snapshot_missing: bool,
        generation: u64,
    ) -> bool {
        if self.unseen_build == Some(generation) {
            return true;
        }
        if !self.seen.is_empty() {
            return self.seen != live;
        }
        if self.baseline.is_empty() {
            return snapshot_missing;
        }
        self.baseline != live
    }

    fn observe(&mut self, live: &[scrap::wayland::display::DisplayRect]) {
        self.live = live.to_vec();
        self.seen = live.to_vec();
        self.unseen_build = None;
    }

    // What a capturer was built against, which seeds the memory when nothing else has. A session
    // init whose wayland query failed leaves an EMPTY baseline, and the capturer's own retry can
    // then succeed - so the capturer is the only thing that knows the layout it is showing, and
    // without this a rotation before the first poll is invisible to `edge`. Only when empty: a
    // capturer built later must not overwrite the memory the poll is keeping, since on a
    // multi-display session that memory is what the OTHER capturers were built against. A build
    // that disagrees with it is flagged instead: the capturer's snapshot read and this record
    // are two steps, and a poll landing between them observes the live layout first, which
    // would otherwise drop the record and leave the capturer on a transform nothing compares.
    fn note_capturer(&mut self, built_on: &[scrap::wayland::display::DisplayRect], built_gen: u64) {
        if built_on.is_empty() {
            return;
        }
        if self.seen.is_empty() {
            self.seen = built_on.to_vec();
        } else if self.seen != built_on {
            // The newest generation wins: a stale record landing late must not hide a fresh one.
            self.unseen_build = Some(self.unseen_build.map_or(built_gen, |g| g.max(built_gen)));
        }
    }
}

// Whether `live` differs from `baseline`. Read on every mouse move, so it is an atomic:
// the common (no-drift) case never touches the layout mutex.
#[cfg(target_os = "linux")]
static WAYLAND_LAYOUT_DRIFTED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "linux")]
pub(super) fn set_wayland_uinput_rect(rect: (i32, i32, i32, i32)) {
    WAYLAND_UINPUT_RECT.lock().unwrap().rect = Some(rect);
}

// The uinput ABS range currently programmed into the device, for the DRM path's "reapply only when
// it changed" check. The PipeWire path compares it inline in refresh_wayland_uinput_rect_if_changed.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) fn wayland_uinput_rect() -> Option<(i32, i32, i32, i32)> {
    WAYLAND_UINPUT_RECT.lock().unwrap().rect
}

#[cfg(target_os = "linux")]
pub(super) fn set_wayland_layout_baseline(baseline: Vec<scrap::wayland::display::DisplayRect>) {
    WAYLAND_LAYOUT_DRIFTED.store(false, Ordering::Relaxed);
    WAYLAND_LAYOUT.lock().unwrap().reset_baseline(baseline);
}

/// Record the layout a capturer was just built against, and the snapshot generation it read
/// before taking that layout. See `WaylandLayout::note_capturer`.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) fn note_capturer_layout(
    displays: &[hbb_common::platform::linux::WaylandDisplayInfo],
    built_gen: u64,
) {
    if displays.is_empty() {
        return;
    }
    let rects = scrap::wayland::display::logical_rects_of_displays(displays);
    WAYLAND_LAYOUT
        .lock()
        .unwrap()
        .note_capturer(&rects, built_gen);
}

// Remap an injected coordinate onto the live compositor layout when it has drifted from
// what the client was told at session init. Lock-free no-op otherwise.
#[cfg(target_os = "linux")]
pub(super) fn remap_wayland_uinput_coord(x: i32, y: i32) -> (i32, i32) {
    if !WAYLAND_LAYOUT_DRIFTED.load(Ordering::Relaxed) {
        return (x, y);
    }
    let lock = WAYLAND_LAYOUT.lock().unwrap();
    scrap::wayland::display::remap_to_live_layout(x, y, &lock.baseline, &lock.live)
}

// The uinput absolute range is set when the session inits. If the compositor layout
// changes afterwards (monitor scale/position change, or a portal virtual output
// appearing once the capture starts), injected coordinates get rescaled by the stale
// range and land offset, https://github.com/rustdesk/rustdesk/issues/15601
#[cfg(target_os = "linux")]
fn refresh_wayland_uinput_rect_if_changed() {
    if is_x11() || !crate::input_service::wayland_use_uinput() {
        return;
    }
    {
        let mut lock = WAYLAND_UINPUT_RECT.lock().unwrap();
        if let Some(last_check) = lock.last_check {
            if last_check.elapsed() < WAYLAND_LAYOUT_CHECK_INTERVAL {
                return;
            }
        }
        lock.last_check = Some(std::time::Instant::now());
    }
    let Some((rect, live_rects)) = scrap::wayland::display::get_layout_for_uinput_live() else {
        return;
    };
    // Refresh the per-display layout every poll: monitor origins can shift (e.g. two
    // displays swap positions) without changing the overall desktop rect, and the mouse
    // path needs the current per-display geometry to correct coordinates.
    let (live_changed, mut drifted) = {
        let mut layout = WAYLAND_LAYOUT.lock().unwrap();
        #[cfg(feature = "drm")]
        let snapshot_missing = scrap::wayland::display::wayland_snapshot_missing();
        #[cfg(not(feature = "drm"))]
        let snapshot_missing = false;
        #[cfg(feature = "drm")]
        let generation = scrap::wayland::display::wayland_snapshot_generation();
        #[cfg(not(feature = "drm"))]
        let generation = 0;
        let live_changed = layout.edge(&live_rects, snapshot_missing, generation);
        let drifted = !layout.baseline.is_empty()
            && !live_rects.is_empty()
            && layout.baseline != live_rects;
        layout.observe(&live_rects);
        (live_changed, drifted)
    };
    // Single owner of the generation bump: on the cache clear it let every session init tear
    // down every other live capturer. Baseline promotes with the clear (rustdesk#15601).
    #[cfg(feature = "drm")]
    {
        // An edge seen while DRM is transiently non-Available stays OWED rather than consumed.
        static PROMOTION_OWED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        // The latch fires when a capturer was built with no wayland snapshot: a later cache
        // refill makes wayland_snapshot_missing lie, so live_changed alone would miss it. Taken
        // UNCONDITIONALLY: short-circuiting past it on a live_changed poll would leave it set and
        // spend a second, spurious promotion one poll later on the freshly rebuilt capturer.
        let blind_build = super::drm_capturer::take_unrotated_snapshot_pending();
        if live_changed || blind_build {
            PROMOTION_OWED.store(true, Ordering::Release);
        }
        if PROMOTION_OWED.load(Ordering::Acquire) && super::drm_capturer::is_available_cached() {
            PROMOTION_OWED.store(false, Ordering::Release);
            scrap::wayland::display::clear_wayland_displays_cache();
            scrap::wayland::display::bump_layout_generation();
            set_wayland_layout_baseline(live_rects.clone());
            WAYLAND_LAYOUT.lock().unwrap().live = live_rects.clone();
            drifted = false;
        }
    }
    #[cfg(not(feature = "drm"))]
    let _ = live_changed;
    // At a login screen the DRM path owns the rect; only the range/remap update is skipped,
    // the snapshot invalidation above must still run (a greeter session has no other trigger).
    #[cfg(feature = "drm")]
    if crate::platform::linux::is_login_screen_wayland_cached() {
        return;
    }
    // The remap corrects for per-display origin shifts; the uinput ABS range corrects for
    // the overall bounding box. Only enable the remap once the range matches the live
    // layout, otherwise moves would be remapped into a range the device is not yet using.
    // A drift with no bbox change (origins swapped) needs no range update and enables now.
    let mut range_ok = WAYLAND_UINPUT_RECT.lock().unwrap().rect == Some(rect);
    if !range_ok {
        let (minx, maxx, miny, maxy) = rect;
        log::info!(
            "desktop layout changed, update mouse resolution: ({}, {}), ({}, {})",
            minx,
            maxx,
            miny,
            maxy
        );
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => {
                // Bound the IPC wait, this runs on the display service loop and
                // `set_resolution()` has no timeout on the response read.
                // timeout must be built inside the runtime, or it panics
                // "there is no reactor running". See clipboard_service.rs.
                match rt.block_on(async {
                    timeout(
                        3_000,
                        crate::input_service::update_mouse_resolution(minx, maxx, miny, maxy),
                    )
                    .await
                }) {
                    // Record the rect only after a successful apply, so a transient
                    // failure is retried on the next check.
                    Ok(Ok(())) => {
                        WAYLAND_UINPUT_RECT.lock().unwrap().rect = Some(rect);
                        range_ok = true;
                    }
                    Ok(Err(err)) => log::error!("Failed to update mouse resolution: {}", err),
                    Err(err) => log::error!("Failed to update mouse resolution: {}", err),
                }
            }
            Err(err) => {
                log::error!("Failed to build tokio runtime: {}", err);
            }
        }
    }
    // Publish the flag last: a `true` read is always backed by a current `live` and a
    // matching uinput range. A failed range apply leaves this false and retries next poll.
    WAYLAND_LAYOUT_DRIFTED.store(drifted && range_ok, Ordering::Relaxed);
}

// https://github.com/rustdesk/rustdesk/pull/8537
static TEMP_IGNORE_DISPLAYS_CHANGED: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct SyncDisplaysInfo {
    displays: Vec<DisplayInfo>,
    is_synced: bool,
}

impl SyncDisplaysInfo {
    fn check_changed(&mut self, displays: &[DisplayInfo]) {
        if self.displays.as_slice() == displays {
            return;
        }

        self.displays = displays.to_vec();
        if !TEMP_IGNORE_DISPLAYS_CHANGED.load(Ordering::Relaxed) {
            self.is_synced = false;
        }
    }

    fn get_update_sync_displays(&mut self) -> Option<Vec<DisplayInfo>> {
        if self.is_synced {
            return None;
        }
        self.is_synced = true;
        Some(self.displays.clone())
    }
}

pub fn temp_ignore_displays_changed() -> SimpleCallOnReturn {
    TEMP_IGNORE_DISPLAYS_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
    SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            // Wait for a while to make sure check_display_changed() is called
            // after video service has sending its `SwitchDisplay` message(`try_broadcast_display_changed()`).
            std::thread::sleep(Duration::from_millis(1000));
            TEMP_IGNORE_DISPLAYS_CHANGED.store(false, Ordering::Relaxed);
            // Trigger the display changed message.
            SYNC_DISPLAYS.lock().unwrap().is_synced = false;
        }),
    }
}

// This function is really useful, though a duplicate check if display changed.
// The video server will then send the following messages to the client:
//  1. the supported resolutions of the {idx} display
//  2. the switch resolution message, so that the client can record the custom resolution.
pub(super) fn check_display_changed(
    ndisplay: usize,
    idx: usize,
    (x, y, w, h): (i32, i32, usize, usize),
) -> Option<DisplayInfo> {
    #[cfg(target_os = "linux")]
    {
        // wayland do not support changing display for now
        if !is_x11() {
            return None;
        }
    }

    let lock = SYNC_DISPLAYS.lock().unwrap();
    // If plugging out a monitor && lock.displays.get(idx) is None.
    //  1. The client version < 1.2.4. The client side has to reconnect.
    //  2. The client version > 1.2.4, The client side can handle the case because sync peer info message will be sent.
    // But it is acceptable to for the user to reconnect manually, because the monitor is unplugged.
    let d = lock.displays.get(idx)?;
    if ndisplay != lock.displays.len() {
        return Some(d.clone());
    }
    if !(d.x == x && d.y == y && d.width == w as i32 && d.height == h as i32) {
        Some(d.clone())
    } else {
        None
    }
}

#[inline]
pub fn set_last_changed_resolution(display_name: &str, original: (i32, i32), changed: (i32, i32)) {
    let mut lock = CHANGED_RESOLUTIONS.write().unwrap();
    match lock.get_mut(display_name) {
        Some(res) => res.changed = changed,
        None => {
            lock.insert(
                display_name.to_owned(),
                ChangedResolution { original, changed },
            );
        }
    }
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn restore_resolutions() {
    for (name, res) in CHANGED_RESOLUTIONS.read().unwrap().iter() {
        let (w, h) = res.original;
        log::info!("Restore resolution of display '{}' to ({}, {})", name, w, h);
        if let Err(e) = crate::platform::change_resolution(name, w as _, h as _) {
            log::error!(
                "Failed to restore resolution of display '{}' to ({},{}): {}",
                name,
                w,
                h,
                e
            );
        }
    }
    // Can be cleared because restore resolutions is called when there is no client connected.
    CHANGED_RESOLUTIONS.write().unwrap().clear();
}

#[inline]
fn is_capturer_mag_supported() -> bool {
    #[cfg(windows)]
    return scrap::CapturerMag::is_supported();
    #[cfg(not(windows))]
    false
}

#[inline]
pub fn capture_cursor_embedded() -> bool {
    scrap::is_cursor_embedded()
}

#[inline]
#[cfg(windows)]
pub fn is_privacy_mode_mag_supported() -> bool {
    return *IS_CAPTURER_MAGNIFIER_SUPPORTED
        && get_version_number(&crate::VERSION) > get_version_number("1.1.9");
}

pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::run(&svc.clone(), run);
    svc.sp
}

fn displays_to_msg(displays: Vec<DisplayInfo>) -> Message {
    let mut pi = PeerInfo {
        ..Default::default()
    };
    pi.displays = displays.clone();

    #[cfg(windows)]
    if crate::platform::is_installed() {
        let m = crate::virtual_display_manager::get_platform_additions();
        pi.platform_additions = serde_json::to_string(&m).unwrap_or_default();
    }

    // current_display should not be used in server.
    // It is set to 0 for compatibility with old clients.
    pi.current_display = 0;
    let mut msg_out = Message::new();
    msg_out.set_peer_info(pi);
    msg_out
}

fn check_get_displays_changed_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    {
        if !is_x11() {
            // On the DRM/KMS capture path the PipeWire enumeration (which is what feeds
            // `SYNC_DISPLAYS` via `check_update_displays`) is bypassed, so populate the sync list
            // from the DRM display list here. Without this the display service broadcasts an empty
            // list that overwrites the login peer-info displays and the client shows "No displays".
            #[cfg(feature = "drm")]
            if super::drm_capturer::is_available_cached() {
                let synced = !SYNC_DISPLAYS.lock().unwrap().displays.is_empty();
                let stamped_before = scrap::wayland::display::wayland_failure_stamped();
                // With nothing published yet, even the unaugmented DRM list beats the empty
                // broadcast below; with a synced layout, a suppressed turn keeps it instead.
                if !synced || !scrap::wayland::display::wayland_lookup_suppressed() {
                    if let Some(displays) = super::drm_capturer::get_display_infos() {
                        // A first failure keeps the synced layout for one backoff; only a
                        // failure that persists across one replaces it with the DRM stack.
                        if !synced
                            || stamped_before
                            || !scrap::wayland::display::wayland_lookup_suppressed()
                        {
                            SYNC_DISPLAYS.lock().unwrap().check_changed(&displays);
                        }
                    }
                }
            }
            return get_displays_msg();
        }
    }
    check_update_displays(&try_get_displays().ok()?);
    get_displays_msg()
}

pub fn check_displays_changed() -> ResultType<()> {
    #[cfg(target_os = "linux")]
    {
        // Currently, wayland need to call wayland::clear() before call Display::all(), otherwise it will cause
        // block, or even crash here, https://github.com/rustdesk/rustdesk/blob/0bb4d43e9ea9d9dfb9c46c8d27d1a97cd0ad6bea/libs/scrap/src/wayland/pipewire.rs#L235
        if !is_x11() {
            return Ok(());
        }
    }
    check_update_displays(&try_get_displays()?);
    Ok(())
}

fn get_displays_msg() -> Option<Message> {
    let displays = SYNC_DISPLAYS.lock().unwrap().get_update_sync_displays()?;
    Some(displays_to_msg(displays))
}

fn run(sp: EmptyExtraFieldService) -> ResultType<()> {
    while sp.ok() {
        sp.snapshot(|sps| {
            if !TEMP_IGNORE_DISPLAYS_CHANGED.load(Ordering::Relaxed) {
                if sps.has_subscribes() {
                    SYNC_DISPLAYS.lock().unwrap().is_synced = false;
                    bail!("new subscriber");
                }
            }
            Ok(())
        })?;

        if let Some(msg_out) = check_get_displays_changed_msg() {
            sp.send(msg_out);
            log::info!("Displays changed");
        }

        #[cfg(target_os = "linux")]
        if sp.has_subscribes() {
            refresh_wayland_uinput_rect_if_changed();
        }

        std::thread::sleep(Duration::from_millis(300));
    }

    Ok(())
}

#[inline]
pub(super) fn get_original_resolution(
    display_name: &str,
    w: usize,
    h: usize,
) -> MessageField<Resolution> {
    #[cfg(windows)]
    let is_rustdesk_virtual_display =
        crate::virtual_display_manager::rustdesk_idd::is_virtual_display(&display_name);
    #[cfg(not(windows))]
    let is_rustdesk_virtual_display = false;
    Some(if is_rustdesk_virtual_display {
        Resolution {
            width: 0,
            height: 0,
            ..Default::default()
        }
    } else {
        let changed_resolutions = CHANGED_RESOLUTIONS.write().unwrap();
        let (width, height) = match changed_resolutions.get(display_name) {
            Some(res) => {
                res.original
                /*
                The resolution change may not happen immediately, `changed` has been updated,
                but the actual resolution is old, it will be mistaken for a third-party change.
                if res.changed.0 != w as i32 || res.changed.1 != h as i32 {
                    // If the resolution is changed by third process, remove the record in changed_resolutions.
                    changed_resolutions.remove(display_name);
                    (w as _, h as _)
                } else {
                    res.original
                }
                */
            }
            None => (w as _, h as _),
        };
        Resolution {
            width,
            height,
            ..Default::default()
        }
    })
    .into()
}

pub(super) fn get_sync_displays() -> Vec<DisplayInfo> {
    SYNC_DISPLAYS.lock().unwrap().displays.clone()
}

pub(super) fn get_display_info(idx: usize) -> Option<DisplayInfo> {
    SYNC_DISPLAYS.lock().unwrap().displays.get(idx).cloned()
}

// True when at least one advertised (synced) display is NOT served by the DRM/KMS capture path,
// i.e. a mixed DRM + PipeWire session. The cursor service (platform::linux::get_cursor /
// get_cursor_data) uses this to decide whether a hidden DRM hardware-cursor sentinel is
// authoritative: in a pure-DRM session it is (the pointer is genuinely off every captured CRTC),
// but in a mixed session the sentinel only means the pointer moved onto a PipeWire-served display,
// whose cursor must come from the normal path instead of being hidden everywhere.
//
// When DRM capture is active the advertised list is enumerated from the DRM display list, so a DRM
// list shorter than the synced list means at least one advertised display is served by PipeWire.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub fn has_non_drm_backed_display() -> bool {
    match super::drm_capturer::display_count_and_any_demoted() {
        // A display served by PipeWire is either ABSENT from the DRM list (a shorter count, e.g. a
        // pure-portal display) or PRESENT-BUT-DEMOTED (kept in place at the same index and marked
        // offline so the index space stays aligned -- see get_display_infos). The count check alone
        // misses the demotion case (same count), so a demoted display is treated as non-DRM-backed
        // too. This is what gates the hidden-cursor sentinel: it stays authoritative only in a
        // pure-DRM session. The scalar accessor is deliberate: this is polled every cursor tick
        // while the sentinel is active, and cloning + geometry-augmenting the whole list per tick
        // (what get_display_infos does) answered the same two facts.
        Some((count, any_demoted)) => {
            count < SYNC_DISPLAYS.lock().unwrap().displays.len() || any_demoted
        }
        None => false,
    }
}

// Display to DisplayInfo
// The DisplayInfo is be sent to the peer.
pub(super) fn check_update_displays(all: &Vec<Display>) {
    let _ = update_sync_displays(all);
}

/// Whether there is a compositor on this seat worth asking. `get_displays()` does not cache
/// its failure, so where there is none it re-probes every call for an answer that cannot
/// change any caller's outcome. Last in the `&&` chain, so it never runs first on a poll.
#[inline]
#[cfg(target_os = "linux")]
fn wayland_has_compositor() -> bool {
    #[cfg(feature = "drm")]
    {
        !crate::platform::linux::is_login_screen_wayland_cached()
    }
    #[cfg(not(feature = "drm"))]
    {
        true
    }
}

// Return the converted input snapshot while updating the shared display cache.
pub(super) fn update_sync_displays(all: &Vec<Display>) -> Vec<DisplayInfo> {
    // For compatibility: if only one display, scale remains 1.0 and we use the physical size for `uinput`.
    // If there are multiple displays, we use the logical size for `uinput` by setting scale to d.scale().
    #[cfg(target_os = "linux")]
    let use_logical_scale = !is_x11()
        && crate::is_server()
        && wayland_has_compositor()
        && scrap::wayland::display::get_displays().displays.len() > 1;
    let displays = all
        .iter()
        .map(|d| {
            let display_name = d.name();
            #[allow(unused_assignments)]
            #[allow(unused_mut)]
            let mut scale = 1.0;
            #[cfg(target_os = "macos")]
            {
                scale = d.scale();
            }
            #[cfg(target_os = "linux")]
            {
                if use_logical_scale {
                    scale = d.scale();
                }
            }
            let original_resolution = get_original_resolution(
                &display_name,
                ((d.width() as f64) / scale).round() as usize,
                (d.height() as f64 / scale).round() as usize,
            );
            DisplayInfo {
                x: d.origin().0 as _,
                y: d.origin().1 as _,
                width: d.width() as _,
                height: d.height() as _,
                name: display_name,
                online: d.is_online(),
                cursor_embedded: false,
                original_resolution,
                scale,
                ..Default::default()
            }
        })
        .collect::<Vec<DisplayInfo>>();
    SYNC_DISPLAYS.lock().unwrap().check_changed(&displays);
    displays
}

pub fn is_inited_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    if !is_x11() {
        return super::wayland::is_inited();
    }
    None
}

// Return the primary index with the refreshed list so login cannot mix display snapshots.
pub async fn update_get_sync_displays_on_login() -> ResultType<(Vec<DisplayInfo>, usize)> {
    #[cfg(target_os = "linux")]
    {
        if !is_x11() {
            let (displays, primary_display_idx) =
                super::wayland::get_displays_and_primary().await?;
            let primary_display_idx =
                normalize_primary_display_idx(primary_display_idx, displays.len());
            return Ok((displays, primary_display_idx));
        }
    }
    #[cfg(not(windows))]
    let displays = display_service::try_get_displays();
    #[cfg(windows)]
    let displays = display_service::try_get_displays_add_amyuni_headless();
    let displays = displays?;
    let primary_display_idx = get_primary_2(&displays);
    let sync_displays = update_sync_displays(&displays);
    let primary_display_idx =
        normalize_primary_display_idx(primary_display_idx, sync_displays.len());
    Ok((sync_displays, primary_display_idx))
}

#[inline]
fn normalize_primary_display_idx(primary_display_idx: usize, display_len: usize) -> usize {
    // Zero is the protocol fallback when the list is empty or its primary index is stale.
    if primary_display_idx < display_len {
        primary_display_idx
    } else {
        0
    }
}

#[inline]
pub fn get_primary_2(all: &Vec<Display>) -> usize {
    all.iter().position(|d| d.is_primary()).unwrap_or(0)
}

#[inline]
#[cfg(windows)]
fn no_displays(displays: &Vec<Display>) -> bool {
    let display_len = displays.len();
    if display_len == 0 {
        true
    } else if display_len == 1 {
        let display = &displays[0];
        if display.width() > DUMMY_DISPLAY_SIDE_MAX_SIZE
            || display.height() > DUMMY_DISPLAY_SIDE_MAX_SIZE
        {
            return false;
        }
        let any_real = crate::platform::resolutions(&display.name())
            .iter()
            .any(|r| {
                (r.height as usize) > DUMMY_DISPLAY_SIDE_MAX_SIZE
                    || (r.width as usize) > DUMMY_DISPLAY_SIDE_MAX_SIZE
            });
        !any_real
    } else {
        false
    }
}

#[inline]
#[cfg(not(windows))]
pub fn try_get_displays() -> ResultType<Vec<Display>> {
    Ok(Display::all()?)
}

#[inline]
#[cfg(windows)]
pub fn try_get_displays() -> ResultType<Vec<Display>> {
    try_get_displays_(false)
}

// We can't get full control of the virtual display if we use amyuni idd.
// If we add a virtual display, we cannot remove it automatically.
// So when using amyuni idd, we only add a virtual display for headless if it is required.
// eg. when the client is connecting.
#[inline]
#[cfg(windows)]
pub fn try_get_displays_add_amyuni_headless() -> ResultType<Vec<Display>> {
    try_get_displays_(true)
}

#[inline]
#[cfg(windows)]
pub fn try_get_displays_(add_amyuni_headless: bool) -> ResultType<Vec<Display>> {
    let mut displays = Display::all()?;

    // Do not add virtual display if the platform is not installed or the virtual display is not supported.
    if !crate::platform::is_installed() || !virtual_display_manager::is_virtual_display_supported()
    {
        return Ok(displays);
    }

    // Enable headless virtual display when
    // 1. `amyuni` idd is not used.
    // 2. `amyuni` idd is used and `add_amyuni_headless` is true.
    if virtual_display_manager::is_amyuni_idd() && !add_amyuni_headless {
        return Ok(displays);
    }

    // The following code causes a bug.
    // The virtual display cannot be added when there's no session(eg. when exiting from RDP).
    // Because `crate::platform::desktop_changed()` always returns true at that time.
    //
    // The code only solves a rare case:
    // 1. The control side is connecting.
    // 2. The windows session is switching, no displays are detected, but they're there.
    // Then the controlled side plugs in a virtual display for "headless".
    //
    // No need to do the following check. But the code is kept here for marking the issue.
    // If there're someones reporting the issue, we may add a better check by waiting for a while. (switching session).
    // But I don't think it's good to add the timeout check without any issue.
    //
    // If is switching session, no displays may be detected.
    // if displays.is_empty() && crate::platform::desktop_changed() {
    //     return Ok(displays);
    // }

    let no_displays_v = no_displays(&displays);
    if no_displays_v {
        log::debug!("no displays, create virtual display");
        if let Err(e) = virtual_display_manager::plug_in_headless() {
            log::error!("plug in headless failed {}", e);
        } else {
            displays = Display::all()?;
        }
    }
    Ok(displays)
}

#[cfg(test)]
mod tests {
    use super::normalize_primary_display_idx;

    #[test]
    fn normalize_primary_display_idx_bounds() {
        assert_eq!(normalize_primary_display_idx(0, 0), 0);
        assert_eq!(normalize_primary_display_idx(0, 2), 0);
        assert_eq!(normalize_primary_display_idx(1, 2), 1);
        assert_eq!(normalize_primary_display_idx(2, 2), 0);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod wayland_layout_tests {
    use super::WaylandLayout;
    use scrap::wayland::display::DisplayRect;

    fn layout(w: i32, h: i32, transform: i32) -> Vec<DisplayRect> {
        vec![DisplayRect {
            name: "DP-1".into(),
            x: 0,
            y: 0,
            w,
            h,
            transform,
        }]
    }

    // rustdesk#15886: a video service starts, the output rotates, and a retry starts before the
    // 1.5 s poll. The baseline is reset on both, so it cannot be the edge detector's memory.
    #[test]
    fn a_rotation_between_two_session_inits_is_still_an_edge() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(upright.clone());
        l.observe(&upright);
        l.reset_baseline(upright.clone());
        l.reset_baseline(rotated.clone());
        assert!(l.edge(&rotated, false, 0));
    }

    // The same, with no poll ever having run: the outgoing baseline is the only record of what
    // the first capturer was built against.
    #[test]
    fn a_rotation_between_two_inits_before_the_first_poll_is_still_an_edge() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(upright.clone());
        l.reset_baseline(rotated.clone());
        assert!(l.edge(&rotated, false, 0));
    }

    // Control: without it the asserts above would pass on a detector that always fires.
    #[test]
    fn repeated_baseline_resets_without_a_rotation_are_not_an_edge() {
        let upright = layout(1920, 1080, 0);
        let mut l = WaylandLayout::default();
        l.reset_baseline(upright.clone());
        l.observe(&upright);
        l.reset_baseline(upright.clone());
        l.reset_baseline(upright.clone());
        assert!(!l.edge(&upright, false, 0));
    }

    // rustdesk#15886: `ensure_inited()` runs the wayland query BEFORE the capturer exists, and a
    // failure there saves an EMPTY baseline. The capturer's own retry can succeed a moment later
    // and build on layout A, and that build is not blind, so nothing else records it. A rotation
    // before the first poll then had no memory to be an edge against.
    #[test]
    fn a_capturer_built_after_a_failed_init_still_owes_a_rebuild() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);

        let mut l = WaylandLayout::default();
        l.reset_baseline(Vec::new());
        l.note_capturer(&upright, 0);
        assert!(l.edge(&rotated, false, 0));

        // The same with another baseline reset between the build and the poll.
        let mut l2 = WaylandLayout::default();
        l2.reset_baseline(Vec::new());
        l2.note_capturer(&upright, 0);
        l2.reset_baseline(rotated.clone());
        assert!(l2.edge(&rotated, false, 0));

        // Control: no rotation, no edge, in both shapes.
        let mut l3 = WaylandLayout::default();
        l3.reset_baseline(Vec::new());
        l3.note_capturer(&upright, 0);
        assert!(!l3.edge(&upright, false, 0));
    }

    // A capturer built while the poll already has a memory must not overwrite it.
    #[test]
    fn a_later_capturer_does_not_overwrite_the_polls_memory() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.observe(&upright);
        l.note_capturer(&rotated, 0);
        assert!(l.edge(&rotated, false, 0), "the poll's memory still says upright");
    }

    // The constructor's snapshot read and its `note_capturer` are two steps, and the poll can
    // land between them. After a failed init (empty baseline) the constructor takes A and
    // publishes it; the output rotates; the poll reads B live, finds nothing recorded and the
    // snapshot present, so no edge, and observes B. The late `note_capturer(A)` then met a
    // non-empty memory and was dropped: the capturer showed A while the detector held B, and B
    // against B never bumped the generation.
    #[test]
    fn a_capturer_record_that_lost_the_race_with_the_first_poll_is_still_an_edge() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(Vec::new());
        assert!(!l.edge(&rotated, false, 0), "nothing recorded and the snapshot is present");
        l.observe(&rotated);
        l.note_capturer(&upright, 0);
        assert!(l.edge(&rotated, false, 0), "the capturer is built on upright, live is rotated");

        // The promotion consumes it: the next poll sees the same layout and stays quiet.
        l.observe(&rotated);
        l.reset_baseline(rotated.clone());
        assert!(!l.edge(&rotated, false, 0));

        // The same with a session init between the late record and the poll.
        let mut l2 = WaylandLayout::default();
        l2.reset_baseline(Vec::new());
        l2.observe(&rotated);
        l2.note_capturer(&upright, 0);
        l2.reset_baseline(rotated.clone());
        assert!(l2.edge(&rotated, false, 0));

        // Control: a late record that agrees with the poll's memory is not an edge.
        let mut l3 = WaylandLayout::default();
        l3.reset_baseline(Vec::new());
        l3.observe(&upright);
        l3.note_capturer(&upright, 0);
        assert!(!l3.edge(&upright, false, 0));
    }

    // The late record can also land after the poll consumed the edge but before the bump that
    // edge promotes, or after the bump with a snapshot taken before it. That capturer is stale
    // by generation and rebuilds on its own, so its record must not buy a second promotion
    // that tears the freshly rebuilt capturers down again.
    #[test]
    fn a_late_record_from_a_generation_already_promoted_is_not_a_second_edge() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(upright.clone());
        l.observe(&upright);
        // The output rotates, the poll consumes the edge, the capturer built on upright at
        // generation 7 records late, and the poll promotes to 8.
        assert!(l.edge(&rotated, false, 7));
        l.observe(&rotated);
        l.note_capturer(&upright, 7);
        l.reset_baseline(rotated.clone());
        assert!(!l.edge(&rotated, false, 8), "the capturer built at 7 rebuilds on its own");

        // Control: a disagreeing record AT the promoted generation is a real edge.
        l.observe(&rotated);
        l.note_capturer(&upright, 8);
        assert!(l.edge(&rotated, false, 8));

        // A stale record landing after a fresh one must not hide the fresh one.
        l.observe(&rotated);
        l.note_capturer(&upright, 8);
        l.note_capturer(&upright, 7);
        assert!(l.edge(&rotated, false, 8));
    }

    // A promotion consumes the edge: the next poll sees the same layout and must stay quiet.
    #[test]
    fn a_promoted_layout_is_not_an_edge_again() {
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(layout(1920, 1080, 0));
        l.observe(&rotated);
        l.reset_baseline(rotated.clone());
        assert!(!l.edge(&rotated, false, 0));
    }
}
