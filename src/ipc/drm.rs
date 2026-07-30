// The DRM/KMS capture half of the `_drm` IPC channel: the types that cross it, the producer that
// serves it from the root `--service`, and the bespoke SCM_RIGHTS framing the channel needs
// because `Framed`/`BytesCodec` cannot carry ancillary data.
//
// It sits beside `ipc/auth.rs` and `ipc/fs.rs`, declared the same `#[path]` way `ipc.rs` already
// declares those, so an opt-in feature does not keep growing the file that every unrelated IPC
// change has to be read through. What stays in `ipc.rs` is only the `Data` variants, which are
// variants of that shared enum and cannot live anywhere else.
//
// The whole module is gated at its `mod` declaration, so the per-item
// `#[cfg(all(target_os = "linux", feature = "drm"))]` attributes this code used to repeat on every
// item are gone; nothing here is compiled into a drm-off build.

use super::ipc_auth::active_uid_cached;
use super::*;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

/// One enumerated DRM display shipped over `_drm` (physical geometry). The serializable IPC
/// form of `scrap::drm_reader::DisplaySnapshot`; the server augments it with the Wayland
/// logical geometry/scale, which needs the user session.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DrmDisplayInfo {
    pub name: String,
    pub crtc_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub active: bool,
    /// Render node of the GPU that EXPORTS this display's scanout, so the
    /// unprivileged converter binds to that device instead of auto-selecting one.
    /// On a multi-GPU host auto-selection can land on a different GPU and the
    /// cross-vendor import then fails on an incompatible tiling modifier. Empty
    /// when the service cannot name it (a pre-0.4.15 libdrmtap, or a display-only
    /// device with no render node), which keeps the previous auto-select
    /// behaviour; `serde(default)` so an older peer's message still decodes.
    #[serde(default)]
    pub render_node: String,
    /// KMS card node (`/dev/dri/card*`) that drives this display, so `DrmStart`
    /// reopens the RIGHT device on a multi-GPU host. A single context enumerates
    /// only one card, so displays on the other cards need this to be captured at
    /// all; crtc_ids are card-local, so the index alone is ambiguous across cards.
    /// Empty when the service enumerated a single auto-detected device (the old
    /// behaviour, and the capture then reopens with auto-detect); `serde(default)`
    /// for wire-compat with an older peer.
    #[serde(default)]
    pub device: String,
}

/// Serializable metadata descriptor of a scanout dma-buf, shipped over `_drm` as the JSON payload of
/// `Data::DrmFrameDmabuf`. It mirrors `scrap::drm_reader::drmtap_dmabuf_desc` field-for-field EXCEPT
/// the process-local `dma_buf_fd` (which never serializes — it rides SCM_RIGHTS ancillary), and adds
/// `buffer_id` (the producer's stable pool key) and `has_fd` (whether this message's `send_msg`
/// carries the fd, vs an import-once cache hit that omits it). The converter rebuilds a
/// `drmtap_dmabuf_desc` from these fields and overwrites its `dma_buf_fd` with the received fd.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DmabufDesc {
    /// Producer-side stable pool key (e.g. fb_id + a connection epoch). Distinct from `fb_id`, which
    /// is libdrmtap's import-once cache key.
    pub buffer_id: u64,
    pub width: u32,
    pub height: u32,
    /// DRM fourcc of the scanout.
    pub format: u32,
    /// DRM format modifier (tiling/compression).
    pub modifier: u64,
    /// KMS framebuffer id — libdrmtap's import-once cache key. 0 disables caching for this frame.
    pub fb_id: u32,
    /// Used entries in `offsets`/`pitches` (1..4); 0 is treated as 1.
    pub num_planes: u32,
    /// Per-plane byte offsets into the dma-buf (CCS main + aux + clear-color).
    pub offsets: [u32; 4],
    /// Per-plane strides in bytes; `pitches[0]` is the main-surface stride.
    pub pitches: [u32; 4],
    /// DRMTAP_EOTF_* (SDR=0, PQ=2, HLG=3). PQ triggers the HDR->SDR tone-map on convert.
    pub hdr_eotf: u32,
    /// Content/mastering peak luminance (cd/m2); 0 = unknown.
    pub hdr_max_nits: u32,
    /// True: this message's `send_msg` attaches the dma-buf fd in an SCM_RIGHTS cmsg. False: an
    /// import-once cache hit for `fb_id` — no fd attached, converter reuses its cached EGLImage.
    pub has_fd: bool,
}

/// Filesystem path of the `_drm` capture socket. It lives beside the hardened `_service` socket in
/// the shared `/tmp/<app>-service` directory (cross-uid, traversable) so the root `--service` and
/// the user `--server` share one uid-independent path. Derived from the real `_service` path so we
/// inherit hbb_common's directory convention WITHOUT teaching hbb_common about a drm-specific
/// postfix (keeps the isolation clean: no shared-lib change). Both ends call this.
pub(crate) fn drm_ipc_path() -> String {
    let service_path = Config::ipc_path("_service");
    let dir = std::path::Path::new(&service_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/tmp"));
    dir.join("ipc_drm").to_string_lossy().into_owned()
}

/// Connect (from the user `--server`) to the root service's `_drm` capture channel. Uses the
/// derived `drm_ipc_path()` rather than `Config::ipc_path` since `_drm` is not a hbb_common
/// service postfix (Option 2 isolation — no shared-lib change). Returns a [`DrmConn`] (bespoke
/// SCM_RIGHTS framing) rather than the `Framed<_, BytesCodec>` `ConnectionTmpl`: the `_drm` channel
/// must carry the scanout dma-buf fd as ancillary data, which the codec cannot do (see `DrmConn`).
pub(crate) async fn connect_drm(ms_timeout: u64) -> ResultType<DrmConn> {
    use std::os::fd::AsRawFd;
    let path = drm_ipc_path();
    let stream = timeout(ms_timeout, tokio::net::UnixStream::connect(&path)).await??;
    // The producer MUST be root. DRM/KMS scanout export is a root-service capability, and the DRM
    // path outranks PipeWire (an available DRM stream suppresses the portal consent prompt), so a
    // non-root peer that won a socket-path race must not be trusted to supply the display list,
    // frames and an arbitrary dma-buf fd. The producer direction is authorized in
    // handle_drm_conn; this closes the same gap on the consumer direction.
    if peer_uid_from_fd(stream.as_raw_fd()) != Some(0) {
        bail!("drm: _drm producer is not root; refusing to consume");
    }
    Ok(DrmConn::new(stream))
}

/// Bind the `_drm` listener. Unlike `new_listener`, this does not route through hbb_common's
/// service-postfix machinery — it places the socket in the shared service dir directly, so the
/// drm-off build needs no hbb_common change. The socket is 0666 (world-connectable) so the
/// unprivileged `--server` can reach it; every accepted peer is still authorized in
/// `handle_drm_conn` (root or the active session uid + exe identity), so connectable != authorized.
fn new_drm_listener() -> ResultType<Incoming> {
    let path = drm_ipc_path();
    // Ensure the shared service dir exists at its hardened (0711) mode. Passing the `_service`
    // postfix reuses hbb_common's expected mode for that directory; it only creates/chmods the
    // directory (no pid/socket side effects) and is idempotent with the real `_service` listener.
    let _ = ensure_secure_ipc_parent_dir(&path, "_service")?;
    // Clear any stale socket from a previous run before binding.
    std::fs::remove_file(&path).ok();
    let mut endpoint = Endpoint::new(path.clone());
    endpoint.set_security_attributes(SecurityAttributes::allow_everyone_create()?);
    let incoming = endpoint.incoming()?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).map_err(|err| {
        std::fs::remove_file(&path).ok();
        err
    })?;
    log::info!("Started drm ipc server at path: {}", &path);
    Ok(incoming)
}

/// Message from a per-connection DRM worker thread (which owns the `!Send` `DrmReader`) to its
/// async socket task. The worker does the blocking device I/O; the task only forwards to the wire.
enum DrmProducerMsg {
    /// Enumerated displays, sent once before any frame so the task can answer the handshake.
    Displays(Vec<DrmDisplayInfo>),
    /// A captured frame (split/zero-copy path): the serializable dma-buf descriptor plus the (owned)
    /// scanout fd to hand to the peer via SCM_RIGHTS. The `OwnedFd` is closed once the send has dup'd
    /// it into the peer.
    Frame {
        desc: DmabufDesc,
        fd: Option<OwnedFd>,
    },
    /// A captured frame (CPU-mapped fallback path): a full packed-BGRA frame body. Used when the
    /// consumer has no render-node convert context (`need_cpu`) or the seat has no transferable
    /// dma-buf (ENOTSUP) -- both hardware/seat facts, with no alternative that keeps the stream.
    /// Forwarded as `Data::DrmFrame{width,height}` + `send_raw(BGRA)`, exactly like the pre-split
    /// protocol, so an unprivileged converter is never required.
    FrameCpu {
        width: u32,
        height: u32,
        data: Bytes,
    },
    /// A changed hardware-cursor shape + its packed RGBA pixels.
    Cursor {
        id: u64,
        width: u32,
        height: u32,
        hotx: i32,
        hoty: i32,
        colors: Vec<u8>,
    },
}

/// Sets the shared stop flag when the async task ends (any path), so the blocking worker thread
/// terminates promptly even while it is between channel sends (e.g. spinning on WouldBlock).
struct DrmStopGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for DrmStopGuard {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Build a [`DrmConn`] from an already-authorized `_drm` `Connection` (root `--service` side). The
/// parity `Connection` wraps a tokio `UnixStream` but exposes no way to move it out, so we `dup()`
/// its fd into a fresh, independently-owned tokio `UnixStream` for the bespoke SCM_RIGHTS framing.
/// A dup gives a NEW fd number, which registers as its own epoll entry in tokio's reactor (reusing
/// the same fd number would double-register); the caller drops the parity `Connection` afterwards,
/// closing ITS fd, while the dup keeps the socket alive via the shared open file description.
fn dup_to_drm_conn(stream: &Connection) -> ResultType<DrmConn> {
    let raw = stream.inner.get_ref().as_raw_fd();
    // F_DUPFD_CLOEXEC, not dup(): `dup` never copies the close-on-exec flag, so the new fd would be
    // inherited by every child this process forks. This process is the ROOT service and it does fork
    // synchronously elsewhere (the `loginctl` active-uid lookup), and this fd is an ALREADY-AUTHORIZED
    // `_drm` socket -- the one thing on the box that hands out scanout dma-bufs. Leaking it into an
    // unrelated child is a privilege leak even if no child ever reads it.
    let dup = unsafe { hbb_common::libc::fcntl(raw, hbb_common::libc::F_DUPFD_CLOEXEC, 0) };
    if dup < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: `dup` is a freshly dup'd, owned fd for a connected SOCK_STREAM unix socket.
    let std_stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(dup) };
    std_stream.set_nonblocking(true)?;
    let tokio_stream = tokio::net::UnixStream::from_std(std_stream)?;
    Ok(DrmConn::new(tokio_stream))
}

/// Cached DRM display enumeration. The pre-warm populates it and each capture open refreshes it, so
/// a consumer's handshake can send the display list without first paying a DRM enumeration open.
static DRM_DISPLAY_CACHE: std::sync::Mutex<Vec<DrmDisplayInfo>> = std::sync::Mutex::new(Vec::new());

/// Monotonic generation bumped by the udev DRM-uevent listener ONLY when a connector-topology change
/// actually altered `DRM_DISPLAY_CACHE` (a monitor hotplug/unplug/modeset). Each live `handle_drm_conn`
/// forward loop watches this (one atomic load per frame) and, on a bump, pushes a `DrmDisplaysChanged`
/// with the fresh list to its consumer — the cheap live-refresh path that avoids a consumer re-probe.
/// `Release`/`Acquire` order it after the cache write so a reader that sees the new generation also sees
/// the new cache (the cache `Mutex` re-synchronizes the contents regardless).
static DRM_DISPLAY_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Snapshot a reader's enumerated displays as the IPC `DrmDisplayInfo` form. `displays()` lists all
/// device outputs regardless of the reader's target CRTC, so a capture reader can refresh the cache.
/// Returns the displays this reader can serve, plus the identity (`device:connector`) of every
/// CONNECTED output dropped for having no CRTC. The identities are RETURNED rather than accumulated
/// in a static: several handshakes enumerate concurrently (a multi-monitor client opens one `_drm`
/// connection per display), and a shared counter meant their looks at the hardware added together --
/// observed as "2 connected display(s) had no CRTC" on a machine with exactly one. They are
/// identities rather than a count so the wake bookkeeping can reason about WHICH output stayed dark
/// (see DRM_WAKE_HOPELESS), not merely how many.
fn drm_displays_from_reader(
    reader: &mut scrap::drm_reader::DrmReader,
    device: &str,
) -> (Vec<DrmDisplayInfo>, Vec<String>) {
    // Every display this reader enumerates belongs to the reader's device, so they
    // all share its render node. Resolved once here rather than per display.
    let render_node = reader.render_node().unwrap_or_default();
    let mut undriven = Vec::new();
    let displays: Vec<DrmDisplayInfo> = reader
        .displays()
        .into_iter()
        // Only offer outputs actually bound to a CRTC (i.e. scanning out). A
        // CONNECTED-but-unbound connector (e.g. a virtual/dummy HDMI plug the
        // compositor is not driving) enumerates with `crtc_id == 0`. Such an
        // entry has no scanout to capture, yet was still shipped to the client as
        // a selectable monitor; picking it made libdrmtap's `open(crtc=0)`
        // AUTO-SELECT the first active CRTC (the primary) and stream ITS frames at
        // the wrong geometry (e.g. a 3840x2160 frame into a 1280x1024 encoder ->
        // `src rect > dst rect`), which failed every frame and drove a ~1/sec
        // capturer restart loop (the flap that leaked EGL contexts to OOM). Drop
        // these here so they are never offered; the client keeps its real monitors.
        .filter(|d| {
            if !d.active || d.crtc_id == 0 {
                // Also the signal that a display EXISTS but is not being driven, which is what an
                // idle compositor leaves behind when it disables an output. Recorded so the
                // handshake can tell "this host has no monitors" apart from "this host has a
                // monitor that is switched off", which look identical once they are filtered out.
                undriven.push(format!("{device}:{name}", name = d.name));
                return false;
            }
            true
        })
        .map(|d| DrmDisplayInfo {
            name: d.name,
            crtc_id: d.crtc_id,
            x: d.x,
            y: d.y,
            width: d.width,
            height: d.height,
            active: d.active,
            render_node: render_node.clone(),
            device: device.to_owned(),
        })
        .collect();
    (displays, undriven)
}

