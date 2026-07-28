// Server-side (`--server`, unprivileged) consumer of the root `--service`'s DRM/KMS capture stream.
//
// The phase-2 split moved only the privileged EXPORT (open + grab the scanout dma-buf fd) into the
// root service; the EGL detile / RGBA convert now runs HERE, in the unprivileged process. So this
// process DOES dlopen libdrmtap again (its unprivileged render half: `drmtap_open_render` +
// `drmtap_convert_dmabuf`), holding one render-node context on the receive thread. It connects to
// the service's `_drm` channel, learns the display geometry, then on each frame receives a small
// dma-buf descriptor + the scanout fd (over SCM_RIGHTS) and converts it to linear pixels locally.
// This mirrors the Windows `portable_service` CapturerPortable split (a privileged process captures,
// this process presents), but over rustdesk's own IPC and with only the fd (not the pixels) crossing
// the socket. A CPU-fallback path is kept: an older `.so` or a seat with no transferable dma-buf
// makes the service send `DrmFrame` + packed-BGRA over the wire, which this side stores as-is.
//
// `TraitCapturer::frame()` is synchronous (the encoder loop calls it) while the IPC receive is
// async, so a dedicated background thread runs the receive loop and keeps only the newest frame
// (latest-wins, so a slow encoder never backs the socket up). `frame()` returns that frame as a
// borrowed `PixelBuffer`, `WouldBlock` when nothing new arrived within the timeout, and a hard
// `Err` once the stream ends (the caller then rebuilds the capturer or falls back to PipeWire).
//
// The render context (`RenderConverter`) is created ONCE on the receive thread and dropped there on
// exit (NOT in `IpcDrmCapturer::Drop`): libdrmtap's EGL state + import-once EGLImage cache are
// thread-local, so both convert and close must run on the same thread.

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

// Upper bound on how long the receive thread waits for the service to answer with the display list.
const HANDSHAKE_TIMEOUT_MS: u64 = 3000;
// How long that thread may spend connecting to `_drm` before the handshake starts.
const DRM_CONNECT_TIMEOUT_MS: u64 = 1000;
/// How long a caller waits for the receive thread to hand back the display list. It must DOMINATE
/// what that thread is allowed to spend, or the outer timer fires first and abandons a handshake
/// that was still inside its own budget: the thread spends up to the connect timeout, then
/// `recv_msg_timeout2` applies HANDSHAKE_TIMEOUT_MS TWICE in the worst case (once waiting for the
/// first byte, once for the body). Derived from those parts rather than written as a constant, so a
/// change to either one cannot silently invert the relationship again.
const HANDSHAKE_WAIT_MS: u64 = DRM_CONNECT_TIMEOUT_MS + HANDSHAKE_TIMEOUT_MS * 2 + 500;

struct FrameSlot {
    // (width, height, pixel format, packed pixels) of the newest frame not yet consumed by
    // `frame()`; latest-wins. The pixel format is carried per frame because the split convert path
    // reads it from the actual convert output (XRGB8888 -> BGRA, XBGR8888 -> RGBA) rather than
    // assuming BGRA; the CPU-fallback path stores BGRA. The row stride is recoverable from
    // `pixels.len() / height` (the convert output may carry a padded stride).
    latest: Option<(usize, usize, Pixfmt, Vec<u8>)>,
    // A frame buffer no longer in use, handed back for the receive path to fill again: by `frame()`
    // when it swaps in a new frame, and by the receive path itself when it supersedes one that was
    // never consumed. A scanout is megabytes (33 MB at 4K), so allocating one per frame and freeing
    // it a moment later is the kind of churn a 30 fps loop should not be doing. One slot is enough:
    // at most one buffer is idle at a time, since the pipeline holds exactly two (the one being
    // filled and the one published) plus the one `frame()` is lending to the encoder.
    free: Option<Vec<u8>>,
    // Set once the stream ends so `frame()` returns a hard error (triggers a capturer rebuild).
    ended: Option<String>,
}

impl FrameSlot {
    /// Publish `buf` as the newest frame, recycling whatever it supersedes.
    fn publish(&mut self, w: usize, h: usize, fmt: Pixfmt, buf: Vec<u8>) {
        if let Some((.., old)) = self.latest.take() {
            self.free = Some(old);
        }
        self.latest = Some((w, h, fmt, buf));
    }
}

struct Shared {
    slot: Mutex<FrameSlot>,
    cv: Condvar,
}

pub struct IpcDrmCapturer {
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    // The buffer `frame()` hands out a borrow of; kept across calls (grow-once) and only replaced
    // when a new frame is taken from the slot.
    // The requested display index this capturer streams, for per-display failure tracking.
    display: i32,
    // What that index MEANT when the stream started. Per-display memory is keyed by this, not by the
    // index, so a hotplug that renumbers the list cannot make one monitor inherit another's verdict.
    connector: Option<String>,
    // The geometry this session was built with, which is also what the encoder was sized from: the
    // video service reads CapturerInfo{width,height} once, at build time. A frame of any other size
    // must not be delivered against it. `None` only if the handshake list did not describe this index.
    session_size: Option<(usize, usize)>,
    cur: Vec<u8>,
    cur_w: usize,
    cur_h: usize,
    // Pixel format of `cur`, taken from the frame stored in the slot (BGRA on the CPU-fallback path;
    // BGRA/RGBA per the convert output on the dma-buf path). Honored by `frame()` instead of a
    // hardcoded BGRA so an EGL-less / source-order convert is not shipped with red/blue swapped.
    cur_fmt: Pixfmt,
    // Whether this capturer ever delivered a frame. Used to distinguish a stream that fails to
    // produce ANY frame (a permanent grab failure — unsupported scanout on that CRTC) from a normal
    // teardown, so DRM can fall back to PipeWire for that display instead of rebuilding it forever.
    got_frame: bool,
}

/// Stable identity of a display: the card it lives on plus its connector name. A list index is NOT an
/// identity - `drm_enumerate_all_displays` concatenates per-card lists, so plugging or unplugging a
/// monitor renumbers every display after it, and a verdict learned about one monitor would silently
/// start applying to another. Everything remembered ABOUT a display is keyed by this instead.
fn connector_key(d: &DrmDisplayInfo) -> String {
    format!("{}:{}", d.device, d.name)
}

/// Resolve a list index to that identity against the currently advertised topology. `None` when no
/// list is available or the index is out of range; callers then simply do not consult the per-display
/// memory, which costs one retry rather than applying someone else's verdict.
/// Takes DRM_STATE, so never call it while holding one of the maps below.
fn connector_key_of(display: i32) -> Option<String> {
    match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.get(display.max(0) as usize).map(connector_key),
        _ => None,
    }
}

/// Everything this consumer has learned about ONE display, keyed by connector identity
/// (`connector_key` = `device:name`) rather than by its position in the display list. Position is not
/// identity: a hotplug renumbers the list, and a verdict pinned to an index then describes whichever
/// monitor moved into that slot.
///
/// The three verdicts live together because they are three answers to one question, "can this display
/// be captured over DRM right now", and they feed each other: the rebuild cadence and the zero-frame
/// streak both end in the same demotion, and the convert verdict is what keeps a multi-GPU display
/// off the dma-buf path so it never gets there. An entry is dropped entirely the moment the display
/// delivers a frame, which is the single reset for all of it.
#[derive(Clone, Copy)]
struct DisplayHealth {
    /// Consecutive capture sessions that ended without ever producing a frame. A display whose
    /// scanout can never be grabbed (an unsupported format on its CRTC, say) enumerates fine but
    /// never streams, so the video service would rebuild it onto DRM forever. Per display, not
    /// global, so a working monitor cannot mask a permanently failing one.
    zero_frame_streak: u32,
    /// When `zero_frame_streak` last moved, i.e. when the current demotion started.
    since: Instant,
    /// How many times this display has been demoted. Survives a cooldown expiry on purpose; see
    /// `demote_cooldown`.
    demotes: u32,
    /// When the capturer for this display was last built, and how many builds have happened inside
    /// `RAPID_REBUILD_WINDOW` of each other. Defense in depth against a flap the zero-frame streak
    /// cannot see: a display that delivers a first frame and then fails downstream every cycle (a
    /// frame the encoder rejects) clears the streak each session and would rebuild about once a
    /// second forever. A capturer that streams longer than the window resets the count, so a healthy
    /// display never accumulates.
    last_build: Option<Instant>,
    rapid_builds: u32,
    /// The consumer-side dma-buf convert failed for this display. The common cause is multi-GPU: the
    /// render node we bound to is not the GPU that exported the scanout, so the cross-device import
    /// fails permanently. The next connection then asks the service for the CPU-converted path, so
    /// the convert happens on the exporting GPU instead of the stream flapping until it demotes to
    /// PipeWire. Which GPU exports a given monitor is a stable property of the host, so this follows
    /// the monitor for the process run.
    prefer_cpu: bool,
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
        }
    }

    /// Whether the display is currently demoted to PipeWire: enough consecutive zero-frame sessions
    /// (or a detected flap), and the cooldown for its demote count has not expired yet.
    fn demoted(&self) -> bool {
        self.zero_frame_streak >= DRM_GRAB_MAX_FAILURES
            && self.since.elapsed() < demote_cooldown(self.demotes)
    }
}

