// Controlled side of a USB forwarding *push*: the controller offers one of its
// own devices (`UsbPushRequest`), and we pull it in headlessly -- the exact
// mirror of `client/usbip_attach.rs`'s `attach()`, just replying over
// `Sender`/`UsbChannel` (there is no `Session<FlutterHandler>`/UI here)
// instead of `push_event_`, and with its own negative `channel_id` space so
// it can't collide with `usbip_mux.rs`'s (see `channel_id` sign convention
// in `usbip_session::UsbSession::handle`).
use super::{connection::Sender, usbip_mux::is_valid_bus_id};
use crate::usbip_flow::{self, Flow};
use base::message_proto::*;
use hbb_common::{
    bytes::Bytes, log,
    regex::Regex,
    tokio::{
        self,
        io::AsyncWriteExt,
        net::{TcpListener, TcpStream},
        sync::{mpsc, OwnedSemaphorePermit, Semaphore},
    },
    tokio_util::sync::CancellationToken,
};
use std::{collections::HashMap, sync::Arc};

// Each live channel here is a privileged `usbip attach` plus its own local
// vhci port, a much heavier resource than a plain relay task -- kept far
// below `usbip_mux::MAX_LIVE_CHANNELS`.
const MAX_PENDING_PUSHES: usize = 4;

fn usb_channel_msg(union: usb_channel::Union) -> Message {
    let mut ch = UsbChannel::new();
    ch.union = Some(union);
    let mut msg = Message::new();
    msg.set_usb_channel(ch);
    msg
}

pub(super) fn push_result_msg(bus_id: String, error: String) -> Message {
    usb_channel_msg(usb_channel::Union::PushResult(UsbPushResult {
        bus_id,
        error,
        ..Default::default()
    }))
}

fn open_msg(channel_id: i32, bus_id: String) -> Message {
    usb_channel_msg(usb_channel::Union::Open(UsbForwardOpen {
        channel_id,
        bus_id,
        ..Default::default()
    }))
}

fn data_msg(channel_id: i32, data: Bytes) -> Message {
    usb_channel_msg(usb_channel::Union::Data(UsbForwardData {
        channel_id,
        data,
        ..Default::default()
    }))
}

fn close_msg(channel_id: i32) -> Message {
    usb_channel_msg(usb_channel::Union::Close(UsbForwardClose {
        channel_id,
        ..Default::default()
    }))
}

fn send(tx: &Sender, msg: Message) {
    tx.send((tokio::time::Instant::now(), Arc::new(msg))).ok();
}

enum Inbound {
    Opened { success: bool, message: String },
    Data(Bytes),
}

struct Entry {
    bus_id: String,
    inbound: mpsc::Sender<Inbound>,
    flow: Flow,
    cancel: CancellationToken,
}

/// Removing a channel by any path stops its listener and relay, even one
/// blocked on its socket. The vhci import lives on the relay's socket, which
/// ends in our own listener: once that closes, the kernel detaches the port
/// exactly as `usbip detach` would (both raise `VDEV_EVENT_DOWN`), so there
/// is no privileged step left that could fail or name a reused port.
impl Drop for Entry {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// The controlled side of every push offered in one session.
pub struct UsbPullState {
    channels: HashMap<i32, Entry>,
    next_id: i32,
    tx: Sender,
    /// Shared with `usbip_mux` (`UsbSession::new`).
    privileged: Arc<Semaphore>,
}

impl UsbPullState {
    pub fn new(tx: Sender, privileged: Arc<Semaphore>) -> Self {
        Self {
            channels: HashMap::new(),
            next_id: -1,
            tx,
            privileged,
        }
    }

    /// Without a reply the controller keeps the device it already shared for
    /// this push bound until the session ends, and never learns why.
    pub fn refuse_push_request(&self, bus_id: String) {
        send(
            &self.tx,
            push_result_msg(bus_id, "No permission of USB forwarding".to_string()),
        );
    }

