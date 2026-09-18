// Controlled side of a `RemoteUsb` *push*: the controller offers one of its
// own devices (`UsbPushRequest`), and we pull it in headlessly -- the exact
// mirror of `client/usbip_attach.rs`'s `attach()`, just replying over
// `Sender`/`UsbChannel` (there is no `Session<FlutterHandler>`/UI here)
// instead of `push_event_`, and with its own negative `channel_id` space so
// it can't collide with `usbip_mux.rs`'s (see `channel_id` sign convention
// in `connection.rs::handle_usb_channel`).
use super::connection::Sender;
use base::message_proto::*;
use hbb_common::{
    bytes::Bytes, log,
    regex::Regex,
    tokio::{
        self,
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::mpsc,
    },
};
use std::{
    collections::HashMap,
    process::Command,
    sync::{Arc, LazyLock, Mutex},
};

fn usb_channel_msg(union: usb_channel::Union) -> Message {
    let mut ch = UsbChannel::new();
    ch.union = Some(union);
    let mut msg = Message::new();
    msg.set_usb_channel(ch);
    msg
}

fn push_result_msg(bus_id: String, error: String) -> Message {
    usb_channel_msg(usb_channel::Union::PushResult(UsbPushResult {
        bus_id,
        error,
        ..Default::default()
    }))
}

