use super::{FlutterSession, SessionID};
use crate::ui_session_interface::InvokeUiSession;

pub(crate) fn refresh_capture(session: &FlutterSession, session_id: &SessionID) {
    let mut handlers = session.ui_handler.session_handlers.write().unwrap();
    let Some(handler) = handlers.get_mut(session_id) else {
        return;
    };
    // Read the current topology instead of indices from an older UI event.
    let count = session.ui_handler.peer_info.read().unwrap().displays.len();
    handler.displays = (0..count).collect();
    if count == 1 {
        session.ui_handler.next_rgba(0);
    }
    // All displays includes every valid selection in the other windows.
    // Switching displays here would also restore a saved custom resolution.
    session.capture_displays(vec![], vec![], (0..count as i32).collect());
    for display in 0..count {
        session.refresh_video(display as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        client::Data,
        flutter::{FlutterHandler, RgbaData, SessionHandler},
        ui_session_interface::Session,
    };
    use base::message_proto::{misc, DisplayInfo};
    use hbb_common::{get_version_number, tokio::sync::mpsc};
    use std::sync::Arc;

    #[test]
    fn hotplug_refresh_only_updates_capture() {
        let session = Arc::new(Session::<FlutterHandler>::default());
        let (sender, mut receiver) = mpsc::unbounded_channel();
        *session.sender.write().unwrap() = Some(sender);
        session.lc.write().unwrap().version = get_version_number("1.4.9");
        let id = SessionID::new_v4();
        session
            .ui_handler
            .session_handlers
            .write()
            .unwrap()
            .insert(id, SessionHandler::default());

        for count in [2, 1, 0, 1, 3, 2] {
            session.ui_handler.peer_info.write().unwrap().displays =
                vec![DisplayInfo::default(); count];
            session.ui_handler.display_rgbas.write().unwrap().insert(
                0,
                RgbaData {
                    valid: true,
                    ..Default::default()
                },
            );
            refresh_capture(&session, &id);
            assert_eq!(
                session.ui_handler.session_handlers.read().unwrap()[&id].displays,
                (0..count).collect::<Vec<_>>()
            );
            let Data::Message(message) = receiver.try_recv().unwrap() else {
                panic!("Expected capture subscriptions");
            };
            assert!(matches!(
                message.misc().union,
                Some(misc::Union::CaptureDisplays(_))
            ));
            assert_eq!(
                message.misc().capture_displays().set,
                (0..count as i32).collect::<Vec<_>>()
            );
            for display in 0..count {
                let Data::Message(message) = receiver.try_recv().unwrap() else {
                    panic!("Expected a video refresh");
                };
                assert!(matches!(message.misc().union,
                    Some(misc::Union::RefreshVideoDisplay(index)) if index == display as i32));
            }
            assert!(receiver.try_recv().is_err());
            if count == 1 {
                assert!(!session.ui_handler.display_rgbas.read().unwrap()[&0].valid);
            }
        }
    }
}
