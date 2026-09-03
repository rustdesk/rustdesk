// Unprivileged consumer of the root `--service`'s DRM/KMS capture stream: the service does the
// privileged export (open + grab the scanout dma-buf fd), the EGL detile / RGBA convert runs here.

use crate::ipc::{connect_drm, Data, DrmDisplayInfo};
use hbb_common::{anyhow::anyhow, bail, log, message_proto::DisplayInfo, tokio, ResultType};
use scrap::drm_render::RenderConverter;
use scrap::drmtap_dl::drmtap_dmabuf_desc;
use scrap::{Frame, Pixfmt, PixelBuffer, TraitCapturer};
use std::collections::BTreeMap;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const HANDSHAKE_TIMEOUT_MS: u64 = 3000;
const DRM_CONNECT_TIMEOUT_MS: u64 = 1000;
/// The service may hold the list back while it wakes sleeping displays: ~3.6s (DRM_WAKE_*).
const DISPLAY_LIST_TIMEOUT_MS: u64 = HANDSHAKE_TIMEOUT_MS + 4000;
/// Covers the connect timeout plus `recv_msg_timeout2` applying DISPLAY_LIST_TIMEOUT_MS TWICE
/// (first byte, then body). The render-node open and the DrmStart send can still overrun it.
const HANDSHAKE_WAIT_MS: u64 = DRM_CONNECT_TIMEOUT_MS + DISPLAY_LIST_TIMEOUT_MS * 2 + 500;
/// Only the header read rechecks `stop`, so bound the body read here rather than relying on
    /// `next_raw_into`'s own cap.
const BODY_READ_TIMEOUT: Duration = Duration::from_secs(5);

struct FrameSlot {
    // Row stride is `pixels.len() / height`, possibly padded; the format is per frame.
    latest: Option<(usize, usize, Pixfmt, Vec<u8>)>,
    // TWO slots: two buffers can be idle at once -- the receive path takes one and publishes in two
    // SEPARATE acquisitions, so the encoder can hand its borrow back in between.
    free: [Option<Vec<u8>>; 2],
    ended: Option<String>,
}

impl FrameSlot {
    fn publish(&mut self, w: usize, h: usize, fmt: Pixfmt, buf: Vec<u8>) {
        if let Some((.., old)) = self.latest.take() {
            self.recycle(old);
        }
        self.latest = Some((w, h, fmt, buf));
    }

    fn recycle(&mut self, buf: Vec<u8>) {
        if let Some(slot) = self.free.iter_mut().find(|s| s.is_none()) {
            *slot = Some(buf);
        }
    }

    fn take_free(&mut self) -> Option<Vec<u8>> {
        self.free.iter_mut().find_map(|s| s.take())
    }
}

/// `Shared.transform` before new() stores the real value: a cursor arriving this early is held
/// back and replayed once the session transform is in, because the producer will not resend it
/// until the shape changes.
const TRANSFORM_PENDING: i32 = i32::MIN;

struct Shared {
    slot: Mutex<FrameSlot>,
    cv: Condvar,
    // Session transform, TRANSFORM_PENDING until new() stores it post-handshake; the receive
    // thread turns cursor bitmaps with it and defers any cursor that races the store.
    transform: std::sync::atomic::AtomicI32,
}

pub struct IpcDrmCapturer {
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    display: i32,
    connector: Option<String>,
    // What the encoder was sized from: CapturerInfo{width,height} is read once, at build time.
    // With a rotated output these are the ROTATED dimensions, matching the frames delivered.
    session_size: Option<(usize, usize)>,
    // Output rotation in degrees: a rotated scanout holds the desktop drawn sideways, so frames
    // are turned back before delivery. Fixed per session; a rotation rebuilds the capturer.
    transform: i32,
    // The wayland snapshot generation this session was built from: a later invalidation means
    // the layout (a rotation included) may have changed, and frame() asks for a rebuild.
    snapshot_gen: u64,
    cur: Vec<u8>,
    cur_w: usize,
    cur_h: usize,
    cur_fmt: Pixfmt,
    got_frame: bool,
}

/// A list index is NOT an identity: `drm_enumerate_all_displays` concatenates per-card lists.
fn connector_key(d: &DrmDisplayInfo) -> String {
    format!("{}:{}", d.device, d.name)
}

/// Frame dimensions after undoing `transform` degrees of output rotation.
fn rotated_dims(transform: i32, w: usize, h: usize) -> (usize, usize) {
    if transform == 90 || transform == 270 {
        (h, w)
    } else {
        (w, h)
    }
}

/// Hotspot of a rotated cursor bitmap: the same point mapping `unrotate_bgra` applies to
/// pixels, applied to the one coordinate that must keep naming the click point.
fn unrotate_hotspot(transform: i32, w: i32, h: i32, hotx: i32, hoty: i32) -> (i32, i32) {
    match transform {
        90 => (h - 1 - hoty, hotx),
        180 => (w - 1 - hotx, h - 1 - hoty),
        270 => (hoty, w - 1 - hotx),
        _ => (hotx, hoty),
    }
}

/// Turn a 4-byte-pixel frame upright into tightly packed `dst`, undoing `transform` degrees;
/// padded `src` rows ok (stride = len/h). Direction pinned by the tests to the measured anchor
/// of rustdesk#15886; libyuv walks pixels, so channel order does not matter.
fn unrotate_bgra(src: &[u8], w: usize, h: usize, transform: i32, dst: &mut Vec<u8>) {
    const PX: usize = 4;
    let stride = if h > 0 { src.len() / h } else { 0 };
    let (dw, dh) = rotated_dims(transform, w, h);
    dst.resize(
        dw.checked_mul(dh).and_then(|p| p.checked_mul(PX)).unwrap_or(0),
        0,
    );
    if dst.is_empty() || stride < w * PX {
        log::error!("unrotate: rejected geometry {w}x{h} stride {stride}; frame left blank");
        return;
    }
    let mode = match transform {
        90 => scrap::RotationMode::kRotate90,
        180 => scrap::RotationMode::kRotate180,
        270 => scrap::RotationMode::kRotate270,
        _ => scrap::RotationMode::kRotate0,
    };
    unsafe {
        scrap::ARGBRotate(
            src.as_ptr(),
            stride as i32,
            dst.as_mut_ptr(),
            (dw * PX) as i32,
            w as i32,
            h as i32,
            mode,
        );
    }
}

/// Transform and augmented origin for one wire entry, derived from ONE wayland snapshot so both
/// reflect the same output assignment; two `get_displays()` reads could straddle a cache
/// invalidation. `None` origin means nothing to augment with (caller keeps the DRM origin).
fn transform_and_origin(
    drm: &[DrmDisplayInfo],
    wire_idx: usize,
    wl: &scrap::wayland::display::Displays,
) -> (i32, Option<(i32, i32)>) {
    if wl.displays.is_empty() || (wl.displays.len() == 1 && drm.len() > 1) {
        if wl.displays.is_empty() && !drm.is_empty() {
            // A later successful enumeration refills the cache and hides this state from
            // wayland_snapshot_missing, so the layout poll needs this durable record to know a
            // capturer was built blind and owes a rebuild.
            UNROTATED_SNAPSHOT_PENDING.store(true, Ordering::Release);
            log::warn!(
                "drm: no wayland snapshot at capturer build for display {:?}; assuming unrotated",
                drm.get(wire_idx).map(|d| d.name.as_str()).unwrap_or("?")
            );
        }
        return (0, None);
    }
    let assignment = assign_wayland_outputs(drm, &wl.displays);
    // The transform comes ONLY from an identity match (name, or unique resolution), through the
    // SAME progressive-taken pass the advertise side keys its swap off: the layout-order
    // fallback is fine for an origin guess, but a rotation pinned on a guess splits the
    // advertised dimensions from the delivered ones.
    let transform = identity_matches(drm, &wl.displays)
        .get(wire_idx)
        .copied()
        .flatten()
        .map(|j| wl.displays[j].transform)
        // Hardware-rotated 180 scans out already upright (i915 advertises rotate-180 and
        // mutter uses it), and wl_output cannot tell hardware from software rotation, so 180
        // keeps master behavior until the plane rotation property travels the wire.
        .map(|t| if t == 90 || t == 270 { t } else { 0 })
        .unwrap_or(0);
    let origin = augment_with_wayland_geometry_from(drm, wl, &assignment)
        .get(wire_idx)
        .map(|di| (di.x, di.y));
    (transform, origin)
}

/// Takes DRM_STATE: never call it while holding one of the per-display maps below.
fn display_info_of(display: i32) -> Option<DrmDisplayInfo> {
    match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.get(display.max(0) as usize).cloned(),
        _ => None,
    }
}

/// A delivered frame resets the streak verdicts (`zero_frame_streak`, `demotes`, `since`) and
    /// nothing else.
#[derive(Clone, Copy)]
struct DisplayHealth {
    zero_frame_streak: u32,
    since: Instant,
    demotes: u32,
    last_build: Option<Instant>,
    rapid_builds: u32,
    /// The dma-buf convert failed for this display. The COMMON cause is multi-GPU: our render node
    /// is not the GPU that exported the scanout. Follows the monitor for the process run.
    prefer_cpu: bool,
    /// The PipeWire fallback for this display was rejected on geometry (a transposed stream), so
    /// the lone-display carve-out in `mark_demoted_displays` must not keep advertising it online.
    fallback_rejected: bool,
}

impl DisplayHealth {
    fn new() -> Self {
        Self {
            zero_frame_streak: 0,
            since: Instant::now(),
            demotes: 0,
            last_build: None,
            rapid_builds: 0,
            prefer_cpu: false,
            fallback_rejected: false,
        }
    }

    fn demoted(&self) -> bool {
        self.zero_frame_streak >= DRM_GRAB_MAX_FAILURES
            && self.since.elapsed() < demote_cooldown(self.demotes)
    }
}

static DRM_DISPLAY_HEALTH: Mutex<BTreeMap<String, DisplayHealth>> = Mutex::new(BTreeMap::new());
const DRM_GRAB_MAX_FAILURES: u32 = 4;
const DEMOTE_COOLDOWN: Duration = Duration::from_secs(30);
const DEMOTE_BACKOFF_MAX_SHIFT: u32 = 4;
const RAPID_REBUILD_WINDOW: Duration = Duration::from_secs(3);
const RAPID_REBUILD_MAX: u32 = 6;

/// Doubling per demotion up to `DEMOTE_BACKOFF_MAX_SHIFT`; a delivered frame zeroes the demote
/// count (see `frame()`), not decayed by time.
fn demote_cooldown(demotes: u32) -> Duration {
    DEMOTE_COOLDOWN * (1u32 << demotes.saturating_sub(1).min(DEMOTE_BACKOFF_MAX_SHIFT))
}

#[derive(Debug, PartialEq, Eq)]
enum RefreshOutcome {
    Publish,
    Unavailable,
    Restamp,
    /// The evidence is about the PRODUCER, not the hardware: give the verdict up to `Unknown`.
    GiveUp,
}

/// `failures` counts consecutive failures INCLUDING this one, so it is 1 on the first.
fn refresh_outcome(probe: Option<usize>, failures: u32) -> RefreshOutcome {
    match probe {
        Some(0) => RefreshOutcome::Unavailable,
        Some(_) => RefreshOutcome::Publish,
        None if failures >= DRM_REFRESH_MAX_FAILURES => RefreshOutcome::GiveUp,
        None => RefreshOutcome::Restamp,
    }
}

