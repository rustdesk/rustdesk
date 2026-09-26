// Flow control shared by all four RemoteUsb relay directions (pull and push,
// on either side), built on `port_forward_mux`'s window accounting. Every
// channel starts with a fixed `CHANNEL_WINDOW` in each direction, so no
// window is advertised in `Open`/`Opened`: a relay never reads more from its
// local socket than the peer has credited, and the peer returns credit
// (`UsbForwardWindowUpdate`) only once it has written that data to its own
// local socket. Both what we buffer for the peer and what the peer can make
// us buffer are therefore bounded per channel.
use crate::port_forward_mux::{charge, RecvWindow, SendCredit, MIN_FRAME_CHARGE};
use base::message_proto::*;
use hbb_common::{
    bytes::Bytes,
    tokio::io::{AsyncRead, AsyncReadExt},
};
use std::sync::{Arc, Mutex};

pub(crate) use crate::port_forward_mux::{CHANNEL_WINDOW, MAX_FRAME as MAX_DATA};

/// For a RemoteUsb connection's stream, on both sides, before the first USB
/// frame: every transport, since the post-login `usize::MAX` lifted them all.
pub fn cap_packet_size(stream: &mut hbb_common::Stream) {
    stream.set_max_packet_length(crate::port_forward_mux::MAX_PACKET);
}

/// Enough queue slots for a whole window of minimum-charge frames, so a
/// channel's inbound queue can only fill up if the peer overran its window,
/// which `Flow::admit` rejects first.
pub const QUEUE_FRAMES: usize = (CHANNEL_WINDOW / MIN_FRAME_CHARGE) as usize;

/// One channel's credit toward the peer and window for the peer, shared by
/// the channel's registry entry (which admits incoming data and applies
/// window updates) and its relay tasks.
#[derive(Clone)]
pub struct Flow {
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
}

impl Flow {
    pub fn new() -> Self {
        Self {
            credit: Arc::new(SendCredit::new(CHANNEL_WINDOW)),
            window: Arc::new(Mutex::new(RecvWindow::new(CHANNEL_WINDOW))),
        }
    }

    /// Charges a data frame from the peer against its window. `false` is a
    /// protocol violation (oversized frame or window overrun): the caller
    /// closes the channel.
    pub fn admit(&self, len: usize) -> bool {
        len <= MAX_DATA && self.window.lock().unwrap().accept(len)
    }

    /// Credit the peer returned in a `UsbForwardWindowUpdate`. Capped by
    /// `SendCredit` at our own window, whatever the peer claims.
    pub fn grant(&self, add: u32) {
        self.credit.add(add);
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
            let got = reader.read(&mut buf[..allow]).await.unwrap_or(0);
            let spent = if got == 0 { 0 } else { charge(got) };
            if (spent as usize) < allow {
                self.credit.add(allow as u32 - spent);
            }
            if got == 0 || !send(Bytes::copy_from_slice(&buf[..got])) {
                return;
            }
        }
    }
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
