use super::connection::{Connection, Sender};
use crate::port_forward_mux::{
    charge, close_msg, effective_window, opened_msg, run_channel, FrameSink, Inbound, RecvWindow,
    SendCredit, CHANNEL_WINDOW, INITIAL_WINDOW, MAX_CHANNELS,
};
use hbb_common::{
    bytes::Bytes,
    log,
    message_proto::*,
    timeout,
    tokio::{self, net::TcpStream, sync::{mpsc, watch}},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const CONNECT_TIMEOUT_MS: u64 = 3000;

/// Before `opened` the controller may only have used `INITIAL_WINDOW`.
/// `charged` is the running total of `charge(len)`, not of raw lengths.
fn pending_fits(charged: usize, add_len: usize) -> bool {
    charged.saturating_add(charge(add_len) as usize) <= INITIAL_WINDOW as usize
}

struct Entry {
    inbound: mpsc::UnboundedSender<Inbound>,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
}

/// The controlled side of one multiplexed tunnel. The main loop owns it and
/// forwards every `PortForwardChannel` frame here; each channel is a task.
pub struct PortForwardMux {
    channels: HashMap<i32, Entry>,
    tx: Sender,
    login_target: String,
    /// Raised once, by `close_all`, for the channels its `clear` cannot reach:
    /// one parked on its target socket is not on the inbound queue.
    teardown: watch::Sender<bool>,
}

impl PortForwardMux {
    pub fn new(tx: Sender, login_target: String) -> Self {
        Self {
            channels: HashMap::new(),
            tx,
            login_target,
            teardown: watch::channel(false).0,
        }
    }

    /// `tunnel_permitted` is consulted for `open` alone, so the lookup is not
    /// made per 64 KiB of data.
    pub fn handle(&mut self, frame: PortForwardChannel, tunnel_permitted: impl FnOnce() -> bool) {
        match frame.union {
            Some(port_forward_channel::Union::Open(open)) => {
                let permitted = tunnel_permitted();
                self.on_open(open, permitted)
            }
            Some(port_forward_channel::Union::Data(d)) => {
                let len = d.data.len();
                let Some(entry) = self.channels.get(&d.channel_id) else {
                    log::debug!("port forward data for unknown channel {}", d.channel_id);
                    return;
                };
                let accepted = entry.window.lock().unwrap().accept(len);
                let delivered = accepted && entry.inbound.send(Inbound::Data(d.data)).is_ok();
                if delivered {
                    return;
                }
                // Dropped here and now, so the peer cannot queue anything more
                // for this id while the task is still on its way out.
                let Some(entry) = self.channels.remove(&d.channel_id) else {
                    return;
                };
                if !accepted {
                    log::warn!("port forward channel {} overran its window", d.channel_id);
                    entry.inbound.send(Inbound::Violation).ok();
                }
            }
            Some(port_forward_channel::Union::Close(c)) => {
                if let Some(entry) = self.channels.remove(&c.channel_id) {
                    entry.inbound.send(Inbound::Close).ok();
                } else {
                    log::debug!("port forward close for unknown channel {}", c.channel_id);
                }
            }
            Some(port_forward_channel::Union::WindowUpdate(u)) => {
                match self.channels.get(&u.channel_id) {
                    Some(entry) => entry.credit.add(u.add),
                    None => log::debug!(
                        "port forward window update for unknown channel {}",
                        u.channel_id
                    ),
                }
            }
            Some(port_forward_channel::Union::Opened(o)) => {
                log::debug!("ignoring opened for channel {} on the controlled side", o.channel_id);
            }
            _ => {}
        }
    }

    fn on_open(&mut self, open: PortForwardOpen, permitted: bool) {
        let id = open.channel_id;
        self.channels.retain(|_, e| !e.inbound.is_closed());
        if !permitted {
            self.reply(opened_msg(id, false, "No permission of IP tunneling", 0));
            return;
        }
        if self.channels.len() >= MAX_CHANNELS {
            self.reply(opened_msg(id, false, "Too many port forward channels", 0));
            return;
        }
        if self.channels.contains_key(&id) {
            log::debug!("ignoring open for live channel {}", id);
            return;
        }
        let mut pf = PortForward {
            host: open.host,
            port: open.port,
            ..Default::default()
        };
        let (addr, is_rdp) = Connection::normalize_port_forward_target(&mut pf);
        // Approval and permission checks saw the login's target; a tunnel
        // serves that one target and nothing else.
        if addr != self.login_target {
            log::warn!(
                "port forward channel {} asked for {} on a tunnel logged in for {}",
                id,
                addr,
                self.login_target
            );
            self.reply(opened_msg(id, false, "Port forward target not authorized", 0));
            return;
        }
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let credit = Arc::new(SendCredit::new(effective_window(open.window)));
        let window = Arc::new(Mutex::new(RecvWindow::new(INITIAL_WINDOW)));
        self.channels.insert(
            id,
            Entry {
                inbound: inbound_tx,
                credit: credit.clone(),
                window: window.clone(),
            },
        );
        tokio::spawn(run_controlled_channel(
            id,
            addr,
            is_rdp,
            credit,
            window,
            inbound_rx,
            FrameSink::Direct(self.tx.clone()),
            self.teardown.subscribe(),
        ));
    }

    fn reply(&self, msg: Message) {
        self.tx
            .send((tokio::time::Instant::now(), Arc::new(msg)))
            .ok();
    }

    #[cfg(test)]
    pub fn live_channels(&self) -> usize {
        self.channels.len()
    }

    #[cfg(test)]
    pub fn recv_window_remaining(&self, id: i32) -> Option<u32> {
        self.channels.get(&id).map(|e| e.window.lock().unwrap().remaining())
    }

    /// Every task ends and drops its target socket: the queue's senders go for
    /// a task on the queue, `teardown` reaches one parked on the socket.
    pub fn close_all(&mut self) {
        self.channels.clear();
        // Not `send`: with no channel live it stores nothing, and one opened
        // as the tunnel closes would never see it.
        self.teardown.send_replace(true);
    }
}

/// Owns the whole channel lifecycle: connect under a `select!` in which a
/// queued command always wins over the connect, buffer what arrives
/// meanwhile, then relay.
async fn run_controlled_channel(
    id: i32,
    addr: String,
    is_rdp: bool,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
    mut inbound: mpsc::UnboundedReceiver<Inbound>,
    sink: FrameSink,
    teardown: watch::Receiver<bool>,
) {
    let mut pending: Vec<Bytes> = Vec::new();
    let mut pending_len = 0usize;
    let connect = timeout(CONNECT_TIMEOUT_MS, TcpStream::connect(&addr));
    tokio::pin!(connect);
    let socket = loop {
        tokio::select! {
            // Biased with the command arm first: a `close` that is already
            // queued must win over a connect that completed on the same poll,
            // or `opened` would go out for a channel the controller has dropped.
            biased;
            cmd = inbound.recv() => match cmd {
                Some(Inbound::Data(b)) => {
                    if !pending_fits(pending_len, b.len()) {
                        log::warn!("port forward channel {} sent more than INITIAL_WINDOW before opened", id);
                        sink.send_ordered(close_msg(id)).await.ok();
                        return;
                    }
                    pending_len += charge(b.len()) as usize;
                    pending.push(b);
                }
                Some(Inbound::Close) | None => return,
                Some(Inbound::Violation) => {
                    sink.send_ordered(close_msg(id)).await.ok();
                    return;
                }
            },
            res = &mut connect => {
                let err = match res {
                    Ok(Ok(s)) => break s,
                    Ok(Err(e)) => e.to_string(),
                    Err(e) => e.to_string(),
                };
                log::debug!("port forward channel {} connect {} failed: {}", id, addr, err);
                sink.send_ordered(opened_msg(id, false, &unreachable_message(&addr, is_rdp), 0)).await.ok();
                return;
            }
        }
    };
    // Granted before `opened` leaves, so the peer can never be ahead of it.
    window.lock().unwrap().grant(CHANNEL_WINDOW - INITIAL_WINDOW);
    if sink
        .send_ordered(opened_msg(id, true, "", CHANNEL_WINDOW))
        .await
        .is_err()
    {
        return;
    }
    let (reader, writer) = socket.into_split();
    run_channel(id, reader, writer, Vec::new(), pending, credit, window, inbound, sink, teardown).await;
}

/// The same words the raw pipe puts in its login error, so one problem reads
/// the same whichever path the peer takes.
fn unreachable_message(addr: &str, is_rdp: bool) -> String {
    format!(
        "Failed to access remote {}. Please make sure it is reachable/open.",
        if is_rdp { "RDP" } else { addr }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port_forward_mux::{CHANNEL_WINDOW, INITIAL_WINDOW, MAX_CHANNELS, MIN_FRAME_CHARGE};
    use hbb_common::{
        message_proto::{message, port_forward_channel},
        tokio::{
            self,
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
            sync::mpsc,
            time::Instant,
        },
    };

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    /// An echo server standing in for the forward target.
    async fn echo_target() -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = l.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    loop {
                        let n = s.read(&mut buf).await.unwrap_or(0);
                        if n == 0 || s.write_all(&buf[..n]).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        port
    }

    fn open(id: i32, port: u16) -> PortForwardChannel {
        let mut ch = PortForwardChannel::new();
        ch.set_open(PortForwardOpen {
            channel_id: id,
            host: "127.0.0.1".to_owned(),
            port: port as i32,
            window: CHANNEL_WINDOW,
            ..Default::default()
        });
        ch
    }

    fn data(id: i32, bytes: &[u8]) -> PortForwardChannel {
        let mut ch = PortForwardChannel::new();
        ch.set_data(PortForwardData {
            channel_id: id,
            data: Bytes::copy_from_slice(bytes),
            ..Default::default()
        });
        ch
    }

    fn close(id: i32) -> PortForwardChannel {
        let mut ch = PortForwardChannel::new();
        ch.set_close(PortForwardClose { channel_id: id, ..Default::default() });
        ch
    }

    async fn next_frame(rx: &mut mpsc::UnboundedReceiver<(Instant, Arc<Message>)>) -> PortForwardChannel {
        let (_, m) = rx.recv().await.unwrap();
        match &m.union {
            Some(message::Union::PortForwardChannel(ch)) => ch.clone(),
            other => panic!("unexpected {:?}", other),
        }
    }

    fn opened(ch: &PortForwardChannel) -> (i32, bool) {
        match &ch.union {
            Some(port_forward_channel::Union::Opened(o)) => (o.channel_id, o.success),
            other => panic!("expected opened, got {:?}", other),
        }
    }

    fn data_of(ch: &PortForwardChannel) -> (i32, Vec<u8>) {
        match &ch.union {
            Some(port_forward_channel::Union::Data(d)) => (d.channel_id, d.data.to_vec()),
            other => panic!("expected data, got {:?}", other),
        }
    }

    #[test]
    fn open_connects_and_echoes_pipelined_data() {
        rt().block_on(async {
            let port = echo_target().await;
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
            mux.handle(open(1, port), || true);
            mux.handle(data(1, b"ping"), || true);
            assert_eq!(opened(&next_frame(&mut rx).await), (1, true));
            assert_eq!(data_of(&next_frame(&mut rx).await), (1, b"ping".to_vec()));
            mux.handle(close(1), || true);
        });
    }

    #[test]
    fn unreachable_target_fails_open_and_discards_pipelined_data() {
        rt().block_on(async {
            let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = l.local_addr().unwrap().port();
            drop(l);
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
            mux.handle(open(1, port), || true);
            mux.handle(data(1, b"lost"), || true);
            assert_eq!(opened(&next_frame(&mut rx).await), (1, false));
            assert!(tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await.is_err());
        });
    }

    #[test]
    fn permission_denied_refuses_without_spawning() {
        rt().block_on(async {
            let port = echo_target().await;
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
            mux.handle(open(1, port), || false);
            assert_eq!(opened(&next_frame(&mut rx).await), (1, false));
            assert_eq!(mux.live_channels(), 0);
        });
    }

    #[test]
    fn a_revoked_permission_refuses_new_channels_and_keeps_live_ones() {
        rt().block_on(async {
            let port = echo_target().await;
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
            mux.handle(open(1, port), || true);
            assert_eq!(opened(&next_frame(&mut rx).await), (1, true));
            // `enable-tunnel` is consulted per `open`, so turning it off
            // mid-session stops new channels; the live one keeps relaying.
            mux.handle(open(2, port), || false);
            assert_eq!(opened(&next_frame(&mut rx).await), (2, false));
            mux.handle(data(1, b"still relayed"), || false);
            assert_eq!(data_of(&next_frame(&mut rx).await), (1, b"still relayed".to_vec()));
            assert_eq!(mux.live_channels(), 1);
        });
    }

    #[test]
    fn close_while_connecting_sends_no_opened() {
        rt().block_on(async {
            // `close` is queued before the task is first polled. Its `select!` is
            // biased towards the command arm, so even a connect that completes on
            // that same poll loses: no `opened` may ever be sent.
            let port = echo_target().await;
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
            mux.handle(open(1, port), || true);
            mux.handle(close(1), || true);
            assert!(tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await.is_err());
            assert_eq!(mux.live_channels(), 0);
        });
    }

    #[test]
    fn over_window_data_closes_only_that_channel() {
        rt().block_on(async {
            let port = echo_target().await;
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
            mux.handle(open(1, port), || true);
            mux.handle(open(2, port), || true);
            let mut seen = 0;
            while seen < 2 {
                opened(&next_frame(&mut rx).await);
                seen += 1;
            }
            let too_much = vec![0u8; CHANNEL_WINDOW as usize + 1];
            mux.handle(data(1, &too_much), || true);
            let ch = next_frame(&mut rx).await;
            match &ch.union {
                Some(port_forward_channel::Union::Close(c)) => assert_eq!(c.channel_id, 1),
                other => panic!("expected close, got {:?}", other),
            }
            mux.handle(data(2, b"still fine"), || true);
            assert_eq!(data_of(&next_frame(&mut rx).await), (2, b"still fine".to_vec()));
        });
    }

    #[test]
    fn an_over_window_frame_drops_the_channel_at_once() {
        rt().block_on(async {
            let port = echo_target().await;
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
            mux.handle(open(1, port), || true);
            opened(&next_frame(&mut rx).await);
            let too_much = vec![0u8; CHANNEL_WINDOW as usize + 1];
            mux.handle(data(1, &too_much), || true);
            // Gone before the channel task has run: whatever the peer keeps
            // sending for this id can no longer queue anything.
            assert_eq!(mux.live_channels(), 0);
            mux.handle(data(1, &too_much), || true);
            assert_eq!(mux.live_channels(), 0);
            let ch = next_frame(&mut rx).await;
            match &ch.union {
                Some(port_forward_channel::Union::Close(c)) => assert_eq!(c.channel_id, 1),
                other => panic!("expected close, got {:?}", other),
            }
        });
    }

    #[test]
    fn open_to_a_target_other_than_the_login_target_is_refused() {
        rt().block_on(async {
            let a = echo_target().await;
            let b = echo_target().await;
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", a));
            mux.handle(open(1, a), || true);
            assert_eq!(opened(&next_frame(&mut rx).await), (1, true));
            // Approval was for target a; b needs a login of its own.
            mux.handle(open(2, b), || true);
            let ch = next_frame(&mut rx).await;
            match &ch.union {
                Some(port_forward_channel::Union::Opened(o)) => {
                    assert_eq!((o.channel_id, o.success), (2, false));
                    assert!(!o.message.is_empty());
                }
                other => panic!("expected opened, got {:?}", other),
            }
            assert_eq!(mux.live_channels(), 1);
        });
    }

    #[test]
    fn demux_admits_only_the_initial_window_before_opened() {
        rt().block_on(async {
            let port = echo_target().await;
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
            mux.handle(open(1, port), || true);
            // The channel task has not run yet: the demultiplexer alone
            // decides what may sit in the queue before `opened`.
            assert_eq!(mux.recv_window_remaining(1), Some(INITIAL_WINDOW));
            assert_eq!(opened(&next_frame(&mut rx).await), (1, true));
            assert_eq!(mux.recv_window_remaining(1), Some(CHANNEL_WINDOW));
        });
    }

    #[test]
    fn pending_bytes_are_bounded_by_initial_window_before_opened() {
        // A loopback connect completes before a task can observe "connecting",
        // so the bound is pinned on the pure predicate the task uses.
        assert!(pending_fits(0, INITIAL_WINDOW as usize));
        assert!(pending_fits(
            INITIAL_WINDOW as usize - MIN_FRAME_CHARGE as usize,
            1
        ));
        // A 1-byte frame costs a whole minimum charge here too.
        assert!(!pending_fits(
            INITIAL_WINDOW as usize - MIN_FRAME_CHARGE as usize + 1,
            1
        ));
        assert!(!pending_fits(usize::MAX, 1));
    }

    /// A target that accepts and hangs up at once, so every channel ends on
    /// the target's EOF — the case where only the next `open` frees the entry.
    async fn drop_target() -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let (s, _) = l.accept().await.unwrap();
                drop(s);
            }
        });
        port
    }

    #[test]
    fn open_frees_dead_entries_so_the_cap_counts_live_channels() {
        rt().block_on(async {
            let port = drop_target().await;
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
            for id in 1..=(MAX_CHANNELS as i32 * 2) {
                mux.handle(open(id, port), || true);
                assert_eq!(opened(&next_frame(&mut rx).await), (id, true));
                // The task sends `close` on the target's EOF and exits; the
                // entry is dead until the next `open` drops it.
                let ch = next_frame(&mut rx).await;
                match &ch.union {
                    Some(port_forward_channel::Union::Close(c)) => assert_eq!(c.channel_id, id),
                    other => panic!("expected close, got {:?}", other),
                }
                tokio::task::yield_now().await;
            }
        });
    }
}
