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
use hbb_common::{anyhow::anyhow, log, message_proto::DisplayInfo, tokio, ResultType};
use scrap::drm_render::RenderConverter;
use scrap::drmtap_dl::drmtap_dmabuf_desc;
use scrap::{Frame, Pixfmt, PixelBuffer, TraitCapturer};
use std::collections::BTreeMap;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

// Upper bound on how long `new()` waits for the service to answer with the display list before
// giving up and letting the caller fall back.
const HANDSHAKE_TIMEOUT_MS: u64 = 3000;

struct FrameSlot {
    // (width, height, pixel format, packed pixels) of the newest frame not yet consumed by
    // `frame()`; latest-wins. The pixel format is carried per frame because the split convert path
    // reads it from the actual convert output (XRGB8888 -> BGRA, XBGR8888 -> RGBA) rather than
    // assuming BGRA; the CPU-fallback path stores BGRA. The row stride is recoverable from
    // `pixels.len() / height` (the convert output may carry a padded stride).
    latest: Option<(usize, usize, Pixfmt, Vec<u8>)>,
    // Set once the stream ends so `frame()` returns a hard error (triggers a capturer rebuild).
    ended: Option<String>,
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

// Consecutive DRM capture sessions, keyed BY requested display index, that ended without ever
// producing a frame. A display whose scanout can never be grabbed (e.g. an unsupported format on its
// CRTC) enumerates fine but never streams, so the video service would keep rebuilding it onto DRM.
// Tracking this per display — not globally — stops a working monitor from masking a permanently
// failing one: after DRM_GRAB_MAX_FAILURES consecutive zero-frame sessions for a given display,
// get_capturer_info() refuses it so the video service falls back to PipeWire for THAT display; any
// session that produces a frame clears that display's entry.
static DRM_DISPLAY_FAILURES: Mutex<BTreeMap<i32, (u32, Instant)>> = Mutex::new(BTreeMap::new());
const DRM_GRAB_MAX_FAILURES: u32 = 4;
// A demotion is recoverable: after this cooldown the display retries DRM. The map is keyed by display
// index (stable within a session); the cooldown also releases a demotion that a hotplug/modeset may
// have pinned to an index a different monitor later occupies, so a stale verdict cannot stick forever.
const DEMOTE_COOLDOWN: Duration = Duration::from_secs(30);

// Rapid-rebuild guard (defense-in-depth against a capturer flap). The zero-frame streak above does
// not catch a display that keeps delivering a first frame and then failing downstream (e.g. a
// frame the encoder rejects), because got_frame clears the streak each session — so such a display
// would rebuild ~once per second forever. Track per-display rebuild cadence: after
// RAPID_REBUILD_MAX rebuilds all within RAPID_REBUILD_WINDOW of each other, demote it to PipeWire
// via the same failure gate. A capturer that streams longer than the window resets the count, so a
// healthy display is never demoted.
static DRM_DISPLAY_REBUILDS: Mutex<BTreeMap<i32, (Instant, u32)>> = Mutex::new(BTreeMap::new());
const RAPID_REBUILD_WINDOW: Duration = Duration::from_secs(3);
const RAPID_REBUILD_MAX: u32 = 6;

impl IpcDrmCapturer {
    /// Connect to the service `_drm` channel, complete the handshake (receive the display list, then
    /// request `display`), and start streaming on a background thread. Returns the capturer plus the
    /// enumerated displays so the caller can populate `display_service`. `Err` if the service has no
    /// DRM capture available or the handshake fails — the caller then falls back to PipeWire/portal.
    pub fn new(display: i32) -> ResultType<(IpcDrmCapturer, Vec<DrmDisplayInfo>)> {
        let shared = Arc::new(Shared {
            slot: Mutex::new(FrameSlot {
                latest: None,
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
        let displays = match rx.recv_timeout(Duration::from_millis(HANDSHAKE_TIMEOUT_MS + 500)) {
            Ok(res) => res?,
            Err(_) => {
                // The recv thread still has its own connect/handshake budget. If we just returned,
                // a handshake that completes after our timeout would leave that thread streaming
                // with no owning capturer (our Drop never runs — the capturer was never built), so
                // signal it to stop before giving up.
                stop.store(true, Ordering::SeqCst);
                return Err(anyhow!("drm capture handshake timed out"));
            }
        };
        Ok((
            IpcDrmCapturer {
                shared,
                stop,
                display,
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
                self.cur = buf;
                self.cur_w = w;
                self.cur_h = h;
                self.cur_fmt = fmt;
                if !self.got_frame {
                    // First frame of this session: DRM capture works for this display, clear its
                    // failure streak.
                    self.got_frame = true;
                    DRM_DISPLAY_FAILURES.lock().unwrap().remove(&self.display);
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
                    let mut map = DRM_DISPLAY_FAILURES.lock().unwrap();
                    let e = map.entry(self.display).or_insert((0, Instant::now()));
                    e.0 += 1;
                    e.1 = Instant::now();
                    if e.0 >= DRM_GRAB_MAX_FAILURES {
                        log::warn!(
                            "drm: display {} produced no frame in {} sessions; falling back to PipeWire for it",
                            self.display,
                            e.0
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
    // Handshake: connect, receive the display list, request the display.
    let mut conn = match connect_drm(1000).await {
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
    // Open the unprivileged render-node convert context ONCE, on THIS thread, BEFORE the handshake; it
    // is dropped on this same thread when the loop exits (its EGL state + import-once cache are
    // thread-local). `None` means no usable render node (a locked-down seat, or an old `.so` without
    // the split symbols): we then ask the service for the CPU-converted `DrmFrame` path via
    // `need_cpu`, so a render-node-less seat still captures instead of the service streaming a dma-buf
    // fd we cannot detile (which would lose the stream and force a PipeWire fallback nobody may be
    // present to approve on an unattended seat).
    let mut converter = RenderConverter::open_render();
    let need_cpu = converter.is_none();
    if need_cpu {
        log::info!(
            "drm: no render-node convert context (drmtap_open_render failed or old .so); \
             requesting the CPU-converted frame path for this stream"
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
                    num_planes: desc.num_planes,
                    offsets: desc.offsets,
                    pitches: desc.pitches,
                    hdr_eotf: desc.hdr_eotf,
                    hdr_max_nits: desc.hdr_max_nits,
                };
                match conv.convert(&mut ddesc, received_fd) {
                    Ok((data, w, h, fmt)) => {
                        let mut slot = shared.slot.lock().unwrap();
                        slot.latest = Some((w as usize, h as usize, fmt, data.to_vec()));
                        shared.cv.notify_one();
                    }
                    // Transient convert contention: skip this frame (latest-wins keeps the newest),
                    // do not tear the stream down.
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => break format!("convert: {err}"),
                }
                // `recv_fd` (the OwnedFd, if any) is dropped/closed at the end of this iteration, AFTER
                // convert has imported it (the EGLImage import holds its own reference to the buffer).
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
                match conn.next_raw().await {
                    Ok(raw) => {
                        if raw.len() < need {
                            break format!(
                                "cpu frame: body {} bytes < {need} for {width}x{height}",
                                raw.len()
                            );
                        }
                        let mut slot = shared.slot.lock().unwrap();
                        slot.latest =
                            Some((width as usize, height as usize, Pixfmt::BGRA, raw.to_vec()));
                        shared.cv.notify_one();
                    }
                    Err(err) => break format!("frame body: {err}"),
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
                match conn.next_raw().await {
                    Ok(raw) => {
                        if raw.len() < need {
                            break format!(
                                "cursor body {} bytes < {need} for {width}x{height}",
                                raw.len()
                            );
                        }
                        set_drm_cursor(
                            display,
                            DrmCursorData {
                                id,
                                width: width as i32,
                                height: height as i32,
                                hotx,
                                hoty,
                                colors: raw.to_vec(),
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
                if !list.is_empty() {
                    swap_available_displays(list);
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
    // other still-active streams.
    remove_drm_cursor(display);
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

static DRM_CURSOR: Mutex<BTreeMap<i32, DrmCursorData>> = Mutex::new(BTreeMap::new());

fn set_drm_cursor(display: i32, c: DrmCursorData) {
    DRM_CURSOR.lock().unwrap().insert(display, c);
}

fn remove_drm_cursor(display: i32) {
    DRM_CURSOR.lock().unwrap().remove(&display);
}

// Pick the cursor to present: prefer the visible one (the pointer is over exactly one captured CRTC
// at a time), else fall back to any (hidden) entry so the client still gets the hidden sentinel when
// the pointer is off every captured monitor. `None` only when no stream is active.
fn pick_drm_cursor() -> Option<DrmCursorData> {
    let map = DRM_CURSOR.lock().unwrap();
    map.values()
        .find(|c| c.id != scrap::drm_reader::HIDDEN_CURSOR_ID)
        .or_else(|| map.values().next())
        .cloned()
}

/// The id of the current DRM hardware cursor (None if no stream). The cursor service polls this to
/// detect shape changes (a change triggers a `get_cursor_data` fetch).
pub fn drm_cursor_id() -> Option<u64> {
    pick_drm_cursor().map(|c| c.id)
}

/// The current DRM hardware-cursor snapshot (RGBA), or None.
pub fn drm_cursor() -> Option<DrmCursorData> {
    pick_drm_cursor()
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
    Available(Vec<DrmDisplayInfo>),
}

static DRM_STATE: Mutex<ProbeState> = Mutex::new(ProbeState::Unknown);
// How long a negative availability verdict is trusted before is_available re-probes.
const NEGATIVE_TTL: Duration = Duration::from_secs(30);

/// Query the service for the current DRM display list without starting a stream: connect, read the
/// list the service sends on connect, then drop the connection (the service closes it when we do
/// not send `DrmStart`). Runs the async work on a throwaway thread so it is safe to call from any
/// context (a nested `#[tokio::main]` would panic when called from inside a runtime).
fn query_displays() -> ResultType<Vec<DrmDisplayInfo>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(query_displays_async());
    });
    rx.recv_timeout(Duration::from_millis(HANDSHAKE_TIMEOUT_MS + 1000))
        .map_err(|_| anyhow!("drm display query timed out"))?
}

#[tokio::main(flavor = "current_thread")]
async fn query_displays_async() -> ResultType<Vec<DrmDisplayInfo>> {
    let mut conn = connect_drm(1000).await?;
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
// Single-flight guard: exactly one caller runs the blocking availability probe at a time, so
// is_available() never calls query_displays() (up to ~4s of IPC) while holding DRM_STATE.
static DRM_PROBE_IN_FLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the root service offers DRM/KMS capture. The positive result and a definitive negative
/// (connected, but no displays) are cached; a transient probe error stays `Unknown` for a few
/// retries. Normally the cache is warmed at `--server` startup (`warm_availability`), so the first
/// client connection hits the fast `Available` path.
pub(super) fn is_available() -> bool {
    // Fast path under the lock: read the cached verdict, expiring a stale negative so a host that had
    // no displays at probe time can still enable DRM once displays appear (without a --server
    // restart). NEVER call the blocking probe while holding DRM_STATE: a cold or expired probe would
    // otherwise serialize every async caller for the whole query_displays() timeout (~4s).
    {
        let mut st = DRM_STATE.lock().unwrap();
        if let ProbeState::Unavailable(since) = &*st {
            if since.elapsed() >= NEGATIVE_TTL {
                *st = ProbeState::Unknown;
                DRM_PROBE_FAILURES.store(0, Ordering::Relaxed);
            }
        }
        match &*st {
            ProbeState::Available(_) => return true,
            ProbeState::Unavailable(_) => return false,
            ProbeState::Unknown => {} // fall through and probe with the lock released
        }
    }
    // Single-flight: exactly one caller probes at a time. While a probe is in flight, others return
    // the current cache-only verdict instead of stacking redundant `_drm` probes or blocking on the
    // mutex across the I/O. warm_availability normally seeds `Available` before clients connect, so
    // this cold path is rare.
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return matches!(&*DRM_STATE.lock().unwrap(), ProbeState::Available(_));
    }
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
            *st = ProbeState::Available(list);
            true
        }
        Ok(_) => {
            log::info!("drm: availability probe -> no displays in {:?}", t.elapsed());
            *st = ProbeState::Unavailable(Instant::now());
            false
        }
        Err(err) => {
            let n = DRM_PROBE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
            if n >= DRM_PROBE_MAX_FAILURES {
                log::info!("drm: availability probe failed {n}x ({err}); disabling DRM");
                *st = ProbeState::Unavailable(Instant::now());
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
    DRM_PROBE_IN_FLIGHT.store(false, Ordering::Release);
    available
}

/// Warm the availability cache at `--server` startup so the first client connection does not race a
/// cold `_drm` probe. A cold probe blocks display enumeration, and if it has not settled when the
/// peer info is built the display list goes out empty and the client shows "No displays" and
/// retries (the "connects on the Nth try" symptom). Probes with a short retry budget and only caches
/// the positive result; a genuinely DRM-less host just falls through to the lazy `is_available()`.
pub(super) fn warm_availability() {
    for _ in 0..10 {
        if matches!(&*DRM_STATE.lock().unwrap(), ProbeState::Available(_)) {
            return;
        }
        match query_displays() {
            Ok(list) if !list.is_empty() => {
                log::info!("drm: consumer cache warmed ({} displays) at startup", list.len());
                *DRM_STATE.lock().unwrap() = ProbeState::Available(list);
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
        ProbeState::Available(list) => list.clone(),
        _ => return None,
    };
    Some(augment_with_wayland_geometry(&list))
}

/// Index (into the cached DRM display list) of the compositor's PRIMARY output. DRM connector order
/// is not the compositor's primary, so match the compositor's primary (from the same Wayland source
/// the geometry augmentation uses) to the DRM list by normalized connector name; fall back to 0 when
/// unknown. Without this the first DRM connector is always streamed, which is the wrong initial
/// display whenever the primary is not connector 0.
pub(super) fn get_primary_index() -> usize {
    let list = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(list) => list.clone(),
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
    let multi = drm.len() > 1 && wl.displays.len() > 1;
    drm.iter()
        .map(|d| {
            let mut info = display_info_from_drm(d);
            if multi {
                if let Some(w) = match_wayland_display(d, &wl.displays) {
                    info.x = w.x;
                    info.y = w.y;
                    if let Some((lw, lh)) = w.logical_size {
                        if lw > 0 && lh > 0 {
                            info.scale = d.width as f64 / lw as f64;
                            // original_resolution is the logical size (physical / scale).
                            info.original_resolution = super::display_service::get_original_resolution(
                                &d.name,
                                lw as usize,
                                lh as usize,
                            );
                        }
                    }
                }
            }
            info
        })
        .collect()
}

/// Match a DRM display to its compositor output: by normalized connector name first, then by a
/// uniquely-matching physical resolution.
fn match_wayland_display<'a>(
    d: &DrmDisplayInfo,
    wl: &'a [hbb_common::platform::linux::WaylandDisplayInfo],
) -> Option<&'a hbb_common::platform::linux::WaylandDisplayInfo> {
    let dn = normalize_connector(&d.name);
    if let Some(w) = wl.iter().find(|w| normalize_connector(&w.name) == dn) {
        return Some(w);
    }
    let same_res: Vec<_> = wl
        .iter()
        .filter(|w| w.width == d.width as i32 && w.height == d.height as i32)
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

/// Reset the probe cache so the next session re-probes (called on capture teardown).
pub(super) fn clear() {
    *DRM_STATE.lock().unwrap() = ProbeState::Unknown;
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
    if matches!(&*st, ProbeState::Available(_)) {
        log::info!("drm: hotplug refresh -> {} display(s)", list.len());
        *st = ProbeState::Available(list);
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
    // Refuse a display already demoted (repeated zero-frame sessions, or a detected flap below), so
    // the video service uses PipeWire for it instead of rebuilding onto DRM forever. Per-display, not
    // a global DRM disable.
    {
        // Refuse a demoted display UNLESS its demotion has aged past DEMOTE_COOLDOWN, in which case
        // drop it so the display retries DRM (recoverable, and releases a stale index-pinned verdict).
        let mut map = DRM_DISPLAY_FAILURES.lock().unwrap();
        if let Some((count, since)) = map.get(&(display_idx as i32)).copied() {
            if count >= DRM_GRAB_MAX_FAILURES {
                if since.elapsed() >= DEMOTE_COOLDOWN {
                    map.remove(&(display_idx as i32));
                } else {
                    return Err(anyhow!(
                        "drm capture for display {display_idx} repeatedly produced no frame; using PipeWire"
                    ));
                }
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
    {
        let now = Instant::now();
        let mut rebuilds = DRM_DISPLAY_REBUILDS.lock().unwrap();
        let count = match rebuilds.get(&(display_idx as i32)) {
            Some((last, c)) if now.duration_since(*last) < RAPID_REBUILD_WINDOW => c + 1,
            _ => 0,
        };
        rebuilds.insert(display_idx as i32, (now, count));
        if count >= RAPID_REBUILD_MAX {
            log::warn!(
                "drm: display {display_idx} rebuilt {count} times within {RAPID_REBUILD_WINDOW:?}; flapping, falling back to PipeWire"
            );
            DRM_DISPLAY_FAILURES
                .lock()
                .unwrap()
                .insert(display_idx as i32, (DRM_GRAB_MAX_FAILURES, Instant::now()));
            return Err(anyhow!(
                "drm capture for display {display_idx} is flapping; using PipeWire"
            ));
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
    *DRM_STATE.lock().unwrap() = ProbeState::Available(displays);
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