/// Enumerate the active displays of EVERY DRM device, so a multi-GPU host advertises
/// the monitors on all cards, not just the first one a single context settles on. Each
/// display carries its own `device` (card node) and `render_node`, so `DrmStart` reopens
/// the right card and the converter binds the right GPU. Falls back to a single
/// auto-detected device when libdrmtap cannot enumerate (a pre-0.4.15 `.so`) or found
/// nothing to open -- in which case `device` is left empty and capture reopens with
/// auto-detect, exactly the previous behaviour.
/// Every active display of every DRM device, plus the identities of the CONNECTED outputs currently
/// NOT being driven. Both come from the SAME look at the hardware, which is the point: the two were
/// once a list and a separate static, and a handshake could act on a count that belonged to somebody
/// else's enumeration.
fn drm_enumerate_all_displays() -> (Vec<DrmDisplayInfo>, Vec<String>) {
    if let Some(devices) = scrap::drm_reader::list_devices() {
        if devices.len() > 1 {
            log::info!(
                "drm: {} DRM devices: {}",
                devices.len(),
                devices
                    .iter()
                    .map(|d| format!(
                        "{} ({}, render {})",
                        d.path,
                        d.display_count,
                        if d.render_node.is_empty() { "none" } else { &d.render_node }
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        let mut all = Vec::new();
        let mut undriven_total = Vec::new();
        let mut any_opened = false;
        for dev in devices {
            // A card with no active CRTC used to be SKIPPED here, on the grounds that opening it and
            // enumerating "would add nothing". That was true while the only question was which
            // displays can be captured. It is false now: a card whose monitors are all off is exactly
            // where a display sits that could be captured if the compositor switched it back on, and
            // drm_displays_from_reader records those and RETURNS them, so the handshake can
            // wake them. Skipping the card meant they were never seen and the wake never fired, which
            // is how an Apple T2 handed a client its Touch Bar strip while the 2880x1800 panel sat
            // disabled next to it.
            //
            // Opening such a card is fine and was measured: the context opens, list_displays reports
            // the connector as `crtc=0 (inactive)`, and only a GRAB would fail with "no active CRTC".
            // It contributes zero entries to the list, exactly as before -- the only difference is
            // that we now know it is there. The cost is one device open per idle card per
            // enumeration, which happens off the capture path.
            if let Some(mut r) = scrap::drm_reader::DrmReader::open(Some(&dev.path), 0) {
                any_opened = true;
                let (mut got, mut undriven) = drm_displays_from_reader(&mut r, &dev.path);
                all.append(&mut got);
                undriven_total.append(&mut undriven);
            } else if dev.display_count == 0 {
                log::debug!(
                    "drm: {} has no active display and did not open; cannot tell whether it has a \
                     connected output that is merely switched off",
                    dev.path
                );
            }
        }
        // Answer from the per-device enumeration whenever ANY card opened -- including when the
        // active list is EMPTY. "Every connected output is idle-disabled" is exactly the state the
        // wake exists for, and it used to fall through to the auto-detect path below, which
        // enumerates the same connectors under `device = ""`. That re-keying is not cosmetic: the
        // undriven identities feed DRM_WAKE_HOPELESS, and an entry latched under the fallback key
        // (":eDP-1") could never be refuted by an enumeration that sees the panel driven, because a
        // driven panel makes this list non-empty and its identity is then "/dev/dri/cardN:eDP-1" --
        // a permanent latch on any single-GPU host whose only panel idles, i.e. the most common
        // machine there is. (The fall-through also silently threw undriven_total away.) The
        // auto-detect fallback below now runs only when NO card opened (no rights, or a
        // pre-0.4.15 .so without list_devices), regimes in which every enumeration consistently
        // uses the "" key, so identities still match each other.
        if any_opened {
            return (all, undriven_total);
        }
    }
    match scrap::drm_reader::DrmReader::open(None, 0) {
        Some(mut r) => drm_displays_from_reader(&mut r, ""),
        None => (Vec::new(), Vec::new()),
    }
}

/// Connector identities (`device:connector`) a wake was already tried on and did NOT bring back:
/// whatever is still undriven when a fired wake's recheck window closes lands here, and later
/// handshakes no longer count those connectors as a reason to wake (or to wait). A connected
/// connector the compositor will never drive -- a dummy HDMI plug, a lid-closed docked laptop's
/// eDP, a monitor the user disabled in display settings -- would otherwise invite a wake plus a
/// full recheck wait on every connection, forever, for a display that is never coming.
///
/// The set is SELF-REFUTING, which is what makes it safe where a single global latch was not: an
/// entry that a later enumeration sees DRIVEN is removed (drm_wakeable_undriven), because a lit
/// panel is direct proof the "never coming" verdict was wrong -- a modeset that outran the recheck
/// deadline, or a lid that opened. One slow wake therefore costs one stale entry until that panel
/// is next seen alight, not the whole feature for the rest of the service's life. And because the
/// latch is per-connector, a permanently dark connector cannot suppress the wake for a DIFFERENT
/// panel that idles later, which a global flag deterministically did (latched by the dock's closed
/// lid, it would have refused to wake the real monitor).
///
/// A Vec, not a HashSet, for const init; it holds at most a handful of entries.
static DRM_WAKE_HOPELESS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// The undriven connectors still worth waking: `undriven` minus the hopeless set. Also where the
/// hopeless set is REFUTED: any entry the current enumeration shows driven is removed, so the latch
/// heals itself the moment reality disproves it.
fn drm_wakeable_undriven(displays: &[DrmDisplayInfo], undriven: &[String]) -> Vec<String> {
    let mut hopeless = DRM_WAKE_HOPELESS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !hopeless.is_empty() {
        hopeless.retain(|id| {
            let driven_now = displays
                .iter()
                .any(|d| format!("{}:{}", d.device, d.name) == *id);
            if driven_now {
                log::info!("drm: {id} is scanning out after all; treating it as wakeable again");
            }
            !driven_now
        });
    }
    undriven
        .iter()
        .filter(|id| !hopeless.iter().any(|h| h == *id))
        .cloned()
        .collect()
}

/// Last time a display wake was emitted, as seconds since the service started, so a reconnect storm
/// cannot turn into an input-injection storm. 0 = never.
static DRM_LAST_WAKE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Set once /dev/uinput has been found unusable, so the diagnosis is logged once instead of per
/// connection. A host without uinput cannot inject input at all on Wayland (there is no XTEST), so a
/// failure here means the session was already view-only -- it is not a new failure mode.
static DRM_WAKE_UNAVAILABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Minimum gap between two wakes. Long enough that a client reconnecting in a loop cannot flood the
/// compositor with synthetic activity, short enough to be useless as a way to keep a screen lit.
const DRM_WAKE_MIN_GAP: std::time::Duration = std::time::Duration::from_secs(20);
/// How long to let udev bind a freshly created uinput device before writing to it. Measured, not
/// guessed: see drm_wake_displays.
const DRM_WAKE_DEVICE_SETTLE: std::time::Duration = std::time::Duration::from_millis(400);
/// How long to keep re-enumerating after a wake before giving up on the outputs coming back. A
/// modeset is asynchronous: the compositor has to see the input, decide to un-idle, and commit.
const DRM_WAKE_RECHECK_TOTAL: std::time::Duration = std::time::Duration::from_secs(3);
/// How long after a wake its outcome may still be developing: the device-bind pause, the emits, the
/// full recheck, plus slack for the enumerations in between. A handshake that was rate-limited away
/// from waking looks at this to decide whether the recent wake is still in flight (then it waits for
/// the outcome like the winner does) or is old news (then the current state IS the settled state).
const DRM_WAKE_SETTLE_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

/// Seconds since the service started, on a monotonic clock. This is the clock the wake rate limiter
/// stores in DRM_LAST_WAKE; SystemTime would let a clock step re-open the gate.
fn drm_wake_clock_secs() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs()
}

/// Ask the compositor to bring its outputs back, by looking like user activity for one pixel.
///
/// A compositor that has been idle long enough does not merely blank the panel: it DISABLES the
/// connector, i.e. commits a modeset that leaves it with no CRTC. At that point there is no scanout
/// anywhere, so there is nothing for ANY capture backend to read -- not this one, not PipeWire, not
/// X11. The image does not exist rather than being unreadable. (X11 does not have this problem for a
/// different reason: its root window is a software surface the X server keeps regardless of what the
/// physical output is doing.)
///
/// Measured on an Apple T2 MacBook whose greeter had idled: `card2-eDP-1 dpms=Off enabled=disabled`
/// and zero active displays enumerated on that card; one synthetic relative move restored
/// `dpms=On enabled=enabled` with a full 2880x1800 scanout, and capture then worked.
///
/// A relative +1/-1 round trip is deliberate: it nets ZERO displacement, so the pointer does not
/// actually move, and it needs no knowledge of the desktop rect (an absolute device would have to
/// invent a coordinate). The device is created and destroyed around the emit rather than kept alive,
/// so nothing persists in the input stack between wakes.
///
/// Two alternatives were measured and rejected. The connector `dpms` attribute in sysfs is read-only,
/// and a modeset of our own would need DRM master, which the compositor holds. A third,
/// `org.gnome.ScreenSaver.SetActive(false)`, does work and does NOT fake input -- but the session bus
/// authenticates by uid and refuses root even though the socket is world-writable, so the root
/// service would have to drop privilege to the session user, and the interface is GNOME-specific.
/// uinput is the only route that works from where this code already runs, on any desktop.
///
/// Returns true when a wake was actually emitted.
fn drm_wake_displays(reason: &str) -> bool {
    use std::sync::atomic::Ordering;

    if DRM_WAKE_UNAVAILABLE.load(Ordering::Relaxed) {
        return false;
    }
    // Rate limit on the shared monotonic clock (see drm_wake_clock_secs).
    let now = drm_wake_clock_secs();
    // CLAIM the slot before emitting, not after. A multi-monitor client opens one `_drm` connection per
    // captured display, so several handshakes run concurrently and a check-then-emit lets all of them
    // through: two wakes were observed in the SAME millisecond. compare_exchange makes exactly one
    // winner, and the losers log at debug and move on -- the winner's wake serves them all.
    loop {
        let last = DRM_LAST_WAKE.load(Ordering::Acquire);
        if last != 0 && now.saturating_sub(last) < DRM_WAKE_MIN_GAP.as_secs() {
            log::debug!(
                "drm: not waking displays ({reason}): a wake {}s ago is still recent",
                now.saturating_sub(last)
            );
            return false;
        }
        if DRM_LAST_WAKE
            .compare_exchange(last, now.max(1), Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            break;
        }
        // Another thread claimed it between our load and our exchange; re-read and let the rate-limit
        // branch above turn us away.
    }

    // It has to look like a MOUSE, not merely like something that emits a relative axis. libinput
    // classifies devices before it will treat their events as pointer activity, and a device with a
    // single relative axis and no buttons does not qualify: it is ignored outright, so the events go
    // nowhere and the compositor never un-idles. Measured three ways on the same machine in the same
    // state -- REL_X + REL_Y + BTN_LEFT woke the panel, REL_X alone did not, and neither did REL_X
    // with the settle removed. Declaring both axes and a button is the part that makes it real.
    let mut axes = evdev::AttributeSet::<evdev::RelativeAxisType>::new();
    axes.insert(evdev::RelativeAxisType::REL_X);
    axes.insert(evdev::RelativeAxisType::REL_Y);
    let mut keys = evdev::AttributeSet::<evdev::Key>::new();
    keys.insert(evdev::Key::BTN_LEFT);
    let built = evdev::uinput::VirtualDeviceBuilder::new()
        .and_then(|b| b.name("RustDesk DRM display wake").with_relative_axes(&axes))
        .and_then(|b| b.with_keys(&keys))
        .and_then(|b| b.build());
    let mut dev = match built {
        Ok(d) => d,
        Err(err) => {
            // Sticky: without /dev/uinput this can never succeed, and retrying it per connection
            // would log the same failure forever.
            DRM_WAKE_UNAVAILABLE.store(true, Ordering::Relaxed);
            log::warn!(
                "drm: cannot wake displays ({reason}): no uinput device ({err}). A compositor that \
                 disabled its outputs will keep them disabled, so there is no scanout to capture \
                 until something else generates input. Note input injection needs uinput too, so \
                 this session cannot control the host either."
            );
            return false;
        }
    };

    // A FRESH uinput device is not listening yet: udev has to notice it and the compositor's input
    // stack has to open it, and until that happens the events are written to a device nobody reads and
    // are simply lost. Measured on the same machine in the same state, back to back: with this pause
    // the panel went `disabled -> enabled`, without it `disabled -> disabled`. This is the entire
    // difference between the wake working and silently doing nothing, so it is not a "give it a
    // moment" superstition -- it is the binding window.
    std::thread::sleep(DRM_WAKE_DEVICE_SETTLE);

    // +1 then -1 on the same axis: activity without displacement. evdev's emit() appends the
    // SYN_REPORT itself, so each call is a complete packet.
    let step = |v: i32| {
        evdev::InputEvent::new(
            evdev::EventType::RELATIVE,
            evdev::RelativeAxisType::REL_X.0,
            v,
        )
    };
    let ok = dev.emit(&[step(1)]).and_then(|_| {
        std::thread::sleep(std::time::Duration::from_millis(120));
        dev.emit(&[step(-1)])
    });
    if let Err(err) = ok {
        log::warn!("drm: display wake ({reason}) failed to emit: {err}");
        return false;
    }
    log::info!("drm: no display was scanning out ({reason}); asked the compositor to wake up");
    true
}

/// One enumeration a handshake can answer with: enumerate, wake sleeping displays if that could
/// help, and WAIT for the outcome before returning. The wait is the load-bearing part, and it
/// applies to every handshake that saw a connected-but-undriven display -- not only the one whose
/// wake attempt won the rate limit. The losers used to return immediately with the pre-wake list,
/// which is exactly the intermediate state the winner was waiting out: with one `_drm` connection
/// per captured display plus the consumer's availability refresher, a wake in flight had its
/// half-done topology served to whichever consumer asked at the wrong moment, and the client ended
/// up with duplicate, misindexed monitors. Every caller of this function gets the settled truth or
/// a bounded timeout -- never the transition.
///
/// A compositor that has idled long enough DISABLES its outputs, and a disabled output has no
/// scanout for anything to read -- not this backend, not PipeWire, not X11. The trigger is "a
/// connected display is not being driven", NOT "no display at all": on a laptop with a second DRM
/// card (an Apple T2's Touch Bar strip) the list is never empty, so an emptiness check never fires
/// and the client would be handed whatever is still scanning out. libdrmtap does report the idle
/// panel (`crtc=0 (inactive)`); it is our own active-CRTC filter that drops it, so the count of
/// what was dropped is exactly the right signal.
fn drm_enumerate_settled(reason: &str) -> Vec<DrmDisplayInfo> {
    use std::sync::atomic::Ordering;

    let (displays, undriven) = drm_enumerate_all_displays();
    let wakeable = drm_wakeable_undriven(&displays, &undriven);
    if wakeable.is_empty() {
        return displays;
    }
    let fired = drm_wake_displays(&format!(
        "{reason} and {n} connected display(s) had no CRTC",
        n = wakeable.len()
    ));
    if !fired {
        if DRM_WAKE_UNAVAILABLE.load(Ordering::Relaxed) {
            // No uinput on this host: nothing will ever wake these displays, so the pre-wake list
            // is not "pre" anything -- it is the state of the world.
            return displays;
        }
        // Rate-limited: somebody woke recently. If that wake may still be developing, wait for its
        // outcome below, exactly like the winner. If it is old news (the panel re-idled inside
        // DRM_WAKE_MIN_GAP, or the winner's recheck expired long ago), what we enumerated IS the
        // settled state and waiting would just tax this handshake for nothing.
        let last = DRM_LAST_WAKE.load(Ordering::Acquire);
        if last == 0
            || drm_wake_clock_secs().saturating_sub(last) > DRM_WAKE_SETTLE_WINDOW.as_secs()
        {
            return displays;
        }
    }
    // Poll for the outcome rather than looking once at a fixed delay: the wake is asynchronous on
    // the compositor side, and a single re-enumeration is a bet on how long a modeset takes. Two
    // exits: nothing WAKEABLE is left undriven (connectors already latched hopeless do not hold
    // the answer hostage -- on a host with a dummy plug next to a real panel, the poll ends when
    // the panel lights, because the plug stopped counting after its first failed wake), or the
    // deadline (the wake failed, or the winner's did and we were waiting on it).
    let before_len = displays.len();
    let deadline = std::time::Instant::now() + DRM_WAKE_RECHECK_TOTAL;
    let mut cur = displays;
    let mut cur_wakeable = wakeable;
    while !cur_wakeable.is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let (next, next_undriven) = drm_enumerate_all_displays();
        cur_wakeable = drm_wakeable_undriven(&next, &next_undriven);
        cur = next;
    }
    if cur.len() > before_len {
        log::info!(
            "drm: {} display(s) came back after the wake ({} -> {}{})",
            cur.len() - before_len,
            before_len,
            cur.len(),
            if cur_wakeable.is_empty() {
                String::new()
            } else {
                format!(", {} still undriven", cur_wakeable.len())
            }
        );
        // Publish through the single cache writer so the topology push and the udev listener's
        // cache converge on the woken state without another consumer having to repeat the wake.
        schedule_drm_cache_refresh();
    }
    if fired && !cur_wakeable.is_empty() {
        // Whatever OUR OWN wake could not bring back inside its window is latched hopeless, so the
        // next connection neither wakes for it nor waits on it. Only the handshake that fired
        // latches (a loser timing out says nothing: its baseline was taken mid-transition), and
        // the latch is per-connector and self-refuting -- see DRM_WAKE_HOPELESS for why both of
        // those properties are load-bearing.
        let mut hopeless = DRM_WAKE_HOPELESS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for id in &cur_wakeable {
            if !hopeless.iter().any(|h| h == id) {
                hopeless.push(id.clone());
            }
        }
        log::info!(
            "drm: the wake did not bring back {list}; not asking again for {these} until {it_is} \
             seen scanning out",
            list = cur_wakeable.join(", "),
            these = if cur_wakeable.len() == 1 { "it" } else { "them" },
            it_is = if cur_wakeable.len() == 1 { "it is" } else { "they are" },
        );
    }
    cur
}

/// The SINGLE writer of DRM_DISPLAY_CACHE (+ DRM_DISPLAY_GENERATION): enumerate every card, diff
/// against the cache, and on a real change swap it and bump the generation so live consumers get the
/// push. Runs OFF the caller's thread and SINGLE-FLIGHT -- at most one enumeration at a time, and a
/// request arriving during one coalesces into exactly one follow-up. Enumerating reopens every card,
/// so it must never sit on a connection's first-frame path, and a reconnect / uevent storm must not
/// spawn unbounded threads (the reason this is not a bare `thread::spawn` per call). Because every
/// refresh path funnels through here, the cache has one writer and the pre-0.4.15 hotplug diff/bump
/// race between the udev thread and a per-connection refresh cannot happen.
fn schedule_drm_cache_refresh() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    static PENDING: AtomicBool = AtomicBool::new(false);
    // Ownership of RUNNING, released on every exit including an unwind and a failed spawn. Same
    // shape as UinputRefreshGuard in drm_capturer: the flag is deliberately handed back and
    // re-taken mid-loop, so the guard tracks whether WE still hold it -- an unconditional release
    // on drop would clear a flag a replacement worker owns. Without this, a panic anywhere in the
    // loop body outside the catch_unwind below, or `thread::spawn` itself failing (it panics on
    // EAGAIN, and that happens AFTER the swap), leaves RUNNING true for the process lifetime and
    // every later refresh -- including every udev hotplug -- returns early forever.
    struct RefreshSlot(bool);
    impl RefreshSlot {
        fn release(&mut self) {
            if self.0 {
                self.0 = false;
                RUNNING.store(false, Ordering::Release);
            }
        }
        fn retake(&mut self) -> bool {
            self.0 = !RUNNING.swap(true, Ordering::AcqRel);
            self.0
        }
    }
    impl Drop for RefreshSlot {
        fn drop(&mut self) {
            self.release();
        }
    }
    // Announce a refresh is wanted before trying to run, so an active worker is guaranteed to see it.
    PENDING.store(true, Ordering::Release);
    if RUNNING.swap(true, Ordering::AcqRel) {
        return; // a worker is already active; it will observe PENDING and refresh again
    }
    // We hold the slot from the swap above; hand it to the guard NOW, before the spawn, so a spawn
    // failure releases it too (the closure that owns it is dropped along with the error).
    let mut slot = RefreshSlot(true);
    let spawned = std::thread::Builder::new()
        .name("drm-cache-refresh".into())
        .spawn(move || loop {
            PENDING.store(false, Ordering::Release);
            // Panic-safety, two layers: enumeration panics are caught here so a flaky driver does
            // not lose the refresh; anything else that unwinds is covered by `slot`'s Drop.
            // Only the LIST matters to the cache: the undriven identities belong to the moment they
            // were taken, and caching them is exactly the desynchronisation this refactor removed.
            let fresh = std::panic::catch_unwind(drm_enumerate_all_displays)
                .unwrap_or_else(|_| {
                    log::error!("drm: display enumeration panicked; treating as no displays");
                    (Vec::new(), Vec::new())
                })
                .0;
            let changed = {
                let mut cache = match DRM_DISPLAY_CACHE.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                if *cache != fresh {
                    *cache = fresh;
                    true
                } else {
                    false
                }
            };
            if changed {
                DRM_DISPLAY_GENERATION.fetch_add(1, Ordering::Release);
                log::info!("drm: display cache refreshed (topology changed)");
            }
            // Exit only if no request arrived during this enumeration. The re-check after releasing
            // the slot closes the lost-wakeup window (a request that set PENDING just before the
            // release).
            if !PENDING.load(Ordering::Acquire) {
                slot.release();
                if !PENDING.load(Ordering::Acquire) {
                    break;
                }
                if !slot.retake() {
                    break; // another caller re-acquired the slot; it will handle the pending refresh
                }
            }
        });
    if let Err(err) = spawned {
        // The closure was dropped without running, and the guard inside it released RUNNING, so the
        // next request retries the spawn instead of being locked out forever.
        log::error!("drm: could not spawn the display-cache refresh worker: {err}");
    }
}

/// True if a kernel uevent datagram is a DRM-subsystem topology change (a connector hotplug/modeset).
/// A uevent is NUL-separated `KEY=value` records; we require `SUBSYSTEM=drm` plus a `change` action or
/// `HOTPLUG=1`, so an `add`/`remove` of an unrelated node (a render device, a fb) does not trigger a
/// re-enumeration. Byte-exact record matching avoids any allocation/UTF-8 handling on the hot recv path.
fn uevent_is_drm_change(msg: &[u8]) -> bool {
    let mut is_drm = false;
    let mut is_change = false;
    for rec in msg.split(|&b| b == 0) {
        if rec == b"SUBSYSTEM=drm" {
            is_drm = true;
        } else if rec == b"ACTION=change" || rec == b"HOTPLUG=1" {
            is_change = true;
        }
    }
    is_drm && is_change
}

/// Listen for DRM connector hotplug/modeset uevents and refresh the display cache when the topology
/// actually changes. Uses a raw `NETLINK_KOBJECT_UEVENT` socket (the same hotplug stream udev consumes)
/// so no libudev dependency is added; the root `--service` already runs privileged and joining the
/// kernel-uevent multicast group needs no extra cap. On a real change it re-enumerates (off any hot
/// path — this is a dedicated thread, so the blocking `open`/`displays` is fine), and only when the
/// enumerated set differs does it swap `DRM_DISPLAY_CACHE` and bump `DRM_DISPLAY_GENERATION`; live
/// `handle_drm_conn` loops then push the fresh list to their consumers. Best-effort: if the socket is
/// unavailable it logs and returns, and DRM capture still works (a consumer reconnect re-reads the
/// fresh list) — just without the mid-session live refresh.
fn drm_udev_listener() {
    use hbb_common::libc;

    let sock = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
            libc::NETLINK_KOBJECT_UEVENT,
        )
    };
    if sock < 0 {
        log::info!(
            "drm: udev uevent socket unavailable ({}); hotplug refresh disabled",
            std::io::Error::last_os_error()
        );
        return;
    }
    // Own the fd so it is closed on every return / unwind path.
    let _owned = unsafe { OwnedFd::from_raw_fd(sock) };
    let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    addr.nl_family = libc::AF_NETLINK as u16;
    // Group 1 = kernel-originated uevents (udev re-broadcasts on group 2); pid 0 => kernel assigns.
    addr.nl_groups = 1;
    let rc = unsafe {
        libc::bind(
            sock,
            &addr as *const libc::sockaddr_nl as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };
    if rc < 0 {
        log::info!(
            "drm: udev uevent bind failed ({}); hotplug refresh disabled",
            std::io::Error::last_os_error()
        );
        return;
    }
    log::info!("drm: udev DRM-uevent listener started");
    // Fixed-size receive buffer (preallocated model): a uevent is well under 8 KiB; a rare larger
    // datagram is truncated by `recv` and simply re-enumerates on the next matching event.
    let mut buf = [0u8; 8192];
    loop {
        // recvmsg (not recv) so the source address is available: bound to the kernel-uevent multicast
        // group, a genuine uevent comes from the kernel (source nl_pid == 0) via a multicast group
        // (nl_groups != 0). A local unprivileged process could otherwise UNICAST a spoofed
        // "change@.../drm/..." datagram to this root listener and drive it to re-enumerate at will;
        // dropping any non-kernel/non-multicast source closes that.
        let mut src: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        let mut iov = libc::iovec {
            iov_base: buf.as_mut_ptr() as *mut libc::c_void,
            iov_len: buf.len(),
        };
        let mut mhdr: libc::msghdr = unsafe { std::mem::zeroed() };
        mhdr.msg_name = &mut src as *mut libc::sockaddr_nl as *mut libc::c_void;
        mhdr.msg_namelen = std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
        mhdr.msg_iov = &mut iov;
        mhdr.msg_iovlen = 1;
        let n = unsafe { libc::recvmsg(sock, &mut mhdr, 0) };
        if n <= 0 {
            let err = std::io::Error::last_os_error();
            if n < 0 && err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            log::info!("drm: udev uevent recv ended ({err}); hotplug refresh stopped");
            break;
        }
        // Trust only a kernel-originated (nl_pid == 0), multicast-delivered (nl_groups != 0) datagram
        // with a full source address; drop a unicast or user-spoofed message.
        if (mhdr.msg_namelen as usize) < std::mem::size_of::<libc::sockaddr_nl>()
            || src.nl_pid != 0
            || src.nl_groups == 0
        {
            continue;
        }
        if !uevent_is_drm_change(&buf[..n as usize]) {
            continue;
        }
        // Request a refresh through the single cache writer, which re-enumerates every card, diffs,
        // and bumps the generation on a real change (an empty result -- the last monitor unplugged --
        // is a legitimate change and is published). Routing through it (rather than enumerating
        // inline here) coalesces uevent storms and keeps one writer, so a per-connection refresh
        // racing this thread cannot cause a hotplug to be diffed away without a generation bump.
        schedule_drm_cache_refresh();
    }
}

