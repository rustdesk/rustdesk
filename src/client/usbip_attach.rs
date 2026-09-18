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
use std::sync::{LazyLock, OnceLock};

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
    let local_port = tokio::task::spawn_blocking(move || {
        usb_attach_privileged(port, &attach_bus_id)
    })
    .await
    .ok()
    .flatten();
    match local_port {
        Some(local_port) => {
            session.ui_handler.push_event_(
                "usb_attached",
                &[
                    ("bus_id", json!(bus_id)),
                    ("success", json!(true)),
                    ("port", json!(local_port)),
                    ("message", json!("")),
                ],
                &[],
                &[],
            );
        }
        None => {
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

static USB_PORT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Port (\d+):").unwrap());
// The remote bus id is the last path segment of the `usbip://host:port/busid`
// URL, not the token before the arrow (that's some other local identifier,
// e.g. "5-1" for a remote busid of "18-1").
static USB_PORT_BUS_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"->\s+usbip://[^/]+/(\S+)").unwrap());

/// Blocking; call via `spawn_blocking`. Two problems in one: `usbip attach`
/// needs root and writes its attach record (`/var/run/vhci_hcd/...`) as
/// root too, so a follow-up unprivileged `usbip port` can't `fopen`/read
/// that record back to report the remote bus id -- query it as part of the
/// *same* privileged command instead (via a temp file `chmod`ed readable
/// afterwards) rather than a second sudo prompt. Separately, even run right
/// after as root, the kernel's vhci state can lag a moment behind `usbip
/// attach` returning success, so the retry loop is shell-side too (same
/// privileged session) rather than a second `run_cmds_privileged` call.
fn usb_attach_privileged(port: u16, bus_id: &str) -> Option<i32> {
    let tmp_path = std::env::temp_dir().join(format!(
        "rustdesk-usbip-port-{}-{}.txt",
        std::process::id(),
        bus_id
    ));
    let ok = crate::platform::run_cmds_privileged(&format!(
        "usbip -t {port} attach -r 127.0.0.1 -b {bus_id} && \
         for i in 1 2 3 4 5 6 7 8 9 10; do \
           usbip port > {0} 2>&1; \
           grep -q -- '/{bus_id}$' {0} && break; \
           sleep 0.2; \
         done && chmod 644 {0}",
        tmp_path.display()
    ));
    log::info!(
        "usb attach: privileged attach+port command for {} ok={} tmp={}",
        bus_id,
        ok,
        tmp_path.display()
    );
    let local_port = if ok {
        match std::fs::read_to_string(&tmp_path) {
            Ok(output) => {
                log::info!("usb attach: `usbip port` output for {}:\n{}", bus_id, output);
                parse_attached_port(&output, bus_id)
            }
            Err(err) => {
                log::error!(
                    "usb attach: failed to read {}: {}",
                    tmp_path.display(),
                    err
                );
                None
            }
        }
    } else {
        None
    };
    let _ = std::fs::remove_file(&tmp_path);
    local_port
}

fn parse_attached_port(output: &str, bus_id: &str) -> Option<i32> {
    let mut current_port: Option<i32> = None;
    for line in output.lines() {
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

#[cfg(test)]
mod tests {
    use super::*;

    const USBIP_PORT_OUTPUT: &str = "\
Imported USB devices
====================
Port 00: <Port in Use> at High Speed(480Mbps)
       Transcend Information, Inc. : JetFlash (8564:1000)
       5-1 -> usbip://127.0.0.1:38963/18-1
           -> remote bus/dev 018/002
";

    #[test]
    fn parse_attached_port_matches_by_trailing_url_segment() {
        // The token right after "Port NN:" before the arrow ("5-1" here) is
        // some other local identifier, not the remote bus id -- only the
        // last path segment of the usbip:// URL ("18-1") is.
        assert_eq!(parse_attached_port(USBIP_PORT_OUTPUT, "18-1"), Some(0));
    }

    #[test]
    fn parse_attached_port_no_match_for_unrelated_bus_id() {
        assert_eq!(parse_attached_port(USBIP_PORT_OUTPUT, "3-2"), None);
    }

    #[test]
    fn parse_attached_port_empty_output() {
        assert_eq!(parse_attached_port("", "18-1"), None);
    }

    #[test]
    fn parse_attached_port_picks_the_right_port_among_several() {
        let output = "\
Imported USB devices
====================
Port 00: <Port in Use> at High Speed(480Mbps)
       Transcend Information, Inc. : JetFlash (8564:1000)
       5-1 -> usbip://127.0.0.1:38963/2-2
           -> remote bus/dev 018/002
Port 01: <Port in Use> at High Speed(480Mbps)
       unknown vendor : unknown product (1a86:7523)
       3-1 -> usbip://127.0.0.1:38963/18-1
           -> remote bus/dev 003/007
";
        assert_eq!(parse_attached_port(output, "18-1"), Some(1));
        assert_eq!(parse_attached_port(output, "2-2"), Some(0));
    }

    #[test]
    fn parse_attached_port_ignores_unreadable_record_fallback_line() {
        // When the attach record can't be read (permissions, or queried too
        // soon after attach), `usbip port` falls back to a line with no
        // "-> usbip://..." at all -- must not spuriously match.
        let output = "\
Imported USB devices
====================
Port 00: <Port in Use> at High Speed(480Mbps)
       Transcend Information, Inc. : JetFlash (8564:1000)
       5-1 -> unknown host, remote port and remote busid
           -> remote bus/dev 018/002
";
        assert_eq!(parse_attached_port(output, "18-1"), None);
    }
}