    pub fn handle_push_request(&mut self, bus_id: String) {
        // A channel whose `pull()`/`run_channel` task already ended (failed
        // listener bind, failed `usbip attach`, or a relay that hit EOF) has
        // dropped its `inbound` receiver, but nothing else removes the entry
        // -- without this, those stale channels would count against
        // `MAX_PENDING_PUSHES` forever and eventually block every future
        // push for the rest of the session. Mirrors the identical cleanup in
        // `server/usbip_mux.rs::on_open`.
        self.channels.retain(|_, e| !e.inbound.is_closed());
        if self.channels.len() >= MAX_PENDING_PUSHES {
            log::warn!(
                "usb push: rejecting push of {}, {} channels already pending/live",
                bus_id, self.channels.len()
            );
            send(&self.tx, push_result_msg(bus_id, "Too many pending USB pushes".into()));
            return;
        }
        if self.channels.values().any(|e| e.bus_id == bus_id) {
            send(
                &self.tx,
                push_result_msg(bus_id, "This USB device is already pushed".into()),
            );
            return;
        }
        let Ok(permit) = self.privileged.clone().try_acquire_owned() else {
            send(
                &self.tx,
                push_result_msg(bus_id, "Another USB operation is in progress".into()),
            );
            return;
        };
        let tx = self.tx.clone();
        let id = self.next_channel_id();
        log::info!("usb push: peer offered {} on channel {}", bus_id, id);

        let (inbound_tx, inbound_rx) = mpsc::channel(usbip_flow::QUEUE_FRAMES);
        let cancel = CancellationToken::new();
        let flow = Flow::new();
        self.channels.insert(
            id,
            Entry {
                bus_id: bus_id.clone(),
                inbound: inbound_tx,
                flow: flow.clone(),
                cancel: cancel.clone(),
            },
        );

        tokio::spawn(pull(id, bus_id, tx, inbound_rx, cancel, flow, permit));
    }

    /// Routes `Opened`/`Data`/`Close` frames whose `channel_id` belongs to
    /// this side (negative ids only -- non-negative ones are `usbip_mux`'s).
    pub fn handle_data(&mut self, d: UsbForwardData) {
        self.forward(d.channel_id, Inbound::Data(d.data));
    }

    pub fn handle_window_update(&mut self, w: UsbForwardWindowUpdate) {
        if let Some(entry) = self.channels.get(&w.channel_id) {
            if !entry.flow.grant(w.add) {
                hbb_common::throttled_log!(
                    usbip_flow::PEER_LOG_INTERVAL,
                    warn,
                    "usb push: window update for more than was sent on channel {}, closing",
                    w.channel_id
                );
                self.close_overflowed_channel(w.channel_id);
            }
        }
    }

    pub fn handle_opened(&mut self, o: UsbForwardOpened) {
        self.forward(
            o.channel_id,
            Inbound::Opened {
                success: o.success,
                message: o.message,
            },
        );
    }

    /// The controller unpushing: dropping the entry detaches the port, or,
    /// if `usbip attach` is still running, makes `pull()` drop it once it is
    /// done.
    pub fn handle_close(&mut self, c: UsbForwardClose) {
        if self.channels.remove(&c.channel_id).is_some() {
            log::info!("usb push: peer unpushed channel {}", c.channel_id);
        }
    }

    /// Negative, so this side's channel ids can never collide with
    /// `usbip_mux.rs`'s non-negative, controller-allocated ones -- see the
    /// sign dispatch in `usbip_session::UsbSession::handle`.
    fn next_channel_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id -= 1;
        id
    }

    fn forward(&mut self, channel_id: i32, msg: Inbound) {
        let Some(entry) = self.channels.get(&channel_id) else {
            log::trace!("usb push: frame for unknown channel {}", channel_id);
            return;
        };
        if let Inbound::Data(data) = &msg {
            if !entry.flow.admit(data.len()) {
                hbb_common::throttled_log!(
                    usbip_flow::PEER_LOG_INTERVAL,
                    warn,
                    "usb push: data frame ({} bytes) over the window on channel {}, closing",
                    data.len(),
                    channel_id
                );
                self.close_overflowed_channel(channel_id);
                return;
            }
        }
        if entry.inbound.try_send(msg).is_err() {
            // Only a relay task that already ended: an admitted frame always
            // fits (`usbip_flow::QUEUE_FRAMES`).
            self.close_overflowed_channel(channel_id);
        }
    }

    /// A protocol violation: drop the channel (detaching its port, see
    /// `Entry`) and tell the peer, which otherwise keeps its side alive until
    /// the session ends.
    fn close_overflowed_channel(&mut self, channel_id: i32) {
        self.channels.remove(&channel_id);
        send(&self.tx, close_msg(channel_id));
    }