fn open_msg(channel_id: i32, bus_id: String) -> Message {
    usb_channel_msg(usb_channel::Union::Open(UsbForwardOpen {
        channel_id,
        bus_id,
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

fn send(tx: &Sender, msg: Message) {
    tx.send((tokio::time::Instant::now(), Arc::new(msg))).ok();
}

enum Inbound {
    Opened { success: bool, message: String },
    Data(Bytes),
}

struct Entry {
    inbound: mpsc::UnboundedSender<Inbound>,
    /// Local vhci port, once `usbip attach` reports it -- needed to run
    /// `usbip detach -p <port>` when the controller unpushes. Set from the
    /// spawned `pull()` task, which has no `&mut self` access back here.
    attached_port: Arc<Mutex<Option<i32>>>,
}

/// The controlled side of every push offered in one `RemoteUsb` session.
pub struct UsbPullState {
    channels: HashMap<i32, Entry>,
    next_id: i32,
    tx: Sender,
}

impl UsbPullState {
    pub fn new(tx: Sender) -> Self {
        Self {
            channels: HashMap::new(),
            next_id: -1,
            tx,
        }
    }

    pub fn handle_push_request(&mut self, bus_id: String) {
        let tx = self.tx.clone();
        let id = self.next_id;
        self.next_id -= 1;
        log::info!("usb push: peer offered {} on channel {}", bus_id, id);

        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let attached_port = Arc::new(Mutex::new(None));
        self.channels.insert(
            id,
            Entry {
                inbound: inbound_tx,
                attached_port: attached_port.clone(),
            },
        );

        tokio::spawn(pull(id, bus_id, tx, inbound_rx, attached_port));
    }

    /// Routes `Opened`/`Data`/`Close` frames whose `channel_id` belongs to
    /// this side (negative ids only -- non-negative ones are `usbip_mux`'s).
    pub fn handle_data(&mut self, d: UsbForwardData) {
        self.forward(d.channel_id, Inbound::Data(d.data));
    }

    pub fn handle_opened(&mut self, o: UsbForwardOpened) {
        self.forward(
            o.channel_id,
            Inbound::Opened {
                success: o.success,
                message: o.message,
            },
        );
    }

    /// The controller unpushing: detach locally, then drop the channel so
    /// its relay task ends.
    pub fn handle_close(&mut self, c: UsbForwardClose) {
        let Some(entry) = self.channels.remove(&c.channel_id) else {
            return;
        };
        let port = *entry.attached_port.lock().unwrap();
        if let Some(port) = port {
            log::info!("usb push: peer unpushed channel {}, detaching port {}", c.channel_id, port);
            tokio::task::spawn_blocking(move || {
                if crate::platform::run_cmds_privileged(&format!("usbip detach -p {port}")) {
                    log::info!("usb push: detached port {}", port);
                } else {
                    log::error!("usb push: failed to detach port {}", port);
                }
            });
        } else {
            log::info!(
                "usb push: peer unpushed channel {} before it finished attaching",
                c.channel_id
            );
        }
    }

    fn forward(&mut self, channel_id: i32, msg: Inbound) {
        let Some(entry) = self.channels.get(&channel_id) else {
            log::debug!("usb push: frame for unknown channel {}", channel_id);
            return;
        };
        if entry.inbound.send(msg).is_err() {
            self.channels.remove(&channel_id);
        }
    }

    /// Drops every channel's inbound sender, ending its relay task. Doesn't
    /// run `usbip detach` -- an abrupt connection loss shouldn't silently
    /// rip a device out from under whatever's using it locally.
    pub fn close_all(&mut self) {
        self.channels.clear();
    }
}

/// Mirrors `client/usbip_attach.rs::attach()`: bind a local listener to play
/// the role of the controller's `usbipd`, then run `usbip attach` against
/// it while relaying the resulting TCP stream over `channel_id` (`id`, a
/// negative id already reserved in `UsbPullState.channels`).
async fn pull(
    id: i32,
    bus_id: String,
    tx: Sender,
    inbound: mpsc::UnboundedReceiver<Inbound>,
    attached_port: Arc<Mutex<Option<i32>>>,
) {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(err) => {
            log::error!("usb push: failed to bind local listener: {}", err);
            send(&tx, push_result_msg(bus_id, format!("Failed to push: {}", err)));
            return;
        }
    };
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(err) => {
            log::error!("usb push: failed to read local listener port: {}", err);
            send(&tx, push_result_msg(bus_id, format!("Failed to push: {}", err)));
            return;
        }
    };

    let relay_tx = tx.clone();
    let relay_bus_id = bus_id.clone();
    tokio::spawn(async move {
        match listener.accept().await {
            Ok((socket, _)) => run_channel(id, relay_bus_id, socket, relay_tx, inbound).await,
            Err(err) => log::error!("usb push: accept failed: {}", err),
        }
    });

    let attach_bus_id = bus_id.clone();
    let attach_result =
        tokio::task::spawn_blocking(move || usb_attach_privileged(port, &attach_bus_id)).await;
    match attach_result {
        Ok(true) => {
            let local_port = tokio::task::spawn_blocking({
                let bus_id = bus_id.clone();
                move || find_attached_port(&bus_id)
            })
            .await
            .ok()
            .flatten();
            log::info!(
                "usb push: attached {} on local port {:?} (channel {})",
                bus_id, local_port, id
            );
            *attached_port.lock().unwrap() = local_port;
            send(&tx, push_result_msg(bus_id, String::new()));
        }
        Ok(false) | Err(_) => {
            let message = format!("Failed to push {}", bus_id);
            log::error!("usb push: `usbip attach` failed: {}", message);
            send(&tx, push_result_msg(bus_id, message));
        }
    }
}

fn usb_attach_privileged(port: u16, bus_id: &str) -> bool {
    crate::platform::run_cmds_privileged(&format!(
        "usbip -t {port} attach -r 127.0.0.1 -b {bus_id}"
    ))
}

static USB_PORT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Port (\d+):").unwrap());
static USB_PORT_BUS_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\S+)\s+->\s+usbip://").unwrap());

fn usbip_command() -> Command {
    let mut cmd = Command::new("usbip");
    let path = std::env::var("PATH").unwrap_or_default();
    cmd.env("PATH", format!("{path}:/usr/sbin:/sbin:/usr/local/sbin"));
    cmd
}

fn find_attached_port(bus_id: &str) -> Option<i32> {
    let output = usbip_command().arg("port").output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_port: Option<i32> = None;
    for line in stdout.lines() {
        if let Some(caps) = USB_PORT_RE.captures(line) {
            current_port = caps[1].parse().ok();
            continue;
        }
        if let Some(caps) = USB_PORT_BUS_ID_RE.captures(line) {
            if &caps[1] == bus_id {
                return current_port;
            }
        }
    }
    None
}

async fn run_channel(
    id: i32,
    bus_id: String,
    socket: TcpStream,
    tx: Sender,
    mut inbound: mpsc::UnboundedReceiver<Inbound>,
) {
    send(&tx, open_msg(id, bus_id));

    let success = loop {
        match inbound.recv().await {
            Some(Inbound::Opened { success, message }) => {
                if !success {
                    log::error!("usb push: peer refused channel {}: {}", id, message);
                }
                break success;
            }
            Some(_) => continue,
            None => return,
        }
    };
    if !success {
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
                Ok(n) => send(&tx_read, data_msg(id, Bytes::copy_from_slice(&buf[..n]))),
            }
        }
    });

    while let Some(msg) = inbound.recv().await {
        match msg {
            Inbound::Data(data) => {
                if writer.write_all(&data).await.is_err() {
                    break;
                }
            }
            Inbound::Opened { .. } => break,
        }
    }
    to_tunnel.abort();
}
