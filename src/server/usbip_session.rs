// Bundles the controlled side's two USB/IP state machines (`usbip_mux` for
// pull, `usbip_pull` for push) behind one type and one `handle` call, so
// `connection.rs` -- a shared file that AGENTS.md says should only carry thin
// one-line hooks for feature-specific work -- doesn't need to know the
// channel_id sign convention or dispatch between them itself.
use super::{
    connection::Sender,
    usbip_mux::{self, UsbipMux},
    usbip_pull::{self, UsbPullState},
};
use base::{config::keys, message_proto::*};
use hbb_common::{
    config::Config, log, rendezvous_proto::ControlPermissions, tokio::sync::Semaphore,
};
use std::sync::Arc;

pub struct UsbSession {
    mux: UsbipMux,
    pull: UsbPullState,
}

lazy_static::lazy_static! {
    // One privileged `usbip` step (and its elevation dialog) at a time across
    // every session and both directions, so concurrent sessions cannot bind,
    // attach or detach against each other.
    static ref PRIVILEGED: Arc<Semaphore> = Arc::new(Semaphore::new(1));
}

impl UsbSession {
    pub fn new(tx: Sender) -> Self {
        let privileged = PRIVILEGED.clone();
        usbip_mux::retry_pending_releases(privileged.clone());
        Self {
            mux: UsbipMux::new(tx.clone(), privileged.clone()),
            pull: UsbPullState::new(tx, privileged),
        }
    }

    /// A negative `channel_id` means the frame belongs to a push channel we
    /// opened ourselves (see `usbip_pull`'s doc comment); everything else,
    /// including `PushRequest` (no id yet) and any non-negative id, still
    /// means "peer pulling from us" and goes to the mux.
    pub fn handle(&mut self, ch: UsbChannel, permitted: impl FnOnce() -> bool) {
        let for_pull = match &ch.union {
            Some(usb_channel::Union::PushRequest(_)) => true,
            Some(usb_channel::Union::Opened(o)) => o.channel_id < 0,
            Some(usb_channel::Union::Data(d)) => d.channel_id < 0,
            Some(usb_channel::Union::Close(c)) => c.channel_id < 0,
            Some(usb_channel::Union::WindowUpdate(w)) => w.channel_id < 0,
            _ => false,
        };

        if !for_pull {
            self.mux.handle(ch, permitted);
            return;
        }

        match ch.union {
            Some(usb_channel::Union::PushRequest(r)) => {
                if !permitted() {
                    log::trace!("usb push denied: no permission");
                    self.pull.refuse_push_request(r.bus_id);
                    return;
                }
                self.pull.handle_push_request(r.bus_id);
            }
            Some(usb_channel::Union::Opened(o)) => self.pull.handle_opened(o),
            Some(usb_channel::Union::Data(d)) => self.pull.handle_data(d),
            Some(usb_channel::Union::Close(c)) => self.pull.handle_close(c),
            Some(usb_channel::Union::WindowUpdate(w)) => self.pull.handle_window_update(w),
            _ => {}
        }
    }

    pub fn close_all(&mut self) {
        self.mux.close_all();
        self.pull.close_all();
    }

    /// Answers a control request on a connection without the USB forwarding
    /// permission, without creating any USB state for it.
    pub fn refuse(tx: &Sender, ch: UsbChannel) {
        const DENIED: &str = "No permission of USB forwarding";
        let msg = match ch.union {
            Some(usb_channel::Union::ListDevices(_)) => usbip_mux::device_list_msg(Vec::new()),
            Some(usb_channel::Union::Bind(b)) => {
                usbip_mux::bind_result_msg(b.bus_id, b.bind, DENIED.into())
            }
            Some(usb_channel::Union::Open(o)) if o.channel_id >= 0 => {
                usbip_mux::opened_msg(o.channel_id, false, DENIED.into())
            }
            Some(usb_channel::Union::PushRequest(r)) => {
                usbip_pull::push_result_msg(r.bus_id, DENIED.into())
            }
            _ => return,
        };
        usbip_mux::send(tx, msg);
    }
}

