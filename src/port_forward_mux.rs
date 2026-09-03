use hbb_common::{
    bytes::Bytes,
    message_proto::*,
    tokio::{self, sync::Notify},
};
use std::sync::Mutex;

/// On the wire and fixed forever: what the controller may have in flight on a
/// channel before `opened` brings the peer's window.
pub const INITIAL_WINDOW: u32 = 64 * 1024;
/// Also on the wire and fixed forever: the window a `data` frame costs at
/// minimum, whatever its length. It bounds the per-frame bookkeeping a peer
/// can make us hold — 1-byte frames would otherwise cost it one byte and us
/// a queue entry.
pub const MIN_FRAME_CHARGE: u32 = 64;
pub const CHANNEL_WINDOW: u32 = 256 * 1024;
pub const MAX_FRAME: usize = 64 * 1024;
pub const UPDATE_THRESHOLD: u32 = CHANNEL_WINDOW / 2;
pub const MAX_CHANNELS: usize = 256;
pub const DATA_QUEUE_FRAMES: usize = 128;

pub fn effective_window(advertised: u32) -> u32 {
    advertised.max(INITIAL_WINDOW)
}

/// What a `data` frame of this length costs its channel's window.
pub fn charge(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX).max(MIN_FRAME_CHARGE)
}

/// Receiver-side accounting: what we granted, what arrived, what we have
/// drained to the local socket since the last `window_update`.
pub struct RecvWindow {
    granted: u32,
    received: u32,
    drained_since_update: u32,
}

impl RecvWindow {
    pub fn new(granted: u32) -> Self {
        Self {
            granted: effective_window(granted),
            received: 0,
            drained_since_update: 0,
        }
    }

    /// False means the peer overran the window: a protocol violation.
    pub fn accept(&mut self, len: usize) -> bool {
        let len = charge(len);
        match self.received.checked_add(len) {
            Some(total) if total <= self.granted => {
                self.received = total;
                true
            }
            _ => false,
        }
    }

    /// Returns the amount to advertise in a `window_update` once enough has
    /// been drained; the grant is extended by the same amount.
    pub fn drained(&mut self, n: usize) -> Option<u32> {
        let n = charge(n);
        self.drained_since_update = self.drained_since_update.saturating_add(n);
        if self.drained_since_update < UPDATE_THRESHOLD {
            return None;
        }
        let add = self.drained_since_update;
        self.drained_since_update = 0;
        self.granted = self.granted.saturating_add(add);
        Some(add)
    }
}

/// Sender-side credit. `take` parks until credit is available; the lock is
/// never held across an await.
pub struct SendCredit {
    credit: Mutex<u32>,
    notify: Notify,
}

impl SendCredit {
    pub fn new(initial: u32) -> Self {
        Self {
            credit: Mutex::new(initial),
            notify: Notify::new(),
        }
    }

    /// `max` must be at least `MIN_FRAME_CHARGE` (`MAX_FRAME` is), and the
    /// caller pays `charge(bytes_read)` and refunds the rest — so this parks
    /// until a whole minimum charge is available rather than at zero.
    pub async fn take(&self, max: usize) -> usize {
        debug_assert!(max >= MIN_FRAME_CHARGE as usize);
        loop {
            {
                let mut credit = self.credit.lock().unwrap();
                if *credit >= MIN_FRAME_CHARGE {
                    let n = (*credit as usize).min(max);
                    *credit -= n as u32;
                    return n;
                }
            }
            self.notify.notified().await;
        }
    }

    pub fn add(&self, n: u32) {
        {
            let mut credit = self.credit.lock().unwrap();
            *credit = credit.saturating_add(n);
        }
        self.notify.notify_waiters();
        self.notify.notify_one();
    }

    /// The controller starts a channel with `INITIAL_WINDOW` of credit; when
    /// `opened` advertises the peer's real window this re-bases to it.
    pub fn raise_initial(&self, total: u32) {
        let extra = effective_window(total) - INITIAL_WINDOW;
        if extra > 0 {
            self.add(extra);
        }
    }
}

fn channel_msg(union: port_forward_channel::Union) -> Message {
    let mut ch = PortForwardChannel::new();
    ch.union = Some(union);
    let mut msg = Message::new();
    msg.set_port_forward_channel(ch);
    msg
}

pub fn open_msg(id: i32, host: &str, port: i32, window: u32) -> Message {
    channel_msg(port_forward_channel::Union::Open(PortForwardOpen {
        channel_id: id,
        host: host.to_owned(),
        port,
        window,
        ..Default::default()
    }))
}

pub fn opened_msg(id: i32, success: bool, message: &str, window: u32) -> Message {
    channel_msg(port_forward_channel::Union::Opened(PortForwardOpened {
        channel_id: id,
        success,
        message: message.to_owned(),
        window,
        ..Default::default()
    }))
}

pub fn data_msg(id: i32, data: Bytes) -> Message {
    channel_msg(port_forward_channel::Union::Data(PortForwardData {
        channel_id: id,
        data,
        ..Default::default()
    }))
}

pub fn close_msg(id: i32) -> Message {
    channel_msg(port_forward_channel::Union::Close(PortForwardClose {
        channel_id: id,
        ..Default::default()
    }))
}

