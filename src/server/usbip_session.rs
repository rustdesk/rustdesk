// Bundles the controlled side's two USB/IP state machines (`usbip_mux` for
// pull, `usbip_pull` for push) behind one type and one `handle` call, so
// `connection.rs` -- a shared file that AGENTS.md says should only carry thin
// one-line hooks for feature-specific work -- doesn't need to know the
// channel_id sign convention or dispatch between them itself.
use super::{connection::Sender, usbip_mux::UsbipMux, usbip_pull::UsbPullState};
use base::message_proto::*;
use hbb_common::log;

pub struct UsbSession {
    mux: UsbipMux,
    pull: UsbPullState,
}

impl UsbSession {
    pub fn new(tx: Sender) -> Self {
        Self {
            mux: UsbipMux::new(tx.clone()),
            pull: UsbPullState::new(tx),
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
                    log::debug!("usb push denied: no permission");
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::tokio::sync::mpsc;

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
