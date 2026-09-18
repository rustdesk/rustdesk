// Controller side of a `RemoteUsb` *push*: the controller's own local
// devices. Listing/binding is 100% local (no network -- same as running
// `usbip list -p -l`/`usbip bind` directly), mirroring
// `server/usbip_mux.rs`'s equivalent functions. The only network-facing
// piece here is the relay for a device the peer is pulling from us, which
// mirrors `usbip_mux.rs`'s `on_open`/`run_channel`, replying through the
// session's own `usb_reply_opened`/`usb_forward_data`/`usb_close_forward`
// instead of a raw `Sender`.
use crate::flutter::FlutterSession;
use base::message_proto::*;
use hbb_common::{
    bytes::Bytes, log, timeout,
    regex::Regex,
    tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
        sync::mpsc,
    },
};
use std::{process::Command, sync::LazyLock};

const USBIPD_ADDR: &str = "127.0.0.1:3240";
const CONNECT_TIMEOUT_MS: u64 = 3000;
const USBIP_HOST_DRIVER_DIR: &str = "/sys/bus/usb/drivers/usbip-host";

static USB_DEVICE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"busid=([0-9]+-[0-9.]+)#usbid=([0-9a-fA-F]{4}):([0-9a-fA-F]{4})#").unwrap());

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

/// Blocking; call off the FFI thread. Bind/unbind needs root, so it goes
/// through the same on-demand privilege-elevation prompt the service
/// install/uninstall path already uses.
pub fn bind_device(bus_id: &str, bind: bool) -> bool {
    let sub_cmd = if bind { "bind" } else { "unbind" };
    crate::platform::run_cmds_privileged(&format!("usbip {} -b {}", sub_cmd, bus_id))
}

/// The peer pulling one of our shared devices: dial our own local `usbipd`,
/// relay bytes both ways until either side closes. Mirrors
/// `server/usbip_mux.rs`'s `on_open`/`run_channel`.
pub(crate) async fn run_channel(
    id: i32,
    session: FlutterSession,
    mut inbound: mpsc::UnboundedReceiver<Bytes>,
) {
    let socket = match timeout(CONNECT_TIMEOUT_MS, TcpStream::connect(USBIPD_ADDR)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            log::error!("usb share: channel {} usbipd connect failed: {}", id, e);
            session.usb_reply_opened(id, false, format!("usbipd connect failed: {}", e));
            session.ui_handler.unregister_usb_share_channel(id);
            return;
        }
        Err(e) => {
            log::error!("usb share: channel {} usbipd connect timed out: {}", id, e);
            session.usb_reply_opened(id, false, format!("usbipd connect timed out: {}", e));
            session.ui_handler.unregister_usb_share_channel(id);
            return;
        }
    };
    log::info!("usb share: channel {} connected to local usbipd, peer pulling", id);
    session.usb_reply_opened(id, true, String::new());

    let (mut reader, mut writer) = socket.into_split();
    let session_read = session.clone();
    let to_tunnel = hbb_common::tokio::spawn(async move {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => {
                    log::info!("usb share: channel {} local usbipd connection ended", id);
                    session_read.usb_close_forward(id);
                    return;
                }
                Ok(n) => session_read.usb_forward_data(id, Bytes::copy_from_slice(&buf[..n])),
            }
        }
    });

    while let Some(chunk) = inbound.recv().await {
        if writer.write_all(&chunk).await.is_err() {
            break;
        }
    }
    to_tunnel.abort();
    log::info!("usb share: channel {} relay closed", id);
    session.ui_handler.unregister_usb_share_channel(id);
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
