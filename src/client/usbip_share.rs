// Controller side of a `RemoteUsb` *push*: the controller's own local
// devices. Listing/binding is 100% local (no network -- same as running
// `usbip list -p -l`/`usbip bind` directly), mirroring
// `server/usbip_mux.rs`'s equivalent functions. The only network-facing
// piece here is the relay for a device the peer is pulling from us, which
// mirrors `usbip_mux.rs`'s `on_open`/`run_channel`, replying through the
// session's own `usb_reply_opened`/`usb_forward_data`/`usb_close_forward`
// instead of a raw `Sender`.
use crate::{
    client::{Data, Interface},
    flutter::{FlutterHandler, FlutterSession},
    ui_session_interface::Session,
    usbip_flow::{self, Flow},
};
use base::message_proto::*;
use hbb_common::{
    bytes::Bytes, log, timeout,
    regex::Regex,
    tokio::{
        io::AsyncWriteExt,
        net::TcpStream,
        sync::mpsc,
    },
};
use serde_json::json;
use std::process::Command;

const USBIPD_ADDR: &str = "127.0.0.1:3240";
const CONNECT_TIMEOUT_MS: u64 = 3000;
// A peer that opens a channel and then goes silent (never finishing the
// 40-byte `OP_REQ_IMPORT` prefix) would otherwise leak this relay task and
// its `usbipd` connection until the whole session ends.
const IMPORT_REQUEST_TIMEOUT_MS: u64 = 5000;
const USBIP_HOST_DRIVER_DIR: &str = "/sys/bus/usb/drivers/usbip-host";

// `Option`, not `Regex` directly -- see the identical comment in
// `client/usbip_attach.rs`.
lazy_static::lazy_static! {
    static ref USB_DEVICE_RE: Option<Regex> =
        Regex::new(r"busid=([0-9]+-[0-9.]+)#usbid=([0-9a-fA-F]{4}):([0-9a-fA-F]{4})#")
            .map_err(|err| log::error!("usb share: invalid USB_DEVICE_RE: {}", err))
            .ok();
}

fn usbip_command() -> Command {
    let mut cmd = Command::new("usbip");
    let path = std::env::var("PATH").unwrap_or_default();
    cmd.env("PATH", format!("{path}:/usr/sbin:/sbin:/usr/local/sbin"));
    cmd
}