/// Best-effort warm-up at listener start: loads libdrmtap, initializes EGL, enumerates displays into
/// the cache, and maps the first framebuffer once. Moves that one-time cost (which otherwise lands
/// on the first consumer and can push the first frame past the client's initial-frame timeout) off
/// the critical path. Runs on its own thread since `DrmReader` is `!Send` and `open`/`grab` block.
fn drm_prewarm() {
    // The X11 gate lives HERE and not at the call site. Every consumer gates on `!is_x11()`, so on a
    // real X11 host none will ever connect and warming the root service is pure waste -- but the
    // answer is not yet knowable in the first moments of a boot. `get_display_server()` reads the
    // active seat0 session through loginctl and falls back to "x11" when it cannot tell, so a check
    // made ONCE at service start can answer "x11" on a Wayland host, and nothing revisits it: the
    // prewarm is then skipped for the life of the service. Measured on a boot: the skip was logged
    // 0.8s in, while loginctl reported the seat0 greeter session as wayland in that same second and
    // graphical-session.target only at +5s. That silently disabled the prewarm in the deployment this
    // feature is for (unit enabled at boot, unattended access at the login screen) -- it ran only
    // after a manual restart, which is how every deploy happened to exercise it.
    //
    // So re-ask, bounded. A genuine X11 or headless host exhausts the budget and returns having
    // opened no DrmReader and no DRM fd, which is what the gate is for. A host that boots to X11 and
    // gains a Wayland session much later gives its first connection the cold path -- exactly today's
    // behaviour for every boot, already handled, and not worth a permanent watcher thread.
    //
    // What this does NOT fix, deliberately: `_get_values_of_seat0` skips an active seat0 session
    // whose user `is_gdm_user()` AND whose type is wayland, and that filter is permanent, not a boot
    // transient. On a display manager whose greeter session user is literally `gdm` or `sddm`, an
    // unattended login screen therefore reads as X11 forever and this wait just times out -- no
    // worse than before, but no better either. It is not the common case on a current GDM, where the
    // greeter runs as a separate `gdm-greeter` account that the filter does not match (verified: the
    // prewarm primes while sitting at the Wayland greeter with nobody logged in). Widening the gate
    // to a greeter-inclusive check would mean a new helper in the shared platform code, which is a
    // bigger blast radius than the milliseconds justify.
    //
    // `scrap::is_x11()` is deliberately the UNMEMOISED path (it re-runs loginctl per call).
    // `crate::platform::linux::is_x11()` latches its answer in a lazy_static, so switching this loop
    // to it would leave the first reading cached and make the whole re-check a no-op.
    const PREWARM_SESSION_RECHECK: std::time::Duration = std::time::Duration::from_secs(2);
    const PREWARM_SESSION_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);
    let waited = std::time::Instant::now();
    while scrap::is_x11() {
        if waited.elapsed() >= PREWARM_SESSION_BUDGET {
            log::info!(
                "drm: session still reads as X11 after {:?}; skipping the pre-warm \
                 (the _drm listener still runs)",
                PREWARM_SESSION_BUDGET
            );
            return;
        }
        std::thread::sleep(PREWARM_SESSION_RECHECK);
    }
    // Timed from AFTER the session wait, so the number stays comparable to a restart-time prewarm
    // instead of silently including however long the boot took to bring a session up.
    let t = std::time::Instant::now();
    // Populate the cache (every card) through the single writer, then
    // warm the first framebuffer export on one auto-detected reader (the priming cost is per-process,
    // not per-card). A connection arriving before the async populate finishes self-enumerates.
    schedule_drm_cache_refresh();
    match scrap::drm_reader::DrmReader::open(None, 0) {
        Some(mut r) => {
            // Warm the first framebuffer export with grab_desc(), which exports a dma-buf fd WITHOUT
            // loading libEGL/libGLESv2 into the root service (the convert runs in the unprivileged
            // --server). Deliberately NOT grab(): that maps and detiles, so warming with it would
            // pull the vendor GL stack into the privileged process on every start, before any
            // consumer has even asked for a frame. A libdrmtap without grab_desc never loads (see
            // drmtap_dl::abi_accepted), so there is no older-library branch to fall back to here.
            if let Ok((fd, _desc)) = r.grab_desc() {
                drop(fd); // close the warm-up fd; we only wanted to prime the device/import path
            }
            log::info!("drm: pre-warm framebuffer primed in {:?}", t.elapsed());
        }
        None => log::info!("drm: pre-warm skipped (no reader; cache refresh requested)"),
    }
}

