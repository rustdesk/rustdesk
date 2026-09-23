// Client-side coordinator for `RemoteUsb` `UsbChannel` frames, mirroring
// `server/usbip_session.rs`'s role on the controlled side: `flutter.rs`'s
// `handle_usb_channel` is a shared trait-method implementation touched by
// every message type the session receives, so the actual routing/
// authorization/task-spawning logic belongs here instead of inline there,
// per AGENTS.md's "shared files carry thin one-line hooks" rule.
use crate::{flutter::FlutterHandler, ui_session_interface::InvokeUiSession};
use base::message_proto::usb_channel::Union;
use hbb_common::{log, tokio};

// Each queued chunk is up to 64KiB (the relay tasks' read buffer size), so
// this bounds one relay channel to a few MiB, not unbounded process memory --
// see the identical comment in `server/usbip_mux.rs`.
const USB_RELAY_CHANNEL_CAPACITY: usize = 256;

/// Everything except `DeviceList`/`BindResult`, which `flutter.rs` handles
/// itself (plain event translation shared by every message type, not
/// RemoteUsb-specific routing).
pub(crate) fn handle(handler: &FlutterHandler, union: Option<Union>) {
    match union {
        Some(Union::PushResult(r)) => {
            // `r.bus_id` is peer-supplied: only a device this session pushed
            // itself may be touched, and a failed result is honored at most
            // once, so an unsolicited or repeated one can't trigger a
            // privileged unbind.
            if !crate::client::usbip_attach::is_valid_bus_id(&r.bus_id)
                || !handler.usb_share_owned(&r.bus_id)
            {
                log::warn!(
                    "usb push: ignoring result for {:?}: not pushed by this session",
                    r.bus_id
                );
                return;
            }
            if r.error.is_empty() {
                log::info!("usb push: peer confirmed attach of {}", r.bus_id);
            } else {
                log::error!(
                    "usb push: peer failed to attach {}: {}, rolling back share",
                    r.bus_id, r.error
                );
                handler.usb_share_pending_remove(&r.bus_id);
                if !handler.usb_share_owned_take(&r.bus_id) {
                    return;
                }
                if let Some(rt) = handler.session_runtime() {
                    let bus_id = r.bus_id.clone();
                    rt.spawn_blocking(move || {
                        if crate::client::usbip_share::bind_device(&bus_id, false) {
                            log::info!("usb push: rolled back share of {}", bus_id);
                        } else {
                            log::error!("usb push: rollback unshare of {} also failed", bus_id);
                        }
                    });
                }
            }
            let event_data: Vec<(&str, serde_json::Value)> = vec![
                ("bus_id", serde_json::json!(&r.bus_id)),
                ("error", serde_json::json!(&r.error)),
            ];
            handler.push_event_("usb_push_result", &event_data, &[], &[]);
        }
        // `channel_id >= 0` is the local usbip-attach relay (pull,
        // `client::usbip_attach`); `< 0` is the local usbip-share relay
        // (push, `client::usbip_share`) -- see `usbip_pull.rs`'s doc
        // comment on the controlled side for the sign convention.
        Some(Union::Opened(o)) if o.channel_id >= 0 => handler.usb_forward_send(
            o.channel_id,
            crate::client::usbip_attach::Inbound::Opened {
                success: o.success,
                message: o.message,
            },
        ),
        Some(Union::Data(d)) if d.channel_id >= 0 => handler
            .usb_forward_send(d.channel_id, crate::client::usbip_attach::Inbound::Data(d.data)),
        Some(Union::Close(c)) if c.channel_id >= 0 => handler
            .usb_forward_send(c.channel_id, crate::client::usbip_attach::Inbound::Closed),
        Some(Union::Data(d)) => handler.usb_share_send(d.channel_id, d.data),
        Some(Union::Close(c)) => {
            log::info!("usb push: peer closed channel {}", c.channel_id);
            handler.unregister_usb_share_channel(c.channel_id);
        }
        // The peer pulling one of the devices we're sharing. Only ever
        // valid for a negative id (see the sign convention above) and
        // only for a bus_id we actually offered via `usb_push` -- a
        // locally shared device stays reachable through the OS's own
        // `usbipd` to anyone who can open a channel here at all, so
        // accepting an `Open` for a bus_id we never pushed would let any
        // authenticated peer pull any device we happen to have shared,
        // not just the one they were offered.
        Some(Union::Open(open)) if open.channel_id < 0 => {
            let id = open.channel_id;
            if handler.usb_share_channel_live(id) {
                return;
            }
            if !handler.usb_share_pending_take(&open.bus_id) {
                log::warn!(
                    "usb push: rejecting open for {} (channel {}): not pending",
                    open.bus_id, id
                );
                return;
            }
            let Some(session) = handler.any_session() else {
                log::error!("usb push: peer opened channel {} but no session to relay it", id);
                return;
            };
            log::info!("usb push: peer opened channel {} for {}", id, open.bus_id);
            let (tx, rx) = tokio::sync::mpsc::channel(USB_RELAY_CHANNEL_CAPACITY);
            handler.register_usb_share_channel(id, open.bus_id.clone(), tx);
            tokio::spawn(crate::client::usbip_share::run_channel(
                id,
                open.bus_id,
                session,
                rx,
            ));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::message_proto::UsbPushResult;

    fn failed_push(bus_id: &str) -> Option<Union> {
        Some(Union::PushResult(UsbPushResult {
            bus_id: bus_id.to_string(),
            error: "attach failed".to_string(),
            ..Default::default()
        }))
    }

    #[test]
    fn failed_push_result_ignored_unless_pushed_by_this_session() {
        let handler = FlutterHandler::default();
        handler.usb_share_owned_add("1-3".to_string());
        handle(&handler, failed_push("1-2"));
        handle(&handler, failed_push("1-3; touch /tmp/x"));
        assert!(handler.usb_share_owned("1-3"));
    }

    #[test]
    fn failed_push_result_rolls_back_only_once() {
        let handler = FlutterHandler::default();
        handler.usb_share_owned_add("1-2".to_string());
        handle(&handler, failed_push("1-2"));
        assert!(!handler.usb_share_owned("1-2"));
        assert!(!handler.usb_share_owned_take("1-2"));
    }
}