/// Runs `usbip list -p -l` and cross-references `/sys/.../usbip-host` to
/// report which devices are already shared. Blocking; call off the FFI
/// thread.
pub fn list_local_devices() -> Vec<UsbDevice> {
    let output = match usbip_command().args(["list", "-p", "-l"]).output() {
        Ok(o) => o,
        Err(err) => {
            log::error!("usb share: usbip list failed: {}", err);
            return Vec::new();
        }
    };
    let shared = shared_bus_ids();
    let stdout = String::from_utf8_lossy(&output.stdout);
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

/// Blocking; call off the FFI thread. Bind/unbind needs root, so it goes
/// through the same on-demand privilege-elevation prompt the service
/// install/uninstall path already uses.
pub fn bind_device(bus_id: &str, bind: bool) -> bool {
    if !crate::client::usbip_attach::is_valid_bus_id(bus_id) {
        log::error!("usb share: rejected malformed bus id {:?}", bus_id);
        return false;
    }
    let sub_cmd = if bind { "bind" } else { "unbind" };
    crate::platform::run_usbip_privileged(&[sub_cmd, "-b", bus_id])
}

/// Blocking. The relay's `usbipd` connection closes asynchronously after its
/// channel is dropped, and `usbip unbind` fails while the device is still in
/// use, so retry briefly.
pub fn unbind_device_retrying(bus_id: &str) -> bool {
    for attempt in 1..=10 {
        if bind_device(bus_id, false) {
            return true;
        }
        log::debug!("usb push: {} still busy unsharing, retry {}/10", bus_id, attempt);
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    false
}

// USB/IP `OP_REQ_IMPORT`: 2-byte version + 2-byte command code (0x8003) +
// 4-byte status, followed by a 32-byte NUL-padded busid -- see the Linux
// kernel's `drivers/usb/usbip/usbip_common.h` (`op_common`) and userspace
// `usbip`'s `src/usbip_network.h` (`SYSFS_BUS_ID_SIZE` = 32).
const USBIP_OP_REQ_IMPORT_LEN: usize = 2 + 2 + 4 + 32;
const USBIP_OP_REQ_IMPORT_CODE: u16 = 0x8003;

/// The busid the peer's real USB/IP client actually asked to import, parsed
/// from the start of the raw protocol bytes it sends once the channel opens.
/// `None` if `prefix` isn't (yet, or ever) a well-formed import request.
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

/// The peer pulling one of our shared devices: dial our own local `usbipd`,
/// relay bytes both ways until either side closes. Mirrors
/// `server/usbip_mux.rs`'s `on_open`/`run_channel`.
///
/// `bus_id` is the one authorized for this channel (checked against the
/// pending-push set in `usbip_channel`'s `Open` handler before this is even
/// called) -- but that's only the peer's self-reported metadata for the
/// `Open` message. Nothing stops it from opening a channel claiming one
/// bus_id while actually sending a USB/IP import request for a different
/// one, and our local `usbipd` would happily serve whatever bus id the
/// request names as long as it's shared for some other, unrelated reason.
/// So the actual `OP_REQ_IMPORT` busid is inspected before any byte reaches
/// `usbipd`, not just trusted from `Open`.
pub(crate) async fn run_channel(
    id: i32,
    bus_id: String,
    session: FlutterSession,
    mut inbound: mpsc::Receiver<Bytes>,
    flow: Flow,
) {
    let socket = match timeout(CONNECT_TIMEOUT_MS, TcpStream::connect(USBIPD_ADDR)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            log::error!("usb share: channel {} usbipd connect failed: {}", id, e);
            session.usb_reply_opened(id, false, format!("usbipd connect failed: {}", e));
            session.ui_handler.usb.unregister_share_channel(id);
            return;
        }
        Err(e) => {
            log::error!("usb share: channel {} usbipd connect timed out: {}", id, e);
            session.usb_reply_opened(id, false, format!("usbipd connect timed out: {}", e));
            session.ui_handler.usb.unregister_share_channel(id);
            return;
        }
    };
    log::info!("usb share: channel {} connected to local usbipd, peer pulling", id);
    session.usb_reply_opened(id, true, String::new());

    let (reader, mut writer) = socket.into_split();

    let mut prefix = Vec::with_capacity(USBIP_OP_REQ_IMPORT_LEN);
    // Each frame was charged separately against the peer's window, so its
    // credit is returned per frame too.
    let mut prefix_frames = Vec::new();
    loop {
        if prefix.len() >= USBIP_OP_REQ_IMPORT_LEN {
            break;
        }
        match timeout(IMPORT_REQUEST_TIMEOUT_MS, inbound.recv()).await {
            Ok(Some(chunk)) => {
                prefix_frames.push(chunk.len());
                prefix.extend_from_slice(&chunk);
            }
            Ok(None) => {
                log::info!("usb share: channel {} closed before import request", id);
                session.usb_close_forward(id);
                session.ui_handler.usb.unregister_share_channel(id);
                return;
            }
            Err(_) => {
                log::warn!("usb share: channel {} import request timed out, closing", id);
                session.usb_close_forward(id);
                session.ui_handler.usb.unregister_share_channel(id);
                return;
            }
        }
    }
    match parse_import_request_busid(&prefix) {
        Some(requested) if requested == bus_id => {}
        Some(requested) => {
            log::error!(
                "usb share: channel {} import request for {:?} does not match authorized {:?}, refusing",
                id, requested, bus_id
            );
            session.usb_close_forward(id);
            session.ui_handler.usb.unregister_share_channel(id);
            return;
        }
        None => {
            log::error!(
                "usb share: channel {} sent a malformed USB/IP import request, refusing",
                id
            );
            session.usb_close_forward(id);
            session.ui_handler.usb.unregister_share_channel(id);
            return;
        }
    }
    if writer.write_all(&prefix).await.is_err() {
        session.usb_close_forward(id);
        session.ui_handler.usb.unregister_share_channel(id);
        return;
    }
    for len in prefix_frames {
        if let Some(add) = flow.drained(len) {
            session.send(Data::Message(usbip_flow::window_update_msg(id, add)));
        }
    }

    let session_read = session.clone();
    let flow_read = flow.clone();
    let to_tunnel = hbb_common::tokio::spawn(async move {
        flow_read
            .socket_to_peer(reader, |chunk| {
                session_read.usb_forward_data(id, chunk);
                true
            })
            .await;
        log::info!("usb share: channel {} local usbipd connection ended", id);
        session_read.usb_close_forward(id);
    });

    while let Some(chunk) = inbound.recv().await {
        if writer.write_all(&chunk).await.is_err() {
            break;
        }
        if let Some(add) = flow.drained(chunk.len()) {
            session.send(Data::Message(usbip_flow::window_update_msg(id, add)));
        }
    }
    to_tunnel.abort();
    log::info!("usb share: channel {} relay closed", id);
    session.ui_handler.usb.unregister_share_channel(id);
}

fn usbipd_listening() -> bool {
    USBIPD_ADDR.parse().map_or(false, |addr| {
        std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(CONNECT_TIMEOUT_MS)).is_ok()
    })
}

impl Session<FlutterHandler> {
    /// Purely local (no network): what's shareable on this machine, for the
    /// "My local devices" push section.
    pub fn usb_local_devices(&self) {
        let devices = list_local_devices();
        let devices: Vec<serde_json::Value> = devices
            .iter()
            .map(|d| {
                json!({
                    "bus_id": d.bus_id,
                    "vendor": d.vendor,
                    "product": d.product,
                    "shared": d.shared,
                })
            })
            .collect();
        self.push_event_("usb_local_device_list", &[("devices", json!(devices))], &[], &[]);
    }

