// The DRM/KMS capture half of the `_drm` IPC channel: types, root-service producer, framing.

use super::ipc_auth::active_uid_cached;
use super::*;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DrmDisplayInfo {
    pub name: String,
    pub crtc_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub active: bool,
    /// Render node of the GPU that EXPORTS this display's scanout; on a multi-GPU host auto-select
    /// can bind a different GPU whose cross-vendor import then fails. Empty when the service cannot
    /// name it: the consumer then auto-selects on a single-render-node host, and forces the CPU
    /// path where there are several.
    #[serde(default)]
    pub render_node: String,
    /// KMS card node (`/dev/dri/card*`) driving this display. crtc_ids are card-local, so the index
    /// alone is ambiguous across cards. Empty = the single auto-detected device.
    #[serde(default)]
    pub device: String,
}

/// Mirrors `scrap::drm_reader::drmtap_dmabuf_desc` except `dma_buf_fd` (never serializes — it rides
/// SCM_RIGHTS ancillary), and adds `buffer_id` (fb_id tagged with a per-connection epoch; no consumer reads it today) and `has_fd`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DmabufDesc {
    pub buffer_id: u64,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    /// KMS framebuffer id — libdrmtap's import-once cache key. 0 disables caching for this frame.
    pub fb_id: u32,
    /// Used entries in `offsets`/`pitches` (1..4); 0 is treated as 1.
    pub num_planes: u32,
    pub offsets: [u32; 4],
    pub pitches: [u32; 4],
    /// DRMTAP_EOTF_* (SDR=0, PQ=2, HLG=3). PQ triggers the HDR->SDR tone-map on convert.
    pub hdr_eotf: u32,
    pub hdr_max_nits: u32,
    /// True: the fd rides this message's SCM_RIGHTS cmsg. False: import-once cache hit for `fb_id`.
    pub has_fd: bool,
}

