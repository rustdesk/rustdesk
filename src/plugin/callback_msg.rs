use super::*;
use crate::hbbs_http::create_http_client;
use crate::{
    flutter::{self, APP_TYPE_CM, APP_TYPE_MAIN, SESSIONS},
    ui_interface::get_api_server,
};
use hbb_common::{lazy_static, log, message_proto::PluginRequest};
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::{
    collections::HashMap,
    ffi::{c_char, c_void},
    sync::Arc,
    thread,
    time::Duration,
};

const MSG_TO_RUSTDESK_TARGET: &str = "rustdesk";
const MSG_TO_PEER_TARGET: &str = "peer";
const MSG_TO_UI_TARGET: &str = "ui";
const MSG_TO_CONFIG_TARGET: &str = "config";
const MSG_TO_EXT_SUPPORT_TARGET: &str = "ext-support";

const MSG_TO_RUSTDESK_SIGNATURE_VERIFICATION: &str = "signature_verification";

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

#[derive(Deserialize)]
pub struct MsgToRustDesk {
    pub r#type: String,
    pub data: Vec<u8>,
}

#[derive(Deserialize)]
pub struct SignatureVerification {
    pub version: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct ConfigToUi {
    channel: u16,
    location: String,
}

#[derive(Debug, Deserialize)]
struct MsgToConfig {
    r#type: String,
    key: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ui: Option<ConfigToUi>, // If not None, send msg to ui.
}

#[derive(Debug, Deserialize)]
pub(super) struct MsgToExtSupport {
    pub r#type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize)]
struct PluginSignReq {
    plugin_id: String,
    version: String,
    msg: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct PluginSignResp {
    signed_msg: Vec<u8>,
}

macro_rules! cb_msg_field {
    ($field: ident) => {
        let $field = match cstr_to_string($field) {
            Err(e) => {
                let msg = format!("Failed to convert {} to string, {}", stringify!($field), e);
                log::error!("{}", &msg);
                return PluginReturn::new(errno::ERR_CALLBACK_INVALID_ARGS, &msg);
            }
            Ok(v) => v,
        };
    };
}

macro_rules! early_return_value {
    ($e:expr, $code: ident, $($arg:tt)*) => {
        match $e {
            Err(e) => return PluginReturn::new(
                errno::$code,
                &format!("Failed to {} '{}'", format_args!($($arg)*), e),
            ),
            Ok(v) => v,
        }
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
///
/// Return null ptr if success.
/// Return the error message if failed.  `i32-String` without dash, i32 is a signed little-endian number, the String is utf8 string.
/// The plugin allocate memory with `libc::malloc` and return the pointer.
#[no_mangle]
pub(super) extern "C" fn cb_msg(
    peer: *const c_char,
    target: *const c_char,
    id: *const c_char,
    content: *const c_void,
    len: usize,
) -> PluginReturn {
    cb_msg_field!(target);
    cb_msg_field!(id);

    match &target as _ {
        MSG_TO_PEER_TARGET => {
            cb_msg_field!(peer);
            if let Some(session) = SESSIONS.write().unwrap().get_mut(&peer) {
                let content_slice =
                    unsafe { std::slice::from_raw_parts(content as *const u8, len) };
                let content_vec = Vec::from(content_slice);
                let request = PluginRequest {
                    id,
                    content: bytes::Bytes::from(content_vec),
                    ..Default::default()
                };
                session.send_plugin_request(request);
                PluginReturn::success()
            } else {
                PluginReturn::new(
                    errno::ERR_CALLBACK_PEER_NOT_FOUND,
                    &format!("Failed to find session for peer '{}'", peer),
                )
            }
        }
        MSG_TO_UI_TARGET => {
            cb_msg_field!(peer);
            let content_slice = unsafe { std::slice::from_raw_parts(content as *const u8, len) };
            let channel = u16::from_le_bytes([content_slice[0], content_slice[1]]);
            let content = std::string::String::from_utf8(content_slice[2..].to_vec())
                .unwrap_or("".to_string());
            push_event_to_ui(channel, &peer, &content);
            PluginReturn::success()
        }
        MSG_TO_CONFIG_TARGET => {
            cb_msg_field!(peer);
            let s = early_return_value!(
                std::str::from_utf8(unsafe { std::slice::from_raw_parts(content as _, len) }),
                ERR_CALLBACK_INVALID_MSG,
                "parse msg string"
            );
            // No need to merge the msgs. Handling the msg one by one is ok.
            let msg = early_return_value!(
                serde_json::from_str::<MsgToConfig>(s),
                ERR_CALLBACK_INVALID_MSG,
                "parse msg '{}'",
                s
            );
            match &msg.r#type as _ {
                config::CONFIG_TYPE_SHARED => {
                    let _r = early_return_value!(
                        config::SharedConfig::set(&id, &msg.key, &msg.value),
                        ERR_CALLBACK_INVALID_MSG,
                        "set local config"
                    );
                    if let Some(ui) = &msg.ui {
                        // No need to set the peer id for location config.
                        push_option_to_ui(ui.channel, &id, "", &msg, ui);
                    }
                    PluginReturn::success()
                }
                config::CONFIG_TYPE_PEER => {
                    let _r = early_return_value!(
                        config::PeerConfig::set(&id, &peer, &msg.key, &msg.value),
                        ERR_CALLBACK_INVALID_MSG,
                        "set peer config"
                    );
                    if let Some(ui) = &msg.ui {
                        push_option_to_ui(ui.channel, &id, &peer, &msg, ui);
                    }
                    PluginReturn::success()
                }
                _ => PluginReturn::new(
                    errno::ERR_CALLBACK_TARGET_TYPE,
                    &format!("Unknown target type '{}'", &msg.r#type),
                ),
            }
        }
        MSG_TO_EXT_SUPPORT_TARGET => {
            cb_msg_field!(peer);
            let s = early_return_value!(
                std::str::from_utf8(unsafe { std::slice::from_raw_parts(content as _, len) }),
                ERR_CALLBACK_INVALID_MSG,
                "parse msg string"
            );
            let msg = early_return_value!(
                serde_json::from_str::<MsgToExtSupport>(s),
                ERR_CALLBACK_INVALID_MSG,
                "parse msg '{}'",
                s
            );
            super::callback_ext::ext_support_callback(&id, &peer, &msg)
        }
        MSG_TO_RUSTDESK_TARGET => handle_msg_to_rustdesk(id, content, len),
        _ => PluginReturn::new(
            errno::ERR_CALLBACK_TARGET,
            &format!("Unknown target '{}'", target),
        ),
    }
}

#[inline]
fn is_peer_channel(channel: u16) -> bool {
    channel & MSG_TO_UI_FLUTTER_CHANNEL_REMOTE != 0
        || channel & MSG_TO_UI_FLUTTER_CHANNEL_TRANSFER != 0
        || channel & MSG_TO_UI_FLUTTER_CHANNEL_FORWARD != 0
}

fn handle_msg_to_rustdesk(id: String, content: *const c_void, len: usize) -> PluginReturn {
    let s = early_return_value!(
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(content as _, len) }),
        ERR_CALLBACK_INVALID_MSG,
        "parse msg string"
    );
    let msg_to_rustdesk = early_return_value!(
        serde_json::from_str::<MsgToRustDesk>(s),
        ERR_CALLBACK_INVALID_MSG,
        "parse msg '{}'",
        s
    );
    match &msg_to_rustdesk.r#type as &str {
        MSG_TO_RUSTDESK_SIGNATURE_VERIFICATION => request_plugin_sign(id, msg_to_rustdesk),
        t => PluginReturn::new(
            errno::ERR_CALLBACK_TARGET_TYPE,
            &format!(
                "Unknown target type '{}' for target {}",
                t, MSG_TO_RUSTDESK_TARGET
            ),
        ),
    }
}

fn request_plugin_sign(id: String, msg_to_rustdesk: MsgToRustDesk) -> PluginReturn {
    let signature_data = early_return_value!(
        std::str::from_utf8(&msg_to_rustdesk.data),
        ERR_CALLBACK_INVALID_MSG,
        "parse signature data string"
    );
    let signature_data = early_return_value!(
        serde_json::from_str::<SignatureVerification>(signature_data),
        ERR_CALLBACK_INVALID_MSG,
        "parse signature data '{}'",
        signature_data
    );
    thread::spawn(move || {
        let sign_url = format!("{}/lic/web/api/plugin-sign", get_api_server());
        let client = create_http_client();
        let req = PluginSignReq {
            plugin_id: id.clone(),
            version: signature_data.version,
            msg: signature_data.data,
        };
        match client
            .post(sign_url)
            .json(&req)
            .timeout(Duration::from_secs(10))
            .send()
        {
            Ok(response) => match response.json::<PluginSignResp>() {
                Ok(sign_resp) => {
                    match super::plugins::plugin_call(
                        &id,
                        super::plugins::METHOD_HANDLE_SIGNATURE_VERIFICATION,
                        "",
                        &sign_resp.signed_msg,
                    ) {
                        Ok(..) => {
                            match super::plugins::plugin_call_get_return(
                                &id,
                                super::plugins::METHOD_HANDLE_STATUS,
                                "",
                                &[],
                            ) {
                                Ok(ret) => {
                                    debug_assert!(!ret.msg.is_null(), "msg is null");
                                    if ret.msg.is_null() {
                                        // unreachable
                                        log::error!(
                                            "The returned message pointer of plugin status is null, plugin id: '{}', code: {}",
                                            id,
                                            ret.code,
                                        );
                                        return;
                                    }
                                    let msg = cstr_to_string(ret.msg).unwrap_or_default();
                                    free_c_ptr(ret.msg as _);
                                    if ret.code == super::errno::ERR_SUCCESS {
                                        log::info!("Plugin '{}' status: '{}'", id, msg);
                                    } else {
                                        log::error!(
                                            "Failed to handle plugin event, id: {}, method: {}, code: {}, msg: {}",
                                            id,
                                            std::string::String::from_utf8(super::plugins::METHOD_HANDLE_STATUS.to_vec()).unwrap_or_default(),
                                            ret.code,
                                            msg
                                        );
                                    }
                                }
                                Err(e) => {
                                    log::error!(
                                        "Failed to call status for plugin '{}': {}",
                                        &id,
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "Failed to call signature verification for plugin '{}': {}",
                                &id,
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to decode response for plugin '{}': {}", &id, e);
                }
            },
            Err(e) => {
                log::error!("Failed to request sign for plugin '{}', {}", &id, e);
            }
        }
    });
    PluginReturn::success()
}

fn push_event_to_ui(channel: u16, peer: &str, content: &str) {
    let mut m = HashMap::new();
    m.insert("name", MSG_TO_UI_TYPE_PLUGIN_EVENT);
    m.insert("peer", &peer);
    m.insert("content", &content);
    let event = serde_json::to_string(&m).unwrap_or("".to_string());
    // Send to main and cm
    for (k, v) in MSG_TO_UI_FLUTTER_CHANNELS.iter() {
        if channel & k != 0 {
            let _res = flutter::push_global_event(v as _, event.to_string());
        }
    }
    if !peer.is_empty() && is_peer_channel(channel) {
        let _res = flutter::push_session_event(
            &peer,
            MSG_TO_UI_TYPE_PLUGIN_EVENT,
            vec![("peer", &peer), ("content", &content)],
        );
    }
}

fn push_option_to_ui(channel: u16, id: &str, peer: &str, msg: &MsgToConfig, ui: &ConfigToUi) {
    let v = [
        ("id", id),
        ("location", &ui.location),
        ("key", &msg.key),
        ("value", &msg.value),
    ];

    // Send main and cm
    let mut m = HashMap::from(v);
    m.insert("name", MSG_TO_UI_TYPE_PLUGIN_OPTION);
    let event = serde_json::to_string(&m).unwrap_or("".to_string());
    for (k, v) in MSG_TO_UI_FLUTTER_CHANNELS.iter() {
        if channel & k != 0 {
            let _res = flutter::push_global_event(v as _, event.to_string());
        }
    }

    // Send remote, transfer and forward
    if !peer.is_empty() && is_peer_channel(channel) {
        let mut v = v.to_vec();
        v.push(("peer", &peer));
        let _res = flutter::push_session_event(&peer, MSG_TO_UI_TYPE_PLUGIN_OPTION, v);
    }
}
