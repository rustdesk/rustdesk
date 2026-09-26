// Controlled side of a `RemoteUsb` session: device listing/bind (via the
// system `usbip` CLI) and the byte relay for the actual USB/IP TCP stream,
// tunneled through `UsbChannel` frames. Mirrors the shape of
// `port_forward_mux.rs`'s `PortForwardMux`, with per-channel windows in both
// directions (`crate::usbip_flow`). A peer that overruns its window is
// closed rather than having frames dropped mid-stream, which would corrupt
// the byte stream a USB/IP connection actually is.
use super::connection::Sender;
use crate::usbip_flow::{self, Flow};
use base::message_proto::*;
use hbb_common::{
    bytes::Bytes,
    log, timeout,
    regex::Regex,
    tokio::{
        self,
        io::AsyncWriteExt,
        net::TcpStream,
        sync::{mpsc, Semaphore},
    },
};
use std::{
    collections::{HashMap, HashSet},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

const USBIPD_ADDR: &str = "127.0.0.1:3240";
const CONNECT_TIMEOUT_MS: u64 = 3000;
const USBIP_HOST_DRIVER_DIR: &str = "/sys/bus/usb/drivers/usbip-host";
// Caps how many relay tasks (and their `usbipd` TCP connections) a single
// permitted peer can make us spawn; with `usbip_flow::CHANNEL_WINDOW` per
// channel it also bounds what one connection can make us buffer.
const MAX_LIVE_CHANNELS: usize = 32;

fn usb_channel_msg(union: usb_channel::Union) -> Message {
    let mut ch = UsbChannel::new();
    ch.union = Some(union);
    let mut msg = Message::new();
    msg.set_usb_channel(ch);
    msg
}

fn device_list_msg(devices: Vec<UsbDevice>) -> Message {
    usb_channel_msg(usb_channel::Union::DeviceList(UsbDeviceList {
        devices,
        ..Default::default()
    }))
}

fn bind_result_msg(bus_id: String, bind: bool, error: String) -> Message {
    usb_channel_msg(usb_channel::Union::BindResult(UsbBindResult {
        bus_id,
        bind,
        error,
        ..Default::default()
    }))
}

fn opened_msg(channel_id: i32, success: bool, message: String) -> Message {
    usb_channel_msg(usb_channel::Union::Opened(UsbForwardOpened {
        channel_id,
        success,
        message,
        ..Default::default()
    }))
}

fn data_msg(channel_id: i32, data: Bytes) -> Message {
    usb_channel_msg(usb_channel::Union::Data(UsbForwardData {
        channel_id,
        data,
        ..Default::default()
    }))
}

fn close_msg(channel_id: i32) -> Message {
    usb_channel_msg(usb_channel::Union::Close(UsbForwardClose {
        channel_id,
        ..Default::default()
    }))
}

struct Entry {
    inbound: mpsc::Sender<Bytes>,
    flow: Flow,
}

/// The controlled side of one `RemoteUsb` session. The connection's main loop
/// owns it and forwards every `UsbChannel` frame here; each forwarded
/// USB/IP TCP connection is its own task.
pub struct UsbipMux {
    channels: HashMap<i32, Entry>,
    tx: Sender,
    /// Bus ids this connection's `Bind` requests shared and have not been
    /// unshared yet; `None` once `close_all` has released them.
    bound: Arc<Mutex<Option<HashSet<String>>>>,
    /// Shared with `usbip_pull` (`UsbSession::new`).
    privileged: Arc<Semaphore>,
    listing: Arc<AtomicBool>,
}

impl UsbipMux {
    pub fn new(tx: Sender, privileged: Arc<Semaphore>) -> Self {
        Self {
            channels: HashMap::new(),
            tx,
            bound: Arc::new(Mutex::new(Some(HashSet::new()))),
            privileged,
            listing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// `permitted` is consulted for `list_devices`/`bind`/`open` alone, not
    /// per data frame.
    pub fn handle(&mut self, frame: UsbChannel, permitted: impl FnOnce() -> bool) {
        match frame.union {
            Some(usb_channel::Union::ListDevices(_)) => {
                if !permitted() {
                    self.reply(device_list_msg(Vec::new()));
                    return;
                }
                // The listing already running answers this request too.
                if self.listing.swap(true, Ordering::SeqCst) {
                    return;
                }
                let tx = self.tx.clone();
                let listing = self.listing.clone();
                tokio::spawn(async move {
                    let devices = match tokio::task::spawn_blocking(list_local_devices).await {
                        Ok(devices) => devices,
                        Err(err) => {
                            log::error!("usb forward: blocking task failed: {}", err);
                            Vec::new()
                        }
                    };
                    listing.store(false, Ordering::SeqCst);
                    send(&tx, device_list_msg(devices));
                });
            }
            Some(usb_channel::Union::Bind(b)) => {
                if !permitted() {
                    self.reply(bind_result_msg(
                        b.bus_id,
                        b.bind,
                        "No permission of USB forwarding".into(),
                    ));
                    return;
                }
                // The peer may only pull what this session shared (`on_open`),
                // so a device the CLI or another session shared is refused
                // rather than adopted, and an unshare request leaves it shared.
                if is_bound(&self.bound, &b.bus_id) == b.bind {
                    self.reply(bind_result_msg(b.bus_id, b.bind, String::new()));
                    return;
                }
                if b.bind && shared_bus_ids().contains(&b.bus_id) {
                    log::info!("usbip: {} is shared outside this session, refusing", b.bus_id);
                    self.reply(bind_result_msg(
                        b.bus_id,
                        b.bind,
                        "The USB device is already shared outside this session".into(),
                    ));
                    return;
                }
                if self.bound.lock().unwrap().is_none() {
                    self.reply(bind_result_msg(b.bus_id, b.bind, "Session is closing".into()));
                    return;
                }
                let Ok(permit) = self.privileged.clone().try_acquire_owned() else {
                    self.reply(bind_result_msg(
                        b.bus_id,
                        b.bind,
                        "Another USB operation is in progress".into(),
                    ));
                    return;
                };
                let tx = self.tx.clone();
                let bound = self.bound.clone();
                tokio::spawn(async move {
                    let bus_id = b.bus_id.clone();
                    let bind = b.bind;
                    let ok = match tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        if bound.lock().unwrap().is_none() {
                            return false;
                        }
                        let ok = bind_device_when_released(&b.bus_id, b.bind);
                        if ok && !record_binding(&bound, &b.bus_id, b.bind) {
                            log::info!("usbip: session closed while sharing {}, unsharing", b.bus_id);
                            bind_device_when_released(&b.bus_id, false);
                            return false;
                        }
                        ok
                    })
                    .await
                    {
                        Ok(ok) => ok,
                        Err(err) => {
                            log::error!("usb forward: blocking task failed: {}", err);
                            false
                        }
                    };
                    let error = if ok {
                        String::new()
                    } else {
                        format!("Failed to {} {}", if bind { "share" } else { "unshare" }, bus_id)
                    };
                    send(&tx, bind_result_msg(bus_id, bind, error));
                });
            }
            Some(usb_channel::Union::Open(open)) => {
                let permitted = permitted();
                self.on_open(open, permitted);
            }
            Some(usb_channel::Union::Data(d)) => {
                let Some(entry) = self.channels.get(&d.channel_id) else {
                    log::debug!("usb forward data for unknown channel {}", d.channel_id);
                    return;
                };
                if !entry.flow.admit(d.data.len()) {
                    log::warn!(
                        "usb forward: data frame ({} bytes) over the window on channel {}, closing",
                        d.data.len(),
                        d.channel_id
                    );
                    self.channels.remove(&d.channel_id);
                    // Otherwise the peer never learns we gave up on this
                    // channel and keeps its own side (and relay task) alive
                    // until the whole session ends.
                    self.reply(close_msg(d.channel_id));
                    return;
                }
                if entry.inbound.try_send(d.data).is_err() {
                    // Only a relay task that already ended: an admitted frame
                    // always fits (`usbip_flow::QUEUE_FRAMES`).
                    self.channels.remove(&d.channel_id);
                    self.reply(close_msg(d.channel_id));
                }
            }
            Some(usb_channel::Union::WindowUpdate(w)) => {
                if let Some(entry) = self.channels.get(&w.channel_id) {
                    entry.flow.grant(w.add);
                }
            }
            Some(usb_channel::Union::Close(c)) => {
                self.channels.remove(&c.channel_id);
            }
            _ => {}
        }
    }

    fn on_open(&mut self, open: UsbForwardOpen, permitted: bool) {
        let id = open.channel_id;
        if id < 0 {
            // Negative ids are the push direction's own id space
            // (`usbip_pull.rs`); an `Open` here only ever means a peer
            // pulling one of our shared devices, which always uses
            // non-negative ids (`UsbClientState::next_channel_id`).
            // Reject before creating any channel/task: `connection.rs`'s
            // sign dispatch would route this id's later Data/Close frames to
            // `usbip_pull` instead of back here, orphaning the relay task
            // and its TCP connection to the local usbipd forever.
            log::warn!("usb forward: rejecting open with non-pull channel id {}", id);
            return;
        }
        self.channels.retain(|_, e| !e.inbound.is_closed());
        if !permitted {
            self.reply(opened_msg(id, false, "No permission of USB forwarding".into()));
            return;
        }
        if !is_bound(&self.bound, &open.bus_id) {
            log::warn!(
                "usb forward: rejecting open of {:?}, not shared by this session",
                open.bus_id
            );
            self.reply(opened_msg(
                id,
                false,
                "USB device not shared by this session".into(),
            ));
            return;
        }
        if self.channels.contains_key(&id) {
            log::debug!("ignoring open for live usb channel {}", id);
            return;
        }
        if self.channels.len() >= MAX_LIVE_CHANNELS {
            log::warn!("usb forward: rejecting open, {} channels already live", self.channels.len());
            self.reply(opened_msg(id, false, "Too many open USB channels".into()));
            return;
        }
        let (inbound_tx, inbound_rx) = mpsc::channel(usbip_flow::QUEUE_FRAMES);
        let flow = Flow::new();
        self.channels.insert(
            id,
            Entry {
                inbound: inbound_tx,
                flow: flow.clone(),
            },
        );
        tokio::spawn(run_channel(id, open.bus_id, inbound_rx, self.tx.clone(), flow));
    }

    fn reply(&self, msg: Message) {
        send(&self.tx, msg);
    }

    /// Drops every channel's inbound sender, ending its relay task, and
    /// unshares the devices this connection shared.
    pub fn close_all(&mut self) {
        self.channels.clear();
        let bound = self.bound.lock().unwrap().take().unwrap_or_default();
        if bound.is_empty() {
            return;
        }
        let privileged = self.privileged.clone();
        tokio::spawn(async move {
            // Waits rather than gives up: a binding left behind stays shared.
            let permit = privileged.acquire_owned().await;
            let _ = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                for bus_id in bound {
                    if !bind_device_when_released(&bus_id, false) {
                        log::error!("usbip: failed to unshare {} on session close", bus_id);
                    }
                }
            })
            .await;
        });
    }
}