/// USB forwarding follows the access mode like the other permissions, except
/// that an explicit denial, local (`enable-usbip=N`) or central
/// (`Permission.usb` off), also holds under Full Access, and Screen Share
/// never allows it.
pub fn permitted(control_permissions: &Option<ControlPermissions>) -> bool {
    use hbb_common::rendezvous_proto::control_permissions::Permission;
    let central = control_permissions
        .as_ref()
        .and_then(|p| crate::get_control_permission(p.permissions, Permission::usb));
    allowed(
        &Config::get_option(keys::OPTION_ACCESS_MODE),
        &Config::get_option(keys::OPTION_ENABLE_USBIP),
        central,
    )
}

fn allowed(access_mode: &str, local: &str, central: Option<bool>) -> bool {
    access_mode != "view" && local != "N" && central != Some(false)
}

/// A `UsbChannel` frame on an authorized desktop connection. USB state exists
/// only while the permission does: a control request without it is refused
/// and ends any USB forwarding the session had.
pub fn handle_channel(
    session: &mut Option<UsbSession>,
    tx: Option<&Sender>,
    ch: UsbChannel,
    permitted: impl Fn() -> bool,
) {
    let Some(tx) = tx else {
        return;
    };
    let data = matches!(
        ch.union,
        Some(usb_channel::Union::Data(_)) | Some(usb_channel::Union::WindowUpdate(_))
    );
    if !data && !permitted() {
        if let Some(mut session) = session.take() {
            session.close_all();
        }
        UsbSession::refuse(tx, ch);
        return;
    }
    if session.is_none() {
        if data {
            return;
        }
        *session = Some(UsbSession::new(tx.clone()));
    }
    if let Some(session) = session.as_mut() {
        session.handle(ch, permitted);
    }
}

/// Ends all USB forwarding once the permission is withdrawn.
pub fn revoke_unless_permitted(session: &mut Option<UsbSession>, permitted: impl FnOnce() -> bool) {
    if session.is_some() && !permitted() {
        log::info!("USB forwarding permission withdrawn, closing USB forwarding");
        if let Some(mut session) = session.take() {
            session.close_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::tokio::sync::mpsc;

    #[test]
    fn explicit_denial_wins_and_screen_share_never_allows() {
        // Full Access and Custom allow by default.
        assert!(allowed("full", "", None));
        assert!(allowed("", "", None));
        // An explicit local or central denial holds in every mode.
        assert!(!allowed("full", "N", None));
        assert!(!allowed("full", "", Some(false)));
        assert!(!allowed("full", "N", Some(true)));
        assert!(!allowed("", "Y", Some(false)));
        // A central allowance cannot open Screen Share.
        assert!(!allowed("view", "", Some(true)));
        assert!(!allowed("view", "Y", None));
    }

    #[test]
    fn bind_without_permission_is_refused_without_state() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut ch = UsbChannel::new();
        ch.set_bind(UsbBind {
            bus_id: "1-2".to_string(),
            bind: true,
            ..Default::default()
        });
        UsbSession::refuse(&tx, ch);
        let (_, msg) = rx.try_recv().expect("the controller must get a reply");
        match &msg.union {
            Some(message::Union::UsbChannel(ch)) => match &ch.union {
                Some(usb_channel::Union::BindResult(r)) => assert!(!r.error.is_empty()),
                other => panic!("expected a BindResult, got {:?}", other),
            },
            other => panic!("expected a UsbChannel message, got {:?}", other),
        }
    }

    #[test]
    fn denied_push_request_is_answered_with_an_error() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut session = UsbSession::new(tx);
        let mut ch = UsbChannel::new();
        ch.set_push_request(UsbPushRequest {
            bus_id: "1-2".to_string(),
            ..Default::default()
        });
        session.handle(ch, || false);
        let (_, msg) = rx.try_recv().expect("the controller must get a reply");
        match &msg.union {
            Some(message::Union::UsbChannel(ch)) => match &ch.union {
                Some(usb_channel::Union::PushResult(r)) => {
                    assert_eq!(r.bus_id, "1-2");
                    assert!(!r.error.is_empty());
                }
                other => panic!("expected a PushResult, got {:?}", other),
            },
            other => panic!("expected a UsbChannel message, got {:?}", other),
        }
        assert!(rx.try_recv().is_err());
    }
}
