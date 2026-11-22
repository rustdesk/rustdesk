use crate::keyboard::UnicodePacket;
use rdev::{Event, EventType};
use winapi::um::winuser::VK_PACKET;

pub(crate) fn is_unicode_packet(event: &Event) -> bool {
    rdev::is_unicode_packet_enabled() && event.platform_code == VK_PACKET as u32
}

pub(crate) fn handle_unicode_packet(event: &Event, decoder: &mut UnicodePacket) -> bool {
    const ERROR_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);
    if !rdev::is_unicode_packet_enabled() {
        return false;
    }
    // Tab/Enter use the current session, which can change before the grab owner.
    // Use the same session snapshot for permissions and sending the packet text.
    #[cfg(feature = "flutter")]
    let Some(session) = crate::flutter::get_cur_session() else {
        *decoder = Default::default();
        return false;
    };
    #[cfg(feature = "flutter")]
    if !*session.server_keyboard_enabled.read().unwrap() || session.lc.read().unwrap().view_only.v {
        *decoder = Default::default();
        return false;
    }
    if matches!(event.event_type, EventType::KeyPress(_)) {
        match decoder.decode(event.position_code) {
            Ok(Some(key_event)) => {
                #[cfg(feature = "flutter")]
                session.send_key_event(&key_event);
                #[cfg(not(feature = "flutter"))]
                crate::keyboard::send_key_event(&key_event);
            }
            Ok(None) => {}
            Err(err) => {
                hbb_common::throttled_log!(ERROR_LOG_INTERVAL, warn, "{err}");
            }
        }
    }
    true
}
