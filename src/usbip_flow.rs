// Flow control shared by all four USB forwarding relay directions (pull and
// push, on either side). Every channel starts with a fixed `CHANNEL_WINDOW` in
// each direction, so no window is advertised in `Open`/`Opened`: a relay never
// reads more from its local socket than the peer has credited, and the peer
// returns credit (`UsbForwardWindowUpdate`) only once it has written that data
// to its own local socket. The window therefore bounds all of a channel's data
// in flight, including what waits in the connection's shared outbound queue
// ahead of desktop traffic, so it is kept small. The accounting mirrors
// `port_forward_mux`'s, with USB's own limits.
use crate::port_forward_mux::{charge, MIN_FRAME_CHARGE};
use base::message_proto::*;
use hbb_common::{
    bytes::Bytes,
    log,
    tokio::{
        io::{AsyncRead, AsyncReadExt},
        sync::Notify,
    },
};
use std::sync::{Arc, Mutex};

/// Per channel and direction: USB data sent but not yet written out by the
/// receiver.
pub const CHANNEL_WINDOW: u32 = 64 * 1024;
/// Largest USB data frame, i.e. the longest one USB frame occupies the shared
/// connection ahead of desktop traffic.
pub const MAX_DATA: usize = 16 * 1024;
const UPDATE_THRESHOLD: u32 = CHANNEL_WINDOW / 2;

/// For log lines a peer can trigger at will, e.g. by sending bad frames.
pub const PEER_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);

/// Enough queue slots for a whole window of minimum-charge frames, so a
/// channel's inbound queue can only fill up if the peer overran its window,
/// which `Flow::admit` rejects first.
pub const QUEUE_FRAMES: usize = (CHANNEL_WINDOW / MIN_FRAME_CHARGE) as usize;

/// Sender side: what was charged for data sent and not yet credited back.
/// Credit is only ever the window minus that, so the peer can return what it
/// was sent and no more.
struct Credit {
    outstanding: Mutex<u32>,
    notify: Notify,
}

impl Credit {
    async fn take(&self, max: usize) -> usize {
        loop {
            {
                let mut outstanding = self.outstanding.lock().unwrap();
                let available = CHANNEL_WINDOW - *outstanding;
                if available >= MIN_FRAME_CHARGE {
                    let n = (available as usize).min(max);
                    *outstanding += n as u32;
                    return n;
                }
            }
            self.notify.notified().await;
        }
    }

    /// `false` if `n` is more than is outstanding.
    fn release(&self, n: u32) -> bool {
        {
            let mut outstanding = self.outstanding.lock().unwrap();
            if n > *outstanding {
                return false;
            }
            *outstanding -= n;
        }
        self.notify.notify_one();
        true
    }
}

/// Receiver-side window.
struct Window {
    remaining: u32,
    drained_since_update: u32,
}

impl Window {
    fn accept(&mut self, len: usize) -> bool {
        match self.remaining.checked_sub(charge(len)) {
            Some(left) => {
                self.remaining = left;
                true
            }
            None => false,
        }
    }

    fn drained(&mut self, n: usize) -> Option<u32> {
        self.drained_since_update = self.drained_since_update.saturating_add(charge(n));
        if self.drained_since_update < UPDATE_THRESHOLD {
            return None;
        }
        let add = self.drained_since_update;
        self.drained_since_update = 0;
        self.remaining = self.remaining.saturating_add(add);
        Some(add)
    }
}

/// One channel's credit toward the peer and window for the peer, shared by
/// the channel's registry entry (which admits incoming data and applies
/// window updates) and its relay tasks.
#[derive(Clone)]
pub struct Flow {
    credit: Arc<Credit>,
    window: Arc<Mutex<Window>>,
}

impl Flow {
    pub fn new() -> Self {
        Self {
            credit: Arc::new(Credit {
                outstanding: Mutex::new(0),
                notify: Notify::new(),
            }),
            window: Arc::new(Mutex::new(Window {
                remaining: CHANNEL_WINDOW,
                drained_since_update: 0,
            })),
        }
    }

    /// Whether `other` is a clone of this channel's flow, i.e. the same channel.
    pub fn same(&self, other: &Flow) -> bool {
        Arc::ptr_eq(&self.credit, &other.credit)
    }

    /// Charges a data frame from the peer against its window. `false` is a
    /// protocol violation (oversized frame or window overrun): the caller
    /// closes the channel.
    pub fn admit(&self, len: usize) -> bool {
        len <= MAX_DATA && self.window.lock().unwrap().accept(len)
    }

    /// Credit the peer returned in a `UsbForwardWindowUpdate`. `false` is a
    /// protocol violation (more than it was sent and has not yet returned):
    /// the caller closes the channel.
    pub fn grant(&self, add: u32) -> bool {
        self.credit.release(add)
    }

