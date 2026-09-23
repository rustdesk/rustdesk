// Controller side of a `RemoteUsb` session: attaching a remote device means
// running the system `usbip attach`, which makes its own local TCP
// connection that has to land somewhere -- so we stand up a local listener
// that plays the role of the (otherwise unreachable) remote `usbipd`, and
// relay everything it sees through `UsbChannel` frames.
use crate::{flutter::FlutterHandler, ui_session_interface::Session};
use hbb_common::{
    bytes::Bytes, log, regex::Regex, timeout,
    tokio::{
        self,
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        runtime::Handle,
        sync::mpsc,
    },
};
use serde_json::json;
use std::sync::LazyLock;

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
    let local_port = match tokio::task::spawn_blocking(move || {
        usb_attach_privileged(port, &attach_bus_id)
    })
    .await
    {
        Ok(local_port) => local_port,
        Err(err) => {
            log::error!("usb attach: blocking task failed: {}", err);
            None
        }
    };
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

// USB/IP `OP_REQ_IMPORT`: 2-byte version + 2-byte command code (0x8003) +
// 4-byte status, followed by a 32-byte NUL-padded busid -- see the identical
// constants/comment in `client/usbip_share.rs`.
const USBIP_OP_REQ_IMPORT_LEN: usize = 2 + 2 + 4 + 32;
const USBIP_OP_REQ_IMPORT_CODE: u16 = 0x8003;
// A local process that raced the real `usbip attach` for the loopback port
// and then went silent (or never sent a well-formed import request) would
// otherwise leak the relay task and channel registration until session end.
const IMPORT_REQUEST_TIMEOUT_MS: u64 = 5000;

/// The busid the connection on our local listener actually asked to import,
/// parsed from the start of the raw protocol bytes -- see the identical
/// function in `client/usbip_share.rs`.
fn parse_import_request_busid(prefix: &[u8]) -> Option<String> {
    if prefix.len() < USBIP_OP_REQ_IMPORT_LEN {
        return None;
    }
    let code = u16::from_be_bytes([prefix[2], prefix[3]]);
    if code != USBIP_OP_REQ_IMPORT_CODE {
        return None;
    }
    let busid = &prefix[8..USBIP_OP_REQ_IMPORT_LEN];
    let end = busid.iter().position(|&b| b == 0).unwrap_or(busid.len());
    std::str::from_utf8(&busid[..end]).ok().map(str::to_string)
}

// `Option`, not `Regex` directly: these patterns are fixed string literals
// that can never actually fail to compile, but `Regex::new(...).unwrap()`
// would still be an unwrap on a production path -- log and fall back to "no
// match" instead.
static USB_PORT_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^Port (\d+):")
        .map_err(|err| log::error!("usb attach: invalid USB_PORT_RE: {}", err))
        .ok()
});
// The remote bus id is the last path segment of the `usbip://host:port/busid`
// URL, not the token before the arrow (that's some other local identifier,
// e.g. "5-1" for a remote busid of "18-1").
static USB_PORT_BUS_ID_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"->\s+usbip://[^/]+/(\S+)")
        .map_err(|err| log::error!("usb attach: invalid USB_PORT_BUS_ID_RE: {}", err))
        .ok()
});

/// `usbip` bus ids are digits, `-`, and `.` only (e.g. "1-2.3"). `bus_id`
/// here comes from the peer's device list and is interpolated into a
/// root-privileged shell command below, so anything else must be rejected
/// before it gets near the shell.
fn is_valid_bus_id(bus_id: &str) -> bool {
    !bus_id.is_empty()
        && bus_id
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == '.')
}