    /// Session teardown or a withdrawn permission: every channel is dropped,
    /// which detaches every port, and the peer is told so it releases what it
    /// shared for them -- by bus id too, since a push still attaching has no
    /// channel the peer knows yet.
    pub fn close_all(&mut self) {
        for (id, entry) in self.channels.drain() {
            send(&self.tx, close_msg(id));
            send(&self.tx, push_result_msg(entry.bus_id.clone(), "USB forwarding ended".into()));
        }
    }
}

/// Mirrors `client/usbip_attach.rs::attach()`: bind a local listener to play
/// the role of the controller's `usbipd`, then run `usbip attach` against
/// it while relaying the resulting TCP stream over `channel_id` (`id`, a
/// negative id already reserved in `UsbPullState.channels`).
async fn pull(
    id: i32,
    bus_id: String,
    tx: Sender,
    inbound: mpsc::Receiver<Inbound>,
    cancel: CancellationToken,
    flow: Flow,
    permit: OwnedSemaphorePermit,
) {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(err) => {
            log::error!("usb push: failed to bind local listener: {}", err);
            send(&tx, push_result_msg(bus_id, format!("Failed to push: {}", err)));
            return;
        }
    };
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(err) => {
            log::error!("usb push: failed to read local listener port: {}", err);
            send(&tx, push_result_msg(bus_id, format!("Failed to push: {}", err)));
            return;
        }
    };

    // Kept so a failed or cancelled attach can abort it below.
    let accept_task = accept_relay(listener, id, bus_id.clone(), tx.clone(), inbound, flow, cancel.clone());

    if cancel.is_cancelled() {
        accept_task.abort();
        return;
    }
    let attach_bus_id = bus_id.clone();
    // An attach that finishes after its channel is gone needs no undoing:
    // its socket closed with the relay, which detached it.
    let local_port = match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        usb_attach_privileged(port, &attach_bus_id)
    })
    .await
    {
        Ok(local_port) => local_port,
        Err(err) => {
            log::error!("usb push: blocking task failed: {}", err);
            None
        }
    };

    if cancel.is_cancelled() {
        accept_task.abort();
        return;
    }

    match local_port {
        Some(local_port) if !accept_task.is_finished() => {
            log::info!(
                "usb push: attached {} on local port {} (channel {})",
                bus_id, local_port, id
            );
            send(&tx, push_result_msg(bus_id, String::new()));
        }
        Some(_) => {
            log::info!("usb push: channel {} ended while {} was attaching", id, bus_id);
            send(&tx, push_result_msg(bus_id, "USB forwarding ended".into()));
        }
        // Also when `usbip port` failed after a successful attach: aborting
        // closes the relay's socket, which detaches what was attached.
        None => {
            accept_task.abort();
            let message = format!("Failed to push {}", bus_id);
            log::error!("usb push: `usbip attach` failed: {}", message);
            send(&tx, push_result_msg(bus_id, message));
        }
    }
}

/// Accepts the one connection `usbip attach` makes and relays it. The import
/// lives on that socket, so ending this task by any path -- the channel's
/// token, an abort, the relay finishing -- also detaches whatever was
/// attached on it.
fn accept_relay(
    listener: TcpListener,
    id: i32,
    bus_id: String,
    tx: Sender,
    inbound: mpsc::Receiver<Inbound>,
    flow: Flow,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let relay = async {
            match listener.accept().await {
                Ok((socket, _)) => {
                    run_channel(id, bus_id, socket, tx, inbound, flow, cancel.clone()).await
                }
                Err(err) => log::error!("usb push: accept failed: {}", err),
            }
        };
        tokio::select! {
            _ = relay => {}
            _ = cancel.cancelled() => {}
        }
    })
}

// `Option`, not `Regex` directly -- see the identical comment in
// `client/usbip_attach.rs`.
lazy_static::lazy_static! {
    static ref USB_PORT_RE: Option<Regex> =
        Regex::new(r"^Port (\d+):")
            .map_err(|err| log::error!("usb push: invalid USB_PORT_RE: {}", err))
            .ok();
}
// The `usbip://host:port/busid` URL: its bus id is the remote one, not the
// token before the arrow (that's some other local identifier, e.g. "5-1" for
// a remote busid of "18-1"), and host:port is our own loopback listener.
lazy_static::lazy_static! {
    static ref USB_PORT_BUS_ID_RE: Option<Regex> =
        Regex::new(r"->\s+usbip://([^/\s]+)/(\S+)")
            .map_err(|err| log::error!("usb push: invalid USB_PORT_BUS_ID_RE: {}", err))
            .ok();
}

