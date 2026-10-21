// Controller-side USB forwarding state for one session: the relay channels in
// both directions, push authorizations, what the session attached or bound
// itself, and the session's runtime handle. `FlutterHandler` holds one of
// these and otherwise only forwards to it.
use crate::{client::usbip_attach::Inbound, usbip_flow::Flow};
use hbb_common::{
    bytes::Bytes,
    tokio::{runtime::Handle, sync::mpsc},
    tokio_util::sync::CancellationToken,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc, RwLock,
    },
};

/// A registered relay channel. Dropping it, by whichever path removes it,
/// stops the relay even while it is blocked on its local socket.
pub(crate) struct Relay<T> {
    tx: mpsc::Sender<T>,
    flow: Flow,
    cancel: CancellationToken,
}

impl<T> Relay<T> {
    fn new(tx: mpsc::Sender<T>, flow: Flow) -> Self {
        Self {
            tx,
            flow,
            cancel: CancellationToken::new(),
        }
    }
}

impl<T> Drop for Relay<T> {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[derive(Default)]
pub(crate) struct UsbClientState {
    // Local usbip-attach relay channels (pull direction), keyed by the id
    // chosen when the channel opened.
    forward_channels: RwLock<HashMap<i32, Relay<Inbound>>>,
    // Local usbip-share relay channels (push direction), keyed by the
    // negative channel_id the peer's usbip_pull chose.
    share_channels: RwLock<HashMap<i32, Relay<Bytes>>>,
    // bus_id -> the channel_id above, so "unpush" (given only a bus_id) can
    // find the channel to close.
    share_bus_ids: RwLock<HashMap<String, i32>>,
    // bus_ids this side has asked the peer to attach (`usb_push_request`
    // sent, no `Open` seen yet) -- gates the `Open` handler so a peer can't
    // pull an arbitrary locally-shared device it was never offered.
    share_pending: RwLock<HashSet<String>>,
    // bus_ids this session bound and pushed and has not unpushed yet -- the
    // only ones a `PushResult` may touch.
    share_owned: RwLock<HashSet<String>>,
    // bus_ids in `share_owned` whose unpush or rollback is running; the
    // record itself stays until that succeeds.
    share_releasing: RwLock<HashSet<String>>,
    // Forward channel id -> the peer's bus id and the local vhci port it is
    // attached on. The import lives on that channel's relay socket, so it
    // is recorded only while the relay is registered and goes with it.
    attached: RwLock<HashMap<i32, (String, i32)>>,
    // Set once `close` has run: a bind finishing after that must be undone
    // by whoever finished it.
    closed: AtomicBool,
    // This session's own `io_loop` runtime, registered once `io_loop` starts
    // running on it -- lets `usb_attach`/`usb_push`/etc, called from
    // Flutter's FFI thread pool, `spawn`/`spawn_blocking` onto it instead of
    // a separate runtime. Tagged with the registering round so a stale
    // round's unregister can't clear a newer round's handle out from under
    // it on reconnect.
    session_runtime: RwLock<Option<(u32, Handle)>>,
}

impl UsbClientState {
    /// Non-negative, so the controller's own channel ids can never collide
    /// with `usbip_pull.rs`'s negative, controlled-side ones -- see the sign
    /// dispatch in `usbip_session::UsbSession::handle`.
    pub(crate) fn next_channel_id() -> i32 {
        static NEXT_ID: AtomicI32 = AtomicI32::new(1);
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    /// The returned token is cancelled once the channel is removed; the
    /// relay runs under it.
    pub(crate) fn register_forward_channel(
        &self,
        channel_id: i32,
        tx: mpsc::Sender<Inbound>,
        flow: Flow,
    ) -> CancellationToken {
        let relay = Relay::new(tx, flow);
        let cancel = relay.cancel.clone();
        self.forward_channels.write().unwrap().insert(channel_id, relay);
        cancel
    }

    /// Dropping the relay closes its socket, and with it ends the import on
    /// that socket: the kernel detaches the port.
    pub(crate) fn unregister_forward_channel(&self, channel_id: i32) {
        let mut channels = self.forward_channels.write().unwrap();
        channels.remove(&channel_id);
        self.attached.write().unwrap().remove(&channel_id);
    }

    /// `true` if the channel had to be dropped; the caller then tells the
    /// peer, which otherwise keeps its side alive until the session ends.
    pub(crate) fn forward_send(&self, channel_id: i32, msg: Inbound) -> bool {
        let failed = match self.forward_channels.read().unwrap().get(&channel_id) {
            Some(relay) => {
                let admitted = match &msg {
                    Inbound::Data(data) => relay.flow.admit(data.len()),
                    _ => true,
                };
                // Over the window, or the relay task already ended: an
                // admitted frame always fits (`usbip_flow::QUEUE_FRAMES`).
                !admitted || relay.tx.try_send(msg).is_err()
            }
            None => return false,
        };
        if failed {
            hbb_common::throttled_log!(
                crate::usbip_flow::PEER_LOG_INTERVAL,
                warn,
                "usb forward: closing channel {} (window overrun or relay gone)",
                channel_id
            );
            self.unregister_forward_channel(channel_id);
        }
        failed
    }

    /// As for `register_forward_channel`.
    pub(crate) fn register_share_channel(
        &self,
        channel_id: i32,
        bus_id: String,
        tx: mpsc::Sender<Bytes>,
        flow: Flow,
    ) -> CancellationToken {
        let relay = Relay::new(tx, flow);
        let cancel = relay.cancel.clone();
        self.share_channels.write().unwrap().insert(channel_id, relay);
        self.share_bus_ids.write().unwrap().insert(bus_id, channel_id);
        cancel
    }

    pub(crate) fn unregister_share_channel(&self, channel_id: i32) {
        self.share_channels.write().unwrap().remove(&channel_id);
        self.share_bus_ids
            .write()
            .unwrap()
            .retain(|_, id| *id != channel_id);
    }

    /// For a push relay ending itself: removes `channel_id` only while it is
    /// still that relay's registration. After a reconnect the peer numbers
    /// push channels from -1 again, and a late relay of the old connection
    /// must not drop the new one.
    pub(crate) fn unregister_share_channel_of(&self, channel_id: i32, flow: &Flow) {
        let mut channels = self.share_channels.write().unwrap();
        if !channels.get(&channel_id).is_some_and(|relay| relay.flow.same(flow)) {
            return;
        }
        channels.remove(&channel_id);
        drop(channels);
        self.share_bus_ids
            .write()
            .unwrap()
            .retain(|_, id| *id != channel_id);
    }

    pub(crate) fn share_channel_for_bus_id(&self, bus_id: &str) -> Option<i32> {
        self.share_bus_ids.read().unwrap().get(bus_id).copied()
    }

    pub(crate) fn share_channel_live(&self, channel_id: i32) -> bool {
        self.share_channels.read().unwrap().contains_key(&channel_id)
    }

    /// `true` if the channel had to be dropped, as for `forward_send`.
    pub(crate) fn share_send(&self, channel_id: i32, data: Bytes) -> bool {
        let failed = match self.share_channels.read().unwrap().get(&channel_id) {
            Some(relay) => !relay.flow.admit(data.len()) || relay.tx.try_send(data).is_err(),
            None => return false,
        };
        if failed {
            hbb_common::throttled_log!(
                crate::usbip_flow::PEER_LOG_INTERVAL,
                warn,
                "usb push: closing channel {} (window overrun or relay gone)",
                channel_id
            );
            self.unregister_share_channel(channel_id);
        }
        failed
    }

    /// Credit the peer returned on channel `channel_id`, whichever direction
    /// it belongs to (see the sign convention in `client::usbip_channel`).
    /// `true` if the channel had to be dropped for claiming more than it was
    /// sent, as for `forward_send`.
    pub(crate) fn window_update(&self, channel_id: i32, add: u32) -> bool {
        let flow = if channel_id >= 0 {
            self.forward_channels
                .read()
                .unwrap()
                .get(&channel_id)
                .map(|relay| relay.flow.clone())
        } else {
            self.share_channels
                .read()
                .unwrap()
                .get(&channel_id)
                .map(|relay| relay.flow.clone())
        };
        if flow.map_or(true, |flow| flow.grant(add)) {
            return false;
        }
        hbb_common::throttled_log!(
            crate::usbip_flow::PEER_LOG_INTERVAL,
            warn,
            "usb: closing channel {} (window update for more than was sent)",
            channel_id
        );
        if channel_id >= 0 {
            self.unregister_forward_channel(channel_id);
        } else {
            self.unregister_share_channel(channel_id);
        }
        true
    }

    pub(crate) fn share_pending_add(&self, bus_id: String) {
        self.share_pending.write().unwrap().insert(bus_id);
    }

    /// Removes and reports whether `bus_id` was pending -- used to gate the
    /// `Open` handler so it only accepts a channel for a device this side
    /// actually asked the peer to attach.
    pub(crate) fn share_pending_take(&self, bus_id: &str) -> bool {
        self.share_pending.write().unwrap().remove(bus_id)
    }

    pub(crate) fn share_pending_remove(&self, bus_id: &str) {
        self.share_pending.write().unwrap().remove(bus_id);
    }

    /// `false` once the session is closing; the caller must then undo its
    /// own bind of `bus_id`, since teardown has already run.
    pub(crate) fn share_owned_add(&self, bus_id: String) -> bool {
        let mut owned = self.share_owned.write().unwrap();
        if self.closed.load(Ordering::SeqCst) {
            return false;
        }
        owned.insert(bus_id);
        true
    }

    pub(crate) fn share_owned(&self, bus_id: &str) -> bool {
        self.share_owned.read().unwrap().contains(bus_id)
    }

    /// Starts undoing a push of `bus_id` by this session -- `false` if it
    /// wasn't pushed here or is already being undone -- so at most one
    /// caller unbinds at a time.
    pub(crate) fn share_release_begin(&self, bus_id: &str) -> bool {
        self.share_owned(bus_id)
            && self
                .share_releasing
                .write()
                .unwrap()
                .insert(bus_id.to_string())
    }

    /// Ends what `share_release_begin` started. The push stays recorded
    /// unless it was undone, so a retry or `close` can still release it.
    /// `false` if it failed after the session closed: nothing here tracks
    /// it any more and the caller must keep it pending.
    pub(crate) fn share_release_end(&self, bus_id: &str, released: bool) -> bool {
        let mut owned = self.share_owned.write().unwrap();
        let tracked = released || !self.closed.load(Ordering::SeqCst);
        if released || !tracked {
            owned.remove(bus_id);
        }
        self.share_releasing.write().unwrap().remove(bus_id);
        tracked
    }

    /// Starts undoing every push not already being undone.
    pub(crate) fn share_release_all_begin(&self) -> Vec<String> {
        let owned = self.share_owned.read().unwrap();
        let mut releasing = self.share_releasing.write().unwrap();
        owned
            .iter()
            .filter(|bus_id| releasing.insert(bus_id.to_string()))
            .cloned()
            .collect()
    }

    /// Records the port `usbip attach` reported for the import on forward
    /// channel `channel_id`. `false` if that relay is already gone: the
    /// import ended with its socket and there is nothing to own.
    pub(crate) fn attached_port_add(&self, channel_id: i32, bus_id: String, port: i32) -> bool {
        let channels = self.forward_channels.read().unwrap();
        if !channels.contains_key(&channel_id) {
            return false;
        }
        self.attached.write().unwrap().insert(channel_id, (bus_id, port));
        true
    }

    /// The forward channel carrying this session's import of the peer's
    /// `bus_id` (a peer exports each bus id once).
    pub(crate) fn attached_channel_of(&self, bus_id: &str) -> Option<i32> {
        self.attached
            .read()
            .unwrap()
            .iter()
            .find_map(|(channel_id, (b, _))| (b == bus_id).then_some(*channel_id))
    }

    /// The local port this session attached the peer's `bus_id` on, for the
    /// device list to show what is actually attached.
    pub(crate) fn attached_port_of(&self, bus_id: &str) -> Option<i32> {
        self.attached
            .read()
            .unwrap()
            .values()
            .find_map(|(b, port)| (b == bus_id).then_some(*port))
    }

    /// The runtime `register_runtime` recorded once `io_loop` started running
    /// on it, for `usb_attach`/`usb_push`/etc (called from Flutter's FFI
    /// thread pool, which has none of its own) to `spawn`/`spawn_blocking`
    /// onto instead of creating a separate one. `None` only if called before
    /// the session's `io_loop` has started or after it's already torn down.
    pub(crate) fn session_runtime(&self) -> Option<Handle> {
        self.session_runtime
            .read()
            .unwrap()
            .as_ref()
            .map(|(_, handle)| handle.clone())
    }

    /// The runtime and the connection round it belongs to, for an operation
    /// to check with `in_round` once it finishes.
    pub(crate) fn session_runtime_round(&self) -> Option<(Handle, u32)> {
        self.session_runtime
            .read()
            .unwrap()
            .as_ref()
            .map(|(round, handle)| (handle.clone(), *round))
    }

    /// Runs `f` only while `round` is still the registered one, so a late
    /// operation of an earlier connection cannot act on a newer one: a new
    /// round registers here before it installs its sender.
    pub(crate) fn in_round<R>(&self, round: u32, f: impl FnOnce() -> R) -> Option<R> {
        let guard = self.session_runtime.read().unwrap();
        (guard.as_ref().map(|(r, _)| *r) == Some(round)).then(f)
    }

    /// Relay channels belong to the connection that opened them, so a new
    /// round starts without any: whatever an earlier round left behind (its
    /// relays may have been cancelled before they could unregister) goes.
    pub(crate) fn register_runtime(&self, round: u32, handle: Handle) {
        *self.session_runtime.write().unwrap() = Some((round, handle));
        self.clear_channels();
    }

    /// Only clears the slot if `round` is still the one registered --
    /// otherwise a newer round already overwrote it (reconnect started a
    /// new `io_loop` before this one finished tearing down), and clearing
    /// unconditionally would drop that newer, still-live handle instead of
    /// this stale one. `true` if it did, i.e. the connection ended.
    pub(crate) fn unregister_runtime(&self, round: u32) -> bool {
        let mut guard = self.session_runtime.write().unwrap();
        if guard.as_ref().map(|(r, _)| *r) != Some(round) {
            return false;
        }
        *guard = None;
        drop(guard);
        self.clear_channels();
        true
    }

    /// Dropping every relay ends it and closes its socket, which also ends
    /// every import this session attached. Pushes (`share_owned`) are left
    /// to the caller.
    fn clear_channels(&self) {
        let mut forward = self.forward_channels.write().unwrap();
        forward.clear();
        self.attached.write().unwrap().clear();
        drop(forward);
        self.share_channels.write().unwrap().clear();
        self.share_bus_ids.write().unwrap().clear();
        self.share_pending.write().unwrap().clear();
    }

    /// Called once the session's last UI session closes. A relay task
    /// (`client::usbip_attach`/`usbip_share`'s `run_channel`) holds its own
    /// clone of `Session<FlutterHandler>`, so it keeps running -- and, for
    /// the push direction, keeps its local `usbipd` connection to the
    /// shared device open -- even after nothing else can reach this state
    /// to ask it to stop. Dropping every registered sender here makes each
    /// task's next `recv()` return `None`, so it notices and cleans itself
    /// up instead of running orphaned indefinitely, and closing their
    /// sockets ends this session's imports. Every push is unshared; one that
    /// fails stays pending process-wide (`usbip_share::release_push`).
    pub(crate) fn close(self: &Arc<Self>) {
        {
            let _owned = self.share_owned.write().unwrap();
            self.closed.store(true, Ordering::SeqCst);
        }
        self.clear_channels();
        let releasing = self.share_release_all_begin();
        if releasing.is_empty() {
            return;
        }
        // The session's runtime is going away with it, so a plain thread.
        let state = self.clone();
        std::thread::spawn(move || {
            for bus_id in releasing {
                crate::client::usbip_share::release_push(&state, &bus_id);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_ids_are_positive_and_increasing() {
        let first = UsbClientState::next_channel_id();
        let second = UsbClientState::next_channel_id();
        assert!(first >= 0);
        assert!(second > first);
    }

    #[tokio::test]
    async fn late_attach_of_an_older_round_neither_registers_nor_reaches_the_newer_one() {
        let state = UsbClientState::default();
        state.register_runtime(1, Handle::current());
        let id = UsbClientState::next_channel_id();
        let (tx, _rx) = mpsc::channel(1);
        state.register_forward_channel(id, tx, Flow::new());
        // Reconnect: the new round drops the old round's relays.
        state.register_runtime(2, Handle::current());
        assert!(!state.attached_port_add(id, "1-1".into(), 0));
        assert_eq!(state.attached_port_of("1-1"), None);
        assert!(state.in_round(1, || ()).is_none());
        assert!(state.in_round(2, || ()).is_some());
    }

    #[test]
    fn attachment_goes_with_its_relay() {
        let state = UsbClientState::default();
        let id = UsbClientState::next_channel_id();
        let (tx, _rx) = mpsc::channel(1);
        state.register_forward_channel(id, tx, Flow::new());
        assert!(state.attached_port_add(id, "1-1".into(), 3));
        assert_eq!(state.attached_port_of("1-1"), Some(3));
        state.unregister_forward_channel(id);
        assert_eq!(state.attached_port_of("1-1"), None);
    }

    #[test]
    fn open_for_unpushed_bus_id_is_not_authorized() {
        // `share_pending_take` gates the `Open` handler: a peer that was
        // never offered a device via `usb_push` must not get a channel just
        // by naming its bus_id.
        let state = UsbClientState::default();
        assert!(!state.share_pending_take("1-1"));

        state.share_pending_add("1-1".into());
        assert!(state.share_pending_take("1-1"));
        // Consumed by the first take -- a second `Open` for the same bus_id
        // must not be authorized by the same push.
        assert!(!state.share_pending_take("1-1"));
    }
}
