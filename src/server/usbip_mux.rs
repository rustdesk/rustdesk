// Controlled side of a `RemoteUsb` session: device listing/bind (via the
// system `usbip` CLI) and the byte relay for the actual USB/IP TCP stream,
// tunneled through `UsbChannel` frames. Mirrors the shape of
// `port_forward_mux.rs`'s `PortForwardMux`, but without its protocol-level
// flow-control windowing -- USB/IP traffic here is bursty control + bulk
// transfers, not a generic proxied protocol that needs backpressure tuning.
// The per-channel relay queue is still capacity-bounded (`RELAY_CHANNEL_CAPACITY`
// below), so a peer that keeps sending faster than the local socket drains
// can't grow it without bound; once full, the channel is torn down rather
// than silently dropping frames mid-stream (which would corrupt the byte
// stream a USB/IP connection actually is).
use super::connection::Sender;
use base::message_proto::*;
use hbb_common::{
    bytes::Bytes,
    log, timeout,
    regex::Regex,
    tokio::{
        self,
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
        sync::mpsc,
    },
};
use std::{
    collections::HashMap,
    process::Command,
    sync::{Arc, LazyLock},
};

const USBIPD_ADDR: &str = "127.0.0.1:3240";
const CONNECT_TIMEOUT_MS: u64 = 3000;
const USBIP_HOST_DRIVER_DIR: &str = "/sys/bus/usb/drivers/usbip-host";
// Each queued chunk is up to 64KiB (`run_channel`'s read buffer size), so
// this bounds one relay channel to a few MiB, not unbounded process memory.
const RELAY_CHANNEL_CAPACITY: usize = 256;
// `run_channel` never reads more than this per frame; a larger `Data.data`
// is a protocol violation (the generic message framing allows up to ~1GiB,
// which would otherwise let a peer balloon a single queued chunk far past
// what `RELAY_CHANNEL_CAPACITY` alone bounds).
const MAX_USB_DATA_LEN: usize = 64 * 1024;
// Mirrors `port_forward_mux::MAX_CHANNELS` -- caps how many relay tasks (and
// their `usbipd` TCP connections) a single permitted peer can make us spawn.
const MAX_LIVE_CHANNELS: usize = 256;

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
}

/// The controlled side of one `RemoteUsb` session. The connection's main loop
/// owns it and forwards every `UsbChannel` frame here; each forwarded
/// USB/IP TCP connection is its own task.
pub struct UsbipMux {
    channels: HashMap<i32, Entry>,
    tx: Sender,
}

impl UsbipMux {
    pub fn new(tx: Sender) -> Self {
        Self {
            channels: HashMap::new(),
            tx,
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
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let devices = match tokio::task::spawn_blocking(list_local_devices).await {
                        Ok(devices) => devices,
                        Err(err) => {
                            log::error!("usb forward: blocking task failed: {}", err);
                            Vec::new()
                        }
                    };
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
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let bus_id = b.bus_id.clone();
                    let bind = b.bind;
                    let ok = match tokio::task::spawn_blocking(move || {
                        bind_device_retrying(&b.bus_id, b.bind)
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
                if d.data.len() > MAX_USB_DATA_LEN {
                    log::warn!(
                        "usb forward: oversized data frame ({} bytes) on channel {}, closing",
                        d.data.len(),
                        d.channel_id
                    );
                    self.channels.remove(&d.channel_id);
                    return;
                }
                if entry.inbound.try_send(d.data).is_err() {
                    // Full (peer outrunning the local socket) or closed --
                    // either way this channel can't keep relaying faithfully,
                    // so drop it instead of growing the queue or silently
                    // losing bytes out of the stream.
                    self.channels.remove(&d.channel_id);
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
            // non-negative ids (`FlutterHandler::next_usb_channel_id`).
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
        if self.channels.contains_key(&id) {
            log::debug!("ignoring open for live usb channel {}", id);
            return;
        }
        if self.channels.len() >= MAX_LIVE_CHANNELS {
            log::warn!("usb forward: rejecting open, {} channels already live", self.channels.len());
            self.reply(opened_msg(id, false, "Too many open USB channels".into()));
            return;
        }
        let (inbound_tx, inbound_rx) = mpsc::channel(RELAY_CHANNEL_CAPACITY);
        self.channels.insert(id, Entry { inbound: inbound_tx });
        tokio::spawn(run_channel(id, inbound_rx, self.tx.clone()));
    }

    fn reply(&self, msg: Message) {
        send(&self.tx, msg);
    }

    /// Drops every channel's inbound sender, ending its relay task.
    pub fn close_all(&mut self) {
        self.channels.clear();
    }
}

fn send(tx: &Sender, msg: Message) {
    tx.send((tokio::time::Instant::now(), Arc::new(msg))).ok();
}

/// One forwarded USB/IP TCP connection: dial the local `usbipd`, relay bytes
/// both ways until either side closes.
async fn run_channel(id: i32, mut inbound: mpsc::Receiver<Bytes>, tx: Sender) {
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
    let (mut reader, mut writer) = socket.into_split();
    let tx_read = tx.clone();
    let to_tunnel = tokio::spawn(async move {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => {
                    send(&tx_read, close_msg(id));
                    return;
                }
                Ok(n) => {
                    if !send_result(&tx_read, data_msg(id, Bytes::copy_from_slice(&buf[..n]))) {
                        return;
                    }
                }
            }
        }
    });
    while let Some(chunk) = inbound.recv().await {
        if writer.write_all(&chunk).await.is_err() {
            break;
        }
    }
    to_tunnel.abort();
}

fn send_result(tx: &Sender, msg: Message) -> bool {
    tx.send((tokio::time::Instant::now(), Arc::new(msg))).is_ok()
}

// `Option`, not `Regex` directly -- see the identical comment in
// `client/usbip_attach.rs`.
static USB_DEVICE_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"busid=([0-9]+-[0-9.]+)#usbid=([0-9a-fA-F]{4}):([0-9a-fA-F]{4})#")
        .map_err(|err| log::error!("usbip: invalid USB_DEVICE_RE: {}", err))
        .ok()
});

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

/// `usbip` bus ids are digits, `-`, and `.` only (e.g. "1-2.3"). `bus_id`
/// here comes straight from the peer's `Bind` request and is interpolated
/// into a root-privileged shell command below, so anything else must be
/// rejected before it gets near the shell.
fn is_valid_bus_id(bus_id: &str) -> bool {
    !bus_id.is_empty()
        && bus_id
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == '.')
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
    crate::platform::run_cmds_privileged(&format!("usbip {} -b {}", sub_cmd, bus_id))
}

/// Blocking; call via `spawn_blocking`. The puller's combined "detach and
/// unshare" sends its unshare request right after detaching, but the
/// detach's own relay teardown here (`run_channel`'s reader noticing EOF
/// once the puller's kernel-level detach closes things) happens
/// asynchronously, not synchronously with the puller's detach call -- so an
/// unbind request can legitimately race the relay that's still holding the
/// device "in use" for a brief moment. Retry rather than surface a spurious
/// error for that race; a share request isn't subject to the same race, so
/// it fails fast.
fn bind_device_retrying(bus_id: &str, bind: bool) -> bool {
    for attempt in 1..=10 {
        if bind_device(bus_id, bind) {
            return true;
        }
        if bind {
            return false;
        }
        log::debug!("usbip: {} still busy unsharing, retry {}/10", bus_id, attempt);
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    false
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
    fn on_open_rejects_negative_channel_id() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let mut mux = UsbipMux::new(tx);
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
