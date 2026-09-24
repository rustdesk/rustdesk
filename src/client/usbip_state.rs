// Controller-side RemoteUsb state for one session: the relay channels in
// both directions, push authorizations, what the session attached or bound
// itself, and the session's runtime handle. `FlutterHandler` holds one of
// these and otherwise only forwards to it.
use crate::{client::usbip_attach::Inbound, usbip_flow::Flow};
use hbb_common::{
    bytes::Bytes,
    log,
    tokio::{runtime::Handle, sync::mpsc},
};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        RwLock,
    },
};

#[derive(Default)]
pub(crate) struct UsbClientState {
    // Local usbip-attach relay channels (pull direction), keyed by the id
    // chosen when the channel opened.
    forward_channels: RwLock<HashMap<i32, (mpsc::Sender<Inbound>, Flow)>>,
    // Local usbip-share relay channels (push direction), keyed by the
    // negative channel_id the peer's usbip_pull chose.
    share_channels: RwLock<HashMap<i32, (mpsc::Sender<Bytes>, Flow)>>,
    // bus_id -> the channel_id above, so "unpush" (given only a bus_id) can
    // find the channel to close.
    share_bus_ids: RwLock<HashMap<String, i32>>,
    // bus_ids this side has asked the peer to attach (`usb_push_request`
    // sent, no `Open` seen yet) -- gates the `Open` handler so a peer can't
    // pull an arbitrary locally-shared device it was never offered.
    share_pending: RwLock<HashSet<String>>,
    // bus_ids this session pushed and has not unpushed yet -- the only ones a
    // `PushResult` may touch -- mapped to whether it also bound them itself
    // (`false` for a device already shared by the CLI or another session),
    // since only those may be unbound again.
    share_owned: RwLock<HashMap<String, bool>>,
    // Local vhci ports this session attached (pull direction) and has not
    // detached yet.
    attached_ports: RwLock<HashSet<i32>>,
    // Set once `close` has run: an attach or bind finishing after that must
    // be undone by whoever finished it.
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
    /// dispatch in `connection.rs::handle_usb_channel`.
    pub(crate) fn next_channel_id() -> i32 {
        static NEXT_ID: AtomicI32 = AtomicI32::new(1);
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn register_forward_channel(
        &self,
        channel_id: i32,
        tx: mpsc::Sender<Inbound>,
        flow: Flow,
    ) {
        self.forward_channels
            .write()
            .unwrap()
            .insert(channel_id, (tx, flow));
    }

    pub(crate) fn unregister_forward_channel(&self, channel_id: i32) {
        self.forward_channels.write().unwrap().remove(&channel_id);
    }

    /// `true` if the channel had to be dropped; the caller then tells the
    /// peer, which otherwise keeps its side alive until the session ends.
    pub(crate) fn forward_send(&self, channel_id: i32, msg: Inbound) -> bool {
        let failed = match self.forward_channels.read().unwrap().get(&channel_id) {
            Some((tx, flow)) => {
                let admitted = match &msg {
                    Inbound::Data(data) => flow.admit(data.len()),
                    _ => true,
                };
                // Over the window, or the relay task already ended: an
                // admitted frame always fits (`usbip_flow::QUEUE_FRAMES`).
                !admitted || tx.try_send(msg).is_err()
            }
            None => return false,
        };
        if failed {
            log::warn!("usb forward: closing channel {} (window overrun or relay gone)", channel_id);
            self.unregister_forward_channel(channel_id);
        }
        failed
    }

    pub(crate) fn register_share_channel(
        &self,
        channel_id: i32,
        bus_id: String,
        tx: mpsc::Sender<Bytes>,
        flow: Flow,
    ) {
        self.share_channels.write().unwrap().insert(channel_id, (tx, flow));
        self.share_bus_ids.write().unwrap().insert(bus_id, channel_id);
    }

    pub(crate) fn unregister_share_channel(&self, channel_id: i32) {
        self.share_channels.write().unwrap().remove(&channel_id);
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
            Some((tx, flow)) => !flow.admit(data.len()) || tx.try_send(data).is_err(),
            None => return false,
        };
        if failed {
            log::warn!("usb push: closing channel {} (window overrun or relay gone)", channel_id);
            self.unregister_share_channel(channel_id);
        }
        failed
    }