    /// Records `n` bytes written to the local socket; returns the credit to
    /// hand back to the peer once enough has accumulated.
    pub fn drained(&self, n: usize) -> Option<u32> {
        self.window.lock().unwrap().drained(n)
    }

    /// Local socket -> peer. Reads only as much as the peer has credited and
    /// hands each chunk to `send`; returns on EOF, a read error, or `send`
    /// returning `false`.
    pub async fn socket_to_peer<R: AsyncRead + Unpin>(
        &self,
        mut reader: R,
        mut send: impl FnMut(Bytes) -> bool,
    ) {
        let mut buf = vec![0u8; MAX_DATA];
        loop {
            let allow = self.credit.take(MAX_DATA).await;
            let got = match reader.read(&mut buf[..allow]).await {
                Ok(got) => got,
                Err(err) => {
                    log::warn!("usb relay: local socket read failed: {}", err);
                    0
                }
            };
            let spent = if got == 0 { 0 } else { charge(got) };
            if (spent as usize) < allow {
                self.credit.release(allow as u32 - spent);
            }
            if got == 0 || !send(Bytes::copy_from_slice(&buf[..got])) {
                return;
            }
        }
    }
}

// A peer that opens a channel and then goes silent (never finishing the
// 40-byte `OP_REQ_IMPORT` prefix) would otherwise leak the relay task and
// its `usbipd` connection until the whole session ends.
pub const IMPORT_REQUEST_TIMEOUT_MS: u64 = 5000;

// USB/IP `OP_REQ_IMPORT`: 2-byte version + 2-byte command code (0x8003) +
// 4-byte status, followed by a 32-byte NUL-padded busid -- see the Linux
// kernel's `drivers/usb/usbip/usbip_common.h` (`op_common`) and userspace
// `usbip`'s `src/usbip_network.h` (`SYSFS_BUS_ID_SIZE` = 32).
pub const USBIP_OP_REQ_IMPORT_LEN: usize = 2 + 2 + 4 + 32;
const USBIP_OP_REQ_IMPORT_CODE: u16 = 0x8003;

/// The busid the peer's real USB/IP client actually asked to import, parsed
/// from the start of the raw protocol bytes it sends once the channel opens.
/// `None` if `prefix` isn't (yet, or ever) a well-formed import request.
pub fn parse_import_request_busid(prefix: &[u8]) -> Option<String> {
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

pub fn window_update_msg(channel_id: i32, add: u32) -> Message {
    let mut ch = UsbChannel::new();
    ch.set_window_update(UsbForwardWindowUpdate {
        channel_id,
        add,
        ..Default::default()
    });
    let mut msg = Message::new();
    msg.set_usb_channel(ch);
    msg
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

    #[test]
    fn admit_rejects_oversized_frames_and_window_overrun() {
        let flow = Flow::new();
        assert!(!flow.admit(MAX_DATA + 1));
        for _ in 0..(CHANNEL_WINDOW as usize / MAX_DATA) {
            assert!(flow.admit(MAX_DATA));
        }
        assert!(!flow.admit(1));
    }

    #[test]
    fn drained_data_reopens_the_window() {
        let flow = Flow::new();
        for _ in 0..(CHANNEL_WINDOW as usize / MAX_DATA) {
            assert!(flow.admit(MAX_DATA));
        }
        let mut returned = 0;
        for _ in 0..(CHANNEL_WINDOW as usize / MAX_DATA) {
            returned += flow.drained(MAX_DATA).unwrap_or(0);
        }
        assert_eq!(returned, CHANNEL_WINDOW);
        assert!(flow.admit(MAX_DATA));
    }

    #[test]
    fn many_tiny_frames_fit_the_queue() {
        let flow = Flow::new();
        let mut admitted = 0;
        while flow.admit(1) {
            admitted += 1;
        }
        assert_eq!(admitted, QUEUE_FRAMES);
    }

    #[tokio::test]
    async fn forged_or_repeated_window_updates_are_refused() {
        let flow = Flow::new();
        assert!(!flow.grant(1));
        let data = vec![7u8; CHANNEL_WINDOW as usize * 2];
        let mut sent = 0usize;
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            flow.socket_to_peer(std::io::Cursor::new(data), |chunk| {
                assert!(chunk.len() <= MAX_DATA);
                sent += chunk.len();
                true
            }),
        )
        .await;
        assert_eq!(sent, CHANNEL_WINDOW as usize);
        assert!(flow.grant(CHANNEL_WINDOW));
        assert!(!flow.grant(CHANNEL_WINDOW));
    }

    #[tokio::test]
    async fn socket_to_peer_stops_at_the_peers_credit() {
        let flow = Flow::new();
        let data = vec![7u8; CHANNEL_WINDOW as usize * 2];
        let mut sent = 0usize;
        let reader = std::io::Cursor::new(data);
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            flow.socket_to_peer(reader, |chunk| {
                sent += chunk.len();
                true
            }),
        )
        .await;
        assert_eq!(sent, CHANNEL_WINDOW as usize);
    }
}