fn drm_prefer_cpu(key: &Option<String>) -> bool {
    key.as_ref().is_some_and(|k| {
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .get(k)
            .is_some_and(|h| h.prefer_cpu)
    })
}

fn drm_set_prefer_cpu(key: &Option<String>) {
    if let Some(k) = key {
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .entry(k.clone())
            .or_insert_with(DisplayHealth::new)
            .prefer_cpu = true;
    }
}

fn render_node_count() -> usize {
    std::fs::read_dir("/dev/dri").map_or(0, |entries| {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .and_then(|n| n.strip_prefix("renderD"))
                    .and_then(|minor| minor.parse::<u32>().ok())
                    .is_some()
            })
            .count()
    })
}

static UINPUT_REFRESH_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// A capturer was built with no wayland snapshot and runs unrotated; the layout poll consumes
/// this to bump the generation once a live snapshot exists.
static UNROTATED_SNAPSHOT_PENDING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(super) fn take_unrotated_snapshot_pending() -> bool {
    UNROTATED_SNAPSHOT_PENDING.swap(false, std::sync::atomic::Ordering::AcqRel)
}
static UINPUT_REFRESH_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl IpcDrmCapturer {
    /// The service resolves indices against ITS OWN enumeration, so the receive thread re-resolves
    /// `expected` by connector identity and returns the index geometry must be read at.
    pub fn new(
        display: i32,
        expected: Option<DrmDisplayInfo>,
    ) -> ResultType<(IpcDrmCapturer, Vec<DrmDisplayInfo>, usize, Option<(i32, i32)>)> {
        let shared = Arc::new(Shared {
            slot: Mutex::new(FrameSlot {
                latest: None,
                free: [None, None],
                ended: None,
            }),
            cv: Condvar::new(),
            transform: std::sync::atomic::AtomicI32::new(TRANSFORM_PENDING),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel::<ResultType<(Vec<DrmDisplayInfo>, usize)>>();
        {
            let shared = shared.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("drm-recv".into())
                .spawn(move || recv_thread(display, expected, shared, stop, tx))
                .map_err(|err| anyhow!("could not spawn the drm receive thread: {err}"))?;
        }
        let (displays, wire_idx) = match rx.recv_timeout(Duration::from_millis(HANDSHAKE_WAIT_MS)) {
            Ok(res) => res?,
            Err(_) => {
                // A handshake completing later would stream unowned: Drop never runs here.
                stop.store(true, Ordering::SeqCst);
                bail!("drm capture handshake timed out");
            }
        };
        // One snapshot for the session: transform, origin and the advertised swap must all
        // reflect the same output assignment. The generation is read BEFORE the snapshot, so a
        // clear racing the build rebuilds once instead of running a session on stale geometry.
        let snapshot_gen = scrap::wayland::display::wayland_snapshot_generation();
        let wl = scrap::wayland::display::get_displays();
        let (transform, origin) = transform_and_origin(&displays, wire_idx, &wl);
        // This capturer now shows that layout. If the session init's own wayland query failed it
        // saved an empty baseline, so this is the only record of what the stream is built on.
        super::display_service::note_capturer_layout(&wl.displays, snapshot_gen);
        shared
            .transform
            .store(transform, std::sync::atomic::Ordering::Release);
        Ok((
            IpcDrmCapturer {
                shared,
                stop,
                display,
                connector: displays.get(wire_idx).map(connector_key),
                session_size: displays
                    .get(wire_idx)
                    .map(|d| rotated_dims(transform, d.width as usize, d.height as usize)),
                transform,
                snapshot_gen,
                cur: Vec::new(),
                cur_w: 0,
                cur_h: 0,
                cur_fmt: Pixfmt::BGRA,
                got_frame: false,
            },
            displays,
            wire_idx,
            origin,
        ))
    }

    /// Without an identity, skip rather than record under "", which get_capturer_info reads back
    /// as the same key: one unidentifiable display would demote the next.
    fn note_session_without_frame(&self) {
        let Some(key) = self.connector.clone() else {
            log::debug!(
                "drm: display {} produced no frame but has no connector identity; \
                 not counting it against any display",
                self.display
            );
            return;
        };
        let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
        let h = map.entry(key).or_insert_with(DisplayHealth::new);
        h.zero_frame_streak += 1;
        h.since = Instant::now();
        if h.zero_frame_streak == DRM_GRAB_MAX_FAILURES {
            h.demotes += 1;
            log::warn!(
                "drm: display {} produced no frame in {} sessions; using PipeWire for it, \
                 retrying DRM in {:?} (demotion {})",
                self.display,
                h.zero_frame_streak,
                demote_cooldown(h.demotes),
                h.demotes
            );
        }
    }
}

impl Drop for IpcDrmCapturer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl TraitCapturer for IpcDrmCapturer {
    fn frame<'a>(&'a mut self, timeout: Duration) -> io::Result<Frame<'a>> {
        let deadline = Instant::now() + timeout;
        {
            let mut slot = self.shared.slot.lock().unwrap();
            loop {
                if slot.latest.is_some() || slot.ended.is_some() {
                    break;
                }
                let now = Instant::now();
                if now >= deadline {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                let (guard, _timed_out) =
                    self.shared.cv.wait_timeout(slot, deadline - now).unwrap();
                slot = guard;
            }
            if let Some((w, h, fmt, buf)) = slot.latest.take() {
                drop(slot);
                // A layout change bumps the generation and is otherwise invisible here (mode
                // and framebuffer keep their size). Rebuild for the new transform; not counted
                // against health: the layout moved, the display did not fail.
                if scrap::wayland::display::wayland_snapshot_generation() != self.snapshot_gen {
                    self.shared.slot.lock().unwrap().recycle(buf);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("drm: display {} layout changed; rebuilding", self.display),
                    ));
                }
                // Frames arrive in scanout orientation, the session was sized rotated, so the
                // guard compares rotated dims. convert_to_yuv only refuses a LARGER source (a
                // smaller one leaves stale edges); first frame: CRTC mode vs scanout fb.
                let (fw, fh) = rotated_dims(self.transform, w, h);
                if self.session_size.is_some_and(|(sw, sh)| (fw, fh) != (sw, sh)) {
                    self.shared.slot.lock().unwrap().recycle(buf);
                    if !self.got_frame {
                        self.note_session_without_frame();
                    }
                    let (sw, sh) = self.session_size.unwrap_or_default();
                    let what = if self.got_frame {
                        "changed geometry mid-session"
                    } else {
                        "never matched its advertised geometry"
                    };
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "drm: display {} {what} ({sw}x{sh} -> {fw}x{fh}); rebuilding",
                            self.display
                        ),
                    ));
                }
                if self.transform == 0 {
                    let previous = std::mem::replace(&mut self.cur, buf);
                    self.shared.slot.lock().unwrap().recycle(previous);
                } else if !matches!(fmt, Pixfmt::BGRA | Pixfmt::RGBA) {
                    // Unreachable with today's producers (the convert path emits 4-byte pixels
                    // and the CPU path hardcodes BGRA); kept so a future non-4-byte producer
                    // fails the session instead of shearing the image.
                    self.shared.slot.lock().unwrap().recycle(buf);
                    if !self.got_frame {
                        self.note_session_without_frame();
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "drm: display {} delivered {fmt:?} on a rotated output; rebuilding",
                            self.display
                        ),
                    ));
                } else {
                    unrotate_bgra(&buf, w, h, self.transform, &mut self.cur);
                    self.shared.slot.lock().unwrap().recycle(buf);
                }
                self.cur_w = fw;
                self.cur_h = fh;
                self.cur_fmt = fmt;
                if !self.got_frame {
                    // Clear ONLY the streak: `rapid_builds` is for a display that delivers a first
                    // frame then fails, and `prefer_cpu` is written on the recv thread.
                    self.got_frame = true;
                    if let Some(key) = &self.connector {
                        if let Some(h) = DRM_DISPLAY_HEALTH.lock().unwrap().get_mut(key) {
                            h.zero_frame_streak = 0;
                            h.demotes = 0;
                            h.since = Instant::now();
                            h.fallback_rejected = false;
                        }
                    }
                }
            } else {
                let err = slot
                    .ended
                    .clone()
                    .unwrap_or_else(|| "drm stream ended".to_owned());
                if !self.got_frame {
                    self.note_session_without_frame();
                }
                return Err(io::Error::new(io::ErrorKind::Other, err));
            }
        }
        Ok(Frame::PixelBuffer(PixelBuffer::new(
            &self.cur,
            self.cur_fmt,
            self.cur_w,
            self.cur_h,
        )))
    }
}