/// Blocking; call via `spawn_blocking`. Attaches through
/// `platform::run_usbip_attach_privileged` and finds the resulting vhci port.
fn usb_attach_privileged(port: u16, bus_id: &str) -> Option<i32> {
    if !is_valid_bus_id(bus_id) {
        log::error!("usb push: rejected malformed bus id {:?}", bus_id);
        return None;
    }
    let output = crate::platform::run_usbip_attach_privileged(port, bus_id)?;
    parse_attached_port(&output, port, bus_id)
}

/// The vhci port importing `bus_id` through our listener on `listener_port`.
/// The bus id alone is ambiguous: two peers can both export e.g. "1-2".
fn parse_attached_port(output: &str, listener_port: u16, bus_id: &str) -> Option<i32> {
    let port_re = USB_PORT_RE.as_ref()?;
    let bus_id_re = USB_PORT_BUS_ID_RE.as_ref()?;
    let mut current_port: Option<i32> = None;
    for line in output.lines() {
        if let Some(caps) = port_re.captures(line) {
            current_port = caps[1].parse().ok();
            continue;
        }
        if let Some(caps) = bus_id_re.captures(line) {
            if caps[1] == format!("127.0.0.1:{listener_port}") && &caps[2] == bus_id {
                return current_port;
            }
        }
    }
    None
}

