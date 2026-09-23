// Controlled side of a `RemoteUsb` *push*: the controller offers one of its
// own devices (`UsbPushRequest`), and we pull it in headlessly -- the exact
// mirror of `client/usbip_attach.rs`'s `attach()`, just replying over
// `Sender`/`UsbChannel` (there is no `Session<FlutterHandler>`/UI here)
// instead of `push_event_`, and with its own negative `channel_id` space so
// it can't collide with `usbip_mux.rs`'s (see `channel_id` sign convention
// in `connection.rs::handle_usb_channel`).
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
        sync::mpsc,
    },
    tokio_util::sync::CancellationToken,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

// Each live channel here is a privileged `usbip attach` plus its own local
// vhci port, a much heavier resource than a plain relay task -- kept far
// below `usbip_mux::MAX_LIVE_CHANNELS`.
const MAX_PENDING_PUSHES: usize = 32;

fn usb_channel_msg(union: usb_channel::Union) -> Message {
    let mut ch = UsbChannel::new();
    ch.union = Some(union);
    let mut msg = Message::new();
    msg.set_usb_channel(ch);
    msg
}

fn push_result_msg(bus_id: String, error: String) -> Message {
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
    inbound: mpsc::Sender<Inbound>,
    flow: Flow,
    /// Local vhci port, once `usbip attach` reports it -- needed to run
    /// `usbip detach -p <port>` when the controller unpushes. Set from the
    /// spawned `pull()` task, which has no `&mut self` access back here.
    attached_port: Arc<Mutex<Option<i32>>>,
    /// Signals the spawned `pull()` task that this channel closed while it
    /// was still attaching, so it detaches the port itself once `usbip
    /// attach` finishes instead of leaving it owned by nothing (`handle_close`
    /// already ran and won't run again for this id by the time that happens).
    cancel: CancellationToken,
}

/// The controlled side of every push offered in one `RemoteUsb` session.
pub struct UsbPullState {
    channels: HashMap<i32, Entry>,
    next_id: i32,
    tx: Sender,
}

impl UsbPullState {
    pub fn new(tx: Sender) -> Self {
        Self {
            channels: HashMap::new(),
            next_id: -1,
            tx,
        }
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
        let tx = self.tx.clone();
        let id = self.next_channel_id();
        log::info!("usb push: peer offered {} on channel {}", bus_id, id);

        let (inbound_tx, inbound_rx) = mpsc::channel(usbip_flow::QUEUE_FRAMES);
        let attached_port = Arc::new(Mutex::new(None));
        let cancel = CancellationToken::new();
        let flow = Flow::new();
        self.channels.insert(
            id,
            Entry {
                inbound: inbound_tx,
                flow: flow.clone(),
                attached_port: attached_port.clone(),
                cancel: cancel.clone(),
            },
        );

        tokio::spawn(pull(id, bus_id, tx, inbound_rx, attached_port, cancel, flow));
    }

    /// Routes `Opened`/`Data`/`Close` frames whose `channel_id` belongs to
    /// this side (negative ids only -- non-negative ones are `usbip_mux`'s).
    pub fn handle_data(&mut self, d: UsbForwardData) {
        self.forward(d.channel_id, Inbound::Data(d.data));
    }