#[tokio::main(flavor = "current_thread")]
async fn recv_thread(
    display: i32,
    expected: Option<DrmDisplayInfo>,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<ResultType<(Vec<DrmDisplayInfo>, usize)>>,
) {
    let cursor_epoch = next_cursor_epoch();
    let mut conn = match connect_drm(DRM_CONNECT_TIMEOUT_MS).await {
        Ok(c) => c,
        Err(err) => {
            let _ = tx.send(Err(err));
            return;
        }
    };
    let displays = match conn.recv_msg_timeout2(DISPLAY_LIST_TIMEOUT_MS).await {
        Some(Ok((Data::DrmDisplayList(v), _fd))) => v,
        Some(Ok((other, _fd))) => {
            let _ = tx.send(Err(anyhow!("expected DrmDisplayList, got {:?}", other)));
            return;
        }
        Some(Err(err)) => {
            let _ = tx.send(Err(err));
            return;
        }
        None => {
            let _ = tx.send(Err(anyhow!("timed out waiting for DrmDisplayList")));
            return;
        }
    };
    // Our monitor's index IN THIS CONNECTION'S LIST; `display` indexes the CLIENT's. Measured on a
    // T2: a woken 2880x1800 panel re-enters ahead of the Touch Bar, flipping index 0.
    let wire_idx = match &expected {
        Some(e) => {
            match displays
                .iter()
                .position(|d| d.device == e.device && d.name == e.name)
            {
                Some(i) => i,
                None => {
                    let _ = tx.send(Err(anyhow!(
                        "display {display} ({}) is no longer in the service's list; \
                         the video service will rebuild against the fresh topology",
                        e.name
                    )));
                    return;
                }
            }
        }
        None => {
            let _ = tx.send(Err(anyhow!(
                "display {display} is not in the advertised list; not guessing a monitor for it"
            )));
            return;
        }
    };
    // (device, crtc_id) survives a topology change; list indices do not.
    let bound_to = displays
        .get(wire_idx)
        .map(|d| (d.device.clone(), d.crtc_id));
    let our_key = displays.get(wire_idx).map(connector_key);
    let render_node = displays
        .get(wire_idx)
        .or_else(|| displays.first())
        .map(|d| d.render_node.clone())
        .unwrap_or_default();
    // An unnamed exporter on a multi-render-node host fails SILENTLY: on a Jetson
    // (scanout nvidia-drm, first render node tegra) the wrong device's import SUCCEEDS and corrupts
    // the pixels, so there is no convert error for prefer_cpu to learn from.
    let ambiguous_gpu = render_node.is_empty() && render_node_count() > 1;
    let force_cpu = drm_prefer_cpu(&our_key) || ambiguous_gpu;
    let mut converter = if force_cpu {
        None
    } else {
        RenderConverter::open_render(Some(render_node.as_str()))
    };
    let need_cpu = converter.is_none();
    if need_cpu {
        log::info!(
            "drm: requesting the CPU-converted frame path for display {display} ({})",
            if ambiguous_gpu {
                "the service did not name the exporting GPU and this host has several render nodes; \
                 auto-selecting one can import the scanout on the wrong device and silently corrupt it"
            } else if force_cpu {
                "a prior consumer convert failed, e.g. multi-GPU render-node mismatch"
            } else {
                "no render-node convert context: libdrmtap did not load here, or \
                 drmtap_open_render found no usable /dev/dri/renderD*"
            }
        );
    }
    if let Err(err) = conn
        .send_msg(
            &Data::DrmStart {
                display: wire_idx as i32,
                need_cpu,
            },
            None,
        )
        .await
    {
        let _ = tx.send(Err(err));
        return;
    }
    let _ = tx.send(Ok((displays, wire_idx)));

    // A cursor that arrived before new() stored the session transform, held for replay. Only the
    // newest matters; the 200 ms recv timeout guarantees this is retried even on an idle wire.
    let mut pending_cursor: Option<(u64, u32, u32, i32, i32, Vec<u8>)> = None;
    let end_reason = loop {
        if stop.load(Ordering::SeqCst) {
            break "stopped".to_owned();
        }
        if pending_cursor.is_some() {
            let t = shared.transform.load(std::sync::atomic::Ordering::Acquire);
            if t != TRANSFORM_PENDING {
                if let Some((id, width, height, hotx, hoty, raw)) = pending_cursor.take() {
                    deliver_drm_cursor(display, cursor_epoch, id, width, height, hotx, hoty, raw, t);
                }
            }
        }
        let (msg, recv_fd) = match conn.recv_msg_timeout2(200).await {
            None => continue, // timeout: re-check stop at the loop top
            Some(Ok(pair)) => pair,
            Some(Err(err)) => break format!("recv: {err}"),
        };
        match msg {
            Data::DrmFrameDmabuf(desc) => {
                let conv = match converter.as_mut() {
                    Some(c) => c,
                    None => break "no DRM render node; cannot convert dma-buf frame".to_owned(),
                };
                // Valid in THIS process; -1 is an import-once cache hit on `fb_id`.
                let received_fd: RawFd = if desc.has_fd {
                    match recv_fd.as_ref() {
                        Some(f) => f.as_raw_fd(),
                        None => {
                            break "dma-buf frame set has_fd but carried no SCM_RIGHTS fd".to_owned()
                        }
                    }
                } else {
                    -1
                };
                let mut ddesc = drmtap_dmabuf_desc {
                    dma_buf_fd: -1,
                    width: desc.width,
                    height: desc.height,
                    format: desc.format,
                    modifier: desc.modifier,
                    fb_id: desc.fb_id,
                    // RAW: `drm_render::convert` REJECTS an out-of-range count rather than
                    // clamping, so the count the C reads is the one that was validated.
                    num_planes: desc.num_planes,
                    offsets: desc.offsets,
                    pitches: desc.pitches,
                    hdr_eotf: desc.hdr_eotf,
                    hdr_max_nits: desc.hdr_max_nits,
                };
                match conv.convert(&mut ddesc, received_fd) {
                    Ok((data, w, h, fmt)) => {
                        // Borrowed from the render context, valid only until the next convert.
                        // Copy into a recycled buffer, and OUTSIDE the slot lock, so a
                        // multi-megabyte memcpy never holds the encoder off the slot.
                        let mut buf = shared.slot.lock().unwrap().take_free().unwrap_or_default();
                        buf.clear();
                        buf.extend_from_slice(data);
                        let mut slot = shared.slot.lock().unwrap();
                        slot.publish(w as usize, h as usize, fmt, buf);
                        shared.cv.notify_one();
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => {
                        drm_set_prefer_cpu(&our_key);
                        break format!("convert: {err}");
                    }
                }
                // `recv_fd` closes at the end of this iteration, AFTER convert imported it.
                // Ack so the producer RELEASES ONE SEND CREDIT and forwards the next; this bounds
                // the socket to a couple of in-flight frames instead of a stale backlog.
                if let Err(err) = conn.send_frame_ack().await {
                    break format!("frame ack: {err}");
                }
            }
            Data::DrmFrame { width, height } => {
                // `frame()` hands this to PixelBuffer::new, which derives the stride as
                // `data.len() / height`: height==0 would DIVIDE BY ZERO.
                if width == 0 || height == 0 {
                    break format!("cpu frame: degenerate geometry {width}x{height}");
                }
                let need = (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(4);
                let mut buf = shared.slot.lock().unwrap().take_free().unwrap_or_default();
                match tokio::time::timeout(BODY_READ_TIMEOUT, conn.next_raw_into(&mut buf)).await {
                    Err(_) => break "cpu frame body read timed out".to_owned(),
                    Ok(Ok(())) => {
                        if buf.len() < need {
                            break format!(
                                "cpu frame: body {} bytes < {need} for {width}x{height}",
                                buf.len()
                            );
                        }
                        let mut slot = shared.slot.lock().unwrap();
                        slot.publish(width as usize, height as usize, Pixfmt::BGRA, buf);
                        shared.cv.notify_one();
                    }
                    Ok(Err(err)) => break format!("frame body: {err}"),
                }
                // Ack this CPU frame too (flow control; see the dma-buf arm above).
                if let Err(err) = conn.send_frame_ack().await {
                    break format!("frame ack: {err}");
                }
            }
            Data::DrmCursor {
                id,
                width,
                height,
                hotx,
                hoty,
            } => {
                // get_cursor_data() hands `colors` straight to the client, which renders
                // width*height*4 RGBA bytes: a short body would make it READ PAST THE BUFFER. A
                // hidden-cursor sentinel arrives as 1x1 with a 4-byte body, so `need` is 4 and the
                // check is live.
                let need = (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(4);
                let mut raw = Vec::new();
                match tokio::time::timeout(BODY_READ_TIMEOUT, conn.next_raw_into(&mut raw)).await {
                    Err(_) => break "cursor body read timed out".to_owned(),
                    Ok(Ok(())) => {
                        if raw.len() < need {
                            break format!(
                                "cursor body {} bytes < {need} for {width}x{height}",
                                raw.len()
                            );
                        }
                        let t = shared.transform.load(std::sync::atomic::Ordering::Acquire);
                        if t == TRANSFORM_PENDING {
                            pending_cursor = Some((id, width, height, hotx, hoty, raw));
                        } else {
                            pending_cursor = None;
                            deliver_drm_cursor(
                                display,
                                cursor_epoch,
                                id,
                                width,
                                height,
                                hotx,
                                hoty,
                                raw,
                                t,
                            );
                        }
                    }
                    Ok(Err(err)) => break format!("cursor body: {err}"),
                }
            }
            Data::DrmDisplaysChanged(list) => {
                // `display` (the CLIENT's index) and NOT `wire_idx`, deliberately. `bound_to` is an
                // identity `(device, crtc_id)`, not a position, so this asks "does that slot still
                // name MY monitor"; and the swap below installs this list as DRM_STATE, which is the
                // client-space list display_service re-advertises and input is mapped through.
                // Probing `wire_idx` stays quiet in exactly the case this guard exists for: a stream
                // whose wire_idx differs from display keeps running while the client's index comes to
                // mean another monitor. Checked BEFORE the swap, against the topology this stream
                // started on.
                let now_at_our_index = list
                    .get(display.max(0) as usize)
                    .map(|d| (d.device.clone(), d.crtc_id));
                if bound_to.is_some() && now_at_our_index != bound_to {
                    swap_available_displays(list);
                    scrap::wayland::display::clear_wayland_displays_cache();
                    break match (&bound_to, &now_at_our_index) {
                        (Some((_, was)), Some((_, now))) => format!(
                            "hotplug renumbered display {display}: it was crtc {was}, now crtc {now}"
                        ),
                        _ => format!("hotplug removed display {display} from the list"),
                    };
                }
                swap_available_displays(list);
                scrap::wayland::display::clear_wayland_displays_cache();
                UINPUT_REFRESH_GEN.fetch_add(1, Ordering::AcqRel);
                if !UINPUT_REFRESH_BUSY.swap(true, Ordering::AcqRel) {
                    // Taken BEFORE the spawn and moved in: `Builder::spawn` can FAIL with EAGAIN after
                    // the swap, so a guard built inside the closure would never exist and the flag
                    // would stay set for the PROCESS LIFETIME.
                    let mut busy = UinputRefreshGuard(true);
                    let spawned = std::thread::Builder::new()
                        .name("drm-uinput-refresh".into())
                        .spawn(move || {
                        let rt = match tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                        {
                            Ok(rt) => rt,
                            Err(err) => {
                                log::warn!(
                                    "drm: uinput refresh worker could not build a runtime: {err}"
                                );
                                return; // the guard hands the slot back
                            }
                        };
                        let mut served = 0u64;
                        loop {
                            let g = UINPUT_REFRESH_GEN.load(Ordering::Acquire);
                            if g != served {
                                served = g;
                                rt.block_on(super::wayland::update_uinput_resolution());
                                continue;
                            }
                            busy.release();
                            if UINPUT_REFRESH_GEN.load(Ordering::Acquire) == served {
                                break;
                            }
                            if !busy.retake() {
                                break; // another handler already started a fresh worker
                            }
                        }
                    });
                    if let Err(err) = spawned {
                        log::error!("drm: could not spawn the uinput refresh worker: {err}");
                    }
                }
            }
            _ => {} // ignore any unexpected control message
        }
    };
    log::info!("drm capture stream ended: {end_reason}");
    // Drop the render context on THIS thread: its EGL state + cached imports are thread-local and
    // a cross-thread close strands them. Never in `Drop`, which runs on the encoder thread.
    drop(converter);
    remove_drm_cursor(display, cursor_epoch);
    let mut slot = shared.slot.lock().unwrap();
    slot.ended = Some(format!("drm stream ended ({end_reason})"));
    shared.cv.notify_one();
}

// Keyed by display index: the cursor lives on whichever CRTC the pointer is over and every other
// stream reports a hidden sentinel, which under a single global would clobber it.
#[derive(Clone)]
pub struct DrmCursorData {
    pub id: u64,
    pub width: i32,
    pub height: i32,
    pub hotx: i32,
    pub hoty: i32,
    pub colors: Vec<u8>,
}

static DRM_CURSOR: Mutex<BTreeMap<i32, (u64, DrmCursorData)>> = Mutex::new(BTreeMap::new());
// Monotonic per-stream tag: a rebuilt stream reuses the display index, so a torn-down stream drops
// its entry ONLY if the epoch still matches.
static DRM_CURSOR_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_cursor_epoch() -> u64 {
    DRM_CURSOR_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// Compare-and-set: a still-draining predecessor stream (older epoch) must not overwrite the entry a
// replacement stream (newer epoch) already published. Only accept a write whose epoch is at least
// the stored one.
fn set_drm_cursor(display: i32, epoch: u64, c: DrmCursorData) {
    let mut map = DRM_CURSOR.lock().unwrap();
    match map.get(&display) {
        Some((stored, _)) if *stored > epoch => {}
        _ => {
            map.insert(display, (epoch, c));
        }
    }
}

fn remove_drm_cursor(display: i32, epoch: u64) {
    let mut map = DRM_CURSOR.lock().unwrap();
    if map.get(&display).map(|(e, _)| *e) == Some(epoch) {
        map.remove(&display);
    }
}

/// Unrotate a wire cursor into the session orientation and publish it. The compositor
/// pre-rotates the bitmap it programs into the cursor plane, so over the unrotated video the
/// cursor alone would stay turned and its hotspot transposed (review finding 11 on
/// rustdesk#15889). The wire id hashes only the plane pixels and geometry, so a stream rebuilt
/// under a new transform resends the SAME id and the client's by-id cursor cache would keep the
/// old orientation: fold the transform in (the producer's own FNV step) so id and orientation
/// can never disagree. The hidden sentinel must survive untouched.
#[allow(clippy::too_many_arguments)]
fn deliver_drm_cursor(
    display: i32,
    cursor_epoch: u64,
    id: u64,
    width: u32,
    height: u32,
    hotx: i32,
    hoty: i32,
    raw: Vec<u8>,
    t: i32,
) {
    let (width, height, hotx, hoty, colors) = if t == 90 || t == 270 {
        let mut turned = Vec::new();
        unrotate_bgra(&raw, width as usize, height as usize, t, &mut turned);
        let (hx, hy) = unrotate_hotspot(t, width as i32, height as i32, hotx, hoty);
        (height as i32, width as i32, hx, hy, turned)
    } else {
        (width as i32, height as i32, hotx, hoty, raw)
    };
    let id = fold_cursor_id(id, t);
    set_drm_cursor(
        display,
        cursor_epoch,
        DrmCursorData {
            id,
            width,
            height,
            hotx,
            hoty,
            colors,
        },
    );
}

fn fold_cursor_id(id: u64, t: i32) -> u64 {
    if id == scrap::drm_reader::HIDDEN_CURSOR_ID {
        id
    } else {
        (id ^ t as u32 as u64).wrapping_mul(1099511628211)
    }
}

fn with_drm_cursor<T>(f: impl Fn(&DrmCursorData) -> T) -> Option<T> {
    let map = DRM_CURSOR.lock().unwrap();
    map.values()
        .map(|(_, c)| c)
        .find(|c| c.id != scrap::drm_reader::HIDDEN_CURSOR_ID)
        .or_else(|| map.values().map(|(_, c)| c).next())
        .map(f)
}

pub fn drm_cursor_id() -> Option<u64> {
    with_drm_cursor(|c| c.id)
}

/// Snapshot of the DRM hardware cursor, or None. The pixels are premultiplied ARGB and are passed
/// through as-is, like the XFixes path, so the client sees one cursor format from either backend.
pub fn drm_cursor() -> Option<DrmCursorData> {
    with_drm_cursor(|c| c.clone())
}

enum ProbeState {
    Unknown,
    Unavailable(Instant),
    Available(Instant, Vec<DrmDisplayInfo>),
}

static DRM_STATE: Mutex<ProbeState> = Mutex::new(ProbeState::Unknown);
const NEGATIVE_TTL: Duration = Duration::from_secs(30);
const POSITIVE_TTL: Duration = Duration::from_secs(15);

/// Runs on a throwaway thread: a nested `#[tokio::main]` panics if called from inside a runtime.
fn query_displays() -> ResultType<Vec<DrmDisplayInfo>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("drm-query".into())
        .spawn(move || {
            let _ = tx.send(query_displays_async());
        })
        .map_err(|err| anyhow!("could not spawn the drm display query thread: {err}"))?;
    rx.recv_timeout(Duration::from_millis(HANDSHAKE_WAIT_MS))
        .map_err(|_| anyhow!("drm display query timed out"))?
}

#[tokio::main(flavor = "current_thread")]
async fn query_displays_async() -> ResultType<Vec<DrmDisplayInfo>> {
    query_displays_inner().await
}

async fn query_displays_inner() -> ResultType<Vec<DrmDisplayInfo>> {
    let mut conn = connect_drm(DRM_CONNECT_TIMEOUT_MS).await?;
    match conn.recv_msg_timeout2(DISPLAY_LIST_TIMEOUT_MS).await {
        Some(Ok((Data::DrmDisplayList(v), _fd))) => Ok(v),
        Some(Ok((other, _fd))) => Err(anyhow!("expected DrmDisplayList, got {:?}", other)),
        Some(Err(err)) => Err(err),
        None => Err(anyhow!("timed out waiting for DrmDisplayList")),
    }
}

static DRM_PROBE_FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const DRM_PROBE_MAX_FAILURES: u32 = 5;
static DRM_REFRESH_FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const DRM_REFRESH_MAX_FAILURES: u32 = 3;
// Single-flight, so is_available() never calls query_displays() (~4s of IPC) holding DRM_STATE.
static DRM_PROBE_IN_FLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Advanced by every publish, so a slow UNLOCKED probe can tell a newer verdict landed meanwhile.
static DRM_STATE_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// EVERY verdict change to DRM_STATE goes through here so the generation stays truthful; the TTL
    /// restamp in `refresh_available_async` is the one direct write.
#[inline]
fn publish_probe_state(st: &mut ProbeState, next: ProbeState) {
    *st = next;
    DRM_STATE_GEN.fetch_add(1, Ordering::Release);
}

/// Releases DRM_PROBE_IN_FLIGHT on EVERY exit; a leaked release wedges all future probes.
struct ProbeInFlightGuard;
impl Drop for ProbeInFlightGuard {
    fn drop(&mut self) {
        DRM_PROBE_IN_FLIGHT.store(false, Ordering::Release);
    }
}

/// Ownership of `UINPUT_REFRESH_BUSY`, released on every exit. It is handed back and re-taken
/// mid-loop, so releasing on drop unconditionally would clear a flag a REPLACEMENT worker owns.
struct UinputRefreshGuard(bool);
impl UinputRefreshGuard {
    fn release(&mut self) {
        if self.0 {
            self.0 = false;
            UINPUT_REFRESH_BUSY.store(false, Ordering::Release);
        }
    }
    fn retake(&mut self) -> bool {
        self.0 = !UINPUT_REFRESH_BUSY.swap(true, Ordering::AcqRel);
        self.0
    }
}
impl Drop for UinputRefreshGuard {
    fn drop(&mut self) {
        self.release();
    }
}

/// Never probes or blocks. Use in hot paths such as `wayland::clear()`, `is_inited()`, and display
/// enumeration, where seconds of IPC would trip "deadline has elapsed".
pub(crate) fn is_available_cached() -> bool {
    matches!(&*DRM_STATE.lock().unwrap(), ProbeState::Available(..))
}

/// A tri-state assessment of DRM capture availability.
/// `Unsettled` means a probe is in flight or failures have not reached the disable threshold.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Availability {
    Available,
    Unavailable,
    Unsettled,
}

/// MAY BLOCK for seconds: never a routing gate, and never on the login request path — that path
/// reads `availability_cached`. This blocking form serves the capture-side callers through
/// `is_available`, where waiting out a settle is acceptable.
fn availability() -> Availability {
    let (verdict, stale_no) = {
        let st = DRM_STATE.lock().unwrap();
        // Keep a settled "no" while an off-thread probe re-verifies it, avoiding a transient
        // Unsettled result whenever the negative cache expires.
        let stale_no =
            matches!(&*st, ProbeState::Unavailable(since) if since.elapsed() >= NEGATIVE_TTL);
        let verdict = match &*st {
            ProbeState::Available(since, _) => {
                Some((Availability::Available, since.elapsed() >= POSITIVE_TTL))
            }
            ProbeState::Unavailable(_) => Some((Availability::Unavailable, false)),
            ProbeState::Unknown => None, // fall through and probe with the lock released
        };
        (verdict, stale_no)
    };
    if let Some((answer, stale)) = verdict {
        if stale {
            refresh_available_async();
        }
        if stale_no {
            refresh_unavailable_async();
        }
        return answer;
    }
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        // Someone else is mid-probe: their result is not in yet, and "not yet" is not "no".
        return match &*DRM_STATE.lock().unwrap() {
            ProbeState::Available(..) => Availability::Available,
            ProbeState::Unavailable(_) => Availability::Unavailable,
            ProbeState::Unknown => Availability::Unsettled,
        };
    }
    let _in_flight = ProbeInFlightGuard;
    probe_and_publish()
}

/// Non-blocking login-path assessment.
/// Unknown starts a probe off-thread; callers require `Available` before admitting a session.
pub(crate) fn availability_cached() -> Availability {
    let (verdict, stale_no) = {
        let st = DRM_STATE.lock().unwrap();
        let stale_no =
            matches!(&*st, ProbeState::Unavailable(since) if since.elapsed() >= NEGATIVE_TTL);
        let verdict = match &*st {
            ProbeState::Available(since, _) => {
                Some((Availability::Available, since.elapsed() >= POSITIVE_TTL))
            }
            ProbeState::Unavailable(_) => Some((Availability::Unavailable, false)),
            ProbeState::Unknown => None,
        };
        (verdict, stale_no)
    };
    if let Some((answer, stale)) = verdict {
        if stale {
            refresh_available_async();
        }
        if stale_no {
            refresh_unavailable_async();
        }
        return answer;
    }
    if !DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        let in_flight = ProbeInFlightGuard;
        let spawned = std::thread::Builder::new()
            .name("drm-avail-probe".into())
            .spawn(move || {
                let _in_flight = in_flight;
                probe_and_publish();
            });
        // On error the guard moved into the dropped closure and released the flag already.
        if let Err(err) = spawned {
            log::warn!("drm: could not spawn the availability probe thread: {err}");
        }
    }
    Availability::Unsettled
}