/// A per-call random value with no dependency on the `rand` crate: each
/// `RandomState` is seeded from the OS RNG, so hashing anything through it
/// yields an unpredictable `u64`. Used to make the temp file name in
/// `usb_attach_privileged` unguessable.
fn random_nonce() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

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
    if !is_valid_bus_id(bus_id) {
        log::error!("usb attach: rejected malformed bus id {:?}", bus_id);
        return None;
    }
    // Unguessable suffix plus `set -C` (noclobber) on the first write below:
    // a world-writable /tmp lets another local user pre-plant a symlink at a
    // predictable path, which a root `>` redirect would otherwise follow and
    // overwrite. Noclobber makes that first redirect fail instead of
    // following an existing path (symlink or not); once it has created the
    // file itself, /tmp's sticky bit stops anyone else from swapping it out
    // from under the retry loop's later overwrites.
    let tmp_path = std::env::temp_dir().join(format!(
        "rustdesk-usbip-port-{}-{}-{:016x}.txt",
        std::process::id(),
        bus_id,
        random_nonce()
    ));
    let ok = crate::platform::run_cmds_privileged(&format!(
        "set -C && : > {0} && set +C && \
         usbip -t {port} attach -r 127.0.0.1 -b {bus_id} && \
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
    let port_re = USB_PORT_RE.as_ref()?;
    let bus_id_re = USB_PORT_BUS_ID_RE.as_ref()?;
    let mut current_port: Option<i32> = None;
    for line in output.lines() {
        if let Some(caps) = port_re.captures(line) {
            current_port = caps[1].parse().ok();
            continue;
        }
        if let Some(caps) = bus_id_re.captures(line) {
            if &caps[1] == bus_id {
                return current_port;
            }
        }
    }
    None
}

pub fn detach(rt: &Handle, port: i32) {
    rt.spawn_blocking(move || {
        if !crate::platform::run_cmds_privileged(&format!("usbip detach -p {port}")) {
            log::error!("usb detach: failed to detach port {}", port);
        }
    });
}

async fn run_channel(session: Session<FlutterHandler>, bus_id: String, socket: TcpStream) {
    let id = FlutterHandler::next_usb_channel_id();
    // Bounded so a peer that keeps sending faster than the local socket
    // drains can't grow this without bound -- see the identical comment in
    // `server/usbip_mux.rs`.
    let (tx, mut rx) = mpsc::channel::<Inbound>(256);
    session.ui_handler.register_usb_forward_channel(id, tx);
    session.usb_open_forward(id, bus_id.clone());

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

    // The local listener this connected to has no peer-credential check
    // (loopback TCP on Linux has none), so another local process could
    // race the real `usbip attach` for this ephemeral port and get a raw
    // pipe into the remote session. Require the first bytes to be a
    // legitimate `OP_REQ_IMPORT` for the exact bus_id we're attaching
    // before relaying anything onward -- raises the bar (an attacker would
    // also need to guess/know that bus_id and speak the wire format), even
    // though it can't fully close the race on its own.
    let mut prefix = Vec::with_capacity(USBIP_OP_REQ_IMPORT_LEN);
    while prefix.len() < USBIP_OP_REQ_IMPORT_LEN {
        let mut buf = [0u8; 4096];
        match timeout(IMPORT_REQUEST_TIMEOUT_MS, reader.read(&mut buf)).await {
            Ok(Ok(0)) | Ok(Err(_)) => {
                log::warn!("usb attach: local connection on channel {} closed before import request", id);
                session.ui_handler.unregister_usb_forward_channel(id);
                session.usb_close_forward(id);
                return;
            }
            Ok(Ok(n)) => prefix.extend_from_slice(&buf[..n]),
            Err(_) => {
                log::warn!("usb attach: local connection on channel {} import request timed out", id);
                session.ui_handler.unregister_usb_forward_channel(id);
                session.usb_close_forward(id);
                return;
            }
        }
    }
    match parse_import_request_busid(&prefix) {
        Some(requested) if requested == bus_id => {}
        other => {
            log::warn!(
                "usb attach: local connection on channel {} sent an import request for {:?}, not the authorized {:?} -- refusing (possible loopback hijack attempt)",
                id, other, bus_id
            );
            session.ui_handler.unregister_usb_forward_channel(id);
            session.usb_close_forward(id);
            return;
        }
    }
    session.usb_forward_data(id, Bytes::from(prefix));

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

    fn import_request(busid: &str) -> Vec<u8> {
        let mut req = vec![0x01, 0x11, 0x80, 0x03, 0x00, 0x00, 0x00, 0x00];
        let mut busid_field = vec![0u8; 32];
        busid_field[..busid.len()].copy_from_slice(busid.as_bytes());
        req.extend_from_slice(&busid_field);
        req
    }

    #[test]
    fn parse_import_request_busid_extracts_busid_from_well_formed_request() {
        assert_eq!(
            parse_import_request_busid(&import_request("1-2.3")),
            Some("1-2.3".to_string())
        );
    }

    #[test]
    fn parse_import_request_busid_rejects_wrong_command_code() {
        let mut req = import_request("1-2.3");
        req[2] = 0x80;
        req[3] = 0x05; // OP_REQ_DEVLIST, not OP_REQ_IMPORT
        assert_eq!(parse_import_request_busid(&req), None);
    }

    #[test]
    fn parse_import_request_busid_none_when_too_short() {
        let req = import_request("1-2.3");
        assert_eq!(parse_import_request_busid(&req[..10]), None);
    }

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