pub fn window_update_msg(id: i32, add: u32) -> Message {
    channel_msg(port_forward_channel::Union::WindowUpdate(PortForwardWindowUpdate {
        channel_id: id,
        add,
        ..Default::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn effective_window_clamps_to_initial() {
        assert_eq!(effective_window(0), INITIAL_WINDOW);
        assert_eq!(effective_window(INITIAL_WINDOW - 1), INITIAL_WINDOW);
        assert_eq!(effective_window(INITIAL_WINDOW), INITIAL_WINDOW);
        assert_eq!(effective_window(CHANNEL_WINDOW), CHANNEL_WINDOW);
    }

    #[test]
    fn recv_window_rejects_over_window_data() {
        // `new` clamps its grant to INITIAL_WINDOW, so the test must fill that.
        let mut w = RecvWindow::new(INITIAL_WINDOW);
        assert!(w.accept(INITIAL_WINDOW as usize - MIN_FRAME_CHARGE as usize));
        assert!(w.accept(MIN_FRAME_CHARGE as usize));
        assert!(!w.accept(1));
    }

    #[test]
    fn tiny_frames_are_charged_at_the_minimum() {
        let mut w = RecvWindow::new(INITIAL_WINDOW);
        for _ in 0..(INITIAL_WINDOW / MIN_FRAME_CHARGE) {
            assert!(w.accept(1));
        }
        // A 64 KiB window holds 1024 one-byte frames, not 65536 of them.
        assert!(!w.accept(1));
    }

    #[test]
    fn recv_window_updates_only_past_threshold() {
        let mut w = RecvWindow::new(CHANNEL_WINDOW);
        assert_eq!(
            w.drained(UPDATE_THRESHOLD as usize - MIN_FRAME_CHARGE as usize),
            None
        );
        assert_eq!(w.drained(MIN_FRAME_CHARGE as usize), Some(UPDATE_THRESHOLD));
        // The update re-grants what was drained, so the same amount is accepted again.
        assert!(w.accept(CHANNEL_WINDOW as usize));
        assert!(w.accept(UPDATE_THRESHOLD as usize));
        assert!(!w.accept(1));
    }

    #[test]
    fn send_credit_blocks_at_zero_and_resumes_on_add() {
        rt().block_on(async {
            let credit = std::sync::Arc::new(SendCredit::new(MIN_FRAME_CHARGE + 4));
            assert_eq!(
                credit.take(MAX_FRAME).await,
                (MIN_FRAME_CHARGE + 4) as usize
            );
            let c = credit.clone();
            let waiter = tokio::spawn(async move { c.take(MAX_FRAME).await });
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            assert!(!waiter.is_finished());
            // Below one minimum charge the taker stays parked: whatever it
            // reads next has to be payable.
            credit.add(MIN_FRAME_CHARGE - 1);
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            assert!(!waiter.is_finished());
            credit.add(1);
            assert_eq!(waiter.await.unwrap(), MIN_FRAME_CHARGE as usize);
        });
    }

    #[test]
    fn raise_initial_rebases_credit_from_initial_window() {
        rt().block_on(async {
            let credit = SendCredit::new(INITIAL_WINDOW);
            assert_eq!(credit.take(1000).await, 1000);
            credit.raise_initial(CHANNEL_WINDOW);
            // Credit is now CHANNEL_WINDOW - 1000, not CHANNEL_WINDOW - 1000 + INITIAL_WINDOW.
            assert_eq!(
                credit.take(usize::MAX).await,
                (CHANNEL_WINDOW - 1000) as usize
            );
            credit.raise_initial(0);
            // A zero or sub-INITIAL_WINDOW advertisement adds nothing.
            let c = std::sync::Arc::new(credit);
            let c2 = c.clone();
            let waiter = tokio::spawn(async move { c2.take(MAX_FRAME).await });
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            assert!(!waiter.is_finished());
            c.add(MIN_FRAME_CHARGE);
            assert_eq!(waiter.await.unwrap(), MIN_FRAME_CHARGE as usize);
        });
    }

    #[test]
    fn frame_builders_set_the_expected_union_variant() {
        use hbb_common::message_proto::{message, port_forward_channel};
        let m = data_msg(7, Bytes::from_static(b"abc"));
        match m.union {
            Some(message::Union::PortForwardChannel(ch)) => match ch.union {
                Some(port_forward_channel::Union::Data(d)) => {
                    assert_eq!(d.channel_id, 7);
                    assert_eq!(d.data, b"abc".to_vec());
                }
                other => panic!("unexpected {:?}", other),
            },
            other => panic!("unexpected {:?}", other),
        }
        let m = opened_msg(3, false, "nope", CHANNEL_WINDOW);
        match m.union {
            Some(message::Union::PortForwardChannel(ch)) => match ch.union {
                Some(port_forward_channel::Union::Opened(o)) => {
                    assert_eq!((o.channel_id, o.success, o.message.as_str(), o.window),
                               (3, false, "nope", CHANNEL_WINDOW));
                }
                other => panic!("unexpected {:?}", other),
            },
            other => panic!("unexpected {:?}", other),
        }
    }
}