/// Probe synchronously and publish the outcome. The caller must hold DRM_PROBE_IN_FLIGHT.
fn probe_and_publish() -> Availability {
    let t = Instant::now();
    let result = query_displays();
    let mut st = DRM_STATE.lock().unwrap();
    let answer = match result {
        Ok(list) if !list.is_empty() => {
            log::debug!(
                "drm: availability probe -> available ({} displays) in {:?}",
                list.len(),
                t.elapsed()
            );
            DRM_PROBE_FAILURES.store(0, Ordering::Relaxed);
            publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
            Availability::Available
        }
        Ok(_) => {
            log::info!("drm: availability probe -> no displays in {:?}", t.elapsed());
            publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
            Availability::Unavailable
        }
        Err(err) => {
            let n = DRM_PROBE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
            if n >= DRM_PROBE_MAX_FAILURES {
                log::info!("drm: availability probe failed {n}x ({err}); disabling DRM");
                publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
                Availability::Unavailable
            } else {
                log::info!(
                    "drm: availability probe failed ({err}), attempt {n}/{DRM_PROBE_MAX_FAILURES}; will retry"
                );
                // Deliberately still Unknown in DRM_STATE: this is a retry window, not a verdict.
                Availability::Unsettled
            }
        }
    };
    drop(st);
    answer
}