pub(crate) fn drm_ipc_path() -> String {
    let service_path = Config::ipc_path("_service");
    let dir = std::path::Path::new(&service_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/tmp"));
    dir.join("ipc_drm").to_string_lossy().into_owned()
}

pub(crate) async fn connect_drm(ms_timeout: u64) -> ResultType<DrmConn> {
    use std::os::fd::AsRawFd;
    let path = drm_ipc_path();
    let stream = timeout(ms_timeout, tokio::net::UnixStream::connect(&path)).await??;
    // The producer MUST be root: a non-root peer that won a socket-path race must not be trusted to
    // supply the display list, frames and an arbitrary dma-buf fd.
    if peer_uid_from_fd(stream.as_raw_fd()) != Some(0) {
        bail!("drm: _drm producer is not root; refusing to consume");
    }
    Ok(DrmConn::new(stream))
}

/// Bind the `_drm` listener 0666: connectable by any local uid, authorized in `handle_drm_conn`.
fn new_drm_listener() -> ResultType<Incoming> {
    let path = drm_ipc_path();
    let _ = ensure_secure_ipc_parent_dir(&path, "_service")?;
    // NOT `std::fs::remove_file`: `unlink(2)` returns EISDIR against a directory-typed squatter and
    // the bind then fails EADDRINUSE; the fd-based helper picks `AT_REMOVEDIR` (empty dirs only).
    if let Err(err) = remove_ipc_entry_via_secure_parent_fd(&path) {
        log::warn!("drm: could not clear a stale entry at {}: {}", &path, err);
    }
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

enum DrmProducerMsg {
    /// Enumerated displays, sent once before any frame.
    Displays(Vec<DrmDisplayInfo>),
    /// Zero-copy path: descriptor + scanout fd; the `OwnedFd` is closed once the send has dup'd it.
    Frame {
        desc: DmabufDesc,
        fd: Option<OwnedFd>,
    },
    /// CPU-mapped fallback (packed BGRA): consumer has no convert context (`need_cpu`), or ENOTSUP.
    FrameCpu {
        width: u32,
        height: u32,
        data: Bytes,
    },
    Cursor {
        id: u64,
        width: u32,
        height: u32,
        hotx: i32,
        hoty: i32,
        colors: Vec<u8>,
    },
}

struct DrmStopGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for DrmStopGuard {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

fn dup_to_drm_conn(stream: &Connection) -> ResultType<DrmConn> {
    let raw = stream.inner.get_ref().as_raw_fd();
    // F_DUPFD_CLOEXEC, not dup(): `dup` never copies close-on-exec, and this process forks (the
    // `loginctl` lookup), so an already-authorized `_drm` socket would leak into children.
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

static DRM_DISPLAY_CACHE: std::sync::Mutex<Vec<DrmDisplayInfo>> = std::sync::Mutex::new(Vec::new());

/// Bumped only when a change altered `DRM_DISPLAY_CACHE`; Release orders it after the cache write.
static DRM_DISPLAY_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Displays this reader serves, plus the identity (`device:connector`) of each undriven output.
fn drm_displays_from_reader(
    reader: &mut scrap::drm_reader::DrmReader,
    device: &str,
) -> (Vec<DrmDisplayInfo>, Vec<String>) {
    let render_node = reader.render_node().unwrap_or_default();
    let mut undriven = Vec::new();
    let displays: Vec<DrmDisplayInfo> = reader
        .displays()
        .into_iter()
        // Only outputs bound to a CRTC: a CONNECTED-but-unbound connector enumerates with
        // `crtc_id == 0`, and `open(crtc=0)` auto-selects the FIRST ACTIVE CRTC and streams ITS frames.
        .filter(|d| {
            if !d.active || d.crtc_id == 0 {
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

/// Active displays of every DRM device + the connected-but-undriven identities, from ONE look.
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
        // Take this even when the list is EMPTY: the fallback re-keys identities under `device = ""`.
        if any_opened {
            return (all, undriven_total);
        }
    }
    // Auto-detect alone is not enough: it picks a card that is SCANNING OUT. Measured on the T2 with
    // the panel idle-disabled it binds card0 (the Touch Bar); the panel on card2 is invisible to it.
    let mut all = Vec::new();
    let mut undriven_total = Vec::new();
    let mut paths: Vec<std::path::PathBuf> = match std::fs::read_dir("/dev/dri") {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("card") && n[4..].chars().all(|c| c.is_ascii_digit()))
            })
            .collect(),
        Err(err) => {
            log::debug!("drm: cannot read /dev/dri to enumerate cards: {err}");
            Vec::new()
        }
    };
    // Deterministic order, so the display list does not depend on directory order.
    paths.sort();
    let n_paths = paths.len();
    for p in paths {
        let Some(path) = p.to_str() else { continue };
        if let Some(mut r) = scrap::drm_reader::DrmReader::open(Some(path), 0) {
            let (mut got, mut undriven) = drm_displays_from_reader(&mut r, path);
            all.append(&mut got);
            undriven_total.append(&mut undriven);
        }
    }
    log::info!(
        "drm: enumerated /dev/dri directly ({} card path(s)): {} active display(s), {} connected \
         but undriven",
        n_paths,
        all.len(),
        undriven_total.len()
    );
    if all.is_empty() && undriven_total.is_empty() {
        if let Some(mut r) = scrap::drm_reader::DrmReader::open(None, 0) {
            log::info!("drm: no card enumerated by path; falling back to the auto-detected reader");
            return drm_displays_from_reader(&mut r, "");
        }
    }
    (all, undriven_total)
}

/// Connectors a wake did NOT bring back. SELF-REFUTING: an entry later seen DRIVEN is removed.
#[cfg(feature = "drm-wake")]
static DRM_WAKE_HOPELESS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

#[cfg(feature = "drm-wake")]
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

#[cfg(feature = "drm-wake")]
static DRM_LAST_WAKE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[cfg(feature = "drm-wake")]
static DRM_WAKE_UNAVAILABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Wake config key; `enable-` is load-bearing: an absent value reads as `!= "N"`, so it defaults ON.
#[cfg(feature = "drm-wake")]
const OPTION_ENABLE_DRM_DISPLAY_WAKE: &str = "enable-drm-display-wake";

#[cfg(feature = "drm-wake")]
const DRM_WAKE_MIN_GAP: std::time::Duration = std::time::Duration::from_secs(20);
#[cfg(feature = "drm-wake")]
const DRM_WAKE_DEVICE_SETTLE: std::time::Duration = std::time::Duration::from_millis(400);
#[cfg(feature = "drm-wake")]
const DRM_WAKE_RECHECK_TOTAL: std::time::Duration = std::time::Duration::from_secs(3);
#[cfg(feature = "drm-wake")]
const DRM_WAKE_SETTLE_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

/// Seconds since service start, monotonic: SystemTime would let a clock step re-open the wake gate.
#[cfg(feature = "drm-wake")]
fn drm_wake_clock_secs() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs()
}

/// Look like user activity so the compositor re-enables an idle-DISABLED connector (until it does,
/// nothing scans out). Measured on a T2 greeter: one relative move restored a 2880x1800 scanout.
#[cfg(feature = "drm-wake")]
fn drm_wake_displays(reason: &str) -> bool {
    use std::sync::atomic::Ordering;

    if DRM_WAKE_UNAVAILABLE.load(Ordering::Relaxed) {
        return false;
    }
    let now = drm_wake_clock_secs();
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
    }

    // It has to look like a MOUSE: libinput ignores a device with a single relative axis and no
    // buttons. Measured: REL_X + REL_Y + BTN_LEFT woke the panel; REL_X alone did not.
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

    // A FRESH uinput device is not bound yet; events written before udev binds it are lost. Measured
    // back to back: with this pause the panel went `disabled -> enabled`, without it it did not.
    std::thread::sleep(DRM_WAKE_DEVICE_SETTLE);

    // +1 then -1: activity with zero net displacement. emit() appends the SYN_REPORT itself.
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

#[cfg(not(feature = "drm-wake"))]
fn drm_enumerate_settled(reason: &str) -> Vec<DrmDisplayInfo> {
    let (displays, undriven) = drm_enumerate_all_displays();
    if !undriven.is_empty() {
        log::debug!(
            "drm: {} connected display(s) have no CRTC ({reason}); this build has no display wake",
            undriven.len()
        );
    }
    displays
}

/// Wake build: wake an undriven display and WAIT for the settled topology. The wait applies to every
/// handshake whose wake may still be in flight, not only the one whose attempt won the rate limit.
#[cfg(feature = "drm-wake")]
fn drm_enumerate_settled(reason: &str) -> Vec<DrmDisplayInfo> {
    use std::sync::atomic::Ordering;

    let (displays, undriven) = drm_enumerate_all_displays();
    if !hbb_common::config::Config::get_bool_option(OPTION_ENABLE_DRM_DISPLAY_WAKE) {
        if !undriven.is_empty() {
            log::info!(
                "drm: {} connected display(s) have no CRTC ({reason}), but the display wake is \
                 disabled by configuration ({OPTION_ENABLE_DRM_DISPLAY_WAKE}=N)",
                undriven.len()
            );
        }
        return displays;
    }
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
            return displays;
        }
        let last = DRM_LAST_WAKE.load(Ordering::Acquire);
        if last == 0
            || drm_wake_clock_secs().saturating_sub(last) > DRM_WAKE_SETTLE_WINDOW.as_secs()
        {
            return displays;
        }
    }
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
        schedule_drm_cache_refresh();
    }
    if fired && !cur_wakeable.is_empty() {
        // Only the handshake that FIRED latches; a loser's baseline was taken mid-transition.
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

/// The SINGLE writer of DRM_DISPLAY_CACHE (+ DRM_DISPLAY_GENERATION), off the caller's thread and
/// SINGLE-FLIGHT: a request arriving during a run coalesces into exactly one follow-up.
fn schedule_drm_cache_refresh() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    static PENDING: AtomicBool = AtomicBool::new(false);
    // Ownership of RUNNING, released on every exit incl. unwind and failed spawn; re-taken mid-loop.
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
    let mut slot = RefreshSlot(true);
    let spawned = std::thread::Builder::new()
        .name("drm-cache-refresh".into())
        .spawn(move || loop {
            PENDING.store(false, Ordering::Release);
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
            // the slot closes the lost-wakeup window (a request that set PENDING just before it).
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
        log::error!("drm: could not spawn the display-cache refresh worker: {err}");
    }
}

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

/// Refresh the display cache on DRM hotplug uevents (raw NETLINK_KOBJECT_UEVENT, no libudev).
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
    let mut buf = [0u8; 8192];
    loop {
        // recvmsg, not recv: a local process could UNICAST a spoofed uevent to this root listener.
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
        if (mhdr.msg_namelen as usize) < std::mem::size_of::<libc::sockaddr_nl>()
            || src.nl_pid != 0
            || src.nl_groups == 0
        {
            continue;
        }
        if !uevent_is_drm_change(&buf[..n as usize]) {
            continue;
        }
        schedule_drm_cache_refresh();
    }
}