fn send(tx: &Sender, msg: Message) {
    tx.send((tokio::time::Instant::now(), Arc::new(msg))).ok();
}

/// One forwarded USB/IP TCP connection: dial the local `usbipd`, relay bytes
/// both ways until either side closes.
/// `bus_id` is the device `on_open` authorized; the `OP_REQ_IMPORT` the peer
/// sends must name it, or nothing reaches `usbipd`.
async fn run_channel(
    id: i32,
    bus_id: String,
    mut inbound: mpsc::Receiver<Bytes>,
    tx: Sender,
    flow: Flow,
) {
    let socket = match timeout(CONNECT_TIMEOUT_MS, TcpStream::connect(USBIPD_ADDR)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            send(&tx, opened_msg(id, false, format!("usbipd connect failed: {}", e)));
            return;
        }
        Err(e) => {
            send(&tx, opened_msg(id, false, format!("usbipd connect timed out: {}", e)));
            return;
        }
    };
    if !send_result(&tx, opened_msg(id, true, String::new())) {
        return;
    }
    let (reader, mut writer) = socket.into_split();
    let mut prefix = Vec::with_capacity(usbip_flow::USBIP_OP_REQ_IMPORT_LEN);
    // Each frame was charged separately against the peer's window, so its
    // credit is returned per frame too.
    let mut prefix_frames = Vec::new();
    while prefix.len() < usbip_flow::USBIP_OP_REQ_IMPORT_LEN {
        match timeout(usbip_flow::IMPORT_REQUEST_TIMEOUT_MS, inbound.recv()).await {
            Ok(Some(chunk)) => {
                prefix_frames.push(chunk.len());
                prefix.extend_from_slice(&chunk);
            }
            Ok(None) => return,
            Err(_) => {
                log::warn!("usb forward: channel {} import request timed out, closing", id);
                send(&tx, close_msg(id));
                return;
            }
        }
    }
    let requested = usbip_flow::parse_import_request_busid(&prefix);
    if requested.as_deref() != Some(bus_id.as_str()) {
        log::error!(
            "usb forward: channel {} import request {:?} does not match authorized {:?}, refusing",
            id, requested, bus_id
        );
        send(&tx, close_msg(id));
        return;
    }
    if writer.write_all(&prefix).await.is_err() {
        send(&tx, close_msg(id));
        return;
    }
    for len in prefix_frames {
        if let Some(add) = flow.drained(len) {
            send(&tx, usbip_flow::window_update_msg(id, add));
        }
    }
    let tx_read = tx.clone();
    let flow_read = flow.clone();
    let mut to_tunnel = tokio::spawn(async move {
        flow_read
            .socket_to_peer(reader, |chunk| send_result(&tx_read, data_msg(id, chunk)))
            .await;
    });
    // Whichever half ends first ends the other; only an end on our side
    // (local EOF or write error) needs a Close, the peer knows about its own.
    let local_ended = loop {
        tokio::select! {
            _ = &mut to_tunnel => break true,
            chunk = inbound.recv() => {
                let Some(chunk) = chunk else { break false };
                if writer.write_all(&chunk).await.is_err() {
                    break true;
                }
                if let Some(add) = flow.drained(chunk.len()) {
                    send(&tx, usbip_flow::window_update_msg(id, add));
                }
            }
        }
    };
    to_tunnel.abort();
    if local_ended {
        send(&tx, close_msg(id));
    }
}