/// The boolean form for capture-path callers, where an unsettled probe and a definitive "no"
/// route the same way (into the non-DRM fallback).
pub(crate) fn is_available() -> bool {
    availability() == Availability::Available
}

/// The negative mirror of `refresh_available_async`: re-verify a stale Unavailable without ever
/// answering Unknown in the meantime. A failed or empty re-probe re-confirms the "no" with a
/// fresh timestamp; only a non-empty display list flips the verdict.
fn refresh_unavailable_async() {
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let in_flight = ProbeInFlightGuard;
    let sampled_gen = {
        let st = DRM_STATE.lock().unwrap();
        match &*st {
            ProbeState::Unavailable(since) if since.elapsed() >= NEGATIVE_TTL => {}
            _ => return,
        }
        DRM_STATE_GEN.load(Ordering::Acquire)
    };
    let spawned = std::thread::Builder::new()
        .name("drm-unavail-refresh".into())
        .spawn(move || {
            let _in_flight = in_flight;
            let result = query_displays();
            let mut st = DRM_STATE.lock().unwrap();
            if DRM_STATE_GEN.load(Ordering::Acquire) != sampled_gen {
                return;
            }
            match result {
                Ok(list) if !list.is_empty() => {
                    log::info!(
                        "drm: availability re-probe -> available ({} displays)",
                        list.len()
                    );
                    DRM_PROBE_FAILURES.store(0, Ordering::Relaxed);
                    publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
                    drop(st);
                    scrap::wayland::display::clear_wayland_displays_cache();
                }
                _ => {
                    // Restamp: a failed or empty re-probe is a fresh confirmation of "no".
                    publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
                }
            }
        });
    // Nothing to release on error: the guard moved into the closure and drops with it either way.
    if let Err(err) = spawned {
        log::warn!("drm: could not spawn the unavailability re-probe thread: {err}");
    }
}

fn refresh_available_async() {
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let in_flight = ProbeInFlightGuard;
    let sampled_gen = {
        let st = DRM_STATE.lock().unwrap();
        if !matches!(&*st, ProbeState::Available(..)) {
            return;
        }
        DRM_STATE_GEN.load(Ordering::Acquire)
    };
    let spawned = std::thread::Builder::new()
        .name("drm-avail-refresh".into())
        .spawn(move || {
            let _in_flight = in_flight;
            let result = query_displays();
            let mut st = DRM_STATE.lock().unwrap();
            if DRM_STATE_GEN.load(Ordering::Acquire) != sampled_gen {
                return;
            }
            let failures = match &result {
                Ok(_) => {
                    DRM_REFRESH_FAILURES.store(0, Ordering::Relaxed);
                    0
                }
                Err(_) => DRM_REFRESH_FAILURES.fetch_add(1, Ordering::Relaxed) + 1,
            };
            match refresh_outcome(result.as_ref().ok().map(|l| l.len()), failures) {
                RefreshOutcome::Publish => {
                    let fresh = result.unwrap_or_default();
                    let changed = match &*st {
                        ProbeState::Available(_, old) => *old != fresh,
                        _ => true,
                    };
                    publish_probe_state(&mut st, ProbeState::Available(Instant::now(), fresh));
                    if changed {
                        drop(st);
                        scrap::wayland::display::clear_wayland_displays_cache();
                    }
                }
                RefreshOutcome::Unavailable => {
                    log::info!("drm: refresh -> 0 displays, marking DRM unavailable");
                    publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
                }
                // Only the TTL stamp moves, so this does NOT go through publish_probe_state.
                RefreshOutcome::Restamp => {
                    if let ProbeState::Available(since, _) = &mut *st {
                        *since = Instant::now();
                    }
                }
                RefreshOutcome::GiveUp => {
                    log::info!(
                        "drm: availability refresh failed {failures}x ({:?}); the producer looks \
                         gone, dropping the cached verdict so the next enumeration re-probes",
                        result.as_ref().err()
                    );
                    DRM_REFRESH_FAILURES.store(0, Ordering::Relaxed);
                    publish_probe_state(&mut st, ProbeState::Unknown);
                }
            }
        });
    // Nothing to release: the guard moved into the closure and drops with it. Clearing the flag
    // explicitly would let TWO PROBES RUN AT ONCE, since another refresh may already hold it.
    if let Err(err) = spawned {
        log::warn!(
            "drm: could not spawn the availability refresh thread: {err}; the cached verdict \
             stays stale until the next probe"
        );
    }
}

pub(super) fn warm_availability() {
    // The gate is INSIDE the loop because `get_display_server()` answers "x11" whenever loginctl
    // cannot yet name the seat0 session. `is_x11_for_drm()` is that form minus the greeter
    // blind spot, where plain `is_x11()` is permanently true.
    for _ in 0..10 {
        if crate::platform::linux::is_x11_for_drm() {
            std::thread::sleep(Duration::from_millis(300));
            continue;
        }
        if matches!(&*DRM_STATE.lock().unwrap(), ProbeState::Available(..)) {
            return;
        }
        match query_displays() {
            Ok(list) if !list.is_empty() => {
                log::info!("drm: consumer cache warmed ({} displays) at startup", list.len());
                publish_probe_state(&mut DRM_STATE.lock().unwrap(), ProbeState::Available(Instant::now(), list));
                return;
            }
            _ => std::thread::sleep(Duration::from_millis(300)),
        }
    }
    log::info!("drm: consumer cache warm found no producer at startup (will probe lazily)");
}

/// The service holds its answer until the topology settles. Replaces only an `Available` verdict.
pub(super) async fn refresh_displays_for_login() {
    let sampled_gen = {
        let st = DRM_STATE.lock().unwrap();
        if !matches!(&*st, ProbeState::Available(..)) {
            return;
        }
        DRM_STATE_GEN.load(Ordering::Acquire)
    };
    let t = Instant::now();
    match query_displays_inner().await {
        Ok(list) if !list.is_empty() => {
            let changed = {
                let mut st = DRM_STATE.lock().unwrap();
                if DRM_STATE_GEN.load(Ordering::Acquire) != sampled_gen {
                    log::debug!(
                        "drm: login display refresh superseded while probing; keeping the newer list"
                    );
                    return;
                }
                match &*st {
                    ProbeState::Available(_, old) => {
                        let changed = *old != list;
                        log::debug!(
                            "drm: login display refresh -> {} display(s) in {:?}{}",
                            list.len(),
                            t.elapsed(),
                            if changed { " (list changed)" } else { "" }
                        );
                        publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
                        changed
                    }
                    _ => return,
                }
            };
            if changed {
                scrap::wayland::display::clear_wayland_displays_cache();
            }
        }
        Ok(_) => log::debug!(
            "drm: login display refresh found no displays in {:?}; keeping the cached list",
            t.elapsed()
        ),
        Err(err) => log::debug!(
            "drm: login display refresh failed in {:?} ({err}); keeping the cached list",
            t.elapsed()
        ),
    }
}

/// Mirrors get_display_infos: only a MULTI-display host advertises a demoted display.
pub(super) fn display_count_and_any_demoted() -> Option<(usize, bool)> {
    // Snapshot the identity keys under DRM_STATE, then consult health with DRM_STATE RELEASED --
    // same order as get_display_infos: never hold DRM_STATE while taking a per-display map.
    let (len, keys): (usize, Vec<String>) = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => (
            list.len(),
            if list.len() > 1 {
                list.iter().map(connector_key).collect()
            } else {
                Vec::new()
            },
        ),
        _ => return None,
    };
    let any_demoted = if len > 1 {
        let health = DRM_DISPLAY_HEALTH.lock().unwrap();
        keys.iter()
            .any(|k| health.get(k).is_some_and(|h| h.demoted()))
    } else {
        false
    };
    Some((len, any_demoted))
}

// A multi-display portal stream cannot replace one demoted connector. Keep its index but mark it
// offline; a single connector remains usable through the whole-desktop fallback - unless that
// fallback itself was rejected on geometry, in which case advertising the lone display online
// would restart-loop the video service against a stream nothing can serve.
fn mark_demoted_displays(list: &[DrmDisplayInfo], infos: &mut [DisplayInfo]) {
    let health = DRM_DISPLAY_HEALTH.lock().unwrap();
    if list.len() <= 1 {
        if let (Some(display), Some(info)) = (list.first(), infos.first_mut()) {
            if health
                .get(&connector_key(display))
                .is_some_and(|health| health.demoted() && health.fallback_rejected)
            {
                info.online = false;
            }
        }
        return;
    }
    for (display, info) in list.iter().zip(infos.iter_mut()) {
        if health
            .get(&connector_key(display))
            .is_some_and(|health| health.demoted())
        {
            info.online = false;
        }
    }
}

/// The PipeWire fallback for this display was rejected on geometry; recorded so the lone-display
/// carve-out above stops advertising a display nothing can serve. Cleared by a delivered frame
/// and by the demote-cooldown re-arm.
pub(super) fn mark_fallback_rejected(display_idx: usize) {
    let Some(expected) = display_info_of(display_idx as i32) else {
        return;
    };
    DRM_DISPLAY_HEALTH
        .lock()
        .unwrap()
        .entry(connector_key(&expected))
        .or_insert_with(DisplayHealth::new)
        .fallback_rejected = true;
}

fn primary_index_from_assignment(assignment: &[Option<usize>], primary: usize) -> usize {
    assignment
        .iter()
        .position(|assigned| *assigned == Some(primary))
        .unwrap_or(0)
}

/// Releases DRM_STATE before taking the Wayland and health locks.
pub(super) fn get_display_infos_and_primary() -> Option<(Vec<DisplayInfo>, usize)> {
    let list = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.clone(),
        _ => return None,
    };
    let wl = scrap::wayland::display::get_displays();
    let assignment = assign_wayland_outputs(&list, &wl.displays);
    let mut infos = augment_with_wayland_geometry_from(&list, &wl, &assignment);
    mark_demoted_displays(&list, &mut infos);
    // Primary and geometry must use the same connector assignment snapshot.
    let primary = primary_index_from_assignment(&assignment, wl.primary);
    Some((infos, primary))
}

pub(super) fn get_display_infos() -> Option<Vec<DisplayInfo>> {
    let list = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.clone(),
        _ => return None,
    };
    let mut infos = augment_with_wayland_geometry(&list);
    mark_demoted_displays(&list, &mut infos);
    Some(infos)
}

/// DRM reports every monitor at physical size and origin (0,0), stacking a multi-monitor client.
///
/// Asked at login screens too, on purpose: a greeter runs a compositor, and the socket fallback in
/// hbb_common lets the enumerator reach it with no environment variables. Where that fallback
/// cannot answer, the list comes back empty and everything stays unaugmented, which is what the
/// old is-login-screen gate produced unconditionally.
fn augment_with_wayland_geometry(drm: &[DrmDisplayInfo]) -> Vec<DisplayInfo> {
    let wl = scrap::wayland::display::get_displays();
    let assignment = assign_wayland_outputs(drm, &wl.displays);
    augment_with_wayland_geometry_from(drm, &wl, &assignment)
}

