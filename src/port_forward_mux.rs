use hbb_common::{
    bytes::{BufMut, Bytes, BytesMut},
    log,
    message_proto::*,
    tokio::{
        self,
        io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
        sync::{mpsc, watch, Notify},
        time::Instant,
    },
    ResultType,
};
use std::sync::{Arc, Mutex};

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

/// Receiver-side accounting: the credit the peer still has, and what we have
/// drained to the local socket since the last `window_update`. Both are
/// bounded — a cumulative counter would wear out on a long transfer.
pub struct RecvWindow {
    remaining: u32,
    drained_since_update: u32,
}

impl RecvWindow {
    pub fn new(granted: u32) -> Self {
        Self {
            remaining: effective_window(granted),
            drained_since_update: 0,
        }
    }

    /// False means the peer overran the window: a protocol violation.
    pub fn accept(&mut self, len: usize) -> bool {
        match self.remaining.checked_sub(charge(len)) {
            Some(left) => {
                self.remaining = left;
                true
            }
            None => false,
        }
    }

    /// Returns the amount to advertise in a `window_update` once enough has
    /// been drained; the same amount is credited back.
    pub fn drained(&mut self, n: usize) -> Option<u32> {
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

/// Where a channel's frames go. The controller keeps two queues so
/// `window_update` can bypass bulk data; the controlled side has the
/// connection's single ordered `inner.tx`. `open` is not control: it rides
/// the ordered queue so it can never arrive after the channel's first `data`.
#[derive(Clone)]
pub enum FrameSink {
    Queued {
        data: mpsc::Sender<Message>,
        control: mpsc::UnboundedSender<Message>,
    },
    Direct(mpsc::UnboundedSender<(Instant, Arc<Message>)>),
}

fn writer_gone(what: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::BrokenPipe, format!("{} gone", what))
}

impl FrameSink {
    pub async fn send_ordered(&self, msg: Message) -> ResultType<()> {
        match self {
            FrameSink::Queued { data, .. } => data
                .send(msg)
                .await
                .map_err(|_| writer_gone("tunnel writer").into()),
            FrameSink::Direct(tx) => tx
                .send((Instant::now(), Arc::new(msg)))
                .map_err(|_| writer_gone("connection writer").into()),
        }
    }

    pub fn send_control(&self, msg: Message) -> ResultType<()> {
        match self {
            FrameSink::Queued { control, .. } => control
                .send(msg)
                .map_err(|_| writer_gone("tunnel writer").into()),
            FrameSink::Direct(tx) => tx
                .send((Instant::now(), Arc::new(msg)))
                .map_err(|_| writer_gone("connection writer").into()),
        }
    }