fn send_result(tx: &Sender, msg: Message) -> bool {
    tx.send((tokio::time::Instant::now(), Arc::new(msg))).is_ok()
}

// `Option`, not `Regex` directly -- see the identical comment in
// `client/usbip_attach.rs`.
lazy_static::lazy_static! {
    static ref USB_DEVICE_RE: Option<Regex> =
        Regex::new(r"busid=([0-9]+-[0-9.]+)#usbid=([0-9a-fA-F]{4}):([0-9a-fA-F]{4})#")
            .map_err(|err| log::error!("usbip: invalid USB_DEVICE_RE: {}", err))
            .ok();
}

/// Debian/Ubuntu install `usbip` under `/usr/sbin`, which is on root's PATH
/// but not a regular desktop user's -- widen it so a plain `Command::new`
/// can still find the binary when RustDesk runs unprivileged.
fn usbip_command() -> Command {
    let mut cmd = Command::new("usbip");
    let path = std::env::var("PATH").unwrap_or_default();
    cmd.env("PATH", format!("{path}:/usr/sbin:/sbin:/usr/local/sbin"));
    cmd
}

/// Runs `usbip list -p -l` and cross-references `/sys/.../usbip-host` to
/// report which devices are already shared. Blocking; call via
/// `spawn_blocking`.
fn list_local_devices() -> Vec<UsbDevice> {
    let output = match usbip_command().args(["list", "-p", "-l"]).output() {
        Ok(o) => o,
        Err(err) => {
            log::error!("usbip list failed: {}", err);
            return Vec::new();
        }
    };
    let shared = shared_bus_ids();
    let stdout = String::from_utf8_lossy(&output.stdout);
    log::debug!(
        "usbip list status={:?} stdout={:?} stderr={:?}",
        output.status,
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
    parse_local_devices(&stdout, &shared)
}

/// Pure text parsing half of `list_local_devices`, split out for testing
/// without a real `usbip`/sysfs on the machine running the tests.
fn parse_local_devices(stdout: &str, shared: &std::collections::HashSet<String>) -> Vec<UsbDevice> {
    let Some(device_re) = USB_DEVICE_RE.as_ref() else {
        return Vec::new();
    };
    stdout
        .lines()
        .filter_map(|line| device_re.captures(line))
        .map(|caps| {
            let bus_id = caps[1].to_string();
            UsbDevice {
                shared: shared.contains(&bus_id),
                bus_id,
                vendor: caps[2].to_string(),
                product: caps[3].to_string(),
                attached_port: -1,
                ..Default::default()
            }
        })
        .collect()
}

/// Bus ids currently bound to the `usbip-host` driver, read from sysfs
/// instead of parsing free-text `usbip list` output.
fn shared_bus_ids() -> std::collections::HashSet<String> {
    let Ok(entries) = std::fs::read_dir(USBIP_HOST_DRIVER_DIR) else {
        return Default::default();
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_symlink())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .collect()
}

/// Linux USB bus ids are `<bus>-<port>[.<port>...]` (e.g. "1-2.3"), shorter
/// than the kernel's 32-byte `SYSFS_BUS_ID_SIZE`. Bus ids reaching this side
/// come from the peer (`Bind`, `PushRequest`) and end up in privileged
/// `usbip` commands, so every one must pass this check first.
pub(super) fn is_valid_bus_id(bus_id: &str) -> bool {
    if bus_id.len() >= 32 {
        return false;
    }
    let Some((bus, ports)) = bus_id.split_once('-') else {
        return false;
    };
    let is_number = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    is_number(bus) && ports.split('.').all(is_number)
}

/// Blocking; call via `spawn_blocking`. Bind/unbind needs root, so it goes
/// through the same on-demand privilege-elevation prompt the service
/// install/uninstall path already uses.
fn bind_device(bus_id: &str, bind: bool) -> bool {
    if !is_valid_bus_id(bus_id) {
        log::error!("usbip: rejected malformed bus id {:?}", bus_id);
        return false;
    }
    let sub_cmd = if bind { "bind" } else { "unbind" };
    crate::platform::run_usbip_privileged(&[sub_cmd, "-b", bus_id])
}

/// Blocking; call via `spawn_blocking`. The puller's combined "detach and
/// unshare" sends its unshare request right after detaching, but the
/// detach's own relay teardown here (`run_channel`'s reader noticing EOF
/// once the puller's kernel-level detach closes things) happens
/// asynchronously -- so an unbind request can race the relay that's still
/// holding the device "in use" for a brief moment. Wait for `usbip-host` to
/// release it (an unprivileged sysfs read) instead of retrying the
/// privileged unbind, each attempt of which would be another password prompt.
fn bind_device_when_released(bus_id: &str, bind: bool) -> bool {
    if !bind && is_valid_bus_id(bus_id) {
        let status = std::path::Path::new(USBIP_HOST_DRIVER_DIR)
            .join(bus_id)
            .join("usbip_status");
        for attempt in 1..=10 {
            // `SDEV_ST_USED`: a peer still has it imported.
            if std::fs::read_to_string(&status).map_or(true, |s| s.trim() != "2") {
                break;
            }
            log::debug!("usbip: {} still in use, waiting {}/10", bus_id, attempt);
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    bind_device(bus_id, bind)
}

fn is_bound(bound: &Mutex<Option<HashSet<String>>>, bus_id: &str) -> bool {
    bound
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|set| set.contains(bus_id))
}

/// Records a successful bind/unbind. `false` if the session already closed,
/// in which case a new binding must be undone by the caller.
fn record_binding(bound: &Mutex<Option<HashSet<String>>>, bus_id: &str, bind: bool) -> bool {
    let mut bound = bound.lock().unwrap();
    let Some(set) = bound.as_mut() else {
        return !bind;
    };
    if bind {
        set.insert(bus_id.to_string());
    } else {
        set.remove(bus_id);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const USBIP_LIST_P_L_OUTPUT: &str = "\
 - busid 1-3 (04f3:0c4f)
   Elan Microelectronics Corp. : unknown product (04f3:0c4f)
busid=1-3#usbid=04f3:0c4f#Elan Microelectronics Corp.#unknown product#
 - busid 2-2 (0dd8:3801)
   Netac Technology Co., Ltd : unknown product (0dd8:3801)
busid=2-2#usbid=0dd8:3801#Netac Technology Co., Ltd#unknown product#
";

    #[test]
    fn parse_local_devices_extracts_bus_id_and_ids() {
        let devices = parse_local_devices(USBIP_LIST_P_L_OUTPUT, &HashSet::new());
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].bus_id, "1-3");
        assert_eq!(devices[0].vendor, "04f3");
        assert_eq!(devices[0].product, "0c4f");
        assert_eq!(devices[1].bus_id, "2-2");
        assert_eq!(devices[1].vendor, "0dd8");
        assert_eq!(devices[1].product, "3801");
    }

    #[test]
    fn parse_local_devices_marks_shared_from_sysfs_set() {
        let shared: HashSet<String> = ["2-2".to_string()].into_iter().collect();
        let devices = parse_local_devices(USBIP_LIST_P_L_OUTPUT, &shared);
        let by_bus_id = |id: &str| devices.iter().find(|d| d.bus_id == id).unwrap();
        assert!(!by_bus_id("1-3").shared);
        assert!(by_bus_id("2-2").shared);
    }

    #[test]
    fn parse_local_devices_empty_output() {
        assert!(parse_local_devices("", &HashSet::new()).is_empty());
    }

    #[test]
    fn is_valid_bus_id_accepts_normal_bus_ids() {
        assert!(is_valid_bus_id("18-1"));
        assert!(is_valid_bus_id("1-2.3"));
    }

    #[test]
    fn is_valid_bus_id_rejects_shell_metacharacters() {
        assert!(!is_valid_bus_id(""));
        assert!(!is_valid_bus_id("1-1; touch /etc/x"));
        assert!(!is_valid_bus_id("1-1 && rm -rf /"));
        assert!(!is_valid_bus_id("$(id)"));
        assert!(!is_valid_bus_id("../etc/passwd"));
    }

    #[test]
    fn is_valid_bus_id_rejects_malformed_and_overlong() {
        assert!(!is_valid_bus_id("1"));
        assert!(!is_valid_bus_id("1-"));
        assert!(!is_valid_bus_id("-1"));
        assert!(!is_valid_bus_id("1-2..3"));
        assert!(!is_valid_bus_id("1.2-3"));
        assert!(!is_valid_bus_id("1-2-3"));
        assert!(!is_valid_bus_id(&format!("1-{}", "1.".repeat(15) + "1")));
    }

    #[test]
    fn unbind_of_device_not_shared_here_leaves_it_shared() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut mux = UsbipMux::new(tx, Arc::new(Semaphore::new(1)));
        let mut frame = UsbChannel::new();
        frame.union = Some(usb_channel::Union::Bind(UsbBind {
            bus_id: "1-2".into(),
            bind: false,
            ..Default::default()
        }));
        mux.handle(frame, || true);
        let (_, msg) = rx.try_recv().unwrap();
        match &msg.union {
            Some(message::Union::UsbChannel(ch)) => match &ch.union {
                // Answered right away: no `usbip unbind` was started for it.
                Some(usb_channel::Union::BindResult(r)) => assert!(r.error.is_empty()),
                _ => panic!("expected a BindResult"),
            },
            _ => panic!("expected a UsbChannel message"),
        }
    }

    #[test]
    fn record_binding_after_close_asks_caller_to_undo_a_bind() {
        let bound = Mutex::new(Some(HashSet::new()));
        assert!(record_binding(&bound, "1-2", true));
        assert!(is_bound(&bound, "1-2"));
        assert!(record_binding(&bound, "1-2", false));
        assert!(!is_bound(&bound, "1-2"));
        bound.lock().unwrap().take();
        assert!(!record_binding(&bound, "1-2", true));
        assert!(record_binding(&bound, "1-2", false));
    }

    fn bind_reply(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<(tokio::time::Instant, Arc<Message>)>,
    ) -> UsbBindResult {
        let (_, msg) = rx.try_recv().unwrap();
        match &msg.union {
            Some(message::Union::UsbChannel(ch)) => match &ch.union {
                Some(usb_channel::Union::BindResult(r)) => r.clone(),
                _ => panic!("expected a BindResult"),
            },
            _ => panic!("expected a UsbChannel message"),
        }
    }

    fn bind_frame(bus_id: &str) -> UsbChannel {
        let mut frame = UsbChannel::new();
        frame.union = Some(usb_channel::Union::Bind(UsbBind {
            bus_id: bus_id.into(),
            bind: true,
            ..Default::default()
        }));
        frame
    }

    #[test]
    fn bind_while_another_privileged_step_runs_is_refused() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let privileged = Arc::new(Semaphore::new(1));
        let mut mux = UsbipMux::new(tx, privileged.clone());
        let _busy = privileged.try_acquire_owned().unwrap();
        mux.handle(bind_frame("99-9"), || true);
        assert!(!bind_reply(&mut rx).error.is_empty());
    }

    #[test]
    fn bind_after_close_is_refused() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let privileged = Arc::new(Semaphore::new(1));
        let mut mux = UsbipMux::new(tx, privileged.clone());
        mux.close_all();
        mux.handle(bind_frame("99-9"), || true);
        assert!(!bind_reply(&mut rx).error.is_empty());
        assert_eq!(privileged.available_permits(), 1);
    }

    #[test]
    fn on_open_rejects_device_not_shared_by_this_session() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut mux = UsbipMux::new(tx, Arc::new(Semaphore::new(1)));
        mux.on_open(
            UsbForwardOpen {
                channel_id: 1,
                bus_id: "1-1".into(),
                ..Default::default()
            },
            true,
        );
        assert!(mux.channels.is_empty());
        let (_, msg) = rx.try_recv().unwrap();
        match &msg.union {
            Some(message::Union::UsbChannel(ch)) => match &ch.union {
                Some(usb_channel::Union::Opened(o)) => assert!(!o.success),
                _ => panic!("expected an Opened"),
            },
            _ => panic!("expected a UsbChannel message"),
        }
    }

    #[test]
    fn on_open_rejects_negative_channel_id() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut mux = UsbipMux::new(tx, Arc::new(Semaphore::new(1)));
        mux.on_open(
            UsbForwardOpen {
                channel_id: -1,
                bus_id: "1-1".into(),
                ..Default::default()
            },
            true,
        );
        assert!(mux.channels.is_empty());
    }
}