fn augment_with_wayland_geometry_from(
    drm: &[DrmDisplayInfo],
    wl: &scrap::wayland::display::Displays,
    matched: &[Option<usize>],
) -> Vec<DisplayInfo> {
    let mut infos: Vec<DisplayInfo> = drm.iter().map(display_info_from_drm).collect();
    // A single display is still augmented: on a multi-GPU host the one connector this service can
    // open may sit at a non-zero origin in the compositor layout, and DRM alone reports (0,0).
    if drm.is_empty() {
        return infos;
    }
    if wl.displays.is_empty() {
        return infos;
    }
    // One connector against one output is the origin-only case: the lone output can still sit at
    // a non-zero origin this side cannot see, but it keeps the scale-1 convention — a single
    // display is advertised at physical size (see `logical_rects_of`), so its logical size must
    // not be adopted. More connectors than the one output is an inconsistent snapshot, and the
    // layout-order fallback in `assign_wayland_outputs` would plant that origin on a guess.
    let origin_only = wl.displays.len() == 1;
    if origin_only && drm.len() > 1 {
        return infos;
    }
    let identity = identity_matches(drm, &wl.displays);
    for (i, info) in infos.iter_mut().enumerate() {
        let Some(w) = matched[i].map(|j| &wl.displays[j]) else {
            continue;
        };
        info.x = w.x;
        info.y = w.y;
        // Rotated size before the origin-only cut: a lone rotated output still delivers rotated
        // frames, so it must advertise them; only the logical-scale adoption stays multi-output.
        // original_resolution follows in the same motion, or the client reads the transposed
        // current size against an untransposed original as a third-party resolution change.
        // Identity matches ONLY, the same rule the capturer's transform follows: swapping on a
        // layout-order guess advertises dimensions the capturer will not deliver.
        let is_identity = identity[i].is_some() && identity[i] == matched[i];
        if is_identity && (w.transform == 90 || w.transform == 270) {
            std::mem::swap(&mut info.width, &mut info.height);
            info.original_resolution = super::display_service::get_original_resolution(
                &drm[i].name,
                info.width as usize,
                info.height as usize,
            );
        }
        if origin_only {
            continue;
        }
        if let Some((lw, lh)) = w.logical_size {
            if lw > 0 && lh > 0 {
                // Post-swap width over logical width, which arrives already swapped when rotated:
                // the unrotated numerator made a rotated 1:1 monitor advertise scale 16/9.
                info.scale = info.width as f64 / lw as f64;
                info.original_resolution = super::display_service::get_original_resolution(
                    &drm[i].name,
                    lw as usize,
                    lh as usize,
                );
            }
        }
    }
    infos
}

/// Each output goes to at most one connector; unmatched ones take the next free output of the same
/// size, else the next free one in layout order, since leaving them unaugmented keeps them all at
/// DRM's (0,0).
/// The identity half of the assignment (name, or unique resolution), same progressive `taken`
/// as the full one. Rotation keys off THIS on both sides: swapping or turning on a layout-order
/// guess splits the advertised dimensions from the delivered frames.
/// Identity assignment in two GLOBAL passes: every exact name match is reserved first, then
/// resolution pairing runs on the unmatched remainder, and only when it is forced - exactly one
/// free output AND exactly one unmatched connector at that resolution. A resolution guess for an
/// earlier connector must never steal an exact name match from a later one.
fn identity_matches(
    drm: &[DrmDisplayInfo],
    wl: &[hbb_common::platform::linux::WaylandDisplayInfo],
) -> Vec<Option<usize>> {
    let mut taken = vec![false; wl.len()];
    let mut matched: Vec<Option<usize>> = vec![None; drm.len()];
    for (i, d) in drm.iter().enumerate() {
        let dn = normalize_connector(&d.name);
        if let Some((j, _)) = wl
            .iter()
            .enumerate()
            .find(|(j, w)| !taken[*j] && normalize_connector(&w.name) == dn)
        {
            matched[i] = Some(j);
            taken[j] = true;
        }
    }
    for (i, d) in drm.iter().enumerate() {
        if matched[i].is_some() {
            continue;
        }
        let free_same: Vec<usize> = wl
            .iter()
            .enumerate()
            .filter(|(j, w)| !taken[*j] && w.width == d.width as i32 && w.height == d.height as i32)
            .map(|(j, _)| j)
            .collect();
        let unmatched_same = drm
            .iter()
            .enumerate()
            .filter(|(k, o)| matched[*k].is_none() && o.width == d.width && o.height == d.height)
            .count();
        if free_same.len() == 1 && unmatched_same == 1 {
            matched[i] = Some(free_same[0]);
            taken[free_same[0]] = true;
        }
    }
    matched
}

fn assign_wayland_outputs(
    drm: &[DrmDisplayInfo],
    wl: &[hbb_common::platform::linux::WaylandDisplayInfo],
) -> Vec<Option<usize>> {
    let mut matched = identity_matches(drm, wl);
    let mut taken = vec![false; wl.len()];
    for m in matched.iter().flatten() {
        taken[*m] = true;
    }
    for (i, d) in drm.iter().enumerate() {
        if matched[i].is_some() {
            continue;
        }
        let free_same_size = wl
            .iter()
            .enumerate()
            .position(|(j, w)| !taken[j] && w.width == d.width as i32 && w.height == d.height as i32);
        let Some(j) = free_same_size.or_else(|| taken.iter().position(|t| !t)) else {
            continue; // more connectors than outputs; leave the rest unaugmented
        };
        log::warn!(
            "drm: connector {} matched no compositor output by name or by a unique resolution; \
             falling back to layout order and taking {} at ({}, {})",
            d.name,
            wl[j].name,
            wl[j].x,
            wl[j].y
        );
        matched[i] = Some(j);
        taken[j] = true;
    }
    matched
}


/// DRM inserts a single-letter type discriminator the compositor drops ("HDMI-A-1" -> "HDMI-1").
/// Only a *letter* folds: a single *digit* is an MST port index, so "DP-1-2" is not "DP-2".
fn normalize_connector(name: &str) -> String {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() == 3 && parts[1].len() == 1 && parts[1].chars().all(|c| c.is_ascii_alphabetic()) {
        format!("{}-{}", parts[0], parts[2])
    } else {
        name.to_string()
    }
}

fn swap_available_displays(list: Vec<DrmDisplayInfo>) {
    let mut st = DRM_STATE.lock().unwrap();
    if matches!(&*st, ProbeState::Available(..)) {
        if list.is_empty() {
            log::info!("drm: hotplug refresh -> 0 displays, marking DRM unavailable");
            publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
        } else {
            log::info!("drm: hotplug refresh -> {} display(s)", list.len());
            publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
        }
    }
}

fn display_info_from_drm(d: &DrmDisplayInfo) -> DisplayInfo {
    let original_resolution =
        super::display_service::get_original_resolution(&d.name, d.width as usize, d.height as usize);
    DisplayInfo {
        x: d.x,
        y: d.y,
        width: d.width as i32,
        height: d.height as i32,
        name: d.name.clone(),
        online: d.active,
        cursor_embedded: false,
        original_resolution,
        scale: 1.0,
        ..Default::default()
    }
}

/// Deliberately does NOT publish the handshake list into DRM_STATE: it is read before a possibly
/// seconds-long stall, and when `wire_idx != display_idx` it is ordered differently.
pub(super) fn get_capturer_info(
    display_idx: usize,
) -> ResultType<super::video_service::CapturerInfo> {
    let expected = display_info_of(display_idx as i32);
    let key = expected.as_ref().map(connector_key);
    {
        let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
        if let Some(h) = key.as_ref().and_then(|k| map.get_mut(k)) {
            if h.zero_frame_streak >= DRM_GRAB_MAX_FAILURES {
                if h.demoted() {
                    bail!(
                        "drm capture for display {display_idx} repeatedly produced no frame; using PipeWire"
                    );
                }
                h.zero_frame_streak = 0;
                h.since = Instant::now();
                // The cooldown re-arms DRM for this display, so the fallback verdict restarts too.
                h.fallback_rejected = false;
            }
        }
    }
    // Built FIRST: a transient `_drm` outage must NOT count toward the flap threshold below.
    let (capturer, displays, wire_idx, origin) = IpcDrmCapturer::new(display_idx as i32, expected)?;
    // The initial build counts 0, so demotion fires on the (RAPID_REBUILD_MAX + 1)-th in a window.
    if let Some(key) = key.clone() {
        let now = Instant::now();
        let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
        let h = map.entry(key).or_insert_with(DisplayHealth::new);
        h.rapid_builds = match h.last_build {
            Some(last) if now.duration_since(last) < RAPID_REBUILD_WINDOW => h.rapid_builds + 1,
            _ => 0,
        };
        h.last_build = Some(now);
        if h.rapid_builds >= RAPID_REBUILD_MAX {
            log::warn!(
                "drm: display {display_idx} rebuilt {} times within {RAPID_REBUILD_WINDOW:?}; flapping, falling back to PipeWire",
                h.rapid_builds
            );
            h.zero_frame_streak = DRM_GRAB_MAX_FAILURES;
            h.since = now;
            h.demotes += 1;
            bail!("drm capture for display {display_idx} is flapping; using PipeWire");
        }
    }
    let ndisplay = displays.len();
    // From the entry the stream was BOUND to; `display_idx` is a position in the CLIENT's list.
    let d = displays
        .get(wire_idx)
        .ok_or_else(|| anyhow!("drm display index {wire_idx} out of range ({ndisplay})"))?
        .clone();
    // Origin and transform come from the ONE snapshot new() resolved, so both reflect the
    // same output assignment; dimensions stay PHYSICAL, rotated to frame orientation.
    let origin = origin.unwrap_or((d.x, d.y));
    let (cap_w, cap_h) = rotated_dims(capturer.transform, d.width as usize, d.height as usize);
    Ok(super::video_service::CapturerInfo {
        origin,
        width: cap_w,
        height: cap_h,
        ndisplay,
        current: display_idx,
        privacy_mode_id: 0,
        _capturer_privacy_mode_id: 0,
        capturer: Box::new(capturer),
    })
}

#[cfg(test)]
mod drm_capturer_tests {
    use super::*;

    fn capturer_with(session: Option<(usize, usize)>) -> IpcDrmCapturer {
        capturer_named(session, None)
    }

    // DRM_DISPLAY_HEALTH is process-wide and tests run in parallel: pass each test its OWN key.
    fn capturer_named(session: Option<(usize, usize)>, key: Option<&str>) -> IpcDrmCapturer {
        let connector = key.map(|k| k.to_owned());
        IpcDrmCapturer {
            shared: Arc::new(Shared {
                slot: Mutex::new(FrameSlot {
                    latest: None,
                    free: [None, None],
                    ended: None,
                }),
                cv: Condvar::new(),
                transform: std::sync::atomic::AtomicI32::new(0),
            }),
            stop: Arc::new(AtomicBool::new(false)),
            display: 0,
            connector,
            session_size: session,
            transform: 0,
            snapshot_gen: scrap::wayland::display::wayland_snapshot_generation(),
            cur: Vec::new(),
            cur_w: 0,
            cur_h: 0,
            cur_fmt: Pixfmt::BGRA,
            got_frame: false,
        }
    }

    /// One BGRA pixel per label byte, so a rotation result reads as a matrix of labels.
    fn px_frame(labels: &[&[u8]], pad_bytes: usize) -> (Vec<u8>, usize, usize) {
        let h = labels.len();
        let w = labels[0].len();
        let mut buf = Vec::new();
        for row in labels {
            for &l in *row {
                buf.extend_from_slice(&[l, l, l, 255]);
            }
            buf.extend(std::iter::repeat(0u8).take(pad_bytes));
        }
        (buf, w, h)
    }

