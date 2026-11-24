use base::message_proto::{
    key_event, message, Clipboard, ControlKey, ImageQuality, KeyEvent, KeyboardMode, Message,
    Misc, MouseEvent, OptionMessage, SwitchDisplay,
};
use serde::Deserialize;
use serde_json::Value;

use crate::video::{self, EncodedPacket};

pub const MOUSE_TYPE_MOVE: i32 = 0;
pub const MOUSE_TYPE_DOWN: i32 = 1;
pub const MOUSE_TYPE_UP: i32 = 2;
pub const MOUSE_TYPE_WHEEL: i32 = 3;
pub const MOUSE_BUTTON_LEFT: i32 = 0x01;
pub const MOUSE_BUTTON_RIGHT: i32 = 0x02;
pub const MOUSE_BUTTON_WHEEL: i32 = 0x04;

/// Codecs the browser proved with `VideoDecoder.isConfigSupported`.
///
/// `reported` is false when the page did not probe. The gateway then offers
/// only VP8 and VP9, which the host can encode in software.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct CodecCaps {
    #[serde(default)]
    pub vp8: bool,
    #[serde(default)]
    pub vp9: bool,
    #[serde(default)]
    pub h264: bool,
    #[serde(default)]
    pub h265: bool,
    #[serde(default)]
    pub av1: bool,
    #[serde(default)]
    pub reported: bool,
}

impl CodecCaps {
    pub fn selected(&self) -> Self {
        if self.reported {
            self.clone()
        } else {
            Self {
                vp8: true,
                vp9: true,
                reported: true,
                ..Self::default()
            }
        }
    }

    pub fn any(&self) -> bool {
        self.vp8 || self.vp9 || self.h264 || self.h265 || self.av1
    }
}

