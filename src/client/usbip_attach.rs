// Controller side of a `RemoteUsb` session: attaching a remote device means
// running the system `usbip attach`, which makes its own local TCP
// connection that has to land somewhere -- so we stand up a local listener
// that plays the role of the (otherwise unreachable) remote `usbipd`, and
// relay everything it sees through `UsbChannel` frames.
use crate::flutter::FlutterHandler;
use crate::ui_session_interface::Session;
use hbb_common::{
    bytes::Bytes, log, regex::Regex,
    tokio::{
        self,
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        runtime::Runtime,
        sync::mpsc,
    },
};
use serde_json::json;
use std::{
    process::Command,
    sync::{LazyLock, OnceLock},
};

/// `usb_attach`/`usb_detach` are called directly from Flutter's FFI worker
/// pool, which has no ambient Tokio runtime -- `tokio::spawn` there panics
/// with "there is no reactor running". Keep one background runtime alive for
/// the process so those entry points (and the long-lived relay task `attach`
/// spawns) have somewhere to run.
static USB_RUNTIME: OnceLock<Option<Runtime>> = OnceLock::new();

pub(crate) fn usb_runtime() -> Option<&'static Runtime> {
    USB_RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .map_err(|err| log::error!("usb: failed to create background runtime: {}", err))
                .ok()
        })
        .as_ref()
}

pub enum Inbound {
    Opened { success: bool, message: String },
    Data(Bytes),
    Closed,
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

pub async fn attach(session: Session<FlutterHandler>, bus_id: String) {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(err) => {
            log::error!("usb attach: failed to bind local listener: {}", err);
            return;
        }
    };
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(err) => {
            log::error!("usb attach: failed to read local listener port: {}", err);
            return;
        }
    };

    let relay_session = session.clone();
    let relay_bus_id = bus_id.clone();
    tokio::spawn(async move {
        match listener.accept().await {
            Ok((socket, _)) => run_channel(relay_session, relay_bus_id, socket).await,
            Err(err) => log::error!("usb attach: accept failed: {}", err),
        }
    });

    let attach_bus_id = bus_id.clone();
    let attach_result = tokio::task::spawn_blocking(move || {
        usb_attach_privileged(port, &attach_bus_id)
    })
    .await;
    match attach_result {
        Ok(true) => {
            let local_port = tokio::task::spawn_blocking({
                let bus_id = bus_id.clone();
                move || find_attached_port(&bus_id)
            })
            .await
            .ok()
            .flatten();
            session.ui_handler.push_event_(
                "usb_attached",
                &[
                    ("bus_id", json!(bus_id)),
                    ("success", json!(true)),
                    ("port", json!(local_port.unwrap_or(-1))),
                    ("message", json!("")),
                ],
                &[],
                &[],
            );
        }
        Ok(false) | Err(_) => {
            let message = format!("Failed to attach {}", bus_id);
            log::error!("usb attach: `usbip attach` failed: {}", message);
            session.ui_handler.push_event_(
                "usb_attached",
                &[
                    ("bus_id", json!(bus_id)),
                    ("success", json!(false)),
                    ("port", json!(-1)),
                    ("message", json!(message)),
                ],
                &[],
                &[],
            );
        }
    }
}

/// Bind/unbind needs root, so `attach`/`detach` go through the same
/// on-demand privilege-elevation prompt, mirroring
/// `usbip_mux.rs::bind_device`.
fn usb_attach_privileged(port: u16, bus_id: &str) -> bool {
    crate::platform::run_cmds_privileged(&format!(
        "usbip -t {port} attach -r 127.0.0.1 -b {bus_id}"
    ))
}

static USB_PORT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Port (\d+):").unwrap());
static USB_PORT_BUS_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\S+)\s+->\s+usbip://").unwrap());

/// Blocking; call via `spawn_blocking`. `usbip attach` doesn't print the
/// local vhci port it landed on, so ask `usbip port` and match it back up
/// by remote bus id.
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

pub fn detach(port: i32) {
    let Some(rt) = usb_runtime() else {
        return;
    };
    rt.spawn_blocking(move || {
        if !crate::platform::run_cmds_privileged(&format!("usbip detach -p {port}")) {
            log::error!("usb detach: failed to detach port {}", port);
        }
    });
}

async fn run_channel(session: Session<FlutterHandler>, bus_id: String, socket: TcpStream) {
    let id = FlutterHandler::next_usb_channel_id();
    let (tx, mut rx) = mpsc::unbounded_channel::<Inbound>();
    session.ui_handler.register_usb_forward_channel(id, tx);
    session.usb_open_forward(id, bus_id);

    let success = loop {
        match rx.recv().await {
            Some(Inbound::Opened { success, message }) => {
                if !success {
                    log::error!("usb attach: remote refused channel {}: {}", id, message);
                }
                break success;
            }
            Some(_) => continue,
            None => {
                session.ui_handler.unregister_usb_forward_channel(id);
                return;
            }
        }
    };
    if !success {
        session.ui_handler.unregister_usb_forward_channel(id);
        return;
    }

    let (mut reader, mut writer) = socket.into_split();
    let session_read = session.clone();
    let to_tunnel = tokio::spawn(async move {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => {
                    session_read.usb_close_forward(id);
                    return;
                }
                Ok(n) => session_read.usb_forward_data(id, Bytes::copy_from_slice(&buf[..n])),
            }
        }
    });

    while let Some(msg) = rx.recv().await {
        match msg {
            Inbound::Data(data) => {
                if writer.write_all(&data).await.is_err() {
                    break;
                }
            }
            Inbound::Closed | Inbound::Opened { .. } => break,
        }
    }
    to_tunnel.abort();
    session.ui_handler.unregister_usb_forward_channel(id);
}