    fn labels_of(buf: &[u8], w: usize, h: usize) -> Vec<Vec<u8>> {
        (0..h)
            .map(|y| (0..w).map(|x| buf[(y * w + x) * 4]).collect())
            .collect()
    }

    #[test]
    fn a_lone_display_goes_offline_only_when_its_fallback_was_rejected() {
        // Unique name = unique health key; DRM_DISPLAY_HEALTH is process-wide.
        let list = vec![drm_display("TEST-lone-fallback", 1080, 1920)];
        let key = connector_key(&list[0]);
        let demoted = DisplayHealth {
            zero_frame_streak: DRM_GRAB_MAX_FAILURES,
            demotes: 1,
            ..DisplayHealth::new()
        };
        // Demoted alone keeps the lone display online: the whole-desktop fallback is usable.
        DRM_DISPLAY_HEALTH.lock().unwrap().insert(key.clone(), demoted);
        let mut infos = vec![DisplayInfo {
            online: true,
            ..Default::default()
        }];
        mark_demoted_displays(&list, &mut infos);
        assert!(infos[0].online, "the lone-display carve-out must survive");
        // A rejected fallback ends the carve-out: advertising online would restart-loop.
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .get_mut(&key)
            .expect("just inserted")
            .fallback_rejected = true;
        mark_demoted_displays(&list, &mut infos);
        assert!(!infos[0].online, "a rejected fallback must take the lone display offline");
        // Once the demotion cooldown lapses the display is no longer demoted, and online returns
        // even with the rejection still latched (the re-arm will clear it on the next build).
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .get_mut(&key)
            .expect("still there")
            .since = Instant::now() - demote_cooldown(1) - Duration::from_secs(1);
        infos[0].online = true;
        mark_demoted_displays(&list, &mut infos);
        assert!(infos[0].online, "past the cooldown the verdict is DRM's to retry");
    }

    #[test]
    fn the_cursor_id_names_the_orientation_too() {
        // Same wire cursor under two transforms must publish as two ids, or the client's by-id
        // cache serves the previous orientation after a mid-session rotation.
        let wire = 0xDEAD_BEEF_u64;
        assert_ne!(fold_cursor_id(wire, 0), fold_cursor_id(wire, 90));
        assert_ne!(fold_cursor_id(wire, 90), fold_cursor_id(wire, 270));
        // Deterministic per (id, transform), so an unchanged cursor is still deduped.
        assert_eq!(fold_cursor_id(wire, 90), fold_cursor_id(wire, 90));
        // The hidden sentinel is compared by VALUE at the consumers, so it must pass unfolded.
        let hidden = scrap::drm_reader::HIDDEN_CURSOR_ID;
        assert_eq!(fold_cursor_id(hidden, 90), hidden);
    }

    #[test]
    fn unrotate_hotspot_follows_the_pixel_mapping() {
        // 3 wide x 2 tall, hotspot at (2,0) (top-right): after the 90 turn (left column to top
        // row) that pixel sits at (1,2) in the 2x3 result; 270 sends it to (0,0).
        assert_eq!(unrotate_hotspot(90, 3, 2, 2, 0), (1, 2));
        assert_eq!(unrotate_hotspot(270, 3, 2, 2, 0), (0, 0));
        assert_eq!(unrotate_hotspot(180, 3, 2, 2, 0), (0, 1));
        assert_eq!(unrotate_hotspot(0, 3, 2, 2, 0), (2, 0));
    }

