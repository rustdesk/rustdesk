// Controller side of a USB forwarding *push*: the controller's own local
// devices. Listing/binding is 100% local (no network -- same as running
// `usbip list -p -l`/`usbip bind` directly), mirroring
// `server/usbip_mux.rs`'s equivalent functions. The only network-facing
// piece here is the relay for a device the peer is pulling from us, which
// mirrors `usbip_mux.rs`'s `on_open`/`run_channel`, replying through the
// session's own `usb_reply_opened`/`usb_forward_data`/`usb_close_forward`
// instead of a raw `Sender`.
use crate::{
    client::{usbip_state::UsbClientState, Data, Interface},
    flutter::{FlutterHandler, FlutterSession},
    ui_session_interface::Session,
    usbip_flow::{
        self, parse_import_request_busid, Flow, IMPORT_REQUEST_TIMEOUT_MS, USBIP_OP_REQ_IMPORT_LEN,
    },
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
    tokio_util::sync::CancellationToken,
};
use serde_json::json;
use std::process::Command;

const USBIPD_ADDR: &str = "127.0.0.1:3240";
const CONNECT_TIMEOUT_MS: u64 = 3000;
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
/// use, so wait for usbip-host to release it (an unprivileged sysfs read)
/// rather than retry the privileged unbind, each try of which is a prompt.
pub fn unbind_device_when_released(bus_id: &str) -> bool {
    if crate::client::usbip_attach::is_valid_bus_id(bus_id) {
        let status = std::path::Path::new(USBIP_HOST_DRIVER_DIR)
            .join(bus_id)
            .join("usbip_status");
        for attempt in 1..=10 {
            // `SDEV_ST_USED`: the peer still has it imported.
            if std::fs::read_to_string(&status).map_or(true, |s| s.trim() != "2") {
                break;
            }
            log::debug!("usb push: {} still in use, waiting {}/10", bus_id, attempt);
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    bind_device(bus_id, false)
}

lazy_static::lazy_static! {
    // Bindings a closed session made and failed to unshare. A push of one of
    // them is refused and retries the unshare instead of adopting it.
    static ref RELEASE_PENDING: std::sync::Mutex<std::collections::HashSet<String>> =
        Default::default();
}

/// Blocking. Unshares a binding no session owns any more, keeping it pending
/// if that fails.
pub(crate) fn release_orphaned(bus_id: &str) {
    if unbind_device_when_released(bus_id) {
        RELEASE_PENDING.lock().unwrap().remove(bus_id);
    } else {
        log::error!("usb push: failed to unshare {} after its session closed, will retry", bus_id);
        RELEASE_PENDING.lock().unwrap().insert(bus_id.to_owned());
    }
}

fn is_release_pending(bus_id: &str) -> bool {
    RELEASE_PENDING.lock().unwrap().contains(bus_id)
}

/// Retries every pending unshare on its own thread.
fn retry_pending_releases() {
    let pending: Vec<String> = RELEASE_PENDING.lock().unwrap().iter().cloned().collect();
    if pending.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        for bus_id in pending {
            release_orphaned(&bus_id);
        }
    });
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
    cancel: CancellationToken,
) {
    let socket = match timeout(CONNECT_TIMEOUT_MS, TcpStream::connect(USBIPD_ADDR)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            log::error!("usb share: channel {} usbipd connect failed: {}", id, e);
            session.usb_reply_opened(id, false, format!("usbipd connect failed: {}", e));
            session.ui_handler.usb.unregister_share_channel_of(id, &flow);
            return;
        }
        Err(e) => {
            log::error!("usb share: channel {} usbipd connect timed out: {}", id, e);
            session.usb_reply_opened(id, false, format!("usbipd connect timed out: {}", e));
            session.ui_handler.usb.unregister_share_channel_of(id, &flow);
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
                session.ui_handler.usb.unregister_share_channel_of(id, &flow);
                return;
            }
            Err(_) => {
                log::warn!("usb share: channel {} import request timed out, closing", id);
                session.usb_close_forward(id);
                session.ui_handler.usb.unregister_share_channel_of(id, &flow);
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
            session.ui_handler.usb.unregister_share_channel_of(id, &flow);
            return;
        }
        None => {
            log::error!(
                "usb share: channel {} sent a malformed USB/IP import request, refusing",
                id
            );
            session.usb_close_forward(id);
            session.ui_handler.usb.unregister_share_channel_of(id, &flow);
            return;
        }
    }
    if writer.write_all(&prefix).await.is_err() {
        session.usb_close_forward(id);
        session.ui_handler.usb.unregister_share_channel_of(id, &flow);
        return;
    }
    for len in prefix_frames {
        if let Some(add) = flow.drained(len) {
            session.send(Data::Message(usbip_flow::window_update_msg(id, add)));
        }
    }

    let session_read = session.clone();
    let flow_read = flow.clone();
    let mut to_tunnel = hbb_common::tokio::spawn(async move {
        let read = flow_read.socket_to_peer(reader, |chunk| {
            session_read.usb_forward_data(id, chunk);
            true
        });
        hbb_common::tokio::select! {
            _ = read => {}
            _ = cancel.cancelled() => {}
        }
    });

    // Whichever half ends first ends the other; only an end on our side
    // (local EOF or write error) needs a Close, the peer knows about its own.
    let local_ended = loop {
        hbb_common::tokio::select! {
            _ = &mut to_tunnel => break true,
            chunk = inbound.recv() => {
                let Some(chunk) = chunk else { break false };
                if writer.write_all(&chunk).await.is_err() {
                    break true;
                }
                if let Some(add) = flow.drained(chunk.len()) {
                    session.send(Data::Message(usbip_flow::window_update_msg(id, add)));
                }
            }
        }
    };
    to_tunnel.abort();
    if local_ended {
        log::info!("usb share: channel {} local usbipd connection ended", id);
        session.usb_close_forward(id);
    }
    log::info!("usb share: channel {} relay closed", id);
    session.ui_handler.usb.unregister_share_channel_of(id, &flow);
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
        retry_pending_releases();
        let devices = list_local_devices();
        let devices: Vec<serde_json::Value> = devices
            .iter()
            .map(|d| {
                json!({
                    "bus_id": d.bus_id,
                    "vendor": d.vendor,
                    "product": d.product,
                    "shared": d.shared,
                    "pushed": self.ui_handler.usb.share_owned(&d.bus_id),
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
            self.push_event_(
                "usb_push_result",
                &[("bus_id", json!(&bus_id)), ("error", json!("Not connected"))],
                &[],
                &[],
            );
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
            if is_release_pending(&bus_id) {
                retry_pending_releases();
                session.push_event_(
                    "usb_push_result",
                    &[
                        ("bus_id", json!(&bus_id)),
                        ("error", json!("Unsharing this USB device after an earlier session failed; retrying")),
                    ],
                    &[],
                    &[],
                );
                return;
            }
            // Whoever shared it (the CLI or another session) may unshare it
            // under the peer, so it is not taken over.
            if shared_bus_ids().contains(&bus_id) {
                log::info!("usb push: {} is already shared outside this session, refusing", bus_id);
                session.push_event_(
                    "usb_push_result",
                    &[
                        ("bus_id", json!(&bus_id)),
                        ("error", json!("The USB device is already shared outside this session")),
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
                log::info!("usb push: session closed while sharing {}", bus_id);
                unbind_device_when_released(&bus_id);
                return;
            }
            session.ui_handler.usb.share_pending_add(bus_id.clone());
            session.usb_push_request(bus_id);
        });
    }

    /// Push direction "Unpush": tell the peer to detach and free our own
    /// relay slot immediately, then unshare. Our relay's TCP connection to
    /// the local usbipd is what keeps the device "in use", so the entry is
    /// dropped here rather than waiting for the peer. The unshare is local
    /// and runs on its own thread: it must still work after the connection
    /// (and with it the session's runtime) is gone.
    pub fn usb_unpush(&self, bus_id: String) {
        // Revoke the authorization to open a channel for this device in case
        // the peer never did (e.g. unpushed before its `Open` arrived).
        self.ui_handler.usb.share_pending_remove(&bus_id);
        if let Some(channel_id) = self.ui_handler.usb.share_channel_for_bus_id(&bus_id) {
            log::info!("usb push: unpushing {} (channel {})", bus_id, channel_id);
            self.usb_close_forward(channel_id);
            self.ui_handler.usb.unregister_share_channel(channel_id);
        } else {
            log::info!("usb push: unpushing {} with no live channel", bus_id);
        }
        let release = self.ui_handler.usb.share_release_begin(&bus_id);
        let session = self.clone();
        std::thread::spawn(move || {
            let released = release.then(|| release_push(&session.ui_handler.usb, &bus_id));
            let error = match released {
                Some(false) => {
                    log::error!("usb push: failed to unshare {} after unpush", bus_id);
                    format!("Failed to unshare {}", bus_id)
                }
                _ => {
                    log::info!("usb push: unpushed {}", bus_id);
                    String::new()
                }
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

/// Finishes undoing a push that `share_release_begin` started: unbinds
/// `bus_id`, and keeps it recorded if that fails, so a retry or closing the
/// session can still release it. Blocking.
pub(crate) fn release_push(state: &UsbClientState, bus_id: &str) -> bool {
    let released = unbind_device_when_released(bus_id);
    state.share_release_end(bus_id, released);
    released
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
}