    /// Credit the peer returned on channel `channel_id`, whichever direction
    /// it belongs to (see the sign convention in `client::usbip_channel`).
    pub(crate) fn window_update(&self, channel_id: i32, add: u32) {
        let flow = if channel_id >= 0 {
            self.forward_channels
                .read()
                .unwrap()
                .get(&channel_id)
                .map(|(_, flow)| flow.clone())
        } else {
            self.share_channels
                .read()
                .unwrap()
                .get(&channel_id)
                .map(|(_, flow)| flow.clone())
        };
        if let Some(flow) = flow {
            flow.grant(add);
        }
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
    pub(crate) fn share_owned_add(&self, bus_id: String, bound: bool) -> bool {
        let mut owned = self.share_owned.write().unwrap();
        if self.closed.load(Ordering::SeqCst) {
            return false;
        }
        owned.insert(bus_id, bound);
        true
    }

    pub(crate) fn share_owned(&self, bus_id: &str) -> bool {
        self.share_owned.read().unwrap().contains_key(bus_id)
    }

    /// Removes a push of `bus_id` by this session, reporting whether this
    /// session also bound it -- `None` if it wasn't pushed here -- so at
    /// most one caller undoes that binding.
    pub(crate) fn share_owned_take(&self, bus_id: &str) -> Option<bool> {
        self.share_owned.write().unwrap().remove(bus_id)
    }

    /// `false` once the session is closing; the caller must then detach
    /// `port` itself, since teardown has already run.
    pub(crate) fn attached_port_add(&self, port: i32) -> bool {
        let mut ports = self.attached_ports.write().unwrap();
        if self.closed.load(Ordering::SeqCst) {
            return false;
        }
        ports.insert(port);
        true
    }

    pub(crate) fn attached_port_take(&self, port: i32) -> bool {
        self.attached_ports.write().unwrap().remove(&port)
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

    pub(crate) fn register_runtime(&self, round: u32, handle: Handle) {
        *self.session_runtime.write().unwrap() = Some((round, handle));
    }

    /// Only clears the slot if `round` is still the one registered --
    /// otherwise a newer round already overwrote it (reconnect started a
    /// new `io_loop` before this one finished tearing down), and clearing
    /// unconditionally would drop that newer, still-live handle instead of
    /// this stale one.
    pub(crate) fn unregister_runtime(&self, round: u32) {
        let mut guard = self.session_runtime.write().unwrap();
        if guard.as_ref().map(|(r, _)| *r) == Some(round) {
            *guard = None;
        }
    }

    /// Called once the session's last UI session closes. A relay task
    /// (`client::usbip_attach`/`usbip_share`'s `run_channel`) holds its own
    /// clone of `Session<FlutterHandler>`, so it keeps running -- and, for
    /// the push direction, keeps its local `usbipd` connection to the
    /// shared device open -- even after nothing else can reach this state
    /// to ask it to stop. Dropping every registered sender here makes each
    /// task's next `recv()` return `None`, so it notices and cleans itself
    /// up instead of running orphaned indefinitely.
    pub(crate) fn close(&self) {
        self.forward_channels.write().unwrap().clear();
        self.share_channels.write().unwrap().clear();
        self.share_bus_ids.write().unwrap().clear();
        self.share_pending.write().unwrap().clear();
        // Only what this session attached or bound itself is released: a
        // device shared by the CLI or another session is left alone.
        let ports = {
            let mut ports = self.attached_ports.write().unwrap();
            self.closed.store(true, Ordering::SeqCst);
            std::mem::take(&mut *ports)
        };
        let owned: Vec<String> = std::mem::take(&mut *self.share_owned.write().unwrap())
            .into_iter()
            .filter_map(|(bus_id, bound)| bound.then_some(bus_id))
            .collect();
        if ports.is_empty() && owned.is_empty() {
            return;
        }
        // The session's runtime is going away with it, so a plain thread.
        std::thread::spawn(move || {
            for port in ports {
                crate::client::usbip_attach::detach_blocking(port);
            }
            for bus_id in owned {
                crate::client::usbip_share::unbind_device_retrying(&bus_id);
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

    #[test]
    fn attach_or_bind_finishing_after_close_is_left_to_the_caller() {
        let state = UsbClientState::default();
        state.close();
        assert!(!state.share_owned_add("1-3".into(), true));
        assert!(!state.attached_port_add(4));
        assert!(!state.share_owned("1-3"));
        assert!(!state.attached_port_take(4));
    }

    #[test]
    fn push_of_a_device_shared_elsewhere_is_not_unbound() {
        let state = UsbClientState::default();
        assert!(state.share_owned_add("1-2".into(), false));
        assert!(state.share_owned_add("1-3".into(), true));
        assert!(state.share_owned("1-2"));
        assert_eq!(state.share_owned_take("1-2"), Some(false));
        assert_eq!(state.share_owned_take("1-3"), Some(true));
        assert_eq!(state.share_owned_take("1-3"), None);
    }
}