    #[test]
    fn a_stale_snapshot_generation_asks_for_a_rebuild_without_blaming_the_display() {
        let mut c = capturer_named(Some((64, 32)), Some("test:gen-rebuild"));
        c.snapshot_gen = c.snapshot_gen.wrapping_sub(1);
        put_frame(&c, 64, 32);
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a stale generation must rebuild, not deliver"),
        };
        assert!(err.to_string().contains("layout changed"), "{err}");
        assert!(!c.got_frame);
        assert_eq!(
            zero_frame_streak_of(&c),
            0,
            "a layout rebuild must not count against display health"
        );
    }

    #[test]
    fn unrotate_90_maps_the_left_column_to_the_top_row() {
        // The measured anchor from rustdesk#15886: mutter transform=1 carries the panel bar down
        // the scanout's LEFT edge, and upright means that edge becomes the TOP row.
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
        let mut dst = Vec::new();
        unrotate_bgra(&src, w, h, 90, &mut dst);
        // src left column top-to-bottom = [1, 4]; clockwise puts it on the top row as [4, 1].
        assert_eq!(labels_of(&dst, h, w), vec![vec![4, 1], vec![5, 2], vec![6, 3]]);
    }

    #[test]
    fn unrotate_270_is_the_inverse_of_90() {
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
        let mut once = Vec::new();
        unrotate_bgra(&src, w, h, 90, &mut once);
        let mut back = Vec::new();
        unrotate_bgra(&once, h, w, 270, &mut back);
        assert_eq!(back, src);
    }

    #[test]
    fn unrotate_180_reverses_both_axes() {
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
        let mut dst = Vec::new();
        unrotate_bgra(&src, w, h, 180, &mut dst);
        assert_eq!(labels_of(&dst, w, h), vec![vec![6, 5, 4], vec![3, 2, 1]]);
    }

    #[test]
    fn unrotate_reads_padded_strides_and_writes_tight() {
        // Row stride is derived from len/h, so a padded source must not shear the result.
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 8);
        let mut dst = Vec::new();
        unrotate_bgra(&src, w, h, 90, &mut dst);
        assert_eq!(dst.len(), w * h * 4);
        assert_eq!(labels_of(&dst, h, w), vec![vec![4, 1], vec![5, 2], vec![6, 3]]);
        let mut plain = Vec::new();
        unrotate_bgra(&src, w, h, 0, &mut plain);
        assert_eq!(labels_of(&plain, w, h), vec![vec![1, 2, 3], vec![4, 5, 6]]);
    }

    #[test]
    fn a_rotated_session_delivers_rotated_frames_and_guards_in_rotated_dims() {
        use scrap::TraitPixelBuffer;
        let mut c = capturer_with(Some((32, 64))); // rotated session of a 64x32 scanout
        c.transform = 90;
        put_frame(&c, 64, 32);
        match c.frame(Duration::from_millis(50)) {
            Ok(Frame::PixelBuffer(pb)) => {
                assert_eq!((pb.width(), pb.height()), (32, 64));
            }
            Ok(_) => panic!("expected a pixel-buffer frame"),
            Err(err) => panic!("expected a delivered frame, got {err}"),
        }
        // A scanout change still ends the session, reported in rotated dimensions.
        put_frame(&c, 32, 64);
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a scanout change must end a rotated session too"),
        };
        assert!(err.to_string().contains("(32x64 -> 64x32)"), "{err}");
    }

    fn zero_frame_streak_of(c: &IpcDrmCapturer) -> u32 {
        let key = c.connector.clone().expect("this check needs an identity");
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .get(&key)
            .map(|h| h.zero_frame_streak)
            .unwrap_or(0)
    }

    fn put_frame(c: &IpcDrmCapturer, w: usize, h: usize) {
        let mut buf = c.shared.slot.lock().unwrap().take_free().unwrap_or_default();
        buf.clear();
        buf.resize(w * h * 4, 0);
        let mut slot = c.shared.slot.lock().unwrap();
        slot.publish(w, h, Pixfmt::BGRA, buf);
    }

    #[test]
    fn a_delivered_frame_clears_the_streak_but_keeps_the_cadence_and_the_convert_verdict() {
        let key = "test:frame-keeps-cadence";
        let mut c = capturer_named(Some((64, 32)), Some(key));
        {
            let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
            let h = map.entry(key.to_owned()).or_insert_with(DisplayHealth::new);
            h.zero_frame_streak = 2;
            h.demotes = 1;
            h.rapid_builds = 3;
            h.last_build = Some(Instant::now());
            h.prefer_cpu = true;
            h.fallback_rejected = true;
        }
        put_frame(&c, 64, 32);
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));

        // Copy out and RELEASE the guard before asserting: a failing assertion while holding
        // process-wide DRM_DISPLAY_HEALTH poisons the mutex for every sibling test.
        let h = {
            let map = DRM_DISPLAY_HEALTH.lock().unwrap();
            *map.get(key).expect("the entry must SURVIVE a delivered frame")
        };
        assert_eq!(h.zero_frame_streak, 0, "a delivered frame refutes the zero-frame streak");
        assert_eq!(h.demotes, 0, "and the demotion count that streak drove");
        assert!(
            !h.fallback_rejected,
            "a delivered frame also refutes the rejected-fallback verdict"
        );
        assert_eq!(
            h.rapid_builds, 3,
            "but it says NOTHING about the rebuild cadence: keeping it is what lets the flap guard \
             reach RAPID_REBUILD_MAX for a display that delivers a first frame and then fails"
        );
        assert!(h.last_build.is_some(), "same for the timestamp the cadence is measured from");
        assert!(
            h.prefer_cpu,
            "and nothing about which GPU exports the scanout: only a topology change may clear it"
        );
    }

    #[test]
    fn frame_of_the_session_size_is_delivered() {
        let mut c = capturer_with(Some((64, 32)));
        put_frame(&c, 64, 32);
        assert!(
            matches!(c.frame(Duration::from_millis(50)), Ok(_)),
            "a frame matching the session geometry must be delivered"
        );
        assert!(c.got_frame);
    }

    #[test]
    fn a_smaller_frame_ends_the_session_instead_of_being_encoded() {
        let mut c = capturer_named(Some((1920, 1080)), Some("test:mid-session-shrink"));
        put_frame(&c, 1920, 1080);
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
        put_frame(&c, 1280, 720);
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a mid-session shrink must be a hard error, not a delivered frame"),
        };
        assert!(err.to_string().contains("changed geometry mid-session"));
        assert!(
            c.got_frame,
            "the rebuild must not look like a display that never produced a frame"
        );
        assert_eq!(
            zero_frame_streak_of(&c),
            0,
            "a session that streamed must not be counted as one that produced nothing"
        );
    }

    #[test]
    fn a_first_frame_that_never_matched_counts_as_a_session_without_frames() {
        let mut c = capturer_named(Some((1920, 1080)), Some("test:never-matched"));
        put_frame(&c, 1280, 720);
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a first frame off the advertised geometry must be a hard error"),
        };
        assert!(err.to_string().contains("never matched its advertised geometry"));
        assert!(!c.got_frame, "no frame reached the encoder, so none was produced");
        assert_eq!(
            zero_frame_streak_of(&c),
            1,
            "the display must be on its way to a PipeWire demotion, not just rebuilding"
        );
    }

    #[test]
    fn a_larger_frame_ends_the_session_too() {
        let mut c = capturer_with(Some((1280, 720)));
        put_frame(&c, 1920, 1080);
        assert!(matches!(c.frame(Duration::from_millis(50)), Err(_)));
    }

    #[test]
    fn unknown_session_size_delivers_whatever_arrives() {
        let mut c = capturer_with(None);
        put_frame(&c, 800, 600);
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
    }

    fn drm_display(name: &str, w: u32, h: u32) -> DrmDisplayInfo {
        DrmDisplayInfo {
            name: name.to_owned(),
            crtc_id: 1,
            x: 0,
            y: 0,
            width: w,
            height: h,
            active: true,
            render_node: String::new(),
            device: String::new(),
        }
    }

    fn wl_display(
        name: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> hbb_common::platform::linux::WaylandDisplayInfo {
        hbb_common::platform::linux::WaylandDisplayInfo {
            name: name.to_owned(),
            x,
            y,
            width: w,
            height: h,
            logical_size: Some((w, h)),
            refresh_rate: 60,
            transform: 0,
        }
    }

    #[test]
    fn a_lone_rotated_output_advertises_delivered_dimensions() {
        // Fix for the origin-only cut: one connector, one rotated output. The capturer will
        // deliver rotated frames, so the advertised size must swap even in the origin-only case,
        // while the logical scale is still not adopted (stays 1.0).
        let drm = [drm_display("HDMI-A-1", 1920, 1080)];
        let mut out = wl_display("HDMI-1", 0, 0, 1920, 1080);
        out.transform = 90;
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![out],
        };
        let assignment = assign_wayland_outputs(&drm, &wl.displays);
        let infos = augment_with_wayland_geometry_from(&drm, &wl, &assignment);
        assert_eq!((infos[0].width, infos[0].height), (1080, 1920));
        assert_eq!(infos[0].scale, 1.0);
    }

    #[test]
    fn transform_and_origin_come_from_the_same_snapshot() {
        // Both derive from ONE Displays snapshot: the rotated output's transform and its origin
        // must belong to the same assignment, and the multi-connector one-output guard zeroes
        // both rather than mixing a guessed origin with a real transform.
        let drm = [
            drm_display("HDMI-A-1", 1920, 1080),
            drm_display("DP-1", 2560, 1440),
        ];
        let mut rotated = wl_display("DP-1", 1920, 0, 2560, 1440);
        rotated.transform = 270;
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![rotated, wl_display("HDMI-1", 0, 0, 1920, 1080)],
        };
        let (t, origin) = transform_and_origin(&drm, 1, &wl);
        assert_eq!(t, 270);
        assert_eq!(origin, Some((1920, 0)));
        let lone = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![wl_display("HDMI-1", 0, 0, 1920, 1080)],
        };
        assert_eq!(transform_and_origin(&drm, 1, &lone), (0, None));
    }

    #[test]
    fn one_connector_assignment_drives_geometry_and_primary() {
        let drm = [
            drm_display("HDMI-A-1", 1920, 1080),
            drm_display("DP-1", 2560, 1440),
        ];
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![
                wl_display("DP-1", 1920, 0, 2560, 1440),
                wl_display("HDMI-1", 0, 0, 1920, 1080),
            ],
        };

        let assignment = assign_wayland_outputs(&drm, &wl.displays);
        let infos = augment_with_wayland_geometry_from(&drm, &wl, &assignment);
        assert_eq!((infos[0].x, infos[1].x), (0, 1920));
        assert_eq!(primary_index_from_assignment(&assignment, wl.primary), 1);
    }

    #[test]
    fn frame_buffers_circulate_instead_of_being_reallocated() {
        let mut c = capturer_with(Some((64, 32)));
        put_frame(&c, 64, 32);
        put_frame(&c, 64, 32);
        let recycled = c
            .shared
            .slot
            .lock()
            .unwrap()
            .free
            .iter()
            .find_map(|b| b.as_ref())
            .map(|b| b.as_ptr());
        assert!(
            recycled.is_some(),
            "a superseded frame must be handed back, not dropped"
        );
        put_frame(&c, 64, 32);
        assert_eq!(
            c.shared
                .slot
                .lock()
                .unwrap()
                .latest
                .as_ref()
                .map(|(.., b)| b.as_ptr()),
            recycled,
            "the receive path must refill the recycled buffer rather than allocate"
        );
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
        assert!(
            c.shared.slot.lock().unwrap().free.iter().any(|b| b.is_some()),
            "the buffer the encoder finished with must be handed back to the receive path"
        );
    }

    // Against a single free slot this asserts red: counting the offers is the point.
    #[test]
    fn two_idle_buffers_are_both_kept_rather_than_one_being_dropped() {
        let mut c = capturer_with(Some((64, 32)));
        put_frame(&c, 64, 32);
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
        while c.shared.slot.lock().unwrap().take_free().is_some() {}

        put_frame(&c, 64, 32); // fills a fresh buffer (nothing on offer) and publishes it
        put_frame(&c, 64, 32); // supersedes it -> deposit #1
        assert_eq!(
            c.shared.slot.lock().unwrap().free.iter().flatten().count(),
            1,
            "the superseded frame is the first idle buffer"
        );
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
        assert_eq!(
            c.shared.slot.lock().unwrap().free.iter().flatten().count(),
            2,
            "both idle buffers must be kept; a single slot dropped the older one"
        );
    }

    #[test]
    fn a_resolution_guess_never_steals_an_exact_name_match() {
        // The review's scenario: an earlier connector with an unmatchable name shares the
        // resolution of a later connector's exact name match. Names reserve globally first.
        let drm = vec![
            drm_display("DSI-1", 1920, 1080),
            drm_display("HDMI-A-1", 1920, 1080),
        ];
        let wl = vec![
            wl_display("HDMI-1", 0, 0, 1920, 1080),
            wl_display("Unknown-9", 1920, 0, 2560, 1440),
        ];
        let m = identity_matches(&drm, &wl);
        assert_eq!(m[1], Some(0), "the exact name match must win globally");
        assert_eq!(m[0], None, "the leftover pairing is not forced, so no identity");
        // Two unmatched connectors at the lone free resolution: ambiguous on the DRM side too,
        // so rotation must not be pinned on either.
        let drm2 = vec![
            drm_display("DSI-1", 1920, 1080),
            drm_display("DSI-2", 1920, 1080),
        ];
        let wl2 = vec![wl_display("HDMI-1", 0, 0, 1920, 1080)];
        let m2 = identity_matches(&drm2, &wl2);
        assert!(m2[0].is_none() && m2[1].is_none());
    }

    #[test]
    fn outputs_are_matched_by_name_across_the_drm_naming_difference() {
        let drm = [drm_display("HDMI-A-1", 1920, 1080), drm_display("DP-1", 2560, 1440)];
        let wl = [wl_display("DP-1", 1920, 0, 2560, 1440), wl_display("HDMI-1", 0, 0, 1920, 1080)];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(1), Some(0)]);
    }

    // The M10 case: same model and resolution, names that do not normalize to the compositor's.
    #[test]
    fn identical_monitors_that_match_no_name_take_layout_order() {
        let drm = [drm_display("DP-1", 1920, 1080), drm_display("DP-2", 1920, 1080)];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("Unknown-2", 1920, 0, 1920, 1080),
        ];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(0), Some(1)]);
    }

    #[test]
    fn one_output_is_never_claimed_by_two_connectors() {
        let drm = [drm_display("DP-1", 1920, 1080), drm_display("DP-2", 1920, 1080)];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("Unknown-2", 1920, 0, 3840, 2160),
        ];
        let got = assign_wayland_outputs(&drm, &wl);
        assert_eq!(got[0], Some(0));
        assert_ne!(got[0], got[1], "two connectors must not share one output");
    }

    #[test]
    fn a_name_match_beats_the_positional_fallback() {
        let drm = [drm_display("DP-1", 1920, 1080), drm_display("HDMI-A-1", 1920, 1080)];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("HDMI-1", 1920, 0, 1920, 1080),
        ];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(0), Some(1)]);
    }

    #[test]
    fn extra_connectors_stay_unmatched() {
        let drm = [
            drm_display("DP-1", 1920, 1080),
            drm_display("DP-2", 1920, 1080),
            drm_display("DP-3", 1920, 1080),
        ];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("Unknown-2", 1920, 0, 1920, 1080),
        ];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(0), Some(1), None]);
    }

    #[test]
    fn refresh_keeps_a_verdict_through_one_failure_and_gives_it_up_after_a_run() {
        assert_eq!(refresh_outcome(Some(3), 0), RefreshOutcome::Publish);
        assert_eq!(refresh_outcome(Some(1), 0), RefreshOutcome::Publish);
        assert_eq!(refresh_outcome(Some(0), 0), RefreshOutcome::Unavailable);
        assert_eq!(refresh_outcome(None, 1), RefreshOutcome::Restamp);
        assert_eq!(
            refresh_outcome(None, DRM_REFRESH_MAX_FAILURES - 1),
            RefreshOutcome::Restamp
        );
        assert_eq!(
            refresh_outcome(None, DRM_REFRESH_MAX_FAILURES),
            RefreshOutcome::GiveUp
        );
        assert_eq!(
            refresh_outcome(None, DRM_REFRESH_MAX_FAILURES + 5),
            RefreshOutcome::GiveUp
        );
    }

    #[test]
    fn a_dead_producer_stops_being_advertised() {
        let mut outcome = RefreshOutcome::Restamp;
        for failures in 1..=DRM_REFRESH_MAX_FAILURES {
            outcome = refresh_outcome(None, failures);
        }
        assert_eq!(outcome, RefreshOutcome::GiveUp);
        assert!(
            DRM_REFRESH_MAX_FAILURES >= 2,
            "a single transient failure must never be enough to drop the verdict"
        );
    }

    #[test]
    fn health_reports_demoted_only_while_the_cooldown_runs() {
        let mut h = DisplayHealth::new();
        assert!(!h.demoted(), "a fresh display is not demoted");
        h.zero_frame_streak = DRM_GRAB_MAX_FAILURES - 1;
        assert!(!h.demoted(), "one session short of the threshold is not demoted");
        h.zero_frame_streak = DRM_GRAB_MAX_FAILURES;
        h.demotes = 1;
        assert!(h.demoted(), "at the threshold, inside the cooldown");
        h.since = Instant::now() - demote_cooldown(h.demotes) - Duration::from_secs(1);
        assert!(!h.demoted(), "past the cooldown the display must be retried");
        h.demotes = 4;
        assert!(h.demoted(), "the backoff must still be holding it at demotion 4");
    }

    #[test]
    fn demote_cooldown_doubles_per_cycle_and_caps() {
        assert_eq!(demote_cooldown(1), DEMOTE_COOLDOWN);
        assert_eq!(demote_cooldown(2), DEMOTE_COOLDOWN * 2);
        assert_eq!(demote_cooldown(3), DEMOTE_COOLDOWN * 4);
        let cap = DEMOTE_COOLDOWN * (1 << DEMOTE_BACKOFF_MAX_SHIFT);
        assert_eq!(demote_cooldown(1 + DEMOTE_BACKOFF_MAX_SHIFT), cap);
        assert_eq!(demote_cooldown(50), cap);
        assert_eq!(demote_cooldown(u32::MAX), cap);
        assert_eq!(demote_cooldown(0), DEMOTE_COOLDOWN);
    }

    #[test]
    fn a_permanently_ungrabbable_display_stops_churning() {
        let burn = Duration::from_secs(5); // four failed sessions
        assert!(demote_cooldown(1) + burn < Duration::from_secs(40));
        assert!(demote_cooldown(5) + burn > Duration::from_secs(8 * 60));
    }
}