/// DRM/KMS capture producer. Runs in the ROOT `--service` (which holds CAP_SYS_ADMIN, so libdrmtap
/// reads the scanout in-process — no helper, no setcap). One dedicated `current_thread` runtime
/// owns the `_drm` listener and `tokio::spawn`s a task per accepted consumer, so a multi-monitor
/// client (which opens one `_drm` connection per captured display) is served CONCURRENTLY instead
/// of serially. The `!Send` `DrmReader` never runs on this runtime: each connection offloads its
/// blocking `grab()` loop to a private std worker thread (see `handle_drm_conn`), which keeps the
/// connection future `Send` (thus spawnable) and lets the tasks multiplex on the one listener
/// thread while the workers capture in parallel.
#[tokio::main(flavor = "current_thread")]
pub async fn start_drm() {
    match new_drm_listener() {
        Ok(mut incoming) => {
            // Warm libdrmtap/EGL + enumeration off-thread so the first consumer does not pay that
            // one-time cost on its critical path. `drm_prewarm` decides for itself whether this host
            // is X11 (see there: deciding it HERE is too early on a boot and skipped the prewarm for
            // good on a Wayland host). The LISTENER below starts either way: the root service
            // outlives sessions, a later Wayland login must find the `_drm` socket, and every
            // handshake enumerates fresh (drm_enumerate_settled), so a skipped prewarm costs that
            // session only the one-time library/EGL warmup the prewarm exists to hide.
            std::thread::spawn(drm_prewarm);
            // Watch for connector hotplug/modeset uevents so a mid-session topology change refreshes
            // the display cache and is pushed to live consumers (best-effort; own thread since it
            // blocks on recv and re-enumeration is a blocking `!Send` open).
            std::thread::spawn(drm_udev_listener);
            loop {
                match incoming.next().await {
                    Some(Ok(stream)) => {
                        tokio::spawn(async move {
                            if let Err(err) = handle_drm_conn(Connection::new(stream)).await {
                                log::info!("drm ipc connection ended: {}", err);
                            }
                        });
                    }
                    Some(Err(err)) => log::error!("Couldn't get drm client: {:?}", err),
                    // Stream exhausted: without this the `if let Some` form would re-poll the dead
                    // stream forever and busy-spin the root service. Stop the producer instead.
                    None => {
                        log::error!("drm ipc listener stream ended; stopping drm producer");
                        break;
                    }
                }
            }
        }
        Err(err) => {
            log::error!("Failed to start drm ipc server: {}", err);
        }
    }
}

/// Handle one `_drm` consumer. `DrmReader` is `!Send` and `grab()` is a blocking C call, so it
/// cannot live on the shared listener runtime; this task spawns a private std worker thread that
/// owns the reader (`drm_capture_worker`) and streams `DrmProducerMsg`s back over a bounded channel
/// (capacity 2 = backpressure: a slow consumer throttles capture instead of growing memory). The
/// task itself stays fully async — hence `Send`, hence `tokio::spawn`able — and only forwards
/// messages to the wire. On any error / disconnect it returns; the `DrmStopGuard` plus dropping the
/// channels tears the worker down, and the client falls back to PipeWire/portal.
/// Concurrency cap on accepted `_drm` consumer connections. Each accepted consumer spawns a worker
/// that opens a DRM context, so even though the peer is authorized we still bound how many a single
/// (buggy or compromised) --server can open, to keep it from exhausting root-service threads/memory.
/// One connection per served display is plenty; the slack covers a reconnect overlapping an old worker
/// still tearing down.
const MAX_DRM_CONNS: usize = 8;

/// Whether a new `_drm` connection is admitted, given the live count taken BEFORE it (the value
/// `AtomicUsize::fetch_add` returns). Pure, so the admission bound is unit-testable without the runtime
/// counter: admit while strictly below the cap, reject at or above it.
fn drm_conn_admitted(prev_count: usize) -> bool {
    prev_count < MAX_DRM_CONNS
}

/// Whether a `_drm` peer may keep receiving frames: root (uid 0) always, any other peer
/// only while it still matches the active-session uid, and an unknown peer never (fail closed). Pure,
/// so the per-frame re-authorization decision is unit-testable without a live logind session.
fn drm_peer_authorized(peer_uid: Option<u32>, active_uid: Option<u32>) -> bool {
    match peer_uid {
        Some(0) => true,
        Some(uid) => active_uid == Some(uid),
        None => false,
    }
}