    pub fn handle_window_update(&mut self, w: UsbForwardWindowUpdate) {
        if let Some(entry) = self.channels.get(&w.channel_id) {
            entry.flow.grant(w.add);
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

    /// The controller unpushing: detach locally, then drop the channel so
    /// its relay task ends. Cancels the entry regardless of whether attach
    /// has finished yet -- if it hasn't, `pull()` notices once it does and
    /// detaches the port itself (see `Entry::cancel`'s doc comment), since
    /// this function only runs once per channel and can't do it later.
    pub fn handle_close(&mut self, c: UsbForwardClose) {
        let Some(entry) = self.channels.remove(&c.channel_id) else {
            return;
        };
        entry.cancel.cancel();
        let port = *entry.attached_port.lock().unwrap();
        if let Some(port) = port {
            log::info!("usb push: peer unpushed channel {}, detaching port {}", c.channel_id, port);
            detach_port(port);
        } else {
            log::info!(
                "usb push: peer unpushed channel {} before it finished attaching",
                c.channel_id
            );
        }
    }

    /// Negative, so this side's channel ids can never collide with
    /// `usbip_mux.rs`'s non-negative, controller-allocated ones -- see the
    /// sign dispatch in `connection.rs::handle_usb_channel`.
    fn next_channel_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id -= 1;
        id
    }

    fn forward(&mut self, channel_id: i32, msg: Inbound) {
        let Some(entry) = self.channels.get(&channel_id) else {
            log::debug!("usb push: frame for unknown channel {}", channel_id);
            return;
        };
        if let Inbound::Data(data) = &msg {
            if !entry.flow.admit(data.len()) {
                log::warn!(
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

    /// Overflow (oversized frame or a full queue): the same port-leak race
    /// item C fixed for an explicit unpush (`handle_close`) applies here too
    /// -- cancel any in-flight attach and detach if one already finished --
    /// plus tell the peer, since it doesn't otherwise learn we gave up on
    /// this channel and would keep its own side alive until session end.
    fn close_overflowed_channel(&mut self, channel_id: i32) {
        if let Some(entry) = self.channels.remove(&channel_id) {
            entry.cancel.cancel();
            if let Some(port) = *entry.attached_port.lock().unwrap() {
                detach_port(port);
            }
        }
        send(&self.tx, close_msg(channel_id));
    }

    /// Session teardown (peer disconnected, connection lost): cancels every
    /// channel's in-flight attach and detaches any port that had already
    /// finished attaching, same as an explicit unpush for each. A device
    /// left attached with no surviving session to unpush it is worse than
    /// detaching it here -- there would be no way to reach it again except
    /// locally on the controlled machine.
    pub fn close_all(&mut self) {
        for (_, entry) in self.channels.drain() {
            entry.cancel.cancel();
            if let Some(port) = *entry.attached_port.lock().unwrap() {
                detach_port(port);
            }
        }
    }
}

/// Fire-and-forget (spawns its own blocking task) so callers don't need to
/// already be inside one.
fn detach_port(port: i32) {
    tokio::task::spawn_blocking(move || {
        if crate::platform::run_usbip_privileged(&["detach", "-p", &port.to_string()]) {
            log::info!("usb push: detached port {}", port);
        } else {
            log::error!("usb push: failed to detach port {}", port);
        }
    });
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
    attached_port: Arc<Mutex<Option<i32>>>,
    cancel: CancellationToken,
    flow: Flow,
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

    let relay_tx = tx.clone();
    let relay_bus_id = bus_id.clone();
    let relay_attached_port = attached_port.clone();
    // Kept so a failed/cancelled attach can abort it below instead of
    // leaving it parked on an `accept()` that the peer's real `usbip
    // attach` process -- which only exists if our own attach succeeded --
    // will now never make.
    let accept_task = tokio::spawn(async move {
        match listener.accept().await {
            Ok((socket, _)) => {
                run_channel(id, relay_bus_id, socket, relay_tx, inbound, relay_attached_port, flow)
                    .await
            }
            Err(err) => log::error!("usb push: accept failed: {}", err),
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
            log::error!("usb push: blocking task failed: {}", err);
            None
        }
    };

    if cancel.is_cancelled() {
        // The controller already unpushed/closed while attach was still in
        // flight, so `handle_close` already ran and won't run again for
        // this id -- if attach succeeded anyway, detaching the port is on
        // us now instead of leaking it.
        accept_task.abort();
        if let Some(local_port) = local_port {
            log::info!(
                "usb push: {} attached on local port {} after channel {} was closed, detaching",
                bus_id, local_port, id
            );
            detach_port(local_port);
        }
        return;
    }

    match local_port {
        Some(local_port) => {
            log::info!(
                "usb push: attached {} on local port {} (channel {})",
                bus_id, local_port, id
            );
            *attached_port.lock().unwrap() = Some(local_port);
            send(&tx, push_result_msg(bus_id, String::new()));
        }
        None => {
            accept_task.abort();
            let message = format!("Failed to push {}", bus_id);
            log::error!("usb push: `usbip attach` failed: {}", message);
            send(&tx, push_result_msg(bus_id, message));
        }
    }
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
    attached_port: Arc<Mutex<Option<i32>>>,
    flow: Flow,
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
    let to_tunnel = tokio::spawn(async move {
        flow_read
            .socket_to_peer(reader, |chunk| {
                send(&tx_read, data_msg(id, chunk));
                true
            })
            .await;
        send(&tx_read, close_msg(id));
        // Otherwise a push whose remote transport just closes on its own (not
        // via an explicit unpush) leaves this port attached to the controlled
        // machine's vhci driver indefinitely -- nothing else ever detaches it
        // once this task returns.
        if let Some(port) = attached_port.lock().unwrap().take() {
            log::info!("usb push: channel {} relay ended, detaching port {}", id, port);
            detach_port(port);
        }
    });

    while let Some(msg) = inbound.recv().await {
        match msg {
            Inbound::Data(data) => {
                if writer.write_all(&data).await.is_err() {
                    break;
                }
                if let Some(add) = flow.drained(data.len()) {
                    send(&tx, usbip_flow::window_update_msg(id, add));
                }
            }
            Inbound::Opened { .. } => break,
        }
    }
    to_tunnel.abort();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> UsbPullState {
        let (tx, _rx) = mpsc::unbounded_channel();
        UsbPullState::new(tx)
    }

    fn fake_entry() -> (Entry, CancellationToken) {
        let (inbound, _rx) = mpsc::channel(1);
        let cancel = CancellationToken::new();
        (
            Entry {
                inbound,
                flow: Flow::new(),
                attached_port: Arc::new(Mutex::new(None)),
                cancel: cancel.clone(),
            },
            cancel,
        )
    }

    #[test]
    fn handle_close_on_unknown_channel_is_a_no_op() {
        let mut state = test_state();
        state.handle_close(UsbForwardClose {
            channel_id: -1,
            ..Default::default()
        });
        assert!(state.channels.is_empty());
    }

    #[test]
    fn handle_close_cancels_and_removes_the_channel() {
        let mut state = test_state();
        let (entry, cancel) = fake_entry();
        state.channels.insert(-1, entry);
        state.handle_close(UsbForwardClose {
            channel_id: -1,
            ..Default::default()
        });
        assert!(state.channels.is_empty());
        assert!(cancel.is_cancelled());
    }

    #[test]
    fn close_all_cancels_every_channel_and_clears_the_map() {
        let mut state = test_state();
        let tokens: Vec<_> = [-1, -2, -3]
            .into_iter()
            .map(|id| {
                let (entry, cancel) = fake_entry();
                state.channels.insert(id, entry);
                cancel
            })
            .collect();
        state.close_all();
        assert!(state.channels.is_empty());
        assert!(tokens.iter().all(|t| t.is_cancelled()));
    }

    #[tokio::test]
    async fn handle_push_request_prunes_stale_entries_before_checking_the_cap() {
        // Fill the map with `MAX_PENDING_PUSHES` already-closed entries --
        // `fake_entry()` drops its receiver immediately, so these look
        // exactly like channels whose `pull()` task already ended (failed
        // setup or a relay that hit EOF) without anything removing them.
        let mut state = test_state();
        for id in 0..MAX_PENDING_PUSHES as i32 {
            let (entry, _cancel) = fake_entry();
            state.channels.insert(id, entry);
        }
        assert_eq!(state.channels.len(), MAX_PENDING_PUSHES);
        state.handle_push_request("1-1".into());
        // The stale entries were pruned, so this push was accepted (one live
        // entry) instead of being wrongly rejected as "too many pending".
        assert_eq!(state.channels.len(), 1);
    }

    #[test]
    fn channel_ids_are_negative_and_decreasing() {
        // Negative so they can never collide with `usbip_mux.rs`'s
        // non-negative, controller-allocated ids -- see the sign dispatch
        // in `connection.rs::handle_usb_channel`.
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
}