async fn run_channel(
    id: i32,
    bus_id: String,
    socket: TcpStream,
    tx: Sender,
    mut inbound: mpsc::Receiver<Inbound>,
    flow: Flow,
    cancel: CancellationToken,
) {
    send(&tx, open_msg(id, bus_id.clone()));

    let (success, message) = loop {
        match inbound.recv().await {
            Some(Inbound::Opened { success, message }) => {
                if !success {
                    log::error!("usb push: peer refused channel {}: {}", id, message);
                }
                break (success, message);
            }
            Some(_) => continue,
            None => return,
        }
    };
    if !success {
        // Otherwise the controller's UI never learns the push failed and
        // stays stuck showing it as pushed -- the pending-authorization
        // bookkeeping it cleans up on this reply (`usbip_channel.rs`'s
        // `PushResult` handler) would then never run either.
        send(&tx, push_result_msg(bus_id, message));
        return;
    }

    let (reader, mut writer) = socket.into_split();
    let tx_read = tx.clone();
    let flow_read = flow.clone();
    let mut to_tunnel = tokio::spawn(async move {
        let read = flow_read.socket_to_peer(reader, |chunk| {
            send(&tx_read, data_msg(id, chunk));
            true
        });
        tokio::select! {
            _ = read => {}
            _ = cancel.cancelled() => {}
        }
    });

    // Whichever half ends first ends the other; only an end on our side
    // (local EOF or write error) needs a Close, the peer knows about its own.
    let local_ended = loop {
        tokio::select! {
            _ = &mut to_tunnel => break true,
            msg = inbound.recv() => match msg {
                Some(Inbound::Data(data)) => {
                    if writer.write_all(&data).await.is_err() {
                        break true;
                    }
                    if let Some(add) = flow.drained(data.len()) {
                        send(&tx, usbip_flow::window_update_msg(id, add));
                    }
                }
                Some(Inbound::Opened { .. }) | None => break false,
            },
        }
    };
    to_tunnel.abort();
    if local_ended {
        send(&tx, close_msg(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> UsbPullState {
        let (tx, _rx) = mpsc::unbounded_channel();
        UsbPullState::new(tx, Arc::new(Semaphore::new(1)))
    }

    fn fake_entry() -> (Entry, CancellationToken) {
        let (inbound, _rx) = mpsc::channel(1);
        let cancel = CancellationToken::new();
        (
            Entry {
                bus_id: String::new(),
                inbound,
                flow: Flow::new(),
                cancel: cancel.clone(),
            },
            cancel,
        )
    }

    #[test]
    fn close_all_tells_the_peer_about_every_channel() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut state = UsbPullState::new(tx, Arc::new(Semaphore::new(1)));
        for id in [-1, -2] {
            let (entry, _cancel) = fake_entry();
            state.channels.insert(id, entry);
        }
        state.close_all();
        let mut closed: Vec<i32> = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|(_, msg)| match &msg.union {
                Some(message::Union::UsbChannel(ch)) => match &ch.union {
                    Some(usb_channel::Union::Close(c)) => Some(c.channel_id),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        closed.sort();
        assert_eq!(closed, vec![-2, -1]);
    }

    fn push_reply(
        rx: &mut mpsc::UnboundedReceiver<(tokio::time::Instant, Arc<Message>)>,
    ) -> UsbPushResult {
        let (_, msg) = rx.try_recv().unwrap();
        match &msg.union {
            Some(message::Union::UsbChannel(ch)) => match &ch.union {
                Some(usb_channel::Union::PushResult(r)) => r.clone(),
                _ => panic!("expected a PushResult"),
            },
            _ => panic!("expected a UsbChannel message"),
        }
    }

    #[tokio::test]
    async fn push_while_another_privileged_step_runs_is_refused() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let privileged = Arc::new(Semaphore::new(1));
        let mut state = UsbPullState::new(tx, privileged.clone());
        let _busy = privileged.try_acquire_owned().unwrap();
        state.handle_push_request("1-1".into());
        assert!(!push_reply(&mut rx).error.is_empty());
    }

    #[tokio::test]
    async fn second_push_of_the_same_device_is_refused() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let privileged = Arc::new(Semaphore::new(1));
        let mut state = UsbPullState::new(tx, privileged.clone());
        let (inbound, _inbound_rx) = mpsc::channel(1);
        state.channels.insert(
            -1,
            Entry {
                bus_id: "1-1".into(),
                inbound,
                flow: Flow::new(),
                cancel: CancellationToken::new(),
            },
        );
        state.handle_push_request("1-1".into());
        assert!(!push_reply(&mut rx).error.is_empty());
    }

    #[test]
    fn channel_ids_are_negative_and_decreasing() {
        // Negative so they can never collide with `usbip_mux.rs`'s
        // non-negative, controller-allocated ids -- see the sign dispatch
        // in `usbip_session::UsbSession::handle`.
        let mut state = test_state();
        let first = state.next_channel_id();
        let second = state.next_channel_id();
        let third = state.next_channel_id();
        assert_eq!(first, -1);
        assert_eq!(second, -2);
        assert_eq!(third, -3);
        assert!(first < 0 && second < 0 && third < 0);
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
        assert_eq!(parse_attached_port(USBIP_PORT_OUTPUT, 38963, "18-1"), Some(0));
    }

    #[test]
    fn parse_attached_port_no_match_for_unrelated_bus_id() {
        assert_eq!(parse_attached_port(USBIP_PORT_OUTPUT, 38963, "3-2"), None);
    }

    #[test]
    fn parse_attached_port_tells_apart_same_bus_id_on_different_listeners() {
        let output = "\
Imported USB devices
====================
Port 00: <Port in Use> at High Speed(480Mbps)
       unknown vendor : unknown product (0bda:8153)
       5-1 -> usbip://127.0.0.1:30001/1-2
           -> remote bus/dev 001/002
Port 01: <Port in Use> at High Speed(480Mbps)
       unknown vendor : unknown product (1a86:7523)
       3-1 -> usbip://127.0.0.1:30002/1-2
           -> remote bus/dev 001/002
";
        assert_eq!(parse_attached_port(output, 30001, "1-2"), Some(0));
        assert_eq!(parse_attached_port(output, 30002, "1-2"), Some(1));
        assert_eq!(parse_attached_port(output, 30003, "1-2"), None);
    }

    #[test]
    fn parse_attached_port_ignores_unreadable_record_fallback_line() {
        let output = "\
Imported USB devices
====================
Port 00: <Port in Use> at High Speed(480Mbps)
       Transcend Information, Inc. : JetFlash (8564:1000)
       5-1 -> unknown host, remote port and remote busid
           -> remote bus/dev 018/002
";
        assert_eq!(parse_attached_port(output, 38963, "18-1"), None);
    }

    async fn socket_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (ours, theirs) = tokio::join!(TcpStream::connect(addr), listener.accept());
        (ours.unwrap(), theirs.unwrap().0)
    }

    fn close_count(rx: &mut mpsc::UnboundedReceiver<(tokio::time::Instant, Arc<Message>)>) -> usize {
        std::iter::from_fn(|| rx.try_recv().ok())
            .filter(|(_, msg)| match &msg.union {
                Some(message::Union::UsbChannel(ch)) => {
                    matches!(ch.union, Some(usb_channel::Union::Close(_)))
                }
                _ => false,
            })
            .count()
    }

    async fn run_opened_relay(
        socket: TcpStream,
    ) -> (
        tokio::task::JoinHandle<()>,
        mpsc::Sender<Inbound>,
        mpsc::UnboundedReceiver<(tokio::time::Instant, Arc<Message>)>,
    ) {
        let (tx, rx) = mpsc::unbounded_channel();
        let (inbound_tx, inbound_rx) = mpsc::channel(4);
        inbound_tx
            .send(Inbound::Opened {
                success: true,
                message: String::new(),
            })
            .await
            .unwrap();
        let relay = tokio::spawn(run_channel(
            -1,
            "1-1".to_string(),
            socket,
            tx,
            inbound_rx,
            Flow::new(),
            CancellationToken::new(),
        ));
        (relay, inbound_tx, rx)
    }

    /// Plays the kernel's end of an import: connects to the relay's listener
    /// the way `usbip attach` does.
    async fn accepted_import(
        cancel: CancellationToken,
    ) -> (tokio::task::JoinHandle<()>, TcpStream, mpsc::Sender<Inbound>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (inbound_tx, inbound_rx) = mpsc::channel(4);
        let task = accept_relay(listener, -1, "1-1".into(), tx, inbound_rx, Flow::new(), cancel);
        let import = TcpStream::connect(addr).await.unwrap();
        // The relay announces the channel once it holds the socket.
        rx.recv().await.expect("the relay must open its channel");
        (task, import, inbound_tx)
    }

    async fn assert_closed(mut import: TcpStream) {
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 1];
        let read = tokio::time::timeout(std::time::Duration::from_secs(5), import.read(&mut buf));
        assert_eq!(read.await.expect("the import socket must close").unwrap(), 0);
    }

    #[tokio::test]
    async fn attach_finishing_after_its_channel_ended_leaves_no_import() {
        let cancel = CancellationToken::new();
        let (_task, import, _inbound) = accepted_import(cancel.clone()).await;
        // `handle_close`, `close_all` or a protocol violation dropped the entry.
        cancel.cancel();
        assert_closed(import).await;
    }

    #[tokio::test]
    async fn attach_rolled_back_after_port_discovery_failed_leaves_no_import() {
        let (task, import, _inbound) = accepted_import(CancellationToken::new()).await;
        // What `pull()` does when `usbip attach` succeeded but `usbip port` did not.
        task.abort();
        assert_closed(import).await;
    }

    #[tokio::test]
    async fn local_eof_ends_both_halves_and_closes_the_peer_once() {
        let (ours, theirs) = socket_pair().await;
        let (relay, _inbound_tx, mut rx) = run_opened_relay(ours).await;
        drop(theirs);
        tokio::time::timeout(std::time::Duration::from_secs(5), relay)
            .await
            .expect("relay must end on local EOF while the peer side stays open")
            .unwrap();
        assert_eq!(close_count(&mut rx), 1);
    }

    #[tokio::test]
    async fn peer_close_closes_the_import_socket_without_echoing_close() {
        use tokio::io::AsyncReadExt;
        let (ours, mut theirs) = socket_pair().await;
        let (relay, inbound_tx, mut rx) = run_opened_relay(ours).await;
        drop(inbound_tx);
        tokio::time::timeout(std::time::Duration::from_secs(5), relay)
            .await
            .expect("relay must end once the peer's side is gone")
            .unwrap();
        assert_eq!(close_count(&mut rx), 0);
        // The kernel holds this end of an import and detaches it on EOF.
        let mut buf = [0u8; 1];
        let read = tokio::time::timeout(std::time::Duration::from_secs(5), theirs.read(&mut buf));
        assert_eq!(read.await.expect("the import socket must close").unwrap(), 0);
    }
}
