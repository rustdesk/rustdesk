use super::cstr_to_string;
use crate::flutter::{self, APP_TYPE_CM, APP_TYPE_MAIN, SESSIONS};
use hbb_common::{lazy_static, log, message_proto::Plugin};
use serde_json;
use std::{collections::HashMap, ffi::c_char, sync::Arc};

const MSG_TO_PEER_TARGET: &str = "peer";
const MSG_TO_UI_TARGET: &str = "ui";

#[allow(dead_code)]
const MSG_TO_UI_FLUTTER_CHANNEL_MAIN: u16 = 0x01 << 0;
#[allow(dead_code)]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const MSG_TO_UI_FLUTTER_CHANNEL_CM: u16 = 0x01 << 1;
#[cfg(any(target_os = "android", target_os = "ios"))]
const MSG_TO_UI_FLUTTER_CHANNEL_CM: u16 = 0x01;
const MSG_TO_UI_FLUTTER_CHANNEL_REMOTE: u16 = 0x01 << 2;
#[allow(dead_code)]
const MSG_TO_UI_FLUTTER_CHANNEL_TRANSFER: u16 = 0x01 << 3;
#[allow(dead_code)]
const MSG_TO_UI_FLUTTER_CHANNEL_FORWARD: u16 = 0x01 << 4;

lazy_static::lazy_static! {
    static ref MSG_TO_UI_FLUTTER_CHANNELS: Arc<HashMap<u16, String>> = {
        let channels = HashMap::from([
            (MSG_TO_UI_FLUTTER_CHANNEL_MAIN, APP_TYPE_MAIN.to_string()),
            (MSG_TO_UI_FLUTTER_CHANNEL_CM, APP_TYPE_CM.to_string()),
        ]);
        Arc::new(channels)
    };
}

/// Callback to send message to peer or ui.
/// peer, target, id are utf8 strings(null terminated).
///
/// peer:    The peer id.
/// target:  "peer" or "ui".
/// id:      The id of this plugin.
/// content: The content.
/// len:     The length of the content.
pub fn callback_msg(
    peer: *const c_char,
    target: *const c_char,
    id: *const c_char,
    content: *const c_char,
    len: usize,
) {
    macro_rules! callback_msg_field {
        ($field: ident) => {
            let $field = match cstr_to_string($field) {
                Err(e) => {
                    log::error!("Failed to convert {} to string, {}", stringify!($field), e);
                    return;
                }
                Ok(v) => v,
            };
        };
    }
    callback_msg_field!(peer);
    callback_msg_field!(target);
    callback_msg_field!(id);

    match &target as _ {
        MSG_TO_PEER_TARGET => {
            if let Some(session) = SESSIONS.write().unwrap().get_mut(&peer) {
                let content_slice =
                    unsafe { std::slice::from_raw_parts(content as *const u8, len) };
                let content_vec = Vec::from(content_slice);
                let plugin = Plugin {
                    id,
                    content: bytes::Bytes::from(content_vec),
                    ..Default::default()
                };
                session.send_plugin(plugin);
            }
        }
        MSG_TO_UI_TARGET => {
            let content_slice = unsafe { std::slice::from_raw_parts(content as *const u8, len) };
            let channel = u16::from_be_bytes([content_slice[0], content_slice[1]]);
            let content = std::string::String::from_utf8(content_slice[2..].to_vec())
                .unwrap_or("".to_string());
            let mut m = HashMap::new();
            m.insert("name", "plugin_event");
            m.insert("peer", &peer);
            m.insert("content", &content);
            let event = serde_json::to_string(&m).unwrap_or("".to_string());
            for (k, v) in MSG_TO_UI_FLUTTER_CHANNELS.iter() {
                if channel & k != 0 {
                    let _res = flutter::push_global_event(v as _, event.clone());
                }
            }
            if channel & MSG_TO_UI_FLUTTER_CHANNEL_REMOTE != 0
                || channel & MSG_TO_UI_FLUTTER_CHANNEL_TRANSFER != 0
                || channel & MSG_TO_UI_FLUTTER_CHANNEL_FORWARD != 0
            {
                let _res = flutter::push_session_event(
                    &peer,
                    "plugin",
                    vec![("peer", &peer), ("content", &content)],
                );
            }
        }
        _ => {
            log::error!("Unknown target {}", target);
        }
    }
}