pub fn token_matches(expected: Option<&str>, given: &str) -> bool {
    let Some(expected) = expected.map(str::trim).filter(|s| !s.is_empty()) else {
        return true;
    };
    let a = expected.as_bytes();
    let b = given.trim().as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn humanize_error(err: &str) -> String {
    let text = err.to_ascii_lowercase();
    if text.contains("wrong password") {
        return "The host rejected the password. Check the permanent password and connect again.".into();
    }
    if text.contains("empty password") || text.contains("no password access") {
        return "The host requires a password. Enter the permanent password set on the desktop and connect again.".into();
    }
    if text.contains("offline") || text.contains("id does not exist") {
        return "The remote desktop is offline, or the ID does not match this host.".into();
    }
    if text.contains("connection refused")
        || text.contains("failed to connect")
        || text.contains("os error 111")
        || text.contains("network is unreachable")
    {
        return "Cannot reach the host. Check the address and port, and confirm direct IP access is enabled on the desktop.".into();
    }
    if text.contains("timed out") || text.contains("timeout") {
        return "The host did not answer in time. It may be offline, or a firewall is blocking the port.".into();
    }
    if text.contains("gateway access token") {
        return "The gateway access token is missing or incorrect.".into();
    }
    if text.contains("cannot decode") {
        return err.to_owned();
    }
    err.to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMsg {
    Connect {
        mode: String,
        #[serde(default)]
        host: String,
        #[serde(default)]
        port: u16,
        #[serde(default)]
        password: String,
        #[serde(default, rename = "peerId")]
        peer_id: String,
        #[serde(default, rename = "idServer")]
        id_server: String,
        #[serde(default, rename = "relayServer")]
        relay_server: String,
        #[serde(default)]
        key: String,
        #[serde(default, rename = "forceRelay")]
        force_relay: bool,
        #[serde(default)]
        token: String,
        #[serde(default)]
        codecs: CodecCaps,
    },
    Mouse {
        action: String,
        #[serde(default)]
        button: String,
        #[serde(default)]
        x: i32,
        #[serde(default)]
        y: i32,
    },
    Key {
        action: String,
        #[serde(default)]
        control: String,
        #[serde(default)]
        chr: String,
        #[serde(default)]
        text: String,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    Text {
        text: String,
    },
    Clipboard {
        text: String,
    },
    SwitchDisplay {
        display: i32,
    },
    Refresh,
    Quality {
        #[serde(default)]
        quality: String,
        #[serde(default)]
        fps: i32,
    },
    Disconnect,
}

pub fn parse_client_msg(text: &str) -> Result<ClientMsg, String> {
    serde_json::from_str(text).map_err(|err| err.to_string())
}

pub fn to_host_message(msg: &ClientMsg) -> Option<Message> {
    match msg {
        ClientMsg::Mouse { action, button, x, y } => Some(mouse_message(action, button, *x, *y)),
        ClientMsg::Key {
            action,
            control,
            chr,
            text,
            modifiers,
        } => key_message(action, control, chr, text, modifiers),
        ClientMsg::Text { text } => {
            if text.is_empty() {
                None
            } else {
                Some(text_message(text))
            }
        }
        ClientMsg::Clipboard { text } => Some(clipboard_message(text)),
        ClientMsg::SwitchDisplay { display } => Some(switch_display_message(*display)),
        ClientMsg::Refresh => Some(refresh_message()),
        ClientMsg::Quality { quality, fps } => Some(quality_message(quality, *fps)),
        ClientMsg::Connect { .. } | ClientMsg::Disconnect => None,
    }
}

pub fn mouse_message(action: &str, button: &str, x: i32, y: i32) -> Message {
    let kind = match action {
        "down" => MOUSE_TYPE_DOWN,
        "up" => MOUSE_TYPE_UP,
        "wheel" => MOUSE_TYPE_WHEEL,
        _ => MOUSE_TYPE_MOVE,
    };
    let button = if kind == MOUSE_TYPE_WHEEL || kind == MOUSE_TYPE_MOVE {
        0
    } else {
        match button {
            "right" => MOUSE_BUTTON_RIGHT,
            "middle" => MOUSE_BUTTON_WHEEL,
            _ => MOUSE_BUTTON_LEFT,
        }
    };
    let mut ev = MouseEvent::new();
    ev.mask = (button << 3) | kind;
    ev.x = x;
    ev.y = y;
    let mut msg = Message::new();
    msg.set_mouse_event(ev);
    msg
}

pub fn text_message(text: &str) -> Message {
    let mut ev = KeyEvent::new();
    ev.mode = KeyboardMode::Legacy.into();
    ev.press = true;
    ev.set_seq(text.to_owned());
    let mut msg = Message::new();
    msg.set_key_event(ev);
    msg
}

fn key_message(
    action: &str,
    control: &str,
    chr: &str,
    text: &str,
    modifiers: &[String],
) -> Option<Message> {
    let mut ev = KeyEvent::new();
    ev.mode = KeyboardMode::Legacy.into();
    ev.down = action != "up";
    ev.press = action == "press";
    for name in modifiers {
        if let Some(key) = control_key(name) {
            ev.modifiers.push(key.into());
        }
    }
    if !control.is_empty() {
        ev.set_control_key(control_key(control).unwrap_or(ControlKey::Unknown));
    } else if let Some(ch) = chr.chars().next() {
        ev.set_chr(ch as u32);
    } else if !text.is_empty() {
        ev.press = true;
        ev.set_seq(text.to_owned());
    } else {
        return None;
    }
    let mut msg = Message::new();
    msg.set_key_event(ev);
    Some(msg)
}

fn clipboard_message(text: &str) -> Message {
    let mut msg = Message::new();
    msg.set_clipboard(Clipboard {
        content: text.as_bytes().to_vec().into(),
        ..Default::default()
    });
    msg
}

pub fn switch_display_message(display: i32) -> Message {
    let mut misc = Misc::new();
    misc.set_switch_display(SwitchDisplay {
        display,
        ..Default::default()
    });
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

pub fn refresh_message() -> Message {
    let mut misc = Misc::new();
    misc.set_refresh_video(true);
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

/// Tells a 1.5 host that a video message was consumed, so QoS does not treat the link as stalled.
pub fn video_received_message() -> Message {
    let mut misc = Misc::new();
    misc.set_video_received(true);
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

pub fn quality_message(quality: &str, fps: i32) -> Message {
    let mut option = OptionMessage::new();
    option.image_quality = match quality {
        "low" => ImageQuality::Low,
        "best" => ImageQuality::Best,
        "balanced" => ImageQuality::Balanced,
        _ => ImageQuality::NotSet,
    }
    .into();
    if fps > 0 {
        option.custom_fps = fps;
    }
    let mut misc = Misc::new();
    misc.set_option(option);
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

pub fn control_key(name: &str) -> Option<ControlKey> {
    Some(match name {
        "Alt" => ControlKey::Alt,
        "Backspace" => ControlKey::Backspace,
        "CapsLock" => ControlKey::CapsLock,
        "Control" | "Ctrl" => ControlKey::Control,
        "Delete" => ControlKey::Delete,
        "ArrowDown" | "Down" => ControlKey::DownArrow,
        "End" => ControlKey::End,
        "Escape" | "Esc" => ControlKey::Escape,
        "F1" => ControlKey::F1,
        "F2" => ControlKey::F2,
        "F3" => ControlKey::F3,
        "F4" => ControlKey::F4,
        "F5" => ControlKey::F5,
        "F6" => ControlKey::F6,
        "F7" => ControlKey::F7,
        "F8" => ControlKey::F8,
        "F9" => ControlKey::F9,
        "F10" => ControlKey::F10,
        "F11" => ControlKey::F11,
        "F12" => ControlKey::F12,
        "Home" => ControlKey::Home,
        "ArrowLeft" | "Left" => ControlKey::LeftArrow,
        "Meta" | "Super" | "Win" => ControlKey::Meta,
        "PageDown" => ControlKey::PageDown,
        "PageUp" => ControlKey::PageUp,
        "Enter" | "Return" => ControlKey::Return,
        "ArrowRight" | "Right" => ControlKey::RightArrow,
        "Shift" => ControlKey::Shift,
        "Space" => ControlKey::Space,
        "Tab" => ControlKey::Tab,
        "ArrowUp" | "Up" => ControlKey::UpArrow,
        "CtrlAltDel" => ControlKey::CtrlAltDel,
        "LockScreen" => ControlKey::LockScreen,
        _ => return None,
    })
}

pub fn frame_meta_json(packet: &EncodedPacket<'_>) -> Value {
    serde_json::json!({
        "type": "video",
        "codec": video::codec_name(packet.codec),
        "key": packet.key,
        "display": packet.display,
        "width": packet.width,
        "height": packet.height,
        "pts": packet.pts,
    })
}

pub fn key_event_kind(msg: &Message) -> Option<&'static str> {
    let message::Union::KeyEvent(ev) = msg.union.as_ref()? else {
        return None;
    };
    match ev.union {
        Some(key_event::Union::ControlKey(_)) => Some("control"),
        Some(key_event::Union::Chr(_)) => Some("chr"),
        Some(key_event::Union::Seq(_)) => Some("seq"),
        Some(key_event::Union::Unicode(_)) => Some("unicode"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::message_proto::message;

    #[test]
    fn mouse_down_packs_the_rustdesk_mask() {
        let msg = mouse_message("down", "left", 10, 20);
        let message::Union::MouseEvent(ev) = msg.union.unwrap() else {
            panic!("mouse");
        };
        assert_eq!(ev.mask, (MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN);
        assert_eq!((ev.x, ev.y), (10, 20));
    }

    #[test]
    fn wheel_uses_deltas_without_a_button() {
        let msg = mouse_message("wheel", "left", 0, -1);
        let message::Union::MouseEvent(ev) = msg.union.unwrap() else {
            panic!("wheel");
        };
        assert_eq!(ev.mask, MOUSE_TYPE_WHEEL);
        assert_eq!(ev.y, -1);
    }

    #[test]
    fn text_and_escape_become_legacy_key_events() {
        let text = text_message("hi");
        assert_eq!(key_event_kind(&text), Some("seq"));
        let raw = r#"{"type":"key","action":"down","control":"Escape"}"#;
        let parsed = parse_client_msg(raw).unwrap();
        let msg = to_host_message(&parsed).unwrap();
        assert_eq!(key_event_kind(&msg), Some("control"));
    }

    #[test]
    fn header_len_stays_in_sync_with_the_documented_layout() {
        assert_eq!(crate::video::HEADER_LEN, 32);
    }

    #[test]
    fn password_and_unreachable_errors_are_plain_language() {
        assert!(humanize_error("Wrong Password").contains("password"));
        assert!(humanize_error("failed to connect to 127.0.0.1:21118: Connection refused").contains("Cannot reach"));
        assert!(humanize_error("timed out waiting for LoginResponse").contains("did not answer"));
    }

    #[test]
    fn gateway_token_is_optional_and_exact() {
        assert!(token_matches(None, ""));
        assert!(token_matches(Some("  "), "nope"));
        assert!(token_matches(Some("desk"), "desk"));
        assert!(token_matches(Some("desk"), " desk "));
        assert!(!token_matches(Some("desk"), "other"));
    }

    #[test]
    fn unprobed_codecs_offer_only_software_vp8_and_vp9() {
        let caps = CodecCaps::default().selected();
        assert!(caps.vp8 && caps.vp9);
        assert!(!caps.h264 && !caps.av1 && !caps.h265);
    }

    #[test]
    fn video_received_is_a_misc_flag() {
        let msg = video_received_message();
        assert!(matches!(
            msg.union,
            Some(message::Union::Misc(ref misc))
                if matches!(misc.union, Some(base::message_proto::misc::Union::VideoReceived(true)))
        ));
    }
}