    /// Also local -- `usbip bind`/`unbind` needs root, so run it off the FFI
    /// thread the same way `usb_attach` does.
    pub fn usb_local_bind(&self, bus_id: String, bind: bool) {
        let Some(rt) = self.ui_handler.usb.session_runtime() else {
            return;
        };
        let session = self.clone();
        rt.spawn_blocking(move || {
            let ok = bind_device(&bus_id, bind);
            let error = if ok {
                String::new()
            } else {
                format!("Failed to {} {}", if bind { "share" } else { "unshare" }, bus_id)
            };
            session.push_event_(
                "usb_local_bind_result",
                &[
                    ("bus_id", json!(bus_id)),
                    ("bind", json!(bind)),
                    ("error", json!(error)),
                ],
                &[],
                &[],
            );
        });
    }

    /// Push direction "Push": one user action instead of two ("share" then
    /// "push" separately) -- share the device locally first, then ask the
    /// peer to attach it. Keeping "shared" and "pushed" from being two
    /// independently-toggleable states means there's only one thing to
    /// track and show in the UI, and it can't drift out of sync with the
    /// button label the way share-then-separately-push could.
    pub fn usb_push(&self, bus_id: String) {
        let Some(rt) = self.ui_handler.usb.session_runtime() else {
            return;
        };
        let session = self.clone();
        rt.spawn_blocking(move || {
            // Without usbipd the peer's attach can only fail, and only after
            // its user has typed a password for it.
            if !usbipd_listening() {
                log::error!("usb push: usbipd is not listening on {}, not pushing {}", USBIPD_ADDR, bus_id);
                session.push_event_(
                    "usb_push_result",
                    &[
                        ("bus_id", json!(&bus_id)),
                        ("error", json!(format!("usbipd is not running on this computer ({})", USBIPD_ADDR))),
                    ],
                    &[],
                    &[],
                );
                return;
            }
            log::info!("usb push: sharing {} before push", bus_id);
            if !bind_device(&bus_id, true) {
                log::error!("usb push: failed to share {} locally, not pushing", bus_id);
                session.push_event_(
                    "usb_push_result",
                    &[
                        ("bus_id", json!(&bus_id)),
                        ("error", json!(format!("Failed to share {}", bus_id))),
                    ],
                    &[],
                    &[],
                );
                return;
            }
            log::info!("usb push: shared {}, asking peer to attach", bus_id);
            if !session.ui_handler.usb.share_owned_add(bus_id.clone()) {
                log::info!("usb push: session closed while sharing {}, unsharing", bus_id);
                unbind_device_retrying(&bus_id);
                return;
            }
            session.ui_handler.usb.share_pending_add(bus_id.clone());
            session.usb_push_request(bus_id);
        });
    }

    /// Push direction "Unpush": tell the peer to detach and free our own
    /// relay slot immediately, then unshare. The peer's own read loop
    /// aborts as soon as it sees our channel entry gone
    /// (`usbip_pull.rs::handle_close`), so it never echoes a Close back to
    /// confirm -- freeing the entry here (not just sending the close
    /// request and waiting) matters, because our relay's TCP connection to
    /// the local usbipd is exactly what keeps the device "in use" and
    /// `usbip unbind` failing. That connection closes asynchronously a
    /// moment after the entry is dropped, not synchronously with this call,
    /// so the unshare below retries briefly rather than racing it.
    pub fn usb_unpush(&self, bus_id: String) {
        // Revoke the authorization to open a channel for this device in case
        // the peer never did (e.g. unpushed before its `Open` arrived).
        self.ui_handler.usb.share_pending_remove(&bus_id);
        self.ui_handler.usb.share_owned_take(&bus_id);
        // No live channel happens whenever this app instance never saw the
        // push complete -- e.g. restarted after sharing, with the peer
        // still gone. The device can still genuinely be locally bound
        // though (`usbip bind` is a kernel fact, not app state), so the
        // actual unshare below must not depend on a channel existing.
        if let Some(channel_id) = self.ui_handler.usb.share_channel_for_bus_id(&bus_id) {
            log::info!("usb push: unpushing {} (channel {})", bus_id, channel_id);
            self.usb_close_forward(channel_id);
            self.ui_handler.usb.unregister_share_channel(channel_id);
        } else {
            log::info!(
                "usb push: unpushing {} with no live channel (peer gone or app restarted since sharing)",
                bus_id
            );
        }

        let Some(rt) = self.ui_handler.usb.session_runtime() else {
            return;
        };
        let session = self.clone();
        rt.spawn_blocking(move || {
            let ok = unbind_device_retrying(&bus_id);
            let error = if ok {
                log::info!("usb push: unshared {} after unpush", bus_id);
                String::new()
            } else {
                log::error!(
                    "usb push: failed to unshare {} after unpush (still busy after retries)",
                    bus_id
                );
                format!("Failed to unshare {}", bus_id)
            };
            session.push_event_(
                "usb_push_result",
                &[("bus_id", json!(&bus_id)), ("error", json!(error))],
                &[],
                &[],
            );
        });
    }
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
}