async fn handle_drm_conn(stream: Connection) -> ResultType<()> {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    // The `_drm` socket is world-connectable (0666) so the unprivileged `--server` can reach it,
    // so we MUST authorize the peer here — this is a dedicated listener that does not go through
    // the generic `start()` accept loop where service-scoped channels are checked. Same policy as
    // `_service`: peer must be root or the active session uid, with a `/proc/pid/exe` identity
    // match. Without this any local process could connect and receive the screen contents.
    //
    // Run it on the blocking pool. Authorization reads the peer credentials and the ACTIVE session
    // uid, and on a cache miss the latter falls through to a synchronous `loginctl` fork
    // (`get_active_userid`). Because the socket is 0666, any local uid can make us do that, and
    // unlike `_service` this runtime is shared by EVERY live capture stream, so a stall here
    // hitches frames for all of them rather than just delaying one config sync.
    let (stream, authorized) = tokio::task::spawn_blocking(move || {
        let ok = authorize_service_scoped_ipc_connection(&stream, "_drm");
        (stream, ok)
    })
    .await?;
    if !authorized {
        // Deliberately no log here: `log_rejected_service_connection` inside the call above already
        // reports the rejection with the peer and active uid, and rate-limits it to one line per 5 s
        // precisely because these sockets are world-connectable. A second, unthrottled warn would
        // hand anyone who can connect an unbounded log-write primitive.
        return Ok(());
    }

    // Admission bound: each accepted _drm consumer spawns a worker thread that opens a DRM context.
    // The peer is authorized (root/active-session), but we still cap concurrency so a buggy or
    // compromised --server cannot exhaust root-service threads/memory by opening an unbounded number
    // of streams. One connection per served display is plenty; MAX_DRM_CONNS covers multi-monitor
    // plus a little slack for a reconnect overlapping an old worker still tearing down.
    static DRM_CONN_COUNT: AtomicUsize = AtomicUsize::new(0);
    struct DrmConnGuard;
    impl Drop for DrmConnGuard {
        fn drop(&mut self) {
            DRM_CONN_COUNT.fetch_sub(1, Ordering::SeqCst);
        }
    }
    if !drm_conn_admitted(DRM_CONN_COUNT.fetch_add(1, Ordering::SeqCst)) {
        DRM_CONN_COUNT.fetch_sub(1, Ordering::SeqCst);
        log::warn!("drm: too many concurrent _drm connections (>= {MAX_DRM_CONNS}); rejecting");
        return Ok(());
    }
    let _conn_guard = DrmConnGuard;

    // Capture the peer uid now so the forward loop can RE-authorize every frame. The check above runs
    // once at accept, but DRM/KMS capture is NOT session-scoped: `drm_capture_worker` grabs the
    // physical scanout of a CRTC regardless of which session currently owns the display. So a stream
    // authorized for one session must stop the moment the active session changes, or the outgoing
    // user's --server keeps receiving the incoming user's screen (and the greeter in between) until
    // the socket dies. `peer_uid` is the --server's fixed uid.
    let peer_uid = stream.peer_uid();

    // Move the authorized `_drm` stream onto the bespoke SCM_RIGHTS framing (see `DrmConn`). ALL
    // further traffic — display list, `DrmStart`, frame descriptors + their ancillary fd, and the
    // cursor / CPU-fallback bodies — goes through `conn` so no `Framed` read buffer ever competes with
    // a `recvmsg` for the fd. The parity `Connection` (used only for the authorization above) is
    // dropped here, closing its fd; the dup inside `conn` keeps the socket alive.
    let mut conn = dup_to_drm_conn(&stream)?;
    drop(stream);

    // worker -> task: display list, frames, cursor (bounded = backpressure).
    let (frame_tx, mut frame_rx) = tokio::sync::mpsc::channel::<DrmProducerMsg>(2);
    // task -> worker: the chosen CRTC + whether the consumer needs the CPU path, sent once after the
    // client's DrmStart.
    let (crtc_tx, crtc_rx) = std::sync::mpsc::channel::<(String, u32, bool)>();
    let stop = Arc::new(AtomicBool::new(false));
    let _stop_guard = DrmStopGuard(stop.clone());
    let worker_stop = stop.clone();
    // Set while the task is holding a frame it has no send credit for. The worker then skips the
    // scanout grab, which the task would only discard, instead of burning CPU (and a PRIME export
    // on the dma-buf path) inside the privileged service for a consumer that is behind.
    let frames_gated = Arc::new(AtomicBool::new(false));
    let worker_gate = frames_gated.clone();
    std::thread::spawn(move || drm_capture_worker(frame_tx, crtc_rx, worker_stop, worker_gate));

    // Handshake: the worker sends the display list -- a fresh, settled enumeration
    // (drm_enumerate_settled), possibly held back while a display wake completes. A closed channel
    // (no Displays) means the reader was unavailable, so let the client fall back.
    let displays = match frame_rx.recv().await {
        Some(DrmProducerMsg::Displays(d)) => d,
        _ => {
            log::info!("drm: reader unavailable; closing _drm connection (client falls back)");
            return Ok(());
        }
    };
    conn.send_msg(&Data::DrmDisplayList(displays.clone()), None).await?;

    // Wait for the client to choose a display before streaming. ONE receive, and anything that is
    // not `DrmStart` closes the connection: the consumer answers the display list with `DrmStart`
    // and nothing else, so there is no legitimate message to skip past. This used to be a loop that
    // ignored unexpected messages, which restarted the budget on every one of them, so a peer
    // trickling junk just inside the timeout held a worker thread and one of the MAX_DRM_CONNS
    // slots indefinitely, and MAX_DRM_CONNS such peers denied DRM capture entirely. The bound is
    // absolute now (twice the argument in the worst case: `recv_msg_timeout2` applies it to the
    // wait for the first byte and again to the body), and a timeout leaves the stream at a clean
    // frame boundary.
    let (display_idx, need_cpu) = match conn.recv_msg_timeout2(10_000).await {
        Some(Ok((Data::DrmStart { display, need_cpu }, _fd))) => (display, need_cpu),
        Some(Ok((_, _fd))) => {
            // Any stray fd is dropped (closed) with `_fd`.
            log::info!("drm: peer sent something other than DrmStart in the handshake; closing");
            return Ok(());
        }
        Some(Err(e)) => return Err(e),
        None => return Ok(()), // timed out: client never chose a display
    };
    // Resolve the chosen display's CRTC. `displays` here is already filtered to
    // CRTC-bound outputs (see drm_displays_from_reader), so a valid selection
    // always yields a non-zero crtc_id. Reject a 0 (out-of-range index, or an
    // unbound display that somehow slipped through) rather than passing it to
    // `open(crtc=0)`, whose "auto-select the first/primary CRTC" sentinel would
    // silently stream the WRONG monitor at a mismatched geometry and flap the
    // capturer. Closing lets the consumer fall back (PipeWire) for that display.
    // Resolve BOTH the CRTC and the card that owns it: crtc_ids are card-local, so on a multi-GPU
    // host the index must also select the device to reopen (`device` empty == the single
    // auto-detected device, i.e. the pre-multi-device behaviour).
    let selected = usize::try_from(display_idx)
        .ok()
        .and_then(|i| displays.get(i));
    let target_crtc = selected.map(|d| d.crtc_id).unwrap_or(0);
    let target_device = selected.map(|d| d.device.clone()).unwrap_or_default();
    if target_crtc == 0 {
        log::warn!(
            "drm: client selected display {display_idx} with no bound CRTC; closing _drm (client falls back)"
        );
        return Ok(());
    }
    // Hand the device + CRTC + the consumer's CPU-path request to the worker; an error means it
    // already gave up (reader vanished).
    if crtc_tx.send((target_device, target_crtc, need_cpu)).is_err() {
        return Ok(());
    }

    // Forward frames + cursor updates until the worker ends or the client disconnects (a wire send
    // error on a dropped client propagates out and tears the worker down via the guard).
    // Live hotplug: the udev listener bumps DRM_DISPLAY_GENERATION when the connector topology changes.
    // Seed from the value current at handshake (the list already sent reflects it) and, whenever it
    // moves, push the fresh list to this consumer. Piggybacked on the frame cadence so it costs only one
    // atomic load per frame; a genuinely idle stream tears down after MAX_STALLED and the consumer
    // reconnects to a fresh list anyway.
    let mut seen_gen = DRM_DISPLAY_GENERATION.load(Ordering::Acquire);
    // Flow control: allow at most DRM_FRAME_CREDIT frames in flight on the socket. The
    // consumer acks each converted frame (send_frame_ack) and the producer only sends while it has
    // credit, so a slow convert bounds the socket FIFO to a couple of frames instead of accumulating
    // seconds of stale descriptors (a permanently-behind desktop). Backpressure on the capture
    // worker comes from `frames_gated` (we keep draining the channel for cursors, so a full channel
    // no longer stalls it). Cursors and topology updates are not credit-gated.
    const DRM_FRAME_CREDIT: i32 = 2;
    let mut credit: i32 = DRM_FRAME_CREDIT;
    // When we last held send credit; drives the no-credit deadline below.
    let mut credit_since = std::time::Instant::now();
    // The newest frame produced while credit was exhausted. Holding it here (latest-wins, exactly
    // like the coalescing below) is what keeps the gate on FRAMES only: cursor and topology updates
    // are still received and forwarded meanwhile. Gating the whole loop instead would freeze the
    // remote cursor and delay hotplug for as long as a slow convert withholds its ack.
    let mut held_frame: Option<DrmProducerMsg> = None;
    loop {
        // Replenish credit from any acks the consumer has finished. Also detects a closed peer.
        conn.drain_frame_acks(&mut credit, DRM_FRAME_CREDIT)?;
        // Bound how long we may sit with no send credit. While gated the worker does not grab, so it
        // cannot advance its own MAX_STALLED watchdog: a consumer that stops acking without closing
        // the socket would otherwise hold this connection, its worker thread and the privileged DRM
        // context open indefinitely. The deadline is measured from the last time we HAD credit, not
        // from the last wake-up -- cursor traffic keeps flowing while gated and must not keep
        // renewing it.
        const CREDIT_STALL: std::time::Duration = std::time::Duration::from_secs(5);
        if credit > 0 {
            credit_since = std::time::Instant::now();
        } else if credit_since.elapsed() > CREDIT_STALL {
            log::info!("drm: consumer has not acked for {CREDIT_STALL:?}; closing _drm connection");
            break;
        }
        // Pause the worker's grab exactly while we cannot send. Note this must NOT also require that
        // a frame is already held: those grabs are not wasted, they keep the held frame fresh
        // (latest-wins below), so gating on "held" would pin whatever frame happened to be in hand
        // when credit ran out and ship it stale once the ack lands. Gating on credit alone is safe
        // because the wait below watches the socket whenever credit is out, held frame or not, so an
        // ack always wakes us.
        frames_gated.store(credit <= 0, Ordering::Relaxed);
        let first: Option<DrmProducerMsg> = if held_frame.is_some() && credit > 0 {
            // We are holding a frame we are allowed to send: never block. Waiting would park it
            // until the worker happens to produce another message, and if capture then returned
            // WouldBlock it would sit there until the stall teardown.
            frame_rx.try_recv().ok()
        } else if credit <= 0 {
            // No credit: wait for an ack or for another producer message. An ack wake yields no
            // message and falls through to the send decision below. Both arms are cancel-safe
            // (`mpsc::Receiver::recv`, and `wait_readable` is readiness-only).
            //
            // Capped so we wake to re-evaluate the no-credit deadline above even when nothing at
            // all arrives; the deadline itself is enforced there, not here.
            const CREDIT_POLL: std::time::Duration = std::time::Duration::from_secs(1);
            // Ok(None) = woke on an ack; Ok(Some(None)) = worker gone; Ok(Some(Some(m))) = message.
            let waited = tokio::time::timeout(CREDIT_POLL, async {
                tokio::select! {
                    biased;
                    r = conn.wait_readable() => r.map(|_| None),
                    m = frame_rx.recv() => Ok(Some(m)),
                }
            })
            .await;
            match waited {
                Err(_) => None,
                Ok(Err(err)) => return Err(err),
                Ok(Ok(None)) => None,
                Ok(Ok(Some(None))) => break,
                Ok(Ok(Some(Some(m)))) => Some(m),
            }
        } else {
            match frame_rx.recv().await {
                Some(f) => Some(f),
                None => break,
            }
        };
        // Re-authorize per frame: root (0) is always allowed; any other peer must still be
        // the active-session uid. Use the CACHE-ONLY active uid (never a blocking loginctl lookup): this
        // runs on the single-threaded `_drm` runtime, so a per-frame seat0 subprocess -- which is
        // exactly what a fresh lookup does during a session switch, when the cache is momentarily empty
        // -- would stall every stream. A cache miss (`None`) means the active session is momentarily
        // unknown (typically mid-switch), which we treat as fail-closed and stop. The stop latency is
        // therefore bounded by the service loop's active-uid cache cadence (a few hundred ms), plus we
        // stop as soon as the cache goes empty at the start of a switch.
        let peer_ok = drm_peer_authorized(peer_uid, active_uid_cached());
        if !peer_ok {
            log::warn!("drm: _drm peer no longer matches the active session (or it is unknown); closing");
            break;
        }
        let gen = DRM_DISPLAY_GENERATION.load(Ordering::Acquire);
        if gen != seen_gen {
            seen_gen = gen;
            // Recover from poison exactly like the writer does: the cache holds plain data whose
            // invariants cannot be torn, and a panicking holder elsewhere must not make every live
            // connection's topology push panic its task while the refresh worker keeps running.
            let fresh = DRM_DISPLAY_CACHE
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            // Send even an EMPTY list: when the last active CRTC disappears (all monitors
            // unplugged) the consumer must learn the topology is now empty, otherwise it keeps
            // advertising the removed displays indefinitely.
            conn.send_msg(&Data::DrmDisplaysChanged(fresh), None).await?;
        }
        // Coalesce to latest-wins at the source. The `_drm` socket is a FIFO, so a
        // consumer that drains slower than we produce (a 4K convert on a modest GPU) would fall
        // seconds behind stale frames. Drain everything already queued without blocking and forward
        // only the NEWEST frame; each replaced frame drops here, closing its OwnedFd (zero-copy path)
        // and freeing its pixel buffer (CPU path). Cursor updates are latency-insensitive state
        // (latest-wins by id downstream), so they are forwarded in order and never coalesced away.
        // Start from any frame held back for credit, so a newer one supersedes it (latest-wins).
        let mut latest_frame: Option<DrmProducerMsg> = held_frame.take();
        // When we woke on an ack rather than on a message, `first` is None; seed from the channel
        // anyway so anything queued meanwhile still supersedes the held frame and a queued cursor
        // still goes out this iteration instead of waiting for the next producer message.
        let mut msg = first.or_else(|| frame_rx.try_recv().ok());
        while let Some(m) = msg.take() {
            match m {
                f @ (DrmProducerMsg::Frame { .. } | DrmProducerMsg::FrameCpu { .. }) => {
                    latest_frame = Some(f);
                }
                DrmProducerMsg::Cursor {
                    id,
                    width,
                    height,
                    hotx,
                    hoty,
                    colors,
                } => {
                    conn.send_msg(
                        &Data::DrmCursor {
                            id,
                            width,
                            height,
                            hotx,
                            hoty,
                        },
                        None,
                    )
                    .await?;
                    conn.send_raw(Bytes::from(colors)).await?;
                }
                DrmProducerMsg::Displays(_) => {}
            }
            msg = frame_rx.try_recv().ok();
        }
        // Count any ack that landed while we were waiting, so the frame below is not held for an
        // extra round trip.
        conn.drain_frame_acks(&mut credit, DRM_FRAME_CREDIT)?;
        if credit <= 0 {
            // No credit: keep the newest frame back rather than queueing it behind the consumer.
            // Cursors and the topology push above already went out.
            held_frame = latest_frame;
            continue;
        }
        match latest_frame {
            Some(DrmProducerMsg::Frame { mut desc, fd }) => {
                // Every exported frame carries its fd. Eliding it on an fb_id the converter has
                // already imported looks free, but the kernel can recycle an fb_id onto a different
                // buffer with identical geometry and modifier, and this side cannot see the dma-buf
                // inode that would tell the difference, so an elision can serve a stale EGLImage.
                // Sending it is cheap: the converter imports once per buffer and closes the surplus
                // fd. libdrmtap's own import cache keys on fb_id AND inode, and can only re-import
                // when it is handed a real fd.
                let send_fd = fd.is_some();
                desc.has_fd = send_fd;
                let borrowed = if send_fd { fd.as_ref().map(|f| f.as_fd()) } else { None };
                conn.send_msg(&Data::DrmFrameDmabuf(desc), borrowed).await?;
                credit -= 1; // one frame in flight until the consumer acks it
                // `fd` (OwnedFd) is closed here whether or not it was attached (the cmsg dup'd it into
                // the peer). Closing immediately bounds our fd usage to ~1 in flight per frame.
            }
            Some(DrmProducerMsg::FrameCpu {
                width,
                height,
                data,
            }) => {
                // CPU-mapped fallback: pixels cross the wire, exactly like the pre-split protocol.
                conn.send_msg(&Data::DrmFrame { width, height }, None).await?;
                conn.send_raw(data).await?;
                credit -= 1; // one frame in flight until the consumer acks it
            }
            _ => {}
        }
    }
    Ok(())
}