static DRM_DISPLAY_HEALTH: Mutex<BTreeMap<String, DisplayHealth>> = Mutex::new(BTreeMap::new());
const DRM_GRAB_MAX_FAILURES: u32 = 4;
// A demotion is recoverable: after this cooldown the display retries DRM, so a monitor that failed
// for a transient reason is not stuck on PipeWire for the life of the process.
const DEMOTE_COOLDOWN: Duration = Duration::from_secs(30);
// Cap on the doubling below: 30 s << 4 = 8 minutes between retries.
const DEMOTE_BACKOFF_MAX_SHIFT: u32 = 4;
const RAPID_REBUILD_WINDOW: Duration = Duration::from_secs(3);
const RAPID_REBUILD_MAX: u32 = 6;

/// How long a display stays on PipeWire after its `demotes`-th demotion, doubling each time up to
/// `DEMOTE_BACKOFF_MAX_SHIFT`. Pure, so the schedule is unit-testable.
///
/// A flat cooldown has no terminal state for a display that can never be grabbed: it is demoted,
/// waits 30 s, is advertised online again, fails its four sessions in a few seconds and is demoted
/// again, which is PeerInfo churn on a ~35 s cycle for as long as the process lives. Doubling turns
/// that into a handful of retries and then near-silence, while keeping the property that made the
/// cooldown recoverable in the first place, because the entry is dropped entirely the moment the
/// display delivers a frame (see `frame()`), not decayed by time.
fn demote_cooldown(demotes: u32) -> Duration {
    DEMOTE_COOLDOWN * (1u32 << demotes.saturating_sub(1).min(DEMOTE_BACKOFF_MAX_SHIFT))
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

// How many render nodes this host exposes. Only used to tell "there is nothing to pick wrong" (one
// node) from "auto-selection is a guess" (several), so a single readdir per stream start is enough
// and it is deliberately not cached: a GPU can be bound or unbound while the service runs. On an
// unreadable /dev/dri we report 0 and keep the previous auto-select behavior, because a seat that
// cannot even list /dev/dri will fail to open a render node anyway and land on the CPU path.
fn render_node_count() -> usize {
    std::fs::read_dir("/dev/dri").map_or(0, |entries| {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                // `renderD` plus a numeric minor, so a stray `renderD.backup` or `renderDfoo` cannot
                // inflate the count and push a genuinely single-GPU host onto the CPU path.
                e.file_name()
                    .to_str()
                    .and_then(|n| n.strip_prefix("renderD"))
                    .and_then(|minor| minor.parse::<u32>().ok())
                    .is_some()
            })
            .count()
    })
}