    pub fn is_closed(&self) -> bool {
        match self {
            FrameSink::Queued { data, .. } => data.is_closed(),
            FrameSink::Direct(tx) => tx.is_closed(),
        }
    }
}

pub enum Inbound {
    Data(Bytes),
    Close,
    /// The demultiplexer found the peer over its window; the channel task
    /// closes and tells the peer, so the frame still leaves in order.
    Violation,
}

#[derive(Debug, PartialEq)]
pub enum RelayEnd {
    LocalEof,
    PeerClosed,
    Violation,
    Cancelled,
    TunnelGone,
}

/// Local socket -> tunnel, under the peer's credit. `prebuf` is simply the
/// head of the byte stream.
async fn relay_socket_to_tunnel<R: AsyncRead + Unpin>(
    id: i32,
    reader: R,
    prebuf: Vec<u8>,
    credit: Arc<SendCredit>,
    sink: FrameSink,
    mut cancel: watch::Receiver<bool>,
) -> RelayEnd {
    let mut reader = std::io::Cursor::new(prebuf).chain(reader);
    loop {
        let allow = tokio::select! {
            n = credit.take(MAX_FRAME) => n,
            _ = cancel.changed() => return RelayEnd::Cancelled,
        };
        let mut buf = BytesMut::with_capacity(allow);
        let mut limited = (&mut buf).limit(allow);
        let got = tokio::select! {
            r = reader.read_buf(&mut limited) => match r {
                Ok(n) => n,
                Err(_) => 0,
            },
            _ = cancel.changed() => {
                credit.add(allow as u32);
                return RelayEnd::Cancelled;
            }
        };
        let spent = if got == 0 { 0 } else { charge(got) };
        if (spent as usize) < allow {
            credit.add(allow as u32 - spent);
        }
        if got == 0 {
            return RelayEnd::LocalEof;
        }
        if sink.send_ordered(data_msg(id, buf.freeze())).await.is_err() {
            return RelayEnd::TunnelGone;
        }
    }
}

/// Tunnel -> local socket. `initial` is written before anything from the
/// queue (the controlled side's bytes buffered while connecting).
async fn relay_tunnel_to_socket<W: AsyncWrite + Unpin>(
    id: i32,
    mut writer: W,
    initial: Vec<Bytes>,
    mut inbound: mpsc::UnboundedReceiver<Inbound>,
    window: Arc<Mutex<RecvWindow>>,
    sink: FrameSink,
    mut cancel: watch::Receiver<bool>,
) -> RelayEnd {
    let mut pending: std::collections::VecDeque<Bytes> = initial.into();
    loop {
        let chunk = match pending.pop_front() {
            Some(c) => c,
            None => {
                let next = tokio::select! {
                    n = inbound.recv() => n,
                    _ = cancel.changed() => return RelayEnd::Cancelled,
                };
                match next {
                    Some(Inbound::Data(c)) => c,
                    Some(Inbound::Close) => return RelayEnd::PeerClosed,
                    Some(Inbound::Violation) => return RelayEnd::Violation,
                    None => return RelayEnd::TunnelGone,
                }
            }
        };
        let written = tokio::select! {
            r = writer.write_all(&chunk) => r.is_ok(),
            _ = cancel.changed() => return RelayEnd::Cancelled,
        };
        if !written {
            return RelayEnd::LocalEof;
        }
        let update = window.lock().unwrap().drained(chunk.len());
        if let Some(add) = update {
            if sink.send_control(window_update_msg(id, add)).is_err() {
                return RelayEnd::TunnelGone;
            }
        }
    }
}

/// Runs both halves as independent tasks; whichever ends first cancels the
/// other. Sends `close` once, after the last data, and only when the channel
/// ended for a local reason — the peer's own `close` is never echoed.
pub async fn run_channel<R, W>(
    id: i32,
    reader: R,
    writer: W,
    prebuf: Vec<u8>,
    initial_out: Vec<Bytes>,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
    inbound: mpsc::UnboundedReceiver<Inbound>,
    sink: FrameSink,
) where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let mut to_tunnel = tokio::spawn(relay_socket_to_tunnel(
        id, reader, prebuf, credit, sink.clone(), cancel_rx.clone(),
    ));
    let mut to_socket = tokio::spawn(relay_tunnel_to_socket(
        id, writer, initial_out, inbound, window, sink.clone(), cancel_rx,
    ));
    let (first, second) = tokio::select! {
        r = &mut to_tunnel => {
            let _ = cancel_tx.send(true);
            (r.unwrap_or(RelayEnd::Cancelled), to_socket.await.unwrap_or(RelayEnd::Cancelled))
        }
        r = &mut to_socket => {
            let _ = cancel_tx.send(true);
            (r.unwrap_or(RelayEnd::Cancelled), to_tunnel.await.unwrap_or(RelayEnd::Cancelled))
        }
    };
    let peer_closed = first == RelayEnd::PeerClosed || second == RelayEnd::PeerClosed;
    let tunnel_gone = first == RelayEnd::TunnelGone || second == RelayEnd::TunnelGone;
    let local_reason = matches!(first, RelayEnd::LocalEof | RelayEnd::Violation)
        || matches!(second, RelayEnd::LocalEof | RelayEnd::Violation);
    if !peer_closed && !tunnel_gone && local_reason {
        if let Err(e) = sink.send_ordered(close_msg(id)).await {
            log::debug!("port forward channel {} close not sent: {}", id, e);
        }
    }
    log::debug!("port forward channel {} ended: {:?} / {:?}", id, first, second);
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use tunnel::{Claim, Ready, Tunnel, TunnelHandle};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod tunnel {
    use super::*;
    use crate::client::Interface;
    use hbb_common::{
        config::READ_TIMEOUT,
        protobuf::Message as _,
        tokio::net::TcpStream,
        Stream,
    };
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicI32, Ordering},
    };

    // Internal state only; `Claim` and `Ready` are the API listeners see.
    enum TunnelState {
        Unset,
        Establishing,
        Muxed(Arc<TunnelHandle>),
        Legacy,
        Failed,
    }

    pub enum Claim {
        Claimed,
        Wait,
        Muxed(Arc<TunnelHandle>),
        Legacy,
    }

    pub enum Ready {
        Muxed(Arc<TunnelHandle>),
        Legacy,
        Failed,
    }

    /// One per port-forward window. `watch::Sender::send_if_modified` is the
    /// atomic claim; nothing is ever awaited while it runs.
    pub struct Tunnel {
        state: watch::Sender<TunnelState>,
    }

    impl Tunnel {
        pub fn new() -> Self {
            let (state, _) = watch::channel(TunnelState::Unset);
            Self { state }
        }

        pub fn try_claim(&self) -> Claim {
            let mut outcome = Claim::Wait;
            self.state.send_if_modified(|s| match s {
                TunnelState::Unset | TunnelState::Failed => {
                    *s = TunnelState::Establishing;
                    outcome = Claim::Claimed;
                    true
                }
                TunnelState::Establishing => false,
                TunnelState::Muxed(h) => {
                    outcome = Claim::Muxed(h.clone());
                    false
                }
                TunnelState::Legacy => {
                    outcome = Claim::Legacy;
                    false
                }
            });
            outcome
        }

        pub async fn wait_ready(&self) -> Ready {
            let mut rx = self.state.subscribe();
            loop {
                let ready = match &*rx.borrow_and_update() {
                    TunnelState::Muxed(h) => Some(Ready::Muxed(h.clone())),
                    TunnelState::Legacy => Some(Ready::Legacy),
                    // `Unset` here means the tunnel died between the claim
                    // and this wait; the waiter treats it as a failure.
                    TunnelState::Failed | TunnelState::Unset => Some(Ready::Failed),
                    TunnelState::Establishing => None,
                };
                if let Some(r) = ready {
                    return r;
                }
                if rx.changed().await.is_err() {
                    return Ready::Failed;
                }
            }
        }

        pub fn set_muxed(&self, stream: Stream, interface: impl Interface) -> Arc<TunnelHandle> {
            let (data_tx, data_rx) = mpsc::channel(DATA_QUEUE_FRAMES);
            let (control_tx, control_rx) = mpsc::unbounded_channel();
            let handle = Arc::new(TunnelHandle {
                sink: FrameSink::Queued { data: data_tx, control: control_tx },
                channels: Mutex::new(HashMap::new()),
                next_id: AtomicI32::new(1),
            });
            let state = self.state.clone();
            // Publish before spawning: if the loop exits first and resets the
            // state, a later publish here would pin it at Muxed with a dead
            // handle and the window could never re-establish.
            self.state.send_replace(TunnelState::Muxed(handle.clone()));
            tokio::spawn(tunnel_loop(stream, handle.clone(), data_rx, control_rx, interface, state));
            handle
        }

        pub fn set_legacy(&self) {
            self.state.send_replace(TunnelState::Legacy);
        }

        pub fn set_failed(&self) {
            self.state.send_replace(TunnelState::Failed);
        }
    }

    struct ChannelEntry {
        inbound: mpsc::UnboundedSender<Inbound>,
        credit: Arc<SendCredit>,
        window: Arc<Mutex<RecvWindow>>,
        opened: bool,
    }

    pub struct TunnelHandle {
        sink: FrameSink,
        channels: Mutex<HashMap<i32, ChannelEntry>>,
        next_id: AtomicI32,
    }

    impl TunnelHandle {
        /// `open` leaves from inside the channel's own task, down the ordered
        /// data queue, ahead of the channel's first `data`. On the control
        /// queue it could be overtaken by that `data` whenever the tunnel loop
        /// resumes with both queues non-empty.
        pub fn open(
            self: &Arc<Self>,
            host: &str,
            port: i32,
            socket: TcpStream,
            prebuf: Vec<u8>,
        ) -> ResultType<()> {
            if self.sink.is_closed() {
                hbb_common::bail!("port forward tunnel is gone");
            }
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
            let credit = Arc::new(SendCredit::new(INITIAL_WINDOW));
            let window = Arc::new(Mutex::new(RecvWindow::new(CHANNEL_WINDOW)));
            {
                let mut channels = self.channels.lock().unwrap();
                if channels.len() >= MAX_CHANNELS {
                    hbb_common::bail!("too many port forward channels");
                }
                channels.insert(
                    id,
                    ChannelEntry {
                        inbound: inbound_tx,
                        credit: credit.clone(),
                        window: window.clone(),
                        opened: false,
                    },
                );
            }
            let open = open_msg(id, host, port, CHANNEL_WINDOW);
            let (reader, writer) = socket.into_split();
            let sink = self.sink.clone();
            let handle = self.clone();
            tokio::spawn(async move {
                if sink.send_ordered(open).await.is_ok() {
                    run_channel(id, reader, writer, prebuf, Vec::new(), credit, window, inbound_rx, sink).await;
                }
                handle.channels.lock().unwrap().remove(&id);
            });
            Ok(())
        }

        fn on_frame(&self, ch: PortForwardChannel) {
            match ch.union {
                Some(port_forward_channel::Union::Opened(o)) => {
                    let mut channels = self.channels.lock().unwrap();
                    if o.success {
                        // A repeated `opened` must not raise the credit again.
                        if let Some(e) = channels.get_mut(&o.channel_id) {
                            if !e.opened {
                                e.opened = true;
                                e.credit.raise_initial(o.window);
                            }
                        }
                    } else if let Some(e) = channels.remove(&o.channel_id) {
                        log::debug!("port forward channel {} refused: {}", o.channel_id, o.message);
                        e.inbound.send(Inbound::Close).ok();
                    }
                }
                Some(port_forward_channel::Union::Data(d)) => {
                    let mut channels = self.channels.lock().unwrap();
                    let Some(e) = channels.get(&d.channel_id) else {
                        log::debug!("port forward data for unknown channel {}", d.channel_id);
                        return;
                    };
                    let msg = if e.window.lock().unwrap().accept(d.data.len()) {
                        Inbound::Data(d.data)
                    } else {
                        log::warn!("port forward channel {} overran its window", d.channel_id);
                        Inbound::Violation
                    };
                    if e.inbound.send(msg).is_err() {
                        channels.remove(&d.channel_id);
                    }
                }
                Some(port_forward_channel::Union::Close(c)) => {
                    if let Some(e) = self.channels.lock().unwrap().remove(&c.channel_id) {
                        e.inbound.send(Inbound::Close).ok();
                    }
                }
                Some(port_forward_channel::Union::WindowUpdate(u)) => {
                    if let Some(e) = self.channels.lock().unwrap().get(&u.channel_id) {
                        e.credit.add(u.add);
                    }
                }
                Some(port_forward_channel::Union::Open(o)) => {
                    log::debug!("ignoring open for channel {} on the controller", o.channel_id);
                }
                _ => {}
            }
        }

        fn close_all(&self) {
            self.channels.lock().unwrap().clear();
        }

        #[cfg(test)]
        pub fn live_channels(&self) -> usize {
            self.channels.lock().unwrap().len()
        }
    }

    /// The only task that touches the stream. The three arms keep tokio's
    /// default random fairness: a `biased` control -> read -> data order would
    /// starve outbound data whenever inbound is saturated (a LAN-speed
    /// download keeps the read arm ready on every poll), and the reverse would
    /// starve the reads that carry the peer's window updates and pings.
    /// Random order is safe only because nothing order-sensitive is split
    /// across the queues: `open`, `data` and `close` share the data queue and
    /// the control queue carries `window_update` alone.
    async fn tunnel_loop(
        mut stream: Stream,
        handle: Arc<TunnelHandle>,
        mut data_rx: mpsc::Receiver<Message>,
        mut control_rx: mpsc::UnboundedReceiver<Message>,
        interface: impl Interface,
        state: watch::Sender<TunnelState>,
    ) {
        let err = loop {
            tokio::select! {
                Some(msg) = control_rx.recv() => {
                    if let Err(e) = stream.send(&msg).await {
                        break format!("send failed: {}", e);
                    }
                }
                res = stream.next_timeout(READ_TIMEOUT) => match res {
                    Some(Ok(bytes)) => {
                        let Ok(msg) = Message::parse_from_bytes(&bytes) else { continue };
                        match msg.union {
                            Some(message::Union::PortForwardChannel(ch)) => handle.on_frame(ch),
                            Some(message::Union::TestDelay(t)) => {
                                interface.handle_test_delay(t, &mut stream).await;
                            }
                            Some(message::Union::Misc(misc)) => {
                                if let Some(misc::Union::CloseReason(r)) = misc.union {
                                    break format!("closed by peer: {}", r);
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(Err(e)) => break format!("read failed: {}", e),
                    None => break "timeout or reset by the peer".to_owned(),
                },
                Some(msg) = data_rx.recv() => {
                    if let Err(e) = stream.send(&msg).await {
                        break format!("send failed: {}", e);
                    }
                }
            }
        };
        log::info!("port forward tunnel ended: {}", err);
        handle.close_all();
        state.send_replace(TunnelState::Unset);
    }
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
    fn accounting_survives_a_transfer_far_larger_than_the_window() {
        // Cumulative counters used to overflow around 4 GiB on one channel and
        // read as a protocol violation mid-transfer.
        let mut w = RecvWindow::new(CHANNEL_WINDOW);
        let mut moved: u64 = 0;
        while moved < 8 * 1024 * 1024 * 1024 {
            assert!(w.accept(MAX_FRAME));
            w.drained(MAX_FRAME);
            moved += MAX_FRAME as u64;
        }
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

    use hbb_common::message_proto::{message, port_forward_channel};
    use hbb_common::tokio::{self, io::AsyncReadExt, io::AsyncWriteExt, sync::mpsc};
    use std::sync::{Arc, Mutex};

    struct Harness {
        data_rx: mpsc::Receiver<Message>,
        control_rx: mpsc::UnboundedReceiver<Message>,
        inbound_tx: mpsc::UnboundedSender<Inbound>,
        credit: Arc<SendCredit>,
        window: Arc<Mutex<RecvWindow>>,
        local: tokio::io::DuplexStream,
        task: tokio::task::JoinHandle<()>,
    }

    /// A channel whose "local socket" is one end of a duplex pipe and whose
    /// "tunnel" is a pair of queues the test reads directly.
    fn harness(id: i32, prebuf: Vec<u8>, initial_out: Vec<Bytes>) -> Harness {
        let (data_tx, data_rx) = mpsc::channel(DATA_QUEUE_FRAMES);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let (local, remote) = tokio::io::duplex(1 << 20);
        let (r, w) = tokio::io::split(remote);
        let credit = Arc::new(SendCredit::new(INITIAL_WINDOW));
        let window = Arc::new(Mutex::new(RecvWindow::new(CHANNEL_WINDOW)));
        let sink = FrameSink::Queued { data: data_tx, control: control_tx };
        let task = tokio::spawn(run_channel(
            id, r, w, prebuf, initial_out, credit.clone(), window.clone(), inbound_rx, sink,
        ));
        Harness { data_rx, control_rx, inbound_tx, credit, window, local, task }
    }

    // `PortForwardData.data` is generated as `bytes::Bytes` (hbb_common builds
    // rust-protobuf with the bytes feature), so it converts with `to_vec()`, not
    // `clone()`, and needs no wrapping when it becomes an `Inbound::Data`.
    fn frame_kind(m: &Message) -> (&'static str, i32, Vec<u8>) {
        match &m.union {
            Some(message::Union::PortForwardChannel(ch)) => match &ch.union {
                Some(port_forward_channel::Union::Data(d)) => ("data", d.channel_id, d.data.to_vec()),
                Some(port_forward_channel::Union::Close(c)) => ("close", c.channel_id, vec![]),
                Some(port_forward_channel::Union::WindowUpdate(u)) => {
                    ("window_update", u.channel_id, u.add.to_le_bytes().to_vec())
                }
                Some(port_forward_channel::Union::Open(o)) => ("open", o.channel_id, vec![]),
                Some(port_forward_channel::Union::Opened(o)) => ("opened", o.channel_id, vec![]),
                None => ("none", 0, vec![]),
                // `port_forward_channel::Union` is `#[non_exhaustive]` in the
                // generated protobuf code, so it needs a catch-all here even
                // though every current variant is already matched above.
                _ => ("other", 0, vec![]),
            },
            _ => ("other", 0, vec![]),
        }
    }

    #[test]
    fn local_bytes_become_data_frames_capped_at_max_frame() {
        rt().block_on(async {
            let mut h = harness(1, vec![], vec![]);
            let payload = vec![7u8; MAX_FRAME + 10];
            h.local.write_all(&payload).await.unwrap();
            // INITIAL_WINDOW equals MAX_FRAME, so the first frame exhausts
            // it exactly; grant one minimum charge for the 10-byte tail.
            h.credit.add(MIN_FRAME_CHARGE);
            let mut got = Vec::new();
            while got.len() < payload.len() {
                let m = h.data_rx.recv().await.unwrap();
                let (kind, id, bytes) = frame_kind(&m);
                assert_eq!((kind, id), ("data", 1));
                assert!(bytes.len() <= MAX_FRAME);
                got.extend(bytes);
            }
            assert_eq!(got, payload);
        });
    }

    #[test]
    fn prebuf_is_the_head_of_the_send_stream() {
        rt().block_on(async {
            let mut h = harness(2, b"head".to_vec(), vec![]);
            h.local.write_all(b"tail").await.unwrap();
            let mut got = Vec::new();
            while got.len() < 8 {
                let m = h.data_rx.recv().await.unwrap();
                got.extend(frame_kind(&m).2);
            }
            assert_eq!(got, b"headtail".to_vec());
        });
    }

    #[test]
    fn send_side_stops_at_credit_and_resumes_on_add() {
        rt().block_on(async {
            let mut h = harness(3, vec![], vec![]);
            let payload = vec![1u8; INITIAL_WINDOW as usize + 5];
            h.local.write_all(&payload).await.unwrap();
            let mut got = 0usize;
            while got < INITIAL_WINDOW as usize {
                got += frame_kind(&h.data_rx.recv().await.unwrap()).2.len();
            }
            assert_eq!(got, INITIAL_WINDOW as usize);
            assert!(tokio::time::timeout(
                std::time::Duration::from_millis(50),
                h.data_rx.recv()
            )
            .await
            .is_err());
            // One minimum charge is enough to send the 5-byte tail.
            h.credit.add(MIN_FRAME_CHARGE);
            assert_eq!(frame_kind(&h.data_rx.recv().await.unwrap()).2.len(), 5);
        });
    }

    #[test]
    fn inbound_data_is_written_and_window_update_follows_threshold() {
        rt().block_on(async {
            let mut h = harness(4, vec![], vec![Bytes::from_static(b"first")]);
            let mut buf = [0u8; 5];
            h.local.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"first");
            let chunk = Bytes::from(vec![9u8; UPDATE_THRESHOLD as usize]);
            assert!(h.window.lock().unwrap().accept(chunk.len()));
            h.inbound_tx.send(Inbound::Data(chunk.clone())).unwrap();
            let mut sink = vec![0u8; chunk.len()];
            h.local.read_exact(&mut sink).await.unwrap();
            let m = h.control_rx.recv().await.unwrap();
            let (kind, id, add) = frame_kind(&m);
            assert_eq!((kind, id), ("window_update", 4));
            let add = u32::from_le_bytes([add[0], add[1], add[2], add[3]]);
            // The 5-byte `initial` chunk drained a whole minimum charge.
            assert_eq!(add, UPDATE_THRESHOLD + MIN_FRAME_CHARGE);
        });
    }

    #[test]
    fn local_eof_sends_close_exactly_once_after_the_data() {
        rt().block_on(async {
            let mut h = harness(5, vec![], vec![]);
            h.local.write_all(b"bye").await.unwrap();
            drop(h.local);
            assert_eq!(frame_kind(&h.data_rx.recv().await.unwrap()).0, "data");
            assert_eq!(frame_kind(&h.data_rx.recv().await.unwrap()), ("close", 5, vec![]));
            h.task.await.unwrap();
            assert!(h.data_rx.try_recv().is_err());
        });
    }

    #[test]
    fn peer_close_ends_the_channel_without_echoing_close() {
        rt().block_on(async {
            let mut h = harness(6, vec![], vec![]);
            h.inbound_tx.send(Inbound::Close).unwrap();
            h.task.await.unwrap();
            assert!(h.data_rx.try_recv().is_err());
            assert!(h.control_rx.try_recv().is_err());
        });
    }

    #[test]
    fn violation_signalled_by_the_demux_sends_close() {
        rt().block_on(async {
            let mut h = harness(7, vec![], vec![]);
            h.inbound_tx.send(Inbound::Violation).unwrap();
            assert_eq!(frame_kind(&h.data_rx.recv().await.unwrap()), ("close", 7, vec![]));
            h.task.await.unwrap();
        });
    }

    #[test]
    fn dropped_tunnel_ends_the_channel_silently() {
        rt().block_on(async {
            let mut h = harness(8, vec![], vec![]);
            drop(h.inbound_tx);
            h.task.await.unwrap();
            assert!(h.data_rx.try_recv().is_err());
        });
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    mod tunnel {
        use super::*;
        use crate::port_forward_mux::{Claim, Ready, Tunnel, TunnelHandle};
        use hbb_common::{
            protobuf::Message as _,
            tcp::FramedStream,
            tokio::net::{TcpListener, TcpStream},
            Stream,
        };

        /// A loopback TCP pair wrapped as two `Stream`s: one for the tunnel,
        /// one for the fake peer.
        async fn stream_pair() -> (Stream, Stream) {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = l.local_addr().unwrap();
            let client = TcpStream::connect(addr).await.unwrap();
            let (server, _) = l.accept().await.unwrap();
            (
                Stream::Tcp(FramedStream::from(client, addr)),
                Stream::Tcp(FramedStream::from(server, addr)),
            )
        }

        async fn recv_frame(s: &mut Stream) -> PortForwardChannel {
            let bytes = s.next().await.unwrap().unwrap();
            let m = Message::parse_from_bytes(&bytes).unwrap();
            match m.union {
                Some(message::Union::PortForwardChannel(ch)) => ch,
                other => panic!("unexpected {:?}", other),
            }
        }

        async fn local_pair() -> (TcpStream, TcpStream) {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = l.local_addr().unwrap();
            let a = TcpStream::connect(addr).await.unwrap();
            let (b, _) = l.accept().await.unwrap();
            (a, b)
        }

        #[test]
        fn claim_is_exclusive_and_waiters_see_the_outcome() {
            rt().block_on(async {
                let t = Arc::new(Tunnel::new());
                assert!(matches!(t.try_claim(), Claim::Claimed));
                assert!(matches!(t.try_claim(), Claim::Wait));
                let w = { let t = t.clone(); tokio::spawn(async move { t.wait_ready().await }) };
                t.set_failed();
                assert!(matches!(w.await.unwrap(), Ready::Failed));
                assert!(matches!(t.try_claim(), Claim::Claimed));
                t.set_legacy();
                assert!(matches!(t.try_claim(), Claim::Legacy));
                assert!(matches!(t.wait_ready().await, Ready::Legacy));
            });
        }

        #[test]
        fn open_sends_open_then_pipelined_data_and_relays_replies() {
            rt().block_on(async {
                let (ours, mut peer) = stream_pair().await;
                let t = Tunnel::new();
                assert!(matches!(t.try_claim(), Claim::Claimed));
                let h = t.set_muxed(ours, NoUi);
                let (mut app, sock) = local_pair().await;
                h.open("localhost", 80, sock, b"GET / HTTP/1.0\r\n\r\n".to_vec()).unwrap();
                let open = recv_frame(&mut peer).await;
                let id = match &open.union {
                    Some(port_forward_channel::Union::Open(o)) => {
                        assert_eq!((o.host.as_str(), o.port, o.window), ("localhost", 80, CHANNEL_WINDOW));
                        o.channel_id
                    }
                    other => panic!("expected open, got {:?}", other),
                };
                let d = recv_frame(&mut peer).await;
                match &d.union {
                    Some(port_forward_channel::Union::Data(d)) => assert_eq!(d.data, b"GET / HTTP/1.0\r\n\r\n".to_vec()),
                    other => panic!("expected data, got {:?}", other),
                }
                peer.send(&opened_msg(id, true, "", CHANNEL_WINDOW)).await.unwrap();
                peer.send(&data_msg(id, Bytes::from_static(b"HTTP/1.0 200 OK\r\n"))).await.unwrap();
                let mut buf = [0u8; 17];
                app.read_exact(&mut buf).await.unwrap();
                assert_eq!(&buf, b"HTTP/1.0 200 OK\r\n");
                peer.send(&close_msg(id)).await.unwrap();
                assert_eq!(app.read(&mut buf).await.unwrap(), 0);
            });
        }

        #[test]
        fn every_open_precedes_its_own_channels_first_data() {
            rt().block_on(async {
                let (ours, mut peer) = stream_pair().await;
                let t = Tunnel::new();
                assert!(matches!(t.try_claim(), Claim::Claimed));
                let h = t.set_muxed(ours, NoUi);
                // Twenty channels, each with one byte of pipelined data behind
                // its open. An open on the control queue can lose the loop's
                // random tie-break to a data frame, so one channel would be a
                // coin flip; twenty make a wrong implementation fail every run.
                const N: usize = 20;
                let mut apps = Vec::new();
                for i in 0..N {
                    let (app, sock) = local_pair().await;
                    h.open("localhost", 1, sock, vec![i as u8]).unwrap();
                    apps.push(app);
                }
                let mut opened = std::collections::HashSet::new();
                let mut seen_data = 0;
                while seen_data < N {
                    let ch = recv_frame(&mut peer).await;
                    match &ch.union {
                        Some(port_forward_channel::Union::Open(o)) => {
                            assert!(opened.insert(o.channel_id), "duplicate open");
                        }
                        Some(port_forward_channel::Union::Data(d)) => {
                            assert!(
                                opened.contains(&d.channel_id),
                                "data for channel {} arrived before its open",
                                d.channel_id
                            );
                            seen_data += 1;
                        }
                        other => panic!("unexpected {:?}", other),
                    }
                }
            });
        }

        #[test]
        fn failed_open_closes_the_local_socket() {
            rt().block_on(async {
                let (ours, mut peer) = stream_pair().await;
                let t = Tunnel::new();
                t.try_claim();
                let h = t.set_muxed(ours, NoUi);
                let (mut app, sock) = local_pair().await;
                h.open("localhost", 1, sock, vec![]).unwrap();
                let id = match recv_frame(&mut peer).await.union {
                    Some(port_forward_channel::Union::Open(o)) => o.channel_id,
                    other => panic!("expected open, got {:?}", other),
                };
                peer.send(&opened_msg(id, false, "refused", 0)).await.unwrap();
                let mut buf = [0u8; 1];
                assert_eq!(app.read(&mut buf).await.unwrap(), 0);
            });
        }

        #[test]
        fn tunnel_death_closes_channels_and_resets_state() {
            rt().block_on(async {
                let (ours, peer) = stream_pair().await;
                let t = Tunnel::new();
                t.try_claim();
                let h = t.set_muxed(ours, NoUi);
                let (mut app, sock) = local_pair().await;
                h.open("localhost", 1, sock, vec![]).unwrap();
                drop(peer);
                let mut buf = [0u8; 1];
                assert_eq!(app.read(&mut buf).await.unwrap(), 0);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                assert!(matches!(t.try_claim(), Claim::Claimed));
                assert!(h.open("localhost", 1, local_pair().await.1, vec![]).is_err());
            });
        }

        /// An `Interface` that does nothing; the tunnel only needs it for
        /// `handle_test_delay`, which echoes like `Session::handle_test_delay`
        /// (`src/ui_session_interface.rs:1894`) minus the UI stats.
        #[derive(Clone)]
        pub struct NoUi;

        #[async_trait::async_trait]
        impl crate::client::Interface for NoUi {
            fn send(&self, _data: crate::client::Data) {}
            fn msgbox(&self, _msgtype: &str, _title: &str, _text: &str, _link: &str) {}
            fn handle_login_error(&self, _err: &str) -> bool {
                false
            }
            fn handle_peer_info(&self, _pi: PeerInfo) {}
            fn set_multiple_windows_session(&self, _sessions: Vec<WindowsSession>) {}
            async fn handle_hash(&self, _pass: &str, _hash: Hash, _peer: &mut Stream) -> bool {
                false
            }
            async fn handle_login_from_ui(
                &self,
                _os_username: String,
                _os_password: String,
                _password: String,
                _remember: bool,
                _peer: &mut Stream,
            ) {
            }
            async fn handle_test_delay(&self, t: TestDelay, peer: &mut Stream) {
                if !t.from_client {
                    crate::client::handle_test_delay(t, peer).await;
                }
            }
            fn get_lch(&self) -> Arc<std::sync::RwLock<crate::client::LoginConfigHandler>> {
                Arc::new(std::sync::RwLock::new(Default::default()))
            }
        }
    }
}