/// The blocking half of a `_drm` connection: owns the `!Send` `DrmReader`(s) on its own thread and
/// streams messages to the async task. Ends (thread exits, reader closes) when the device is
/// unavailable, errors/stalls, or the task drops the channels / sets the stop flag.
fn drm_capture_worker(
    frame_tx: tokio::sync::mpsc::Sender<DrmProducerMsg>,
    crtc_rx: std::sync::mpsc::Receiver<(String, u32, bool)>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    frames_gated: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    // ~30 fps producer ceiling; the consumer's encoder/QoS sets the effective rate and the bounded
    // channel throttles us further if it is slower. Also avoids a busy-spin when `grab()` returns
    // the same scanout repeatedly.
    const FRAME_INTERVAL: Duration = Duration::from_millis(33);
    // Bound continuous no-frame (WouldBlock) time so a wedged device ends the stream (~5s) instead
    // of freezing forever; the client then falls back.
    const MAX_STALLED: u32 = 150;

    let t_conn = std::time::Instant::now();

    // Enumerate FRESH here rather than serving the warm cache, wake anything sleeping, and answer
    // only once the topology has settled (drm_enumerate_settled). The cache exists to keep this
    // handshake fast, and it cost us correctness twice over:
    //
    //  - It can offer a display that is no longer being driven. A cache published while the panel was
    //    awake still lists it after the compositor disables it; the consumer picks it, the capture
    //    reader OPENS (opening tolerates an inactive CRTC) and then never produces a frame, and the
    //    client sits on "waiting for image" -- the exact symptom this backend was built to remove.
    //  - The wake decision reads a count of connected-but-undriven displays, and when the list came
    //    from the cache that count belonged to some OTHER enumeration. Two things that must agree were
    //    never synchronised, so the wake did not fire on a stale-cache connection.
    //
    // The saving was small and measured: the whole prewarm, which is this work plus priming the export
    // path, runs in 1.4-16 ms. Paying it per connection to always tell the client the truth is the
    // right trade. The cache still serves the topology-change push and the udev listener.
    //
    // Holding the answer back while the wake settles is deliberate, and the consumer's list-read
    // timeout is sized for it (drm_capturer's DISPLAY_LIST_TIMEOUT_MS): one stable truth per
    // handshake beats a fast answer that changes seconds later, because the consumer publishes this
    // list to the client at login and every revision after that walks the hotplug path.
    let displays = drm_enumerate_settled("a consumer connected");
    // Send even an empty list: the consumer treats "0 displays" as Unavailable and falls back
    // promptly, rather than waiting out repeated probe failures.
    if frame_tx
        .blocking_send(DrmProducerMsg::Displays(displays))
        .is_err()
    {
        return;
    }

    // Wait for the task to relay the client's chosen device + CRTC + CPU-path request (Err => the
    // task gave up / disconnected). An empty device means the single auto-detected card.
    let (target_device, target_crtc, need_cpu) = match crtc_rx.recv() {
        Ok(c) => c,
        Err(_) => return,
    };
    let device_arg = if target_device.is_empty() {
        None
    } else {
        Some(target_device.as_str())
    };
    let t_open = std::time::Instant::now();
    let mut reader = match scrap::drm_reader::DrmReader::open(device_arg, target_crtc) {
        Some(r) => r,
        None => {
            log::warn!(
                "drm: failed to open crtc {target_crtc} on {}; closing _drm connection",
                if target_device.is_empty() { "auto" } else { &target_device }
            );
            // The display list handed out a CRTC that no longer opens (a hotplug/modeset likely
            // invalidated it between the handshake and here). Kick an async refresh so the cache the
            // topology push reads converges even without a new connection. The next connection
            // enumerates fresh regardless, so there is no stale-list flag to clear any more.
            schedule_drm_cache_refresh();
            return;
        }
    };
    // Refresh the cache the topology push and the udev listener read (handshakes enumerate fresh
    // and never read it), off this connection's first-frame path and single-flight (see
    // schedule_drm_cache_refresh) so a reconnect storm cannot spawn unbounded enumeration threads.
    schedule_drm_cache_refresh();
    log::debug!(
        "drm: capture reader for crtc {target_crtc} opened in {:?}",
        t_open.elapsed()
    );

    // A per-connection buffer-pool epoch so `buffer_id` is unique across connections even for the same
    // fb_id (the consumer may key a pool by buffer_id).
    static DRM_CONN_EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let conn_epoch = DRM_CONN_EPOCH.fetch_add(1, Ordering::Relaxed);

    // Prefer the zero-copy split export (root does NO EGL / convert / copy). Fall back to the
    // CPU-mapped path for this connection (pixels cross the wire, and root pays the convert) only
    // when the alternative is no stream at all: the consumer asked for it because it has no
    // render-node convert context (need_cpu), so a dma-buf fd would be useless to it, or grab_desc
    // later reports ENOTSUP (no transferable dma-buf on this seat). Both are facts about the seat or
    // the consumer. A stale libdrmtap is NOT one of them: one too old for the split export is
    // refused at load time, so this never demotes root to the in-process convert merely because of
    // which file was on the load path.
    let mut use_dmabuf = !need_cpu;

    let mut last_cursor_id: u64 = 0;
    let mut stalled: u32 = 0;
    let mut logged_first = false;
    while !stop.load(Ordering::Relaxed) {
        // Grab one frame in the current mode, producing an OWNED message (no borrow of `reader`
        // outlives this, so `reader.cursor()` below is free to run). The dma-buf path ships only the
        // descriptor + fd; the CPU path copies the packed BGRA once (Bytes::copy_from_slice).
        let grabbed: Option<std::io::Result<DrmProducerMsg>> = if frames_gated.load(Ordering::Relaxed)
        {
            // The task has no send credit, so a frame grabbed now could not go out and would only
            // be superseded before it could. Skip the scanout work -- and, on the dma-buf path, a
            // PRIME export -- rather than spend it in this privileged process for a consumer that
            // is behind; the task resumes us the moment an ack lands, so what it then sends is a
            // fresh grab rather than a stale one. The cursor poll below still runs so the remote
            // pointer stays live, and `stalled` is left untouched because the device is healthy --
            // the task bounds this state itself (CREDIT_STALL) since our watchdog cannot advance.
            None
        } else if use_dmabuf {
            Some(match reader.grab_desc() {
                Ok((fd, d)) => Ok(DrmProducerMsg::Frame {
                    desc: DmabufDesc {
                        buffer_id: (d.fb_id as u64) | ((conn_epoch as u64) << 32),
                        width: d.width,
                        height: d.height,
                        format: d.format,
                        modifier: d.modifier,
                        fb_id: d.fb_id,
                        num_planes: d.num_planes,
                        offsets: d.offsets,
                        pitches: d.pitches,
                        hdr_eotf: d.hdr_eotf,
                        hdr_max_nits: d.hdr_max_nits,
                        has_fd: true, // every exported frame carries its fd; see the send below
                    },
                    fd: Some(fd),
                }),
                Err(err) => Err(err),
            })
        } else {
            Some(match reader.grab() {
                Ok((buf, w, h)) => Ok(DrmProducerMsg::FrameCpu {
                    width: w as u32,
                    height: h as u32,
                    data: Bytes::copy_from_slice(buf),
                }),
                Err(err) => Err(err),
            })
        };
        match grabbed {
            // Gated: no frame work this tick, fall through to the cursor poll below.
            None => {}
            Some(Ok(msg)) => {
                stalled = 0;
                if !logged_first {
                    logged_first = true;
                    log::debug!(
                        "drm: first frame for crtc {target_crtc} in {:?} ({} path)",
                        t_conn.elapsed(),
                        if use_dmabuf { "dma-buf" } else { "cpu" }
                    );
                }
                if frame_tx.blocking_send(msg).is_err() {
                    break;
                }
            }
            Some(Err(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {
                stalled += 1;
                if stalled > MAX_STALLED {
                    log::info!("drm: capture stalled (no frame); closing _drm connection");
                    break;
                }
                std::thread::sleep(FRAME_INTERVAL);
                continue;
            }
            Some(Err(err)) if use_dmabuf && err.kind() == std::io::ErrorKind::Unsupported => {
                // The split export cannot work on this seat/driver (ENOTSUP). Switch this connection
                // to the CPU-mapped fallback (pixels over the wire) instead of tearing down or
                // rebuild-looping; the reader is already open and usable via grab().
                log::warn!(
                    "drm: grab_desc unsupported ({err}); switching to CPU-mapped fallback for this connection"
                );
                use_dmabuf = false;
                logged_first = false;
                continue;
            }
            Some(Err(err)) => {
                log::warn!("drm: capture error: {err}; closing _drm connection");
                break;
            }
        }

        // Ship the cursor shape only when it changes (id is a content hash or the hidden sentinel).
        if let Some(c) = reader.cursor() {
            if c.id != last_cursor_id {
                last_cursor_id = c.id;
                if frame_tx
                    .blocking_send(DrmProducerMsg::Cursor {
                        id: c.id,
                        width: c.width,
                        height: c.height,
                        hotx: c.hotx,
                        hoty: c.hoty,
                        colors: c.colors,
                    })
                    .is_err()
                {
                    break;
                }
            }
        }

        std::thread::sleep(FRAME_INTERVAL);
    }
}

/// Ancillary-fd transport for the `_drm` channel.
///
/// `ConnectionTmpl`'s `Framed<_, BytesCodec>` cannot carry (nor collect) an SCM_RIGHTS control
/// message: tokio's `AsyncRead` never does a `recvmsg` with a control buffer, so a fd sent alongside
/// a `Framed` byte-frame is silently dropped on receive, and interleaving a raw `sendmsg` with the
/// codec desyncs its internal read buffer. So the WHOLE `_drm` channel moves onto this bespoke
/// length-prefixed `sendmsg`/`recvmsg` framing, owning the raw `tokio::net::UnixStream` directly:
/// handshake (`DrmDisplayList`/`DrmStart`), frame descriptors, and the CPU-fallback/cursor bodies all
/// go through it so no `Framed` read buffer ever competes with a `recvmsg`.
///
/// Framing: each frame is a 4-byte big-endian length prefix + payload. `send_msg`/`recv_msg` carry a
/// JSON `Data`; `send_raw`/`next_raw` carry an opaque body. The dma-buf fd (when present) rides an
/// SCM_RIGHTS cmsg bound to the frame's first (prefix) byte, so reading the prefix with a control
/// buffer reliably collects it (`MSG_CTRUNC` is rejected). Reads use exact-length loops so they never
/// cross a frame boundary and thus never discard a following frame's ancillary fd.
pub(crate) struct DrmConn {
    /// The raw stream. Obtained from `connect_drm` (client) or the accepted `_drm` listener stream
    /// (service). All framing is done by hand on this fd; there is no `Framed` codec.
    stream: tokio::net::UnixStream,
    /// Grow-once accumulation buffer for `recv_msg` length-prefixed reads (preallocated model: it
    /// grows to the largest message seen and is then reused, never per-message reallocated). Raw
    /// bodies do not use it: `next_raw_into` reads into a buffer the caller owns, so a whole frame
    /// can be recycled between the receive path and the slot it is published to.
    read_buf: Vec<u8>,
    /// Set by `drm_read_full` once the current read has consumed at least one byte off the socket.
    /// `recv_msg` clears it before reading, and `recv_msg_timeout2` reads it to tell a spurious
    /// `readable()` wakeup with no frame yet (safe to re-poll -> `None`) from a peer that stalled
    /// mid-frame after sending some bytes (unresumable -> hard error).
    consumed: bool,
}

/// Cap on a JSON `Data` message read by `recv_msg` (headers/handshake are tiny; this only bounds a
/// hostile/oversized length prefix). Distinct from the raw-body cap because a body can be a whole
/// CPU-fallback frame.
const MAX_DRM_JSON_BYTES: usize = 8 * 1024 * 1024;
/// Total budget for one raw body, from the header that announced it. Generous on purpose: the body is
/// a full frame on the CPU path (33 MB at 4K) but it crosses a unix socket, so it is milliseconds in
/// practice and this only has to bound a peer that stopped.
const DRM_BODY_TIMEOUT_MS: u64 = 5_000;

/// Cap on a raw body read by `next_raw_into` (CPU-fallback BGRA / cursor RGBA). Covers a 256 MiB 8K
/// scanout (`DrmReader` bounds a frame to that) with margin.
const MAX_DRM_RAW_BYTES: usize = 512 * 1024 * 1024;
/// Control-buffer capacity for one SCM_RIGHTS cmsg carrying a single fd. `CMSG_SPACE(sizeof(int))` is
/// 24 bytes on our targets; 64 gives headroom and the `align(8)` matches `cmsghdr` alignment.
const DRM_CMSG_CAP: usize = 64;

/// Aligned storage for the SCM_RIGHTS control buffer (`msg_control` must be `cmsghdr`-aligned).
#[repr(align(8))]
struct DrmCmsgBuf([u8; DRM_CMSG_CAP]);

/// One non-blocking `sendmsg`: writes `buf` and, when `pass_fd` is `Some`, attaches exactly one
/// SCM_RIGHTS cmsg carrying that fd. The cmsg is attached ONLY when a fd is present (a -1 fd in an
/// SCM_RIGHTS cmsg fails the whole call). Returns bytes sent, or a `WouldBlock`/other io error.
///
/// SAFETY: `fd` must be a valid open socket fd; `buf` a valid readable slice; `pass_fd` (if any) a
/// valid open fd. The ancillary data is delivered by the kernel with the first byte of `buf`.
unsafe fn drm_sendmsg(fd: RawFd, buf: &[u8], pass_fd: Option<RawFd>) -> std::io::Result<usize> {
    use hbb_common::libc;
    let mut iov = libc::iovec {
        iov_base: buf.as_ptr() as *mut libc::c_void,
        iov_len: buf.len(),
    };
    let mut msg: libc::msghdr = std::mem::zeroed();
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    let mut cbuf = DrmCmsgBuf([0u8; DRM_CMSG_CAP]);
    if let Some(sfd) = pass_fd {
        msg.msg_control = cbuf.0.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = libc::CMSG_SPACE(std::mem::size_of::<libc::c_int>() as u32) as _;
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        // Sized above so CMSG_FIRSTHDR is non-null; guard anyway to avoid UB on any platform quirk.
        if cmsg.is_null() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "drm: CMSG_FIRSTHDR null",
            ));
        }
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(std::mem::size_of::<libc::c_int>() as u32) as _;
        let sfd_c: libc::c_int = sfd;
        std::ptr::copy_nonoverlapping(
            &sfd_c as *const libc::c_int as *const u8,
            libc::CMSG_DATA(cmsg),
            std::mem::size_of::<libc::c_int>(),
        );
    }
    let n = libc::sendmsg(fd, &msg, libc::MSG_NOSIGNAL);
    if n < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(n as usize)
    }
}

/// One non-blocking `recvmsg` into `buf` with a control buffer. Collects at most one SCM_RIGHTS fd
/// (any surplus fds are closed); rejects a truncated cmsg (`MSG_CTRUNC`) as a hard error after closing
/// whatever it parsed. Returns (bytes read, fd). Received fds are `O_CLOEXEC` (`MSG_CMSG_CLOEXEC`).
///
/// SAFETY: `fd` must be a valid open socket fd; `buf` a valid writable slice.
unsafe fn drm_recvmsg(fd: RawFd, buf: &mut [u8]) -> std::io::Result<(usize, Option<OwnedFd>)> {
    use hbb_common::libc;
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr() as *mut libc::c_void,
        iov_len: buf.len(),
    };
    let mut cbuf = DrmCmsgBuf([0u8; DRM_CMSG_CAP]);
    let mut msg: libc::msghdr = std::mem::zeroed();
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cbuf.0.as_mut_ptr() as *mut libc::c_void;
    msg.msg_controllen = cbuf.0.len() as _;
    let n = libc::recvmsg(fd, &mut msg, libc::MSG_CMSG_CLOEXEC);
    if n < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Walk the cmsgs; keep the first SCM_RIGHTS fd, close any extras. Each parsed int is wrapped in an
    // OwnedFd immediately so it is always closed on drop (no fd leak on any error path below).
    let mut got: Option<OwnedFd> = None;
    let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
    while !cmsg.is_null() {
        if (*cmsg).cmsg_level == libc::SOL_SOCKET && (*cmsg).cmsg_type == libc::SCM_RIGHTS {
            let data = libc::CMSG_DATA(cmsg);
            let hdr = libc::CMSG_LEN(0) as usize;
            let payload = ((*cmsg).cmsg_len as usize).saturating_sub(hdr);
            let count = payload / std::mem::size_of::<libc::c_int>();
            for i in 0..count {
                let mut rawfd: libc::c_int = -1;
                std::ptr::copy_nonoverlapping(
                    data.add(i * std::mem::size_of::<libc::c_int>()),
                    &mut rawfd as *mut libc::c_int as *mut u8,
                    std::mem::size_of::<libc::c_int>(),
                );
                if rawfd >= 0 {
                    let owned = OwnedFd::from_raw_fd(rawfd);
                    if got.is_none() {
                        got = Some(owned);
                    } // else: surplus fd, dropped here -> closed
                }
            }
        }
        cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
    }
    // A truncated control message means the kernel dropped fd(s) that did not fit: fail rather than
    // proceed with a missing/partial fd (drop `got` so anything parsed is closed first).
    if msg.msg_flags & libc::MSG_CTRUNC != 0 {
        drop(got);
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "drm: truncated SCM_RIGHTS control message (MSG_CTRUNC)",
        ));
    }
    Ok((n as usize, got))
}

