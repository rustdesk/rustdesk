// Controlled side of a `RemoteUsb` session: device listing/bind (via the
// system `usbip` CLI) and the byte relay for the actual USB/IP TCP stream,
// tunneled through `UsbChannel` frames. Mirrors the shape of
// `port_forward_mux.rs`'s `PortForwardMux`, but without its flow-control
// windowing -- USB/IP traffic here is bursty control + bulk transfers, not a
// generic proxied protocol that needs backpressure tuning.
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
    inbound: mpsc::UnboundedSender<Bytes>,
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
                    let devices = tokio::task::spawn_blocking(list_local_devices)
                        .await
                        .unwrap_or_default();
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
                    let ok = tokio::task::spawn_blocking(move || bind_device(&b.bus_id, b.bind))
                        .await
                        .unwrap_or(false);
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
                if entry.inbound.send(d.data).is_err() {
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
        self.channels.retain(|_, e| !e.inbound.is_closed());
        if !permitted {
            self.reply(opened_msg(id, false, "No permission of USB forwarding".into()));
            return;
        }
        if self.channels.contains_key(&id) {
            log::debug!("ignoring open for live usb channel {}", id);
            return;
        }
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
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
async fn run_channel(id: i32, mut inbound: mpsc::UnboundedReceiver<Bytes>, tx: Sender) {
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

static USB_DEVICE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"busid=([0-9]+-[0-9.]+)#usbid=([0-9a-fA-F]{4}):([0-9a-fA-F]{4})#").unwrap());

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
    stdout
        .lines()
        .filter_map(|line| USB_DEVICE_RE.captures(line))
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

/// Blocking; call via `spawn_blocking`. Bind/unbind needs root, so it goes
/// through the same on-demand privilege-elevation prompt the service
/// install/uninstall path already uses.
fn bind_device(bus_id: &str, bind: bool) -> bool {
    let sub_cmd = if bind { "bind" } else { "unbind" };
    crate::platform::run_cmds_privileged(&format!("usbip {} -b {}", sub_cmd, bus_id))
}