fn drm_prewarm() {
    // Re-ask, bounded: `get_display_server()` falls back to "x11" when it cannot tell (measured:
    // "x11" 0.8 s into a boot on a Wayland host). `is_x11_for_drm()` is that path minus the
    // greeter blind spot, which a login screen never leaves.
    const PREWARM_SESSION_RECHECK: std::time::Duration = std::time::Duration::from_secs(2);
    const PREWARM_SESSION_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);
    let waited = std::time::Instant::now();
    while crate::platform::linux::is_x11_for_drm() {
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
    let t = std::time::Instant::now();
    schedule_drm_cache_refresh();
    match scrap::drm_reader::DrmReader::open(None, 0) {
        Some(mut r) => {
            // grab_desc(), not grab(): exports an fd without loading libEGL into the root service.
            if let Ok((fd, _desc)) = r.grab_desc() {
                drop(fd); // close the warm-up fd; we only wanted to prime the device/import path
            }
            log::info!("drm: pre-warm framebuffer primed in {:?}", t.elapsed());
        }
        None => log::info!("drm: pre-warm skipped (no reader; cache refresh requested)"),
    }
}

/// Capture producer in the ROOT `--service`: one task per consumer, reader on a worker thread.
#[tokio::main(flavor = "current_thread")]
pub async fn start_drm() {
    match new_drm_listener() {
        Ok(mut incoming) => {
            if let Err(err) = std::thread::Builder::new()
                .name("drm-prewarm".into())
                .spawn(drm_prewarm)
            {
                log::warn!("drm: could not spawn the pre-warm thread ({err}); skipping the warmup");
            }
            if let Err(err) = std::thread::Builder::new()
                .name("drm-udev".into())
                .spawn(drm_udev_listener)
            {
                log::warn!(
                    "drm: could not spawn the udev listener ({err}); a mid-session topology change \
                     will not be pushed, and consumers pick it up on their next handshake"
                );
            }
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

const MAX_DRM_CONNS: usize = 8;

fn drm_conn_admitted(prev_count: usize) -> bool {
    prev_count < MAX_DRM_CONNS
}

const MAX_DRM_AUTH_IN_FLIGHT: usize = 4;

fn drm_auth_admitted(prev_in_flight: usize) -> bool {
    prev_in_flight < MAX_DRM_AUTH_IN_FLIGHT
}

fn drm_peer_authorized(peer_uid: Option<u32>, active_uid: Option<u32>) -> bool {
    match peer_uid {
        Some(0) => true,
        Some(uid) => active_uid == Some(uid),
        None => false,
    }
}

/// Handle one `_drm` consumer: a private worker thread owns the `!Send` reader; this task forwards.
async fn handle_drm_conn(stream: Connection) -> ResultType<()> {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    // World-connectable socket, so the peer MUST be authorized here (this listener bypasses the
    // generic `start()` accept loop). On the blocking pool: a cache miss forks `loginctl`.
    static DRM_AUTH_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
    struct DrmAuthGuard;
    impl Drop for DrmAuthGuard {
        fn drop(&mut self) {
            DRM_AUTH_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        }
    }
    if !drm_auth_admitted(DRM_AUTH_IN_FLIGHT.fetch_add(1, Ordering::SeqCst)) {
        DRM_AUTH_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        // Deliberately `debug`, not `warn`: this is reachable by any local uid, so a level that
        // reaches the service log on every attempt is an unbounded log-write primitive for that peer.
        log::debug!("drm: too many _drm authorizations in flight; dropping this connection");
        return Ok(());
    }
    let auth_guard = DrmAuthGuard;
    let (stream, authorized) = tokio::task::spawn_blocking(move || {
        let ok = authorize_service_scoped_ipc_connection(&stream, "_drm");
        (stream, ok)
    })
    .await?;
    drop(auth_guard);
    if !authorized {
        // Deliberately no log here: the call above already reports it -- the uid mismatch through
        // `log_rejected_service_connection`, throttled to one line per 5 s, and the executable
        // mismatch as a plain warn. A second, unthrottled warn here would be the same unbounded
        // log-write primitive.
        return Ok(());
    }

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

    // Re-authorized per frame below: DRM/KMS capture is NOT session-scoped, so unless a stream stops
    // when the active session changes the outgoing user's --server keeps receiving the incoming
    // user's screen (and the greeter in between).
    let peer_uid = stream.peer_uid();

    let mut conn = dup_to_drm_conn(&stream)?;
    drop(stream);

    let (frame_tx, mut frame_rx) = tokio::sync::mpsc::channel::<DrmProducerMsg>(2);
    let (crtc_tx, crtc_rx) = std::sync::mpsc::channel::<(String, u32, bool)>();
    let stop = Arc::new(AtomicBool::new(false));
    let _stop_guard = DrmStopGuard(stop.clone());
    let worker_stop = stop.clone();
    let frames_gated = Arc::new(AtomicBool::new(false));
    let worker_gate = frames_gated.clone();
    std::thread::Builder::new()
        .name("drm-capture".into())
        .spawn(move || drm_capture_worker(frame_tx, crtc_rx, worker_stop, worker_gate))
        .map_err(|err| anyhow::anyhow!("could not spawn the drm capture worker: {err}"))?;

    let displays = match frame_rx.recv().await {
        Some(DrmProducerMsg::Displays(d)) => d,
        _ => {
            log::info!("drm: reader unavailable; closing _drm connection (client falls back)");
            return Ok(());
        }
    };
    conn.send_msg(&Data::DrmDisplayList(displays.clone()), None).await?;

    let (display_idx, need_cpu) = match conn.recv_msg_timeout2(10_000).await {
        Some(Ok((Data::DrmStart { display, need_cpu }, _fd))) => (display, need_cpu),
        Some(Ok((_, _fd))) => {
            log::info!("drm: peer sent something other than DrmStart in the handshake; closing");
            return Ok(());
        }
        Some(Err(e)) => return Err(e),
        None => return Ok(()), // timed out: client never chose a display
    };
    // Reject crtc 0: `open(crtc=0)` auto-selects the FIRST ACTIVE CRTC and streams the WRONG monitor.
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
    if crtc_tx.send((target_device, target_crtc, need_cpu)).is_err() {
        return Ok(());
    }

    let mut seen_gen = DRM_DISPLAY_GENERATION.load(Ordering::Acquire);
    const DRM_FRAME_CREDIT: i32 = 2;
    let mut credit: i32 = DRM_FRAME_CREDIT;
    let mut credit_since = std::time::Instant::now();
    let mut held_frame: Option<DrmProducerMsg> = None;
    loop {
        conn.drain_frame_acks(&mut credit, DRM_FRAME_CREDIT)?;
        // While gated the worker does not grab, so it cannot advance its own MAX_STALLED watchdog: a
        // consumer that stops acking without closing the socket would otherwise hold this connection,
        // its worker thread and the privileged DRM context open indefinitely.
        const CREDIT_STALL: std::time::Duration = std::time::Duration::from_secs(5);
        if credit > 0 {
            credit_since = std::time::Instant::now();
        } else if credit_since.elapsed() > CREDIT_STALL {
            log::info!("drm: consumer has not acked for {CREDIT_STALL:?}; closing _drm connection");
            break;
        }
        // This must NOT also require that a frame is already held: those grabs keep the held frame
        // fresh (latest-wins below), so gating on "held" would pin whatever frame was in hand when
        // credit ran out and ship it stale once the ack lands.
        frames_gated.store(credit <= 0, Ordering::Relaxed);
        let first: Option<DrmProducerMsg> = if held_frame.is_some() && credit > 0 {
            frame_rx.try_recv().ok()
        } else if credit <= 0 {
            const CREDIT_POLL: std::time::Duration = std::time::Duration::from_secs(1);
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
        // Re-authorize per frame with the CACHE-ONLY active uid: a fresh lookup forks `loginctl` and
        // would stall every stream on this single-threaded runtime. A miss is fail-closed for a non-root peer
            // (root stays authorized; see `drm_peer_authorized`).
        let peer_ok = drm_peer_authorized(peer_uid, active_uid_cached());
        if !peer_ok {
            log::warn!("drm: _drm peer no longer matches the active session (or it is unknown); closing");
            break;
        }
        let gen = DRM_DISPLAY_GENERATION.load(Ordering::Acquire);
        if gen != seen_gen {
            seen_gen = gen;
            let fresh = DRM_DISPLAY_CACHE
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            // Send even an EMPTY list, or the consumer keeps advertising removed displays.
            conn.send_msg(&Data::DrmDisplaysChanged(fresh), None).await?;
        }
        let mut latest_frame: Option<DrmProducerMsg> = held_frame.take();
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
        conn.drain_frame_acks(&mut credit, DRM_FRAME_CREDIT)?;
        if credit <= 0 {
            held_frame = latest_frame;
            continue;
        }
        match latest_frame {
            Some(DrmProducerMsg::Frame { mut desc, fd }) => {
                // Every exported frame carries its fd: the kernel can recycle an fb_id onto another
                // buffer with the same geometry/modifier and this side cannot see the dma-buf inode
                // that would tell the difference, so eliding it can serve a stale EGLImage. libdrmtap's
                // import cache keys on fb_id AND inode, and can only re-import when handed a real fd.
                let send_fd = fd.is_some();
                desc.has_fd = send_fd;
                let borrowed = if send_fd { fd.as_ref().map(|f| f.as_fd()) } else { None };
                conn.send_msg(&Data::DrmFrameDmabuf(desc), borrowed).await?;
                credit -= 1; // one frame in flight until the consumer acks it
                // `fd` (OwnedFd) is closed here whether or not it was attached (the cmsg dup'd it
                // into the peer), which bounds our fd usage to ~1 in flight per frame.
            }
            Some(DrmProducerMsg::FrameCpu {
                width,
                height,
                data,
            }) => {
                conn.send_msg(&Data::DrmFrame { width, height }, None).await?;
                conn.send_raw(data).await?;
                credit -= 1; // one frame in flight until the consumer acks it
            }
            _ => {}
        }
    }
    Ok(())
}

fn drm_capture_worker(
    frame_tx: tokio::sync::mpsc::Sender<DrmProducerMsg>,
    crtc_rx: std::sync::mpsc::Receiver<(String, u32, bool)>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    frames_gated: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    const FRAME_INTERVAL: Duration = Duration::from_millis(33);
    // Bound continuous no-frame (WouldBlock) time so a wedged device ends the stream (~5 s).
    const MAX_STALLED: u32 = 150;

    let t_conn = std::time::Instant::now();

    // Enumerate FRESH rather than serve the cache: a cached display may no longer be driven.
    let displays = drm_enumerate_settled("a consumer connected");
    if frame_tx
        .blocking_send(DrmProducerMsg::Displays(displays))
        .is_err()
    {
        return;
    }

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
            schedule_drm_cache_refresh();
            return;
        }
    };
    schedule_drm_cache_refresh();
    log::debug!(
        "drm: capture reader for crtc {target_crtc} opened in {:?}",
        t_open.elapsed()
    );

    static DRM_CONN_EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let conn_epoch = DRM_CONN_EPOCH.fetch_add(1, Ordering::Relaxed);

    let mut use_dmabuf = !need_cpu;

    let mut last_cursor_id: u64 = 0;
    let mut stalled: u32 = 0;
    let mut logged_first = false;
    while !stop.load(Ordering::Relaxed) {
        let grabbed: Option<std::io::Result<DrmProducerMsg>> = if frames_gated.load(Ordering::Relaxed)
        {
            // `stalled` is left untouched because the device is healthy -- the task bounds this
            // state itself (CREDIT_STALL) since our watchdog cannot advance.
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
                log::warn!(
                    "drm: grab_desc unsupported ({err}); switching to CPU-mapped fallback for this connection"
                );
                use_dmabuf = false;
                logged_first = false;
                // The stall counter measured the abandoned path; give the fallback the whole budget.
                stalled = 0;
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

/// Ancillary-fd transport for `_drm`: `Framed`/`BytesCodec` cannot carry an SCM_RIGHTS cmsg, so the
/// messages and raw bodies use a 4-byte big-endian length + payload, with any fd bound to the first
    /// byte. The reverse-direction frame acks are bare bytes, not framed.
pub(crate) struct DrmConn {
    stream: tokio::net::UnixStream,
    read_buf: Vec<u8>,
    /// Set once the current read consumed a byte: a spurious `readable()` vs a mid-frame stall.
    consumed: bool,
}

const MAX_DRM_JSON_BYTES: usize = 8 * 1024 * 1024;
const DRM_BODY_TIMEOUT_MS: u64 = 5_000;
const DRM_SEND_TIMEOUT_MS: u64 = 5_000;

const MAX_DRM_RAW_BYTES: usize = 512 * 1024 * 1024;
/// `CMSG_SPACE(sizeof(int))` is 24 bytes on our targets; 64 gives headroom and the `align(8)`
/// matches `cmsghdr` alignment.
const DRM_CMSG_CAP: usize = 64;

/// Aligned storage for the SCM_RIGHTS control buffer (`msg_control` must be `cmsghdr`-aligned).
#[repr(align(8))]
struct DrmCmsgBuf([u8; DRM_CMSG_CAP]);

/// One non-blocking `sendmsg`; the cmsg is attached ONLY when a fd is present (-1 fails the call).
/// SAFETY: `fd` a valid open socket fd, `buf` a readable slice, `pass_fd` (if any) a valid open fd.
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

/// One non-blocking `recvmsg`: keeps at most one SCM_RIGHTS fd (surplus closed), rejects MSG_CTRUNC.
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
    if msg.msg_flags & libc::MSG_CTRUNC != 0 {
        drop(got);
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "drm: truncated SCM_RIGHTS control message (MSG_CTRUNC)",
        ));
    }
    Ok((n as usize, got))
}

async fn drm_write_all(
    stream: &tokio::net::UnixStream,
    mut buf: &[u8],
    mut pass_fd: Option<RawFd>,
) -> ResultType<()> {
    // ONE deadline for the whole write: arming it per readiness wait lets a dripping peer re-arm it.
    let deadline =
        tokio::time::Instant::now() + std::time::Duration::from_millis(DRM_SEND_TIMEOUT_MS);
    while !buf.is_empty() {
        match tokio::time::timeout_at(deadline, stream.writable()).await {
            Ok(r) => r?,
            Err(_) => bail!(
                "drm: peer did not accept the remaining {} byte(s) within {DRM_SEND_TIMEOUT_MS}ms; closing",
                buf.len()
            ),
        }
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

async fn drm_send_frame(
    stream: &tokio::net::UnixStream,
    payload: &[u8],
    pass_fd: Option<RawFd>,
) -> ResultType<()> {
    if payload.len() > u32::MAX as usize {
        bail!("drm: frame too large ({} bytes)", payload.len());
    }
    let prefix = (payload.len() as u32).to_be_bytes();
    drm_write_all(stream, &prefix, pass_fd).await?;
    drm_write_all(stream, payload, None).await?;
    Ok(())
}

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
        // Any byte off the socket commits us to this frame: a cancellation cannot be re-polled.
        if n > 0 {
            *progress = true;
        }
        off += n;
    }
    Ok(got)
}

impl DrmConn {
    pub fn new(stream: tokio::net::UnixStream) -> Self {
        Self {
            stream,
            read_buf: Vec::new(),
            consumed: false,
        }
    }

    pub async fn send_msg(&mut self, data: &Data, fd: Option<BorrowedFd<'_>>) -> ResultType<()> {
        let payload = serde_json::to_vec(data)?;
        let pass_fd = fd.map(|f| f.as_raw_fd());
        drm_send_frame(&self.stream, &payload, pass_fd).await
    }

    pub async fn send_frame_ack(&self) -> ResultType<()> {
        let deadline =
            tokio::time::Instant::now() + std::time::Duration::from_millis(DRM_SEND_TIMEOUT_MS);
        loop {
            match tokio::time::timeout_at(deadline, self.stream.writable()).await {
                Ok(r) => r?,
                Err(_) => bail!(
                    "drm: _drm frame-ack was not accepted within {DRM_SEND_TIMEOUT_MS}ms; closing"
                ),
            }
            match self.stream.try_write(&[1u8]) {
                Ok(n) if n > 0 => return Ok(()),
                Ok(_) => bail!("drm: _drm frame-ack write returned 0 (peer closed)"),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub fn drain_frame_acks(&self, credit: &mut i32, max: i32) -> ResultType<()> {
        let mut buf = [0u8; 64];
        // BOUNDED: "until WouldBlock" is the peer's promise; a continuous writer would pin us.
        const MAX_ACK_READS: usize = 64;
        for _ in 0..MAX_ACK_READS {
            match self.stream.try_read(&mut buf) {
                Ok(0) => bail!("drm: _drm frame-ack peer closed"),
                Ok(n) => {
                    *credit = (*credit + n as i32).min(max);
                    if *credit >= max {
                        return Ok(());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    pub async fn wait_readable(&self) -> ResultType<()> {
        self.stream.readable().await?;
        Ok(())
    }

    pub async fn recv_msg(&mut self) -> ResultType<(Data, Option<OwnedFd>)> {
        self.consumed = false;
        let mut prefix = [0u8; 4];
        let fd = drm_read_full(&self.stream, &mut prefix, true, &mut self.consumed).await?;
        let len = u32::from_be_bytes(prefix) as usize;
        if len > MAX_DRM_JSON_BYTES {
            bail!("drm: message length {len} exceeds cap {MAX_DRM_JSON_BYTES}");
        }
        if self.read_buf.len() < len {
            self.read_buf.resize(len, 0);
        }
        drm_read_full(&self.stream, &mut self.read_buf[..len], false, &mut self.consumed).await?;
        let data: Data = serde_json::from_slice(&self.read_buf[..len])?;
        Ok((data, fd))
    }

    /// Cancel-safe timeout wrapper around `recv_msg`. `None` = nothing consumed, so re-polling is
    /// safe; past the first byte the frame is committed and an overrun is a hard error.
    pub async fn recv_msg_timeout2(
        &mut self,
        ms_timeout: u64,
    ) -> Option<ResultType<(Data, Option<OwnedFd>)>> {
        let ready = timeout(ms_timeout, self.stream.readable()).await;
        match ready {
            Err(_) => None, // no frame started: clean boundary, caller re-checks `stop`
            Ok(Err(e)) => Some(Err(e.into())),
            Ok(Ok(())) => match timeout(ms_timeout, self.recv_msg()).await {
                Ok(res) => Some(res),
                Err(_) if self.consumed => Some(Err(anyhow::anyhow!(
                    "drm: frame body stalled past {ms_timeout}ms after first byte; closing"
                ))),
                Err(_) => None,
            },
        }
    }

    pub async fn send_raw(&mut self, data: Bytes) -> ResultType<()> {
        drm_send_frame(&self.stream, &data, None).await
    }

    pub async fn next_raw_into(&mut self, out: &mut Vec<u8>) -> ResultType<()> {
        match timeout(DRM_BODY_TIMEOUT_MS, self.next_raw_into_unbounded(out)).await {
            Ok(res) => res,
            Err(_) => bail!(
                "drm: raw body did not arrive within {DRM_BODY_TIMEOUT_MS}ms of its header; closing"
            ),
        }
    }

    async fn next_raw_into_unbounded(&mut self, out: &mut Vec<u8>) -> ResultType<()> {
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
        out.resize(len, 0);
        drm_read_full(&self.stream, &mut out[..], false, &mut self.consumed).await?;
        Ok(())
    }
}

#[cfg(test)]
mod drm_conn_tests {
    use super::*;
    use hbb_common::libc;
    use hbb_common::tokio::{self, io::AsyncWriteExt};
    use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};

    // Added to the wire later: an older peer's message must still decode.
    #[test]
    fn drm_display_info_decodes_without_render_node() {
        let legacy = r#"{"name":"DP-1","crtc_id":386,"x":0,"y":0,
                         "width":3840,"height":2160,"active":true}"#;
        let info: DrmDisplayInfo =
            serde_json::from_str(legacy).expect("a pre-render_node payload must still decode");
        assert_eq!(info.name, "DP-1");
        assert_eq!(info.crtc_id, 386);
        assert!(info.render_node.is_empty(), "missing node; the consumer auto-selects only where there is one render node");
        assert!(info.device.is_empty(), "missing device means auto-detect");

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

    fn pipe() -> (OwnedFd, OwnedFd) {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "pipe() failed");
        unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
    }

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
        let short = Bytes::from(vec![9u8; 10]);
        tx.send_raw(short.clone()).await.unwrap();
        rx.next_raw_into(&mut got).await.unwrap();
        assert_eq!(&got[..], &short[..]);
    }

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

    #[tokio::test]
    async fn a_body_that_never_arrives_times_out() {
        let (mut a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut rx = DrmConn::new(b);
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

    #[tokio::test]
    async fn a_dripping_peer_cannot_re_arm_the_send_deadline() {
        use tokio::io::AsyncReadExt;
        let (mut reader, writer) = tokio::net::UnixStream::pair().unwrap();
        let payload = vec![0u8; 32 * 1024 * 1024];
        // Measured: 1 KiB drains do not re-assert POLLOUT; 64 KiB does, which separates the forms.
        let drip = tokio::spawn(async move {
            let mut sink = vec![0u8; 64 * 1024];
            loop {
                if reader.read(&mut sink).await.unwrap_or(0) == 0 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        });
        let started = std::time::Instant::now();
        let outcome = tokio::time::timeout(
            std::time::Duration::from_millis(DRM_SEND_TIMEOUT_MS * 4),
            drm_write_all(&writer, &payload, None),
        )
        .await;
        drip.abort();
        let inner = outcome.expect(
            "the send deadline did not fire: the budget is being re-armed per readiness wait",
        );
        let err = inner.err().expect("a dripping peer must not complete the write");
        assert!(
            err.to_string().contains("did not accept the remaining"),
            "unexpected error: {err}"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_millis(DRM_SEND_TIMEOUT_MS * 3),
            "took {:?}, which is not the send deadline firing",
            started.elapsed()
        );
    }

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

    // 16 fds need CMSG_LEN(64)=80 > the 64-byte DRM_CMSG_CAP, so the kernel sets MSG_CTRUNC.
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

    #[test]
    fn peer_uid_from_fd_reads_socket_peer() {
        let (a, _b) = std::os::unix::net::UnixStream::pair().unwrap();
        let euid = unsafe { libc::geteuid() };
        assert_eq!(peer_uid_from_fd(a.as_raw_fd()), Some(euid));
    }

    #[test]
    fn drm_peer_authorized_matrix() {
        assert!(drm_peer_authorized(Some(0), Some(1000)));
        assert!(drm_peer_authorized(Some(0), None));
        assert!(drm_peer_authorized(Some(1000), Some(1000)));
        assert!(!drm_peer_authorized(Some(1000), Some(1001)));
        assert!(!drm_peer_authorized(Some(1000), None));
        assert!(!drm_peer_authorized(None, Some(1000)));
        assert!(!drm_peer_authorized(None, None));
    }

    #[test]
    fn accept_time_exe_match_accepts_only_our_own_executable() {
        let me = std::process::id();
        assert!(
            super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(Some(me), "_drm").is_ok(),
            "the test process must match its own executable"
        );

        let mut other = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("/bin/sleep should be spawnable in the test environment");
        // Until the child finishes exec'ing, /proc/<pid>/exe still points at OUR binary.
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

        assert!(super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(None, "_drm").is_err());
    }

    #[test]
    fn drm_conn_admission_bound() {
        assert!(drm_conn_admitted(0));
        assert!(drm_conn_admitted(MAX_DRM_CONNS - 1)); // last admitted slot
        assert!(!drm_conn_admitted(MAX_DRM_CONNS)); // cap reached -> rejected
        assert!(!drm_conn_admitted(MAX_DRM_CONNS + 5)); // over cap -> rejected
    }

    #[test]
    fn drm_auth_admission_bound() {
        assert!(drm_auth_admitted(0));
        assert!(drm_auth_admitted(MAX_DRM_AUTH_IN_FLIGHT - 1)); // last admitted slot
        assert!(!drm_auth_admitted(MAX_DRM_AUTH_IN_FLIGHT)); // cap reached -> rejected
        assert!(!drm_auth_admitted(MAX_DRM_AUTH_IN_FLIGHT + 5)); // over cap -> rejected
        assert!(
            MAX_DRM_AUTH_IN_FLIGHT <= MAX_DRM_CONNS,
            "the pre-auth bound must not be looser than the connection cap"
        );
    }
}