/// Write all of `buf` to `stream`, attaching `pass_fd` (if any) to the FIRST byte (the kernel binds
/// SCM_RIGHTS ancillary to the first data byte of the `sendmsg` that carried it). Loops on
/// `WouldBlock` via `writable()`; the fd is attached only until the first `sendmsg` sends >= 1 byte.
async fn drm_write_all(
    stream: &tokio::net::UnixStream,
    mut buf: &[u8],
    mut pass_fd: Option<RawFd>,
) -> ResultType<()> {
    while !buf.is_empty() {
        stream.writable().await?;
        let raw = stream.as_raw_fd();
        let chunk = buf;
        let fd_now = pass_fd;
        match stream.try_io(tokio::io::Interest::WRITABLE, || unsafe {
            drm_sendmsg(raw, chunk, fd_now)
        }) {
            Ok(0) => bail!("drm: socket write returned 0 (peer closed)"),
            Ok(n) => {
                pass_fd = None; // ancillary delivered with these bytes; do not re-send it
                buf = &buf[n..];
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Write one length-prefixed frame: a 4-byte big-endian length + payload, with `pass_fd` (if any)
/// riding the prefix's first byte.
async fn drm_send_frame(
    stream: &tokio::net::UnixStream,
    payload: &[u8],
    pass_fd: Option<RawFd>,
) -> ResultType<()> {
    if payload.len() > u32::MAX as usize {
        bail!("drm: frame too large ({} bytes)", payload.len());
    }
    let prefix = (payload.len() as u32).to_be_bytes();
    // The fd rides the prefix (its first byte); the payload carries no ancillary.
    drm_write_all(stream, &prefix, pass_fd).await?;
    drm_write_all(stream, payload, None).await?;
    Ok(())
}

/// Read exactly `buf.len()` bytes from `stream`. When `want_cmsg` is true, the FIRST read uses a
/// control buffer to collect an SCM_RIGHTS fd (which the sender bound to the frame's first byte);
/// subsequent reads within the same frame are plain. Returns the collected fd, if any.
async fn drm_read_full(
    stream: &tokio::net::UnixStream,
    buf: &mut [u8],
    want_cmsg: bool,
    progress: &mut bool,
) -> ResultType<Option<OwnedFd>> {
    use hbb_common::libc;
    let mut off = 0usize;
    let mut got: Option<OwnedFd> = None;
    while off < buf.len() {
        stream.readable().await?;
        let raw = stream.as_raw_fd();
        // Only the first read of a frame carries the fd (bound to byte 0); after that, plain reads.
        let use_cmsg = want_cmsg && got.is_none();
        let n = {
            let dst: &mut [u8] = &mut buf[off..];
            match stream.try_io(tokio::io::Interest::READABLE, move || unsafe {
                if use_cmsg {
                    drm_recvmsg(raw, dst)
                } else {
                    let m = libc::read(raw, dst.as_mut_ptr() as *mut libc::c_void, dst.len());
                    if m < 0 {
                        Err(std::io::Error::last_os_error())
                    } else {
                        Ok((m as usize, None))
                    }
                }
            }) {
                Ok((0, _fd)) => bail!("drm: socket closed by peer"),
                Ok((m, fd)) => {
                    if let Some(f) = fd {
                        if got.is_none() {
                            got = Some(f);
                        }
                    }
                    m
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
            }
        };
        // Any byte off the socket commits us to this frame: a later cancellation (e.g. a recv_msg
        // timeout) cannot be safely re-polled, since the consumed bytes are gone from the stream.
        if n > 0 {
            *progress = true;
        }
        off += n;
    }
    Ok(got)
}

impl DrmConn {
    /// Take ownership of an already-connected/accepted raw `_drm` stream.
    pub fn new(stream: tokio::net::UnixStream) -> Self {
        Self {
            stream,
            read_buf: Vec::new(),
            consumed: false,
        }
    }

    /// Send one `Data` message (JSON, length-prefixed). When `fd` is `Some`, attach exactly one
    /// SCM_RIGHTS cmsg carrying that fd on the SAME frame as the payload (a -1 in an SCM_RIGHTS cmsg
    /// fails the whole call, so the cmsg is attached ONLY when a fd is present). `fd` is borrowed so
    /// the caller keeps ownership and closes it after the send has dup'd it into the peer.
    pub async fn send_msg(&mut self, data: &Data, fd: Option<BorrowedFd<'_>>) -> ResultType<()> {
        let payload = serde_json::to_vec(data)?;
        let pass_fd = fd.map(|f| f.as_raw_fd());
        drm_send_frame(&self.stream, &payload, pass_fd).await
    }

    /// Consumer -> producer: one-byte frame ack ("I finished converting one frame"), on the reverse
    /// direction (unused for messages after the handshake). It replenishes the producer's send credit
    /// so only a bounded number of frames are ever in flight on the socket. Without it, the producer
    /// keeps writing descriptors into the socket FIFO faster than a slow convert drains them, and the
    /// consumer processes an ever-growing backlog of stale frames (a permanently-behind desktop).
    /// Uses `&self.stream` directly, so it never conflicts with a concurrent `send_msg`/`recv_msg`.
    pub async fn send_frame_ack(&self) -> ResultType<()> {
        loop {
            self.stream.writable().await?;
            match self.stream.try_write(&[1u8]) {
                Ok(n) if n > 0 => return Ok(()),
                // A non-empty write returning 0 means the write half is shut down (the producer is
                // gone); surface it instead of "succeeding" without ever delivering the ack byte,
                // which would silently starve the producer of a send credit.
                Ok(_) => bail!("drm: _drm frame-ack write returned 0 (peer closed)"),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Producer: non-blockingly drain the frame-ack bytes the consumer has written, adding one send
    /// credit per byte (capped at `max`). A read of 0 means the consumer closed. Cheap when idle
    /// (a single `try_read` that returns WouldBlock).
    pub fn drain_frame_acks(&self, credit: &mut i32, max: i32) -> ResultType<()> {
        let mut buf = [0u8; 64];
        loop {
            match self.stream.try_read(&mut buf) {
                Ok(0) => bail!("drm: _drm frame-ack peer closed"),
                Ok(n) => *credit = (*credit + n as i32).min(max),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Producer: await until the socket is readable (a frame ack arrived, or the peer errored/closed).
    /// Cancel-safe (readiness only, consumes no bytes), so it is safe in a `select!`.
    pub async fn wait_readable(&self) -> ResultType<()> {
        self.stream.readable().await?;
        Ok(())
    }

    /// Receive one `Data` message plus any dma-buf fd delivered via SCM_RIGHTS. Reads the 4-byte
    /// length prefix (with a `CMSG_SPACE(size_of::<c_int>())` control buffer that collects the fd bound
    /// to the frame's first byte, rejecting `MSG_CTRUNC`), then the payload into the reusable
    /// `read_buf`. Returns the decoded `Data` and an `OwnedFd` iff one arrived.
    pub async fn recv_msg(&mut self) -> ResultType<(Data, Option<OwnedFd>)> {
        // Clear the per-frame progress flag before the first read so recv_msg_timeout2 can tell a
        // spurious readable() wakeup (nothing consumed) from a mid-frame stall (see its docs).
        self.consumed = false;
        let mut prefix = [0u8; 4];
        // Disjoint field borrows: &self.stream (read) + &mut self.consumed (progress). `prefix` is local.
        let fd = drm_read_full(&self.stream, &mut prefix, true, &mut self.consumed).await?;
        let len = u32::from_be_bytes(prefix) as usize;
        if len > MAX_DRM_JSON_BYTES {
            // `fd` (if any) is closed on drop.
            bail!("drm: message length {len} exceeds cap {MAX_DRM_JSON_BYTES}");
        }
        if self.read_buf.len() < len {
            self.read_buf.resize(len, 0);
        }
        // Disjoint field borrows: &self.stream (read) + &mut self.read_buf (dest) + &mut self.consumed.
        // No fd on the body.
        drm_read_full(&self.stream, &mut self.read_buf[..len], false, &mut self.consumed).await?;
        let data: Data = serde_json::from_slice(&self.read_buf[..len])?;
        Ok((data, fd))
    }

    /// Cancel-safe timeout wrapper around `recv_msg`, mirroring `ConnectionTmpl::next_timeout2`, so a
    /// dropped consumer re-checks its `stop` flag between frames. Returns `None` when no frame was in
    /// progress at the deadline -- either no byte was ever readable, or `readable()` fired spuriously and
    /// the first read found nothing -- because in that case nothing was consumed and re-polling is safe.
    /// Once at least one byte has been consumed the frame is committed, so the SAME budget bounds the
    /// rest of the read: a peer that sends part of a frame then stalls cannot pin this task forever (the
    /// `readable()` gate alone does not cover the length prefix or payload). Such an overrun is a hard
    /// error, NOT a `None`, because the consumed bytes are gone from the stream and the frame cannot be
    /// resumed -- the caller tears the stream down. `recv_msg` bodies are small length-prefixed JSON
    /// (<= MAX_DRM_JSON_BYTES), well under any caller's budget over a local socket, so this never trips
    /// a healthy peer.
    pub async fn recv_msg_timeout2(
        &mut self,
        ms_timeout: u64,
    ) -> Option<ResultType<(Data, Option<OwnedFd>)>> {
        // Bind the readiness result to a `let` so the borrowed `readable()` future temporary is dropped
        // at the `;` (releasing `&self.stream`) BEFORE `recv_msg()` takes `&mut self` in an arm.
        let ready = timeout(ms_timeout, self.stream.readable()).await;
        match ready {
            Err(_) => None, // no frame started: clean boundary, caller re-checks `stop`
            Ok(Err(e)) => Some(Err(e.into())),
            Ok(Ok(())) => match timeout(ms_timeout, self.recv_msg()).await {
                Ok(res) => Some(res),
                // Deadline hit inside recv_msg. Distinguish a spurious readable() with nothing actually
                // consumed (safe to re-poll -> None) from a genuine mid-frame stall after some bytes
                // were read (unresumable -> hard error).
                Err(_) if self.consumed => Some(Err(anyhow::anyhow!(
                    "drm: frame body stalled past {ms_timeout}ms after first byte; closing"
                ))),
                Err(_) => None,
            },
        }
    }

    /// Send a raw length-prefixed body (cursor pixels, CPU-fallback BGRA). Parity with
    /// `ConnectionTmpl::send_raw`, over the same manual framing (never carries an fd).
    pub async fn send_raw(&mut self, data: Bytes) -> ResultType<()> {
        drm_send_frame(&self.stream, &data, None).await
    }

    /// Receive a raw length-prefixed body INTO `out`, replacing its contents. Parity with
    /// `ConnectionTmpl::next_raw`, except that the caller owns the buffer so it can be recycled
    /// across frames. A raw body never carries an fd; a stray fd (protocol desync) is collected by
    /// `drm_read_full` and dropped/closed.
    ///
    /// Bounded as a whole: the header that announced this body has already been consumed, so a body
    /// that never finishes cannot be resumed, and an overrun is a hard error that ends the stream.
    /// Without the bound a producer that writes a header and then stops (crashed, stopped, wedged)
    /// pins the consumer receive thread forever on `readable()`. That thread is also the one that
    /// observes `stop`, so every capturer rebuild would strand another thread and its render context.
    pub async fn next_raw_into(&mut self, out: &mut Vec<u8>) -> ResultType<()> {
        match timeout(DRM_BODY_TIMEOUT_MS, self.next_raw_into_unbounded(out)).await {
            Ok(res) => res,
            Err(_) => bail!(
                "drm: raw body did not arrive within {DRM_BODY_TIMEOUT_MS}ms of its header; closing"
            ),
        }
    }

    async fn next_raw_into_unbounded(&mut self, out: &mut Vec<u8>) -> ResultType<()> {
        // next_raw is not called through recv_msg_timeout2, so its progress flag is unused; pass the
        // field for signature parity (recv_msg clears it before its own reads).
        let mut prefix = [0u8; 4];
        if drm_read_full(&self.stream, &mut prefix, true, &mut self.consumed)
            .await?
            .is_some()
        {
            log::warn!("drm: unexpected fd on a raw-body frame; dropping");
        }
        let len = u32::from_be_bytes(prefix) as usize;
        if len > MAX_DRM_RAW_BYTES {
            bail!("drm: raw body length {len} exceeds cap {MAX_DRM_RAW_BYTES}");
        }
        // Read straight into the caller's buffer, reusing its allocation. A CPU-fallback frame is a
        // whole packed-BGRA scanout, so allocating and zeroing a fresh one here and handing back a
        // copy of it was two full-frame passes per frame, ~250 MB/s of pure overhead at 4K30 on top
        // of the copy that actually moves the pixels. `resize` does nothing at all once the caller
        // has been through one frame of the same size, which is the steady state.
        out.resize(len, 0);
        drm_read_full(&self.stream, &mut out[..], false, &mut self.consumed).await?;
        Ok(())
    }
}

// Pure-userspace coverage for the bespoke `_drm` SCM_RIGHTS framing. The wire format is
// hand-rolled (length prefix + an fd bound to the frame's first byte) because `Framed`/`BytesCodec`
// cannot carry ancillary data, so it gets direct tests over a socketpair instead of only live runs.
#[cfg(test)]
mod drm_conn_tests {
    use super::*;
    use hbb_common::libc;
    use hbb_common::tokio::{self, io::AsyncWriteExt};
    use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};

    // `render_node` was added to DrmDisplayInfo after the wire already existed, so a service
    // and a server from different builds can disagree about it. `serde(default)` must keep an
    // older peer's message decodable (empty node == "auto-select", the previous behaviour)
    // rather than failing the whole DrmDisplayList and losing DRM capture.
    #[test]
    fn drm_display_info_decodes_without_render_node() {
        let legacy = r#"{"name":"DP-1","crtc_id":386,"x":0,"y":0,
                         "width":3840,"height":2160,"active":true}"#;
        let info: DrmDisplayInfo =
            serde_json::from_str(legacy).expect("a pre-render_node payload must still decode");
        assert_eq!(info.name, "DP-1");
        assert_eq!(info.crtc_id, 386);
        assert!(info.render_node.is_empty(), "missing node means auto-select");
        assert!(info.device.is_empty(), "missing device means auto-detect");

        // And a current payload round-trips the node + device.
        let current = DrmDisplayInfo {
            name: "DP-1".to_owned(),
            crtc_id: 386,
            x: 0,
            y: 0,
            width: 3840,
            height: 2160,
            active: true,
            render_node: "/dev/dri/renderD129".to_owned(),
            device: "/dev/dri/card2".to_owned(),
        };
        let wire = serde_json::to_vec(&current).unwrap();
        let back: DrmDisplayInfo = serde_json::from_slice(&wire).unwrap();
        assert_eq!(back, current);
    }

    // A blocking pipe as a probe fd: (read end, write end). Both are CLOEXEC-agnostic OwnedFds.
    fn pipe() -> (OwnedFd, OwnedFd) {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "pipe() failed");
        unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
    }

    // Raw sendmsg carrying `fds` in a single SCM_RIGHTS cmsg, used to forge the surplus-fd / MSG_CTRUNC
    // case the safe API cannot express (it sends at most one).
    unsafe fn send_with_fds(sock: libc::c_int, data: &[u8], fds: &[libc::c_int]) -> isize {
        let mut iov = libc::iovec {
            iov_base: data.as_ptr() as *mut libc::c_void,
            iov_len: data.len(),
        };
        let fdbytes = fds.len() * std::mem::size_of::<libc::c_int>();
        let space = libc::CMSG_SPACE(fdbytes as u32) as usize;
        let mut cbuf = vec![0u8; space];
        let mut msg: libc::msghdr = std::mem::zeroed();
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = cbuf.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = space as _;
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(fdbytes as u32) as _;
        std::ptr::copy_nonoverlapping(fds.as_ptr() as *const u8, libc::CMSG_DATA(cmsg), fdbytes);
        libc::sendmsg(sock, &msg, 0)
    }

    // A control message with no fd round-trips intact and reports no ancillary fd.
    #[tokio::test]
    async fn roundtrip_msg_no_fd() {
        let (a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut tx = DrmConn::new(a);
        let mut rx = DrmConn::new(b);
        tx.send_msg(&Data::DrmFrame { width: 1920, height: 1080 }, None)
            .await
            .unwrap();
        let (data, fd) = rx.recv_msg().await.unwrap();
        assert!(matches!(
            data,
            Data::DrmFrame {
                width: 1920,
                height: 1080
            }
        ));
        assert!(fd.is_none(), "no fd was sent, none must be reported");
    }

    // An fd bound to a frame's first byte crosses via SCM_RIGHTS and refers to the SAME open file: a
    // byte written into the original write end is readable through the received (dup'd) read end.
    #[tokio::test]
    async fn roundtrip_msg_with_fd_identity() {
        let (a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut tx = DrmConn::new(a);
        let mut rx = DrmConn::new(b);
        let (rd, wr) = pipe();
        tx.send_msg(&Data::DrmFrame { width: 4, height: 4 }, Some(rd.as_fd()))
            .await
            .unwrap();
        let (_data, fd) = rx.recv_msg().await.unwrap();
        let recv_fd = fd.expect("an fd was attached, it must be received");
        let sentinel = [0xABu8];
        assert_eq!(
            unsafe { libc::write(wr.as_raw_fd(), sentinel.as_ptr() as *const libc::c_void, 1) },
            1
        );
        let mut got = [0u8; 1];
        assert_eq!(
            unsafe { libc::read(recv_fd.as_raw_fd(), got.as_mut_ptr() as *mut libc::c_void, 1) },
            1
        );
        assert_eq!(got[0], 0xAB, "received fd must be the same pipe");
    }

    // A raw length-prefixed body (cursor / CPU-fallback path) round-trips byte-for-byte.
    #[tokio::test]
    async fn roundtrip_raw_body() {
        let (a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut tx = DrmConn::new(a);
        let mut rx = DrmConn::new(b);
        let body = Bytes::from(vec![7u8; 5000]);
        tx.send_raw(body.clone()).await.unwrap();
        let mut got = Vec::new();
        rx.next_raw_into(&mut got).await.unwrap();
        assert_eq!(&got[..], &body[..]);
        // The buffer is reused across bodies, including a SHORTER one: a stale tail from the
        // previous frame must not survive into it.
        let short = Bytes::from(vec![9u8; 10]);
        tx.send_raw(short.clone()).await.unwrap();
        rx.next_raw_into(&mut got).await.unwrap();
        assert_eq!(&got[..], &short[..]);
    }

    // A forged length prefix past the JSON cap is rejected at the prefix, before any body allocation.
    #[tokio::test]
    async fn rejects_oversized_length_prefix() {
        let (mut a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut rx = DrmConn::new(b);
        let bogus = (MAX_DRM_JSON_BYTES as u32 + 1).to_be_bytes();
        a.write_all(&bogus).await.unwrap();
        let err = rx
            .recv_msg()
            .await
            .err()
            .expect("a length past the cap must be rejected");
        assert!(
            err.to_string().contains("exceeds cap"),
            "unexpected error: {err}"
        );
    }

    // A header announcing a body that never arrives must not park the receive loop forever. The
    // header and the body are two separate reads, so the caller's own per-message budget does not
    // cover the second one; DRM_BODY_TIMEOUT_MS is what bounds it. This waits out the real deadline
    // rather than a paused clock (tokio's test-util feature is not enabled here), which costs
    // DRM_BODY_TIMEOUT_MS on a thread the rest of the suite is not waiting on.
    #[tokio::test]
    async fn a_body_that_never_arrives_times_out() {
        let (mut a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut rx = DrmConn::new(b);
        // Announce ten bytes and send none of them. `a` is kept alive, so this is a stall and not
        // the EOF the read path already handles.
        a.write_all(&10u32.to_be_bytes()).await.unwrap();
        let mut got = Vec::new();
        let err = rx
            .next_raw_into(&mut got)
            .await
            .err()
            .expect("a body that never arrives must time out");
        assert!(
            err.to_string().contains("did not arrive"),
            "unexpected error: {err}"
        );
    }

    // A peer that packs more than one fd into a single SCM_RIGHTS cmsg (the safe API never does) must
    // not smuggle extra fds into the consumer: drm_recvmsg keeps the FIRST and closes the rest. The
    // frame otherwise decodes normally and the kept fd is the first one sent. (Two fds fit the control
    // buffer thanks to cmsg alignment slack, so this exercises the surplus path, not truncation.)
    #[tokio::test]
    async fn surplus_fds_keep_only_the_first() {
        let (mut a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut rx = DrmConn::new(b);
        let (rd, wr) = pipe();
        let (rd2, _wr2) = pipe();
        let payload = serde_json::to_vec(&Data::DrmFrame {
            width: 8,
            height: 8,
        })
        .unwrap();
        let prefix = (payload.len() as u32).to_be_bytes();
        let n = unsafe { send_with_fds(a.as_raw_fd(), &prefix, &[rd.as_raw_fd(), rd2.as_raw_fd()]) };
        assert!(n >= 0, "sendmsg failed: {}", std::io::Error::last_os_error());
        a.write_all(&payload).await.unwrap();
        let (data, fd) = rx.recv_msg().await.unwrap();
        assert!(matches!(
            data,
            Data::DrmFrame {
                width: 8,
                height: 8
            }
        ));
        let kept = fd.expect("the first surplus fd must be kept");
        let sentinel = [0x5Au8];
        assert_eq!(
            unsafe { libc::write(wr.as_raw_fd(), sentinel.as_ptr() as *const libc::c_void, 1) },
            1
        );
        let mut got = [0u8; 1];
        assert_eq!(
            unsafe { libc::read(kept.as_raw_fd(), got.as_mut_ptr() as *mut libc::c_void, 1) },
            1
        );
        assert_eq!(got[0], 0x5A, "the kept fd must be the FIRST one sent");
    }

    // Enough fds to overflow the receiver's control buffer and force truncation. drm_recvmsg reads
    // into a DRM_CMSG_CAP (64-byte) control buffer, which holds up to 12 fds (CMSG_LEN(48)=64); 16 fds
    // need CMSG_LEN(64)=80 > 64, so the kernel sets MSG_CTRUNC and recv_msg must fail rather than
    // proceed with silently dropped fd(s).
    #[tokio::test]
    async fn rejects_truncated_control_message() {
        let (a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut rx = DrmConn::new(b);
        let (rd, _wr) = pipe();
        let dups: Vec<OwnedFd> = (0..16).map(|_| rd.try_clone().unwrap()).collect();
        let fds: Vec<libc::c_int> = dups.iter().map(|f| f.as_raw_fd()).collect();
        let prefix = 0u32.to_be_bytes(); // the fds ride the prefix read; CTRUNC fires before any body
        let n = unsafe { send_with_fds(a.as_raw_fd(), &prefix, &fds) };
        assert!(n >= 0, "sendmsg failed: {}", std::io::Error::last_os_error());
        let err = rx
            .recv_msg()
            .await
            .err()
            .expect("a truncated control message must be rejected");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("truncat") || msg.contains("ctrunc"),
            "unexpected error: {err}"
        );
    }

    // SO_PEERCRED plumbing: over a socketpair both ends report the creating process euid, so the
    // producer-auth path (connect_drm requires peer_uid == 0) reads a real uid rather than None.
    #[test]
    fn peer_uid_from_fd_reads_socket_peer() {
        let (a, _b) = std::os::unix::net::UnixStream::pair().unwrap();
        let euid = unsafe { libc::geteuid() };
        assert_eq!(peer_uid_from_fd(a.as_raw_fd()), Some(euid));
    }

    // Per-frame _drm re-auth decision: root always passes; a non-root peer passes only
    // while it still equals the active-session uid; an unknown peer or active session fails closed.
    #[test]
    fn drm_peer_authorized_matrix() {
        // root (uid 0) is always authorized, regardless of the active session (even unknown).
        assert!(drm_peer_authorized(Some(0), Some(1000)));
        assert!(drm_peer_authorized(Some(0), None));
        // a non-root peer is authorized only while it matches the active-session uid.
        assert!(drm_peer_authorized(Some(1000), Some(1000)));
        // a non-root peer whose session is no longer active (switched away) is rejected.
        assert!(!drm_peer_authorized(Some(1000), Some(1001)));
        // fail closed when the active session is momentarily unknown (mid session switch).
        assert!(!drm_peer_authorized(Some(1000), None));
        // fail closed when the peer uid could not be determined.
        assert!(!drm_peer_authorized(None, Some(1000)));
        assert!(!drm_peer_authorized(None, None));
    }

    // The accept-time authorization has two halves. `drm_peer_authorized_matrix` above covers the uid
    // half; this covers the other one, the /proc/<pid>/exe identity match, which is what stops a
    // DIFFERENT program running as the right uid from being handed the screen. It needs no second
    // build of rustdesk: any process whose executable differs from ours is a valid negative, so the
    // test spawns one.
    #[test]
    fn accept_time_exe_match_accepts_only_our_own_executable() {
        // Our own pid must match: same /proc/<pid>/exe by construction.
        let me = std::process::id();
        assert!(
            super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(Some(me), "_drm").is_ok(),
            "the test process must match its own executable"
        );

        // A real, live process running a DIFFERENT executable must not.
        let mut other = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("/bin/sleep should be spawnable in the test environment");
        // Wait for the exec to actually happen before looking. `spawn` returns once the child
        // exists, and until it finishes exec'ing, /proc/<pid>/exe still points at OUR binary, so
        // reading it too early sees a match and the assertion below passes for the wrong reason.
        // Synchronize on an INDEPENDENT observation (the link changing) rather than on the check
        // being tested. A real peer has necessarily exec'd and connected before it is authorized,
        // so this window exists only in the test.
        let ours = std::fs::read_link(format!("/proc/{me}/exe")).ok();
        let peer_link = format!("/proc/{}/exe", other.id());
        let mut exec_done = false;
        for _ in 0..200 {
            match std::fs::read_link(&peer_link) {
                Ok(p) if Some(&p) != ours.as_ref() => {
                    exec_done = true;
                    break;
                }
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
        let res = if exec_done {
            super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(Some(other.id()), "_drm")
        } else {
            Err(anyhow::anyhow!("child never exec'd; nothing was tested"))
        };
        let _ = other.kill();
        let _ = other.wait();
        assert!(exec_done, "the spawned child never exec'd, so the negative case was not exercised");
        assert!(
            res.is_err(),
            "a peer running another executable must be rejected, got {res:?}"
        );

        // No pid at all is a rejection, not a pass: the check fails closed when it cannot identify
        // the peer, which is the case that matters if the kernel ever stops answering SO_PEERCRED.
        assert!(super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(None, "_drm").is_err());
    }

    // _drm admission bound: admit strictly below MAX_DRM_CONNS, reject at and above it.
    // `prev_count` is the live count taken before this connection (what fetch_add returns).
    #[test]
    fn drm_conn_admission_bound() {
        assert!(drm_conn_admitted(0));
        assert!(drm_conn_admitted(MAX_DRM_CONNS - 1)); // last admitted slot
        assert!(!drm_conn_admitted(MAX_DRM_CONNS)); // cap reached -> rejected
        assert!(!drm_conn_admitted(MAX_DRM_CONNS + 5)); // over cap -> rejected
    }
}