// Coalesce the uinput-range refresh that every DrmDisplaysChanged triggers. A multi-monitor hotplug
// delivers that message once PER captured display (one recv_thread each), but the uinput desktop rect
// is global and idempotent, so one refresh serves the whole burst. UINPUT_REFRESH_GEN records the
// newest topology; UINPUT_REFRESH_BUSY lets only the first handler spawn a worker, which then keeps
// refreshing until it has served the latest generation. Net: one worker thread per burst, and the
// final layout always wins (no lost update from an out-of-order per-display thread).
static UINPUT_REFRESH_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static UINPUT_REFRESH_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl IpcDrmCapturer {
    /// Connect to the service `_drm` channel, complete the handshake (receive the display list, then
    /// request `display`), and start streaming on a background thread. Returns the capturer plus the
    /// enumerated displays so the caller can populate `display_service`. `Err` if the service has no
    /// DRM capture available or the handshake fails — the caller then falls back to PipeWire/portal.
    pub fn new(display: i32) -> ResultType<(IpcDrmCapturer, Vec<DrmDisplayInfo>)> {
        let shared = Arc::new(Shared {
            slot: Mutex::new(FrameSlot {
                latest: None,
                free: None,
                ended: None,
            }),
            cv: Condvar::new(),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel::<ResultType<Vec<DrmDisplayInfo>>>();
        {
            let shared = shared.clone();
            let stop = stop.clone();
            std::thread::spawn(move || recv_thread(display, shared, stop, tx));
        }
        let displays = match rx.recv_timeout(Duration::from_millis(HANDSHAKE_WAIT_MS)) {
            Ok(res) => res?,
            Err(_) => {
                // The recv thread still has its own connect/handshake budget. If we just returned,
                // a handshake that completes after our timeout would leave that thread streaming
                // with no owning capturer (our Drop never runs — the capturer was never built), so
                // signal it to stop before giving up.
                stop.store(true, Ordering::SeqCst);
                bail!("drm capture handshake timed out");
            }
        };
        Ok((
            IpcDrmCapturer {
                shared,
                stop,
                display,
                connector: displays.get(display.max(0) as usize).map(connector_key),
                session_size: displays
                    .get(display.max(0) as usize)
                    .map(|d| (d.width as usize, d.height as usize)),
                cur: Vec::new(),
                cur_w: 0,
                cur_h: 0,
                cur_fmt: Pixfmt::BGRA,
                got_frame: false,
            },
            displays,
        ))
    }
}

impl Drop for IpcDrmCapturer {
    fn drop(&mut self) {
        // Signal the receive thread to exit; it also exits on its own when the connection drops.
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
            // Deliver a pending frame before surfacing an end, so the last frame is not dropped.
            if let Some((w, h, fmt, buf)) = slot.latest.take() {
                drop(slot);
                // A mid-session modeset. The encoder was sized once from CapturerInfo at build time,
                // and convert_to_yuv only refuses a source LARGER than its destination, so a smaller
                // frame would be encoded into the old canvas and the stale right and bottom edges
                // would stay on screen until the connection was torn down. Fail hard instead, which
                // routes a shrink through the same rebuild an enlargement already takes. got_frame is
                // set first: this session did produce frames, so it must not be counted as one of the
                // zero-frame sessions that demote a display to PipeWire.
                if self.session_size.is_some_and(|(sw, sh)| (w, h) != (sw, sh)) {
                    self.got_frame = true;
                    let (sw, sh) = self.session_size.unwrap_or_default();
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "drm: display {} changed geometry mid-session ({sw}x{sh} -> {w}x{h}); rebuilding",
                            self.display
                        ),
                    ));
                }
                // Hand the buffer this one replaces back to the receive path instead of freeing it.
                // The encoder is done with it: `frame()` takes `&mut self`, so the borrow it lent
                // out last time has ended.
                let previous = std::mem::replace(&mut self.cur, buf);
                self.shared.slot.lock().unwrap().free = Some(previous);
                self.cur_w = w;
                self.cur_h = h;
                self.cur_fmt = fmt;
                if !self.got_frame {
                    // First frame of this session: DRM capture works for this display, clear its
                    // failure streak.
                    self.got_frame = true;
                    if let Some(key) = &self.connector {
                        DRM_DISPLAY_HEALTH.lock().unwrap().remove(key);
                    }
                }
            } else {
                let err = slot
                    .ended
                    .clone()
                    .unwrap_or_else(|| "drm stream ended".to_owned());
                if !self.got_frame {
                    // This session never produced a frame for THIS display. If enough sessions in a
                    // row fail this way for the same display, its scanout is effectively ungrababble;
                    // count it so get_capturer_info() will refuse that display and the video service
                    // falls back to PipeWire for it (other displays are unaffected).
                    // No identity means the handshake list did not describe this index, so there is
                    // nothing safe to attribute the failure to; skip rather than blame a neighbour.
                    // Recording it under the empty key would do exactly that, because an index that
                    // cannot be resolved reads back as the same empty key in get_capturer_info: one
                    // unidentifiable display would demote the next one.
                    if let Some(key) = self.connector.clone() {
                        let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
                        let h = map.entry(key).or_insert_with(DisplayHealth::new);
                        h.zero_frame_streak += 1;
                        h.since = Instant::now();
                        // Count the demote cycle exactly once, as the streak crosses the threshold.
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
                    } else {
                        log::debug!(
                            "drm: display {} produced no frame but has no connector identity; \
                             not counting it against any display",
                            self.display
                        );
                    }
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

// Background receive loop. Owns the `_drm` connection and the async runtime; keeps the newest frame
// in `shared.slot`. Runs on its own thread because `frame()` is sync and one blocking consumer is
// enough for DRM.
#[tokio::main(flavor = "current_thread")]
async fn recv_thread(
    display: i32,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<ResultType<Vec<DrmDisplayInfo>>>,
) {
    // Unique tag for this stream's cursor entries so teardown only erases its own (see
    // remove_drm_cursor); a rebuilt stream for the same display index gets a newer epoch.
    let cursor_epoch = next_cursor_epoch();
    // Handshake: connect, receive the display list, request the display.
    let mut conn = match connect_drm(DRM_CONNECT_TIMEOUT_MS).await {
        Ok(c) => c,
        Err(err) => {
            let _ = tx.send(Err(err));
            return;
        }
    };
    let displays = match conn.recv_msg_timeout2(HANDSHAKE_TIMEOUT_MS).await {
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
    // The service binds this stream to (device, crtc_id) at DrmStart, which survives a topology change.
    // Everything on this side is addressed by LIST INDEX, which does not: drm_enumerate_all_displays
    // concatenates per-card lists, so plugging or unplugging a monitor renumbers them. Record what this
    // stream was actually bound to, so a renumbering can be detected on the next hotplug instead of the
    // client being shown, and having its clicks mapped to, whichever monitor now occupies this index.
    let bound_to = displays
        .get(display.max(0) as usize)
        .map(|d| (d.device.clone(), d.crtc_id));
    let our_key = displays.get(display.max(0) as usize).map(connector_key);
    // Open the unprivileged render-node convert context ONCE, on THIS thread, before we answer the
    // display list with DrmStart; it is dropped on this same thread when the loop exits (its EGL
    // state + import-once cache are thread-local). `None` means no usable render node (a locked-down
    // seat with no /dev/dri/renderD* access): we then ask the service for the CPU-converted `DrmFrame` path via
    // `need_cpu`, so a render-node-less seat still captures instead of the service streaming a dma-buf
    // fd we cannot detile (which would lose the stream and force a PipeWire fallback nobody may be
    // present to approve on an unattended seat).
    // Skip opening the render-node converter entirely when this display previously failed to convert
    // (multi-GPU render-node mismatch): request the CPU path so the service does the conversion on the
    // exporting GPU. Otherwise open it normally and fall back to CPU only if no render node is usable.
    // Bind the converter to the GPU that EXPORTS this display's scanout, which the service
    // named in the display list. Auto-selection can land on a different GPU on a multi-GPU
    // host, and importing a scanout across vendors can fail on an incompatible tiling
    // modifier. Empty means the service could not name it (an older service, or a device with no
    // render node of its own): auto-select then, but only where there is nothing to pick wrong, per
    // the ambiguity check below. Every display of one device carries the same node, so a display
    // index that does not resolve still gets the right answer from the first entry.
    let render_node = displays
        .get(display.max(0) as usize)
        .or_else(|| displays.first())
        .map(|d| d.render_node.clone())
        .unwrap_or_default();
    // An unnamed exporter on a host that HAS more than one render node is not safe to auto-select.
    // The failure is silent: on a single-SoC multi-device host (a Jetson exports the scanout from
    // nvidia-drm while the first render node belongs to tegra) importing the other device's scanout
    // SUCCEEDS and yields corrupted pixels, so there is no convert error for the prefer-cpu bit above
    // to learn from - the stream just looks broken. The node is empty when the service ran against a
    // libdrmtap without `drmtap_render_node` (we dlopen by soname, so the runtime .so can be older
    // than the one this was built against, anywhere in 0.4.10..0.4.14 -- below that it does not load
    // at all). Ask for the CPU path instead: the service converts on the
    // device it already has open, which is correct by construction. Single-render-node hosts (the
    // common case) keep the dma-buf fast path untouched.
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
        .send_msg(&Data::DrmStart { display, need_cpu }, None)
        .await
    {
        let _ = tx.send(Err(err));
        return;
    }
    let _ = tx.send(Ok(displays));

    // Stream until stopped or the connection ends. Poll the header read with a short timeout (rather
    // than blocking indefinitely) so a dropped capturer re-checks `stop` and tears down promptly even
    // when the producer has stalled (no frames arriving). A dma-buf frame carries its fd inline on the
    // header (no body); a CPU-fallback frame and a cursor each carry a `next_raw()` body immediately
    // after their header, so only the header read needs the poll.
    let end_reason = loop {
        if stop.load(Ordering::SeqCst) {
            break "stopped".to_owned();
        }
        // The decoded `Data` plus any SCM_RIGHTS fd that rode this frame (the scanout dma-buf fd).
        let (msg, recv_fd) = match conn.recv_msg_timeout2(200).await {
            None => continue, // timeout: re-check stop at the loop top
            Some(Ok(pair)) => pair,
            Some(Err(err)) => break format!("recv: {err}"),
        };
        match msg {
            // Zero-copy split path: a dma-buf descriptor + (usually) the scanout fd. Import + EGL
            // detile/convert to linear pixels HERE, then copy them latest-wins into the slot. That
            // copy out of the context-owned convert buffer is the ONE remaining pixel copy in the
            // whole pipeline (only the fd + this small descriptor crossed the socket).
            Data::DrmFrameDmabuf(desc) => {
                let conv = match converter.as_mut() {
                    Some(c) => c,
                    None => break "no DRM render node; cannot convert dma-buf frame".to_owned(),
                };
                // The fd number valid in THIS process: the received fd when the producer attached
                // one, or -1 for an import-once cache hit (libdrmtap reuses the EGLImage it holds for
                // `fb_id`). `has_fd` set but no fd delivered is a protocol desync.
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
                // Rebuild the libdrmtap descriptor from the wire fields; `convert` overwrites its
                // `dma_buf_fd` with `received_fd` (the exporter's local int is meaningless here).
                let mut ddesc = drmtap_dmabuf_desc {
                    dma_buf_fd: -1,
                    width: desc.width,
                    height: desc.height,
                    format: desc.format,
                    modifier: desc.modifier,
                    fb_id: desc.fb_id,
                    // Clamped although the producer already normalizes it and must be root: this
                    // value indexes offsets/pitches inside libdrmtap, and the wire is the one place
                    // it arrives from another process.
                    num_planes: desc.num_planes.clamp(1, 4),
                    offsets: desc.offsets,
                    pitches: desc.pitches,
                    hdr_eotf: desc.hdr_eotf,
                    hdr_max_nits: desc.hdr_max_nits,
                };
                match conv.convert(&mut ddesc, received_fd) {
                    Ok((data, w, h, fmt)) => {
                        // The convert output is borrowed from the render context and is only valid
                        // until the next convert, so it must be copied out. Copy into a recycled
                        // buffer, and outside the slot lock, so a multi-megabyte memcpy never holds
                        // the encoder off the slot.
                        let mut buf = shared.slot.lock().unwrap().free.take().unwrap_or_default();
                        buf.clear();
                        buf.extend_from_slice(data);
                        let mut slot = shared.slot.lock().unwrap();
                        slot.publish(w as usize, h as usize, fmt, buf);
                        shared.cv.notify_one();
                    }
                    // Transient convert contention: skip this frame (latest-wins keeps the newest),
                    // do not tear the stream down.
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => {
                        // The consumer render node could not import this buffer. A common multi-GPU
                        // cause: the auto-selected renderD* is not the GPU that exported the scanout,
                        // so cross-device import fails permanently. Prefer the CPU path on reconnect
                        // (service converts on the exporting GPU) instead of flapping to PipeWire.
                        drm_set_prefer_cpu(&our_key);
                        break format!("convert: {err}");
                    }
                }
                // `recv_fd` (the OwnedFd, if any) is dropped/closed at the end of this iteration, AFTER
                // convert has imported it (the EGLImage import holds its own reference to the buffer).
                // Ack this frame so the producer releases one send credit and forwards the next: we
                // have consumed it (converted, or skipped on transient contention -- ready either way).
                // This bounds the socket to a couple of in-flight frames instead of a stale backlog.
                if let Err(err) = conn.send_frame_ack().await {
                    break format!("frame ack: {err}");
                }
            }
            // CPU-fallback path (old `.so` / no transferable dma-buf): the producer packed BGRA and
            // sent it over the wire after the header. Store it as-is (BGRA); no convert needed.
            Data::DrmFrame { width, height } => {
                // Reject degenerate geometry before it reaches the slot: `frame()` hands this to
                // PixelBuffer::new which derives the stride as `data.len() / height`, so height==0
                // would divide by zero, and a zero width is meaningless. Require the body to hold at
                // least width*height*4 BGRA bytes so a short body cannot misframe downstream.
                if width == 0 || height == 0 {
                    break format!("cpu frame: degenerate geometry {width}x{height}");
                }
                let need = (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(4);
                // Read the body straight into a recycled frame buffer and publish that same buffer:
                // the pixels are copied once, by the kernel, on their way out of the socket.
                let mut buf = shared.slot.lock().unwrap().free.take().unwrap_or_default();
                match conn.next_raw_into(&mut buf).await {
                    Ok(()) => {
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
                    Err(err) => break format!("frame body: {err}"),
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
                // width*height*4 RGBA bytes. Require the body to carry at least that many so a short
                // body cannot make the client read past the buffer. A hidden-cursor sentinel arrives
                // as 0x0 with an empty body, for which `need` is 0 and this check is a no-op.
                let need = (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(4);
                // A cursor is tiny and changes rarely, so this one keeps its own buffer (the frame
                // recycler is for scanout-sized bodies) and hands it straight to the cursor cache.
                let mut raw = Vec::new();
                match conn.next_raw_into(&mut raw).await {
                    Ok(()) => {
                        if raw.len() < need {
                            break format!(
                                "cursor body {} bytes < {need} for {width}x{height}",
                                raw.len()
                            );
                        }
                        set_drm_cursor(
                            display,
                            cursor_epoch,
                            DrmCursorData {
                                id,
                                width: width as i32,
                                height: height as i32,
                                hotx,
                                hoty,
                                colors: raw,
                            },
                        );
                    }
                    Err(err) => break format!("cursor body: {err}"),
                }
            }
            // Live hotplug: the service pushed a fresh display list after a connector-topology change.
            // Swap it into the sticky positive availability cache directly (no re-probe over `_drm`, so
            // this never trips the wayland::clear() re-probe restart loop). A subsequent
            // get_display_infos()/get_primary_index() then reports the fresh geometry.
            Data::DrmDisplaysChanged(list) => {
                // Did this stream's index just come to mean a different monitor? Compare against what
                // the service actually bound us to. If it moved, keeping the stream alive would send
                // monitor A's pixels under monitor B's advertised geometry, and route injected input
                // by B's rect, until something else happened to fail. End it instead: the video
                // service rebuilds against the fresh list, which is the same path a resolution change
                // already takes. Checked BEFORE the list is swapped in, so the comparison is against
                // the topology this stream was started on.
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
                // Forward the fresh list INCLUDING an empty one (last monitor unplugged): the
                // availability cache must transition out of Available rather than keep advertising
                // the removed displays. See swap_available_displays.
                swap_available_displays(list);
                // Nothing to invalidate on a topology change any more: the prefer-cpu verdict is
                // keyed by connector identity, so it follows its monitor instead of its position.
                // The raw DRM list is not the whole story: the Wayland LOGICAL geometry cache and
                // the uinput absolute range are both set once at init, so after a hotplug/modeset the
                // augmented geometry and injected-coordinate range are stale. Invalidate the cache so
                // the next augmentation re-reads fresh geometry, and reapply the uinput mouse range
                // for the new desktop layout.
                scrap::wayland::display::clear_wayland_displays_cache();
                // Reapply the uinput range OFF this recv loop, coalesced across the per-display
                // recv_threads (see UINPUT_REFRESH_*). Awaiting update_uinput_resolution inline would
                // stall frame reception for the whole hotplug (it does a Wayland geometry roundtrip),
                // and this recv_thread is a current-thread runtime -- so the worker builds its own.
                // Bump the generation, then let only the first caller spawn the single worker; it
                // refreshes until it has served the newest generation, so a multi-monitor hotplug
                // runs ONE thread and the final layout wins. Not ordered against frame delivery.
                UINPUT_REFRESH_GEN.fetch_add(1, Ordering::AcqRel);
                if !UINPUT_REFRESH_BUSY.swap(true, Ordering::AcqRel) {
                    std::thread::spawn(|| {
                        let rt = match tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                        {
                            Ok(rt) => rt,
                            Err(err) => {
                                // Release the slot so a later topology change can retry, and say
                                // why: silently skipping would leave the uinput range stale for
                                // the new layout with nothing in the log to explain it.
                                log::warn!(
                                    "drm: uinput refresh worker could not build a runtime: {err}"
                                );
                                UINPUT_REFRESH_BUSY.store(false, Ordering::Release);
                                return;
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
                            // Caught up: release, then re-check for a request that raced in after our
                            // load but before the release, taking the worker role back if so.
                            UINPUT_REFRESH_BUSY.store(false, Ordering::Release);
                            if UINPUT_REFRESH_GEN.load(Ordering::Acquire) == served {
                                break;
                            }
                            if UINPUT_REFRESH_BUSY.swap(true, Ordering::AcqRel) {
                                break; // another handler already started a fresh worker
                            }
                        }
                    });
                }
            }
            _ => {} // ignore any unexpected control message
        }
    };
    log::info!("drm capture stream ended: {end_reason}");
    // Drop the render context on THIS thread (its EGL state + cached imports are thread-local; a
    // cross-thread close would strand them — the 0.4.8 EGL-leak/OOM class). Explicit so it releases
    // before the post-loop cleanup rather than at some later scope exit, and NEVER in
    // `IpcDrmCapturer::Drop` (which runs on the encoder thread).
    drop(converter);
    // Drop only THIS stream's cursor entry so a torn-down monitor does not erase the cursor state of
    // other still-active streams, nor a replacement stream that already re-took this display index.
    remove_drm_cursor(display, cursor_epoch);
    let mut slot = shared.slot.lock().unwrap();
    slot.ended = Some(format!("drm stream ended ({end_reason})"));
    shared.cv.notify_one();
}

// The latest DRM hardware-cursor snapshots, published by recv_thread and read by the cursor service
// (platform::linux::get_cursor / get_cursor_data). Keyed by display index because a multi-monitor
// client runs one recv_thread per display and the hardware cursor lives on whichever CRTC the
// pointer is over (the others report the hidden sentinel). Keying per stream — instead of a single
// last-writer-wins global — stops one stream's hidden sentinel from clobbering another stream's
// visible cursor, and lets a torn-down stream drop only its own entry.
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
// Monotonic per-stream tag. A display index can be served by successive recv_threads (a rebuilt
// stream reuses the index), so each cursor entry is stamped with the writing stream's epoch and a
// torn-down stream drops its entry ONLY if the epoch still matches. Without this, a predecessor that
// exits just after its replacement published a fresh cursor for the same index would erase it.
static DRM_CURSOR_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_cursor_epoch() -> u64 {
    DRM_CURSOR_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn set_drm_cursor(display: i32, epoch: u64, c: DrmCursorData) {
    // Compare-and-set: a still-draining predecessor stream (older epoch) must not overwrite the entry a
    // replacement stream (newer epoch) already published for the same display index -- otherwise it
    // would re-stamp the slot with its old epoch and then delete it on teardown via remove_drm_cursor,
    // erasing the fresh cursor. Only accept a write whose epoch is at least the stored one.
    let mut map = DRM_CURSOR.lock().unwrap();
    match map.get(&display) {
        Some((stored, _)) if *stored > epoch => {}
        _ => {
            map.insert(display, (epoch, c));
        }
    }
}

// Compare-and-remove: drop the entry only if THIS stream (epoch) still owns it. A replacement stream
// for the same index holds a newer epoch, so a late-exiting predecessor leaves the fresh cursor intact.
fn remove_drm_cursor(display: i32, epoch: u64) {
    let mut map = DRM_CURSOR.lock().unwrap();
    if map.get(&display).map(|(e, _)| *e) == Some(epoch) {
        map.remove(&display);
    }
}

// Which cursor to present: prefer the visible one (the pointer is over exactly one captured CRTC at
// a time), else fall back to any (hidden) entry so the client still gets the hidden sentinel when the
// pointer is off every captured monitor. Returns what `f` extracts from it, so a caller that only
// wants the id does not pay for a clone of the pixels. `None` only when no stream is active.
fn with_drm_cursor<T>(f: impl Fn(&DrmCursorData) -> T) -> Option<T> {
    let map = DRM_CURSOR.lock().unwrap();
    map.values()
        .map(|(_, c)| c)
        .find(|c| c.id != scrap::drm_reader::HIDDEN_CURSOR_ID)
        .or_else(|| map.values().map(|(_, c)| c).next())
        .map(f)
}

/// The id of the current DRM hardware cursor (None if no stream). The cursor service polls this to
/// detect shape changes (a change triggers a `get_cursor_data` fetch), so it runs at frame cadence
/// and deliberately reads the id WITHOUT copying the pixels: a 256x256 cursor is 256 KiB, and
/// cloning that 30 times a second to look at 8 bytes of it is pure waste.
pub fn drm_cursor_id() -> Option<u64> {
    with_drm_cursor(|c| c.id)
}

/// The current DRM hardware-cursor snapshot (RGBA), or None. The pixels are premultiplied ARGB and
/// are passed through as-is, which is exactly what the XFixes path does (`platform/linux.rs`
/// `get_cursor_data`), so the client sees one cursor format whichever backend produced it.
pub fn drm_cursor() -> Option<DrmCursorData> {
    with_drm_cursor(|c| c.clone())
}

// ---------------------------------------------------------------------------
// Server capture-path integration (the parallel, gated DRM path)
//
// The `--server` selects DRM/KMS capture over PipeWire when the root service offers the `_drm`
// channel. Availability + the display list are probed once and cached: the `_drm` listener now
// serves consumers concurrently (one connection per captured display), but re-probing on every
// enumeration still churns connections needlessly and briefly tripped a restart loop in testing, so
// the result is cached durably. The cache is seeded before capture starts (display enumeration) and
// by the capturer handshake, and only reset by `clear()` on teardown.
// ---------------------------------------------------------------------------

enum ProbeState {
    Unknown,
    // Timestamped so a negative verdict expires instead of permanently disabling DRM (see
    // is_available): displays that appear after startup (a headless boot settling, a monitor
    // hotplug, or a --service restart) can then re-enable it without restarting the --server.
    Unavailable(Instant),
    // Timestamped with the instant the list was probed. A positive verdict is sticky (DRM stays
    // selected), but once it ages past POSITIVE_TTL is_available refreshes the list off the hot path
    // so an idle hotplug does not leave a phantom display in enumeration. The timestamp lives in the
    // variant so every site that publishes an Available list is forced to stamp it.
    Available(Instant, Vec<DrmDisplayInfo>),
}

static DRM_STATE: Mutex<ProbeState> = Mutex::new(ProbeState::Unknown);
// How long a negative availability verdict is trusted before is_available re-probes.
const NEGATIVE_TTL: Duration = Duration::from_secs(30);
// How long a positive verdict is served before is_available kicks a background list refresh. The
// verdict stays true across the refresh; only the cached display list is renewed.
const POSITIVE_TTL: Duration = Duration::from_secs(15);

/// Query the service for the current DRM display list without starting a stream: connect, read the
/// list the service sends on connect, then drop the connection (the service closes it when we do
/// not send `DrmStart`). Runs the async work on a throwaway thread so it is safe to call from any
/// context (a nested `#[tokio::main]` would panic when called from inside a runtime).
fn query_displays() -> ResultType<Vec<DrmDisplayInfo>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(query_displays_async());
    });
    rx.recv_timeout(Duration::from_millis(HANDSHAKE_WAIT_MS))
        .map_err(|_| anyhow!("drm display query timed out"))?
}

#[tokio::main(flavor = "current_thread")]
async fn query_displays_async() -> ResultType<Vec<DrmDisplayInfo>> {
    let mut conn = connect_drm(DRM_CONNECT_TIMEOUT_MS).await?;
    match conn.recv_msg_timeout2(HANDSHAKE_TIMEOUT_MS).await {
        Some(Ok((Data::DrmDisplayList(v), _fd))) => Ok(v),
        Some(Ok((other, _fd))) => Err(anyhow!("expected DrmDisplayList, got {:?}", other)),
        Some(Err(err)) => Err(err),
        None => Err(anyhow!("timed out waiting for DrmDisplayList")),
    }
}

// Transient-failure budget for the cold probe: a `_drm` probe can fail transiently (the producer
// is not up yet, a connection race), so we retry across a few connections before durably giving up.
// This keeps one cold-start hiccup from permanently disabling DRM capture for the session, while
// still settling to `Unavailable` on a genuinely DRM-less host.
static DRM_PROBE_FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const DRM_PROBE_MAX_FAILURES: u32 = 5;
// Same idea for the BACKGROUND refresh of a positive verdict. One failed refresh is not evidence
// that the displays are gone (a transient open, an EACCES), which is why it keeps the verdict; but
// an unbroken run of them is evidence that nothing is serving `_drm` any more. If the root
// `--service` dies while this `--server` lives, every refresh fails forever, the cached verdict
// keeps enumeration advertising a DRM list nothing can stream, and every display restart-loops.
// After this many consecutive failures the verdict drops back to `Unknown`, NOT to `Unavailable`:
// we have no evidence about the hardware, only about the producer, so the next enumeration re-probes
// (and settles to Unavailable itself if DRM really is gone). Refreshes are at most one per
// POSITIVE_TTL, so three of them is a dead producer, not a hiccup.
static DRM_REFRESH_FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const DRM_REFRESH_MAX_FAILURES: u32 = 3;
// Single-flight guard: exactly one caller runs the blocking availability probe at a time, so
// is_available() never calls query_displays() (up to ~4s of IPC) while holding DRM_STATE.
static DRM_PROBE_IN_FLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Advanced by every publish of DRM_STATE. `refresh_available_async` samples it before its slow,
/// UNLOCKED probe and discards its own result if this moved meanwhile, so an older probe can never
/// overwrite a newer verdict (a hotplug push, say) -- which matters because an empty probe result
/// drops to Unavailable and would otherwise disable DRM on a host whose monitor just came back.
static DRM_STATE_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Publish an availability verdict. EVERY write to DRM_STATE goes through here so the generation
/// stays truthful; writing the state directly would silently defeat the staleness check above.
#[inline]
fn publish_probe_state(st: &mut ProbeState, next: ProbeState) {
    *st = next;
    DRM_STATE_GEN.fetch_add(1, Ordering::Release);
}

/// RAII release for the single-flight probe guard. Whichever path acquires DRM_PROBE_IN_FLIGHT (the
/// cold probe in is_available, or refresh_available_async) holds one of these so the guard clears on
/// EVERY exit -- normal return, an early return, a panic in query_displays, or a poisoned DRM_STATE.
/// A single leaked release would wedge the guard true and freeze every future probe AND refresh.
struct ProbeInFlightGuard;
impl Drop for ProbeInFlightGuard {
    fn drop(&mut self) {
        DRM_PROBE_IN_FLIGHT.store(false, Ordering::Release);
    }
}

/// Whether the root service offers DRM/KMS capture. The positive result and a definitive negative
/// (connected, but no displays) are cached; a transient probe error stays `Unknown` for a few
/// retries. Normally the cache is warmed at `--server` startup (`warm_availability`), so the first
/// client connection hits the fast `Available` path.
pub(super) fn is_available() -> bool {
    // Fast path under the lock: read the cached verdict, expiring a stale negative so a host that had
    // no displays at probe time can still enable DRM once displays appear (without a --server
    // restart). NEVER call the blocking probe while holding DRM_STATE: a cold or expired probe would
    // otherwise serialize every async caller for the whole query_displays() timeout (~4s).
    let verdict = {
        let mut st = DRM_STATE.lock().unwrap();
        if let ProbeState::Unavailable(since) = &*st {
            if since.elapsed() >= NEGATIVE_TTL {
                publish_probe_state(&mut st, ProbeState::Unknown);
                DRM_PROBE_FAILURES.store(0, Ordering::Relaxed);
            }
        }
        match &*st {
            // Sticky positive; refresh the list off the hot path if it has gone stale (see below).
            ProbeState::Available(since, _) => Some((true, since.elapsed() >= POSITIVE_TTL)),
            ProbeState::Unavailable(_) => Some((false, false)),
            ProbeState::Unknown => None, // fall through and probe with the lock released
        }
    };
    if let Some((available, stale)) = verdict {
        // A stale positive verdict re-probes on a background thread and returns the still-valid
        // verdict immediately: never block the (often async) caller for the probe timeout, and never
        // flip a live positive to false, which would bounce a capturing session to the portal. The
        // refresh only replaces the list when the probe returns a non-empty set.
        if stale {
            refresh_available_async();
        }
        return available;
    }
    // Single-flight: exactly one caller probes at a time. While a probe is in flight, others return
    // the current cache-only verdict instead of stacking redundant `_drm` probes or blocking on the
    // mutex across the I/O. warm_availability normally seeds `Available` before clients connect, so
    // this cold path is rare.
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return matches!(&*DRM_STATE.lock().unwrap(), ProbeState::Available(..));
    }
    // Release the guard on every exit from here (including a panic in query_displays or a poisoned
    // DRM_STATE lock) so a probe error can never wedge the single-flight guard true.
    let _in_flight = ProbeInFlightGuard;
    let t = Instant::now();
    let result = query_displays();
    let mut st = DRM_STATE.lock().unwrap();
    let available = match result {
        Ok(list) if !list.is_empty() => {
            log::debug!(
                "drm: availability probe -> available ({} displays) in {:?}",
                list.len(),
                t.elapsed()
            );
            // Reset the budget: it is meant to absorb a RUN of failures, and without this it is
            // spent once per process. A negative verdict expires back to Unknown (see the
            // NEGATIVE_TTL branch above), so the next cold probe would inherit an already exhausted
            // counter and demote on its first failure, however long ago the earlier ones were.
            DRM_PROBE_FAILURES.store(0, Ordering::Relaxed);
            publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
            true
        }
        Ok(_) => {
            log::info!("drm: availability probe -> no displays in {:?}", t.elapsed());
            publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
            false
        }
        Err(err) => {
            let n = DRM_PROBE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
            if n >= DRM_PROBE_MAX_FAILURES {
                log::info!("drm: availability probe failed {n}x ({err}); disabling DRM");
                publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
            } else {
                // Stay Unknown so the next connection re-probes (cold-start race).
                log::info!(
                    "drm: availability probe failed ({err}), attempt {n}/{DRM_PROBE_MAX_FAILURES}; will retry"
                );
            }
            false
        }
    };
    drop(st);
    available
    // `_in_flight` drops here, releasing DRM_PROBE_IN_FLIGHT.
}

/// Refresh a stale positive verdict off the hot path: probe on a background thread and, on a non-empty
/// result, renew the cached display list so enumeration stops advertising a display an idle hotplug
/// removed. Single-flight via the same guard as the cold probe so a refresh never stacks with a probe
/// or another refresh. A single failed probe never demotes the verdict (a transient failure must not
/// disable a working DRM session); it re-stamps instead, so we re-probe at most once per POSITIVE_TTL
/// even when the list is unchanged or the probe fails. `DRM_REFRESH_MAX_FAILURES` consecutive ones do
/// give the verdict up, to `Unknown`, because by then the evidence is about the producer being gone
/// rather than about one probe.
fn refresh_available_async() {
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    // Take the guard IMMEDIATELY, before anything that can unwind -- the DRM_STATE lock below can be
    // poisoned. It releases the single-flight flag on every exit: a normal return, an unwind here, a
    // panic inside query_displays, or a failed spawn (the closure it moved into is dropped with it).
    // Otherwise one failure would leave the flag set and freeze every future probe.
    let in_flight = ProbeInFlightGuard;
    // Generation of the verdict we are refreshing. query_displays() below runs UNLOCKED because it is
    // slow, so a hotplug push (swap_available_displays) can publish a newer verdict while we probe;
    // we re-check the generation before publishing so an older probe cannot overwrite it. That
    // matters now that an empty result drops to Unavailable: a probe that started while the monitors
    // were gone would otherwise disable DRM on a host whose monitor has since come back. Read under
    // the lock, together with the state, so the pair is consistent.
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
                // Someone republished while we probed (a hotplug push, or a cold probe). Their
                // verdict is newer than ours; leave it alone.
                return;
            }
            match result {
                Ok(fresh) if fresh.is_empty() => {
                    // No active CRTC left: every monitor is gone. Keeping the previous list here is
                    // what leaves enumeration advertising removed displays indefinitely on an idle
                    // host, where there is no live stream to deliver the hotplug push. Drop to
                    // Unavailable exactly as swap_available_displays does; a later probe restores
                    // Available when a monitor comes back.
                    log::info!("drm: refresh -> 0 displays, marking DRM unavailable");
                    DRM_REFRESH_FAILURES.store(0, Ordering::Relaxed);
                    publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
                }
                Ok(fresh) => {
                    DRM_REFRESH_FAILURES.store(0, Ordering::Relaxed);
                    publish_probe_state(&mut st, ProbeState::Available(Instant::now(), fresh))
                }
                // A failed probe is not evidence that the displays are gone (a transient open or
                // EACCES); keep the verdict, just restamp so we retry after the next TTL. That
                // restamp touches only the TTL, not the verdict, so it deliberately does NOT go
                // through publish_probe_state: there is nothing for a concurrent probe to lose by
                // publishing over it. But a RUN of failures says the producer is gone, and holding
                // a positive verdict then is what leaves enumeration advertising a DRM list nothing
                // can serve, with every display restart-looping; after DRM_REFRESH_MAX_FAILURES
                // give the verdict up (to Unknown, which does go through publish_probe_state) and
                // let the next enumeration decide from scratch.
                Err(err) => {
                    let n = DRM_REFRESH_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
                    if n >= DRM_REFRESH_MAX_FAILURES {
                        log::info!(
                            "drm: availability refresh failed {n}x ({err}); the producer looks gone, \
                             dropping the cached verdict so the next enumeration re-probes"
                        );
                        DRM_REFRESH_FAILURES.store(0, Ordering::Relaxed);
                        publish_probe_state(&mut st, ProbeState::Unknown);
                    } else if let ProbeState::Available(since, _) = &mut *st {
                        *since = Instant::now();
                    }
                }
            }
        });
    // Thread creation can fail (EAGAIN under thread/RLIMIT pressure). Nothing to release here: the
    // guard moved into the closure, which is dropped along with it, so the flag clears on that path
    // too. Releasing it explicitly would be worse than redundant -- by then another refresh may have
    // acquired the flag, and clearing it would let two probes run at once.
    let _ = spawned;
}

/// Warm the availability cache at `--server` startup so the first client connection does not race a
/// cold `_drm` probe. A cold probe blocks display enumeration, and if it has not settled when the
/// peer info is built the display list goes out empty and the client shows "No displays" and
/// retries (the "connects on the Nth try" symptom). Probes with a short retry budget and only caches
/// the positive result; a genuinely DRM-less host just falls through to the lazy `is_available()`.
pub(super) fn warm_availability() {
    // Nothing on X11 can consume a DRM stream, and probing makes the ROOT service open DRM readers,
    // so an X11 host running a drm build would pay that at every startup for a path it can never
    // take. The lazy probe behind is_available is reached only from the Wayland paths already.
    if crate::platform::linux::is_x11() {
        return;
    }
    for _ in 0..10 {
        if matches!(&*DRM_STATE.lock().unwrap(), ProbeState::Available(..)) {
            return;
        }
        match query_displays() {
            Ok(list) if !list.is_empty() => {
                log::info!("drm: consumer cache warmed ({} displays) at startup", list.len());
                publish_probe_state(&mut DRM_STATE.lock().unwrap(), ProbeState::Available(Instant::now(), list));
                return;
            }
            // Producer not ready yet (or no DRM): back off and retry; never cache a negative here.
            _ => std::thread::sleep(Duration::from_millis(300)),
        }
    }
    log::info!("drm: consumer cache warm found no producer at startup (will probe lazily)");
}

/// The cached DRM displays as protobuf `DisplayInfo`, augmented with the compositor's logical layout
/// (per-monitor position + scale). `None` until probed/available.
pub(super) fn get_display_infos() -> Option<Vec<DisplayInfo>> {
    let list = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.clone(),
        _ => return None,
    };
    let multi = list.len() > 1;
    let mut infos = augment_with_wayland_geometry(&list);
    // On a multi-monitor host a display demoted to PipeWire has no geometry-consistent
    // per-connector stream to fall through to -- the portal exposes a single whole-desktop stream, so
    // serving it for one connector would stretch the frame and offset all input. Advertise such a
    // display OFFLINE while keeping its list position, so the index space stays aligned with
    // get_capturer_info() (dropping it would shift every later index) and the client re-enumerates
    // against a consistent list instead of driving a display the server then refuses. A single-display
    // host is left online: there the whole-desktop stream IS that display, so the PipeWire fallback is
    // geometry-consistent and get_capturer_for_display serves it.
    if multi {
        let health = DRM_DISPLAY_HEALTH.lock().unwrap();
        for (idx, info) in infos.iter_mut().enumerate() {
            let key = match list.get(idx) {
                Some(d) => connector_key(d),
                None => continue,
            };
            if health.get(&key).is_some_and(|h| h.demoted()) {
                info.online = false;
            }
        }
    }
    Some(infos)
}

/// Index (into the cached DRM display list) of the compositor's PRIMARY output. DRM connector order
/// is not the compositor's primary, so match the compositor's primary (from the same Wayland source
/// the geometry augmentation uses) to the DRM list by normalized connector name; fall back to 0 when
/// unknown. Without this the first DRM connector is always streamed, which is the wrong initial
/// display whenever the primary is not connector 0.
pub(super) fn get_primary_index() -> usize {
    let list = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.clone(),
        _ => return 0,
    };
    let wl = scrap::wayland::display::get_displays();
    if let Some(pw) = wl.displays.get(wl.primary) {
        let pn = normalize_connector(&pw.name);
        if let Some(idx) = list.iter().position(|d| normalize_connector(&d.name) == pn) {
            return idx;
        }
    }
    0
}

/// The DRM enumeration reports every monitor at physical size and origin (0,0) — it deliberately
/// does not know the compositor's logical desktop layout. On a multi-monitor host that leaves the
/// client stacking all displays at (0,0), and input/cursor coordinates (mapped through each
/// display's logical origin + scale) land on the wrong output. So we augment here from the Wayland
/// outputs — the same source the uinput desktop-rect uses — matching by connector name (normalized:
/// DRM "HDMI-A-1" vs compositor "HDMI-1") and falling back to a unique physical resolution. This is
/// the "server augments the DRM geometry with the Wayland logical geometry" step. A single display
/// (already at 0,0, scale 1.0) needs no augmentation, matching the PipeWire path's logical-scale gate.
fn augment_with_wayland_geometry(drm: &[DrmDisplayInfo]) -> Vec<DisplayInfo> {
    let wl = scrap::wayland::display::get_displays();
    let mut infos: Vec<DisplayInfo> = drm.iter().map(display_info_from_drm).collect();
    if drm.len() < 2 || wl.displays.len() < 2 {
        return infos;
    }
    let matched = assign_wayland_outputs(drm, &wl.displays);
    for (i, info) in infos.iter_mut().enumerate() {
        let Some(w) = matched[i].map(|j| &wl.displays[j]) else {
            continue;
        };
        info.x = w.x;
        info.y = w.y;
        if let Some((lw, lh)) = w.logical_size {
            if lw > 0 && lh > 0 {
                info.scale = drm[i].width as f64 / lw as f64;
                // original_resolution is the logical size (physical / scale).
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

/// Which compositor output each DRM connector corresponds to, as an index into `wl` (or `None` when
/// there is nothing left to give it). Pure, so the assignment is unit-testable without a compositor.
///
/// Each output goes to at most one connector, which the per-display rules alone did not guarantee:
/// the unique-resolution rule could hand the same output to two connectors of that resolution.
///
/// Connectors that match nothing take the next free output in layout order, with a warning. Leaving
/// them unaugmented is not the safe choice it looks like: DRM reports every connector at origin
/// (0,0), so two monitors of the same model and resolution whose names do not normalize to the
/// compositor's would both keep (0,0), the client stacks them, and injected coordinates land on the
/// wrong monitor with certainty. Positional order is at worst a swap of two identically-sized
/// rectangles, and the layout stays coherent either way. A free output of the same physical size is
/// preferred, so a mixed layout does not pair a connector with an output it cannot be.
fn assign_wayland_outputs(
    drm: &[DrmDisplayInfo],
    wl: &[hbb_common::platform::linux::WaylandDisplayInfo],
) -> Vec<Option<usize>> {
    let mut taken = vec![false; wl.len()];
    let mut matched: Vec<Option<usize>> = vec![None; drm.len()];
    for (i, d) in drm.iter().enumerate() {
        if let Some(j) = match_wayland_display(d, wl, &taken) {
            matched[i] = Some(j);
            taken[j] = true;
        }
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

/// Index of the compositor output for a DRM display: by normalized connector name first, then by a
/// uniquely-matching physical resolution. `taken` outputs are skipped so one output cannot be
/// claimed twice.
fn match_wayland_display(
    d: &DrmDisplayInfo,
    wl: &[hbb_common::platform::linux::WaylandDisplayInfo],
    taken: &[bool],
) -> Option<usize> {
    let dn = normalize_connector(&d.name);
    if let Some((j, _)) = wl
        .iter()
        .enumerate()
        .find(|(j, w)| !taken[*j] && normalize_connector(&w.name) == dn)
    {
        return Some(j);
    }
    let same_res: Vec<usize> = wl
        .iter()
        .enumerate()
        .filter(|(j, w)| !taken[*j] && w.width == d.width as i32 && w.height == d.height as i32)
        .map(|(j, _)| j)
        .collect();
    if same_res.len() == 1 {
        return Some(same_res[0]);
    }
    None
}

/// Normalize a connector name for cross-source matching: DRM inserts a single-letter type
/// discriminator that the compositor drops ("HDMI-A-1" -> "HDMI-1", "DVI-D-1" -> "DVI-1"); names
/// like "DP-1" / "eDP-1" pass through unchanged.
///
/// The middle component is only folded when it is a single *letter* (a type discriminator: the "A"
/// in HDMI-A, the "D" in DVI-D). A single *digit* middle component is NOT a discriminator but a
/// DisplayPort MST port index: "DP-1-2" is sink 2 downstream of DP connector 1 and is a DISTINCT
/// output from "DP-2". Folding it (the old `parts[1].len() == 1` guard did) aliased the MST sink onto
/// a real "DP-2", so primary selection and geometry augmentation attached the wrong logical position
/// and scale. The `is_ascii_alphabetic` predicate preserves "DP-1-2" verbatim while still folding the
/// letter discriminators.
fn normalize_connector(name: &str) -> String {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() == 3 && parts[1].len() == 1 && parts[1].chars().all(|c| c.is_ascii_alphabetic()) {
        format!("{}-{}", parts[0], parts[2])
    } else {
        name.to_string()
    }
}

/// Swap the sticky positive availability cache to a freshly-enumerated display list, driven by a
/// service-pushed `DrmDisplaysChanged` hotplug signal on a live stream. This is the off-hot-path cache
/// refresh that keeps mid-session hotplug geometry fresh WITHOUT the blocking `_drm` re-probe that
/// `wayland::clear()` deliberately avoids (that re-probe blocks the async enumeration executor long
/// enough to trip "deadline has elapsed" and spiral into a restart loop). It only replaces an already
/// `Available` verdict — never flips `Unknown`/`Unavailable` to `Available` — so a stray signal cannot
/// force DRM on; establishing availability stays the job of the probe path.
fn swap_available_displays(list: Vec<DrmDisplayInfo>) {
    let mut st = DRM_STATE.lock().unwrap();
    if matches!(&*st, ProbeState::Available(..)) {
        if list.is_empty() {
            // The last active CRTC disappeared (all monitors unplugged). Do NOT keep an
            // Available-but-empty verdict advertising displays that are gone; drop to Unavailable
            // so consumers stop reporting them and can fall back. The probe path re-establishes
            // Available if a monitor comes back.
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

/// Build a `CapturerInfo` backed by a DRM-IPC capturer for `display_idx`, refreshing the cached
/// display list from the capturer's handshake so mid-capture enumeration uses fresh geometry.
pub(super) fn get_capturer_info(
    display_idx: usize,
) -> ResultType<super::video_service::CapturerInfo> {
    // Identity of the display being asked for, resolved ONCE and before any of the per-display maps
    // are locked: connector_key_of takes DRM_STATE, and nesting that inside a map lock would be the
    // one lock order this file does not otherwise have.
    // `None` when the display list does not describe this index (not enumerated yet, or out of
    // range). Kept as an Option rather than collapsed to "": an empty key is a REAL key in the map,
    // so two unidentifiable displays would share one entry and one could demote the other. That is
    // the aliasing frame() already refuses to take part in, and both blocks below skip on None for
    // the same reason. A display with no identity simply carries no health.
    let key = connector_key_of(display_idx as i32);
    // Refuse a display already demoted (repeated zero-frame sessions, or a detected flap below), so
    // the video service uses PipeWire for it instead of rebuilding onto DRM forever. Per-display, not
    // a global DRM disable.
    {
        // Refuse a demoted display UNLESS its demotion has aged past the cooldown for its demote
        // count, in which case clear the failure streak so the display retries DRM (recoverable).
        // The demote count itself is KEPT, so a display that fails again waits twice as long; only a
        // delivered frame erases it (frame() drops the entry outright).
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
            }
        }
    }
    // Build the capturer FIRST. A transient `_drm` outage (e.g. the root --service restarting) makes
    // this fail, and such a failure must NOT count toward the flap threshold — it self-heals once the
    // service returns. Only a SUCCESSFUL (re)build reaches the rapid-rebuild guard below.
    let (capturer, displays) = IpcDrmCapturer::new(display_idx as i32)?;
    // Rapid-rebuild guard (defense-in-depth): a display whose capturer is successfully rebuilt many
    // times in a short window is flapping (delivering a first frame then failing downstream every
    // cycle, which the got_frame streak alone cannot catch). Count the cadence of successful builds
    // and, past the threshold, demote it to PipeWire. A build spaced further apart than the window
    // resets the count, so a healthy display (built once, streams long) never accumulates. The
    // initial build counts 0, so demotion fires on the RAPID_REBUILD_MAX-th rapid rebuild — i.e.
    // the (RAPID_REBUILD_MAX + 1)-th build inside the window.
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
            // Demote through the same gate, and count the cycle so a display that flaps again waits
            // longer (the entry may already carry demotions from the zero-frame path).
            h.zero_frame_streak = DRM_GRAB_MAX_FAILURES;
            h.since = now;
            h.demotes += 1;
            bail!("drm capture for display {display_idx} is flapping; using PipeWire");
        }
    }
    let ndisplay = displays.len();
    let d = displays
        .get(display_idx)
        .ok_or_else(|| anyhow!("drm display index {display_idx} out of range ({ndisplay})"))?
        .clone();
    // Publish the compositor's LOGICAL origin (the same augmentation get_display_infos advertises)
    // so the video service's origin matches the reported display geometry on multi-monitor / scaled
    // layouts; keep the raw physical dimensions for the capture buffer.
    let origin = augment_with_wayland_geometry(&displays)
        .get(display_idx)
        .map(|di| (di.x, di.y))
        .unwrap_or((d.x, d.y));
    publish_probe_state(&mut DRM_STATE.lock().unwrap(), ProbeState::Available(Instant::now(), displays));
    Ok(super::video_service::CapturerInfo {
        origin,
        width: d.width as usize,
        height: d.height as usize,
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

    // Build a capturer with no live stream behind it: the receive thread only ever writes into
    // `shared`, so a test can put a frame there directly and drive `frame()` exactly as the encoder
    // loop does.
    fn capturer_with(session: Option<(usize, usize)>) -> IpcDrmCapturer {
        IpcDrmCapturer {
            shared: Arc::new(Shared {
                slot: Mutex::new(FrameSlot {
                    latest: None,
                    free: None,
                    ended: None,
                }),
                cv: Condvar::new(),
            }),
            stop: Arc::new(AtomicBool::new(false)),
            display: 0,
            connector: None,
            session_size: session,
            cur: Vec::new(),
            cur_w: 0,
            cur_h: 0,
            cur_fmt: Pixfmt::BGRA,
            got_frame: false,
        }
    }

    // Publishes exactly the way the receive path does, so the recycling is exercised too: take a
    // free buffer if one is on offer, fill it, publish it.
    fn put_frame(c: &IpcDrmCapturer, w: usize, h: usize) {
        let mut buf = c.shared.slot.lock().unwrap().free.take().unwrap_or_default();
        buf.clear();
        buf.resize(w * h * 4, 0);
        let mut slot = c.shared.slot.lock().unwrap();
        slot.publish(w, h, Pixfmt::BGRA, buf);
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

    // The regression this guards: the encoder is sized once from CapturerInfo, and convert_to_yuv
    // only refuses a source LARGER than its destination, so without this a smaller frame is encoded
    // into the old canvas and the stale edges stay on screen for the rest of the connection.
    #[test]
    fn a_smaller_frame_ends_the_session_instead_of_being_encoded() {
        let mut c = capturer_with(Some((1920, 1080)));
        put_frame(&c, 1280, 720);
        // `Frame` is not Debug, so match rather than expect_err.
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a mid-session shrink must be a hard error, not a delivered frame"),
        };
        assert!(err.to_string().contains("changed geometry mid-session"));
        // Sessions that delivered frames must not be counted toward the zero-frame demotion streak.
        assert!(c.got_frame, "the rebuild must not look like a display that never produced a frame");
    }

    #[test]
    fn a_larger_frame_ends_the_session_too() {
        let mut c = capturer_with(Some((1280, 720)));
        put_frame(&c, 1920, 1080);
        assert!(matches!(c.frame(Duration::from_millis(50)), Err(_)));
    }

    // An index the handshake list did not describe leaves the size unknown; the guard must then stay
    // out of the way rather than reject every frame.
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
        }
    }

    // A scanout is megabytes, so the buffers must circulate rather than be allocated per frame:
    // whatever a new frame displaces goes back on offer, both when the encoder consumes one and
    // when a frame is superseded before anyone reads it.
    #[test]
    fn frame_buffers_circulate_instead_of_being_reallocated() {
        let mut c = capturer_with(Some((64, 32)));
        put_frame(&c, 64, 32);
        // Superseded before anyone consumed it: its buffer must come back on offer.
        put_frame(&c, 64, 32);
        let recycled = c
            .shared
            .slot
            .lock()
            .unwrap()
            .free
            .as_ref()
            .map(|b| b.as_ptr());
        assert!(
            recycled.is_some(),
            "a superseded frame must be handed back, not dropped"
        );
        // ...and the next frame must be filled into exactly that allocation.
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
            c.shared.slot.lock().unwrap().free.is_some(),
            "the buffer the encoder finished with must be handed back to the receive path"
        );
    }

    #[test]
    fn outputs_are_matched_by_name_across_the_drm_naming_difference() {
        let drm = [drm_display("HDMI-A-1", 1920, 1080), drm_display("DP-1", 2560, 1440)];
        // Deliberately in the other order, and the second entry is the one that matches by name.
        let wl = [wl_display("DP-1", 1920, 0, 2560, 1440), wl_display("HDMI-1", 0, 0, 1920, 1080)];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(1), Some(0)]);
    }

    // The M10 case: two monitors of the same model and resolution whose names do not normalize to
    // the compositor's. Both used to end up unmatched, keeping the DRM origin (0,0), which stacks
    // them on the client. Every connector must now get a distinct output.
    #[test]
    fn identical_monitors_that_match_no_name_take_layout_order() {
        let drm = [drm_display("DP-1", 1920, 1080), drm_display("DP-2", 1920, 1080)];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("Unknown-2", 1920, 0, 1920, 1080),
        ];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(0), Some(1)]);
    }

    // The same aliasing, one step earlier: the unique-resolution rule handed ONE output to both
    // connectors of that resolution, so two displays claimed the same origin.
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

    // A name match must still win over a same-size output that comes earlier in the layout.
    #[test]
    fn a_name_match_beats_the_positional_fallback() {
        let drm = [drm_display("DP-1", 1920, 1080), drm_display("HDMI-A-1", 1920, 1080)];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("HDMI-1", 1920, 0, 1920, 1080),
        ];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(0), Some(1)]);
    }

    // More connectors than outputs: the extras stay unaugmented rather than sharing one.
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
    fn health_reports_demoted_only_while_the_cooldown_runs() {
        let mut h = DisplayHealth::new();
        assert!(!h.demoted(), "a fresh display is not demoted");
        h.zero_frame_streak = DRM_GRAB_MAX_FAILURES - 1;
        assert!(!h.demoted(), "one session short of the threshold is not demoted");
        h.zero_frame_streak = DRM_GRAB_MAX_FAILURES;
        h.demotes = 1;
        assert!(h.demoted(), "at the threshold, inside the cooldown");
        // Age it past the cooldown for its demote count: the display retries DRM.
        h.since = Instant::now() - demote_cooldown(h.demotes) - Duration::from_secs(1);
        assert!(!h.demoted(), "past the cooldown the display must be retried");
        // ...but at a higher demote count the same age is still inside the (doubled) cooldown.
        h.demotes = 4;
        assert!(h.demoted(), "the backoff must still be holding it at demotion 4");
    }

    #[test]
    fn demote_cooldown_doubles_per_cycle_and_caps() {
        // First demotion keeps the historical 30 s, so a transient failure still recovers quickly.
        assert_eq!(demote_cooldown(1), DEMOTE_COOLDOWN);
        assert_eq!(demote_cooldown(2), DEMOTE_COOLDOWN * 2);
        assert_eq!(demote_cooldown(3), DEMOTE_COOLDOWN * 4);
        let cap = DEMOTE_COOLDOWN * (1 << DEMOTE_BACKOFF_MAX_SHIFT);
        assert_eq!(demote_cooldown(1 + DEMOTE_BACKOFF_MAX_SHIFT), cap);
        // Never grows past the cap, and never overflows the shift however many cycles are recorded.
        assert_eq!(demote_cooldown(50), cap);
        assert_eq!(demote_cooldown(u32::MAX), cap);
        // A zero (no demotion recorded yet) must not underflow into the cap.
        assert_eq!(demote_cooldown(0), DEMOTE_COOLDOWN);
    }

    // The reported symptom: a display that can never be grabbed churned PeerInfo about every 35 s
    // forever (30 s cooldown plus the ~5 s it takes to burn four sessions). After a few cycles the
    // retry interval must be minutes, not seconds.
    #[test]
    fn a_permanently_ungrabbable_display_stops_churning() {
        let burn = Duration::from_secs(5); // four failed sessions
        assert!(demote_cooldown(1) + burn < Duration::from_secs(40));
        assert!(demote_cooldown(5) + burn > Duration::from_secs(8 * 60));
    }
}
