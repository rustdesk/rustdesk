//! Native state behind the MCP server that runs in the Flutter main window.

use crate::{
    flutter_ffi::SessionID,
    input::{
        MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT, MOUSE_TYPE_DOWN, MOUSE_TYPE_MASK, MOUSE_TYPE_UP,
    },
};
use base::message_proto::ScreenshotResponse;
use std::{
    collections::{HashSet, VecDeque},
    fs::OpenOptions,
    io::Write,
    sync::{Mutex, RwLock},
};

// Joins a session id and a request id into the sid of a screenshot request.
// The peer echoes the sid, so a result is matched with the request that asked
// for it instead of going to the cache the screenshot dialog reads.
const REQUEST_SEPARATOR: char = '#';
// Results nobody collects, e.g. after a timeout, are dropped oldest first.
const MAX_SCREENSHOTS: usize = 4;

lazy_static::lazy_static! {
    static ref SCREENSHOTS: Mutex<VecDeque<ScreenshotResponse>> = Default::default();
}

pub struct ConnectionInput {
    pub id: SessionID,
    owner: RwLock<InputOwner>,
    /// Keys the human holds through the legacy key API, which reports names
    /// instead of forwarded events. Recorded under the same gate as the send,
    /// so a handoff can release them before the agent takes over.
    human_keys: Mutex<HashSet<String>>,
}

impl Default for ConnectionInput {
    fn default() -> Self {
        Self {
            id: SessionID::new_v4(),
            owner: Default::default(),
            human_keys: Default::default(),
        }
    }
}

#[derive(Default)]
struct InputOwner {
    grant_id: String,
    human_buttons: i32,
    agent_buttons: i32,
    agent_keys: HashSet<String>,
    human_pan: Option<(i32, i32)>,
}

impl InputOwner {
    fn accepts(&self, grant_id: Option<&str>) -> bool {
        match grant_id {
            Some(id) => !id.is_empty() && self.grant_id == id,
            None => self.grant_id.is_empty(),
        }
    }

    fn grant(&mut self, grant_id: String, release: impl FnOnce(i32, Option<(i32, i32)>)) {
        release(
            std::mem::take(&mut self.human_buttons),
            self.human_pan.take(),
        );
        self.grant_id = grant_id;
    }

    fn mouse(&mut self, grant_id: Option<&str>, mask: i32, send: impl FnOnce()) -> bool {
        if !self.accepts(grant_id) {
            return false;
        }
        let buttons = if grant_id.is_none() {
            &mut self.human_buttons
        } else {
            &mut self.agent_buttons
        };
        match mask & MOUSE_TYPE_MASK {
            MOUSE_TYPE_DOWN => *buttons |= mask >> 3,
            MOUSE_TYPE_UP => *buttons &= !(mask >> 3),
            _ => {}
        }
        send();
        true
    }

    fn release_agent(&mut self, session: &crate::flutter::FlutterSession) {
        let buttons = std::mem::take(&mut self.agent_buttons);
        for button in [1, 2, 4, 8, 16] {
            if buttons & button != 0 {
                crate::client::send_mouse(
                    (button << 3) | MOUSE_TYPE_UP,
                    0,
                    0,
                    false,
                    false,
                    false,
                    false,
                    &**session,
                );
            }
        }
        for key in self.agent_keys.drain() {
            session.input_key_unswapped(&key, false, false, false, false, false, false);
        }
    }

    fn pointer(&mut self, msg: &str, send: impl FnOnce()) {
        if !self.accepts(None) {
            return;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(msg) {
            if value["k"] == "touch" {
                let event = &value["v"];
                match event["t"].as_str() {
                    Some("pan_start") => {
                        if let (Some(x), Some(y)) =
                            (event["v"]["x"].as_i64(), event["v"]["y"].as_i64())
                        {
                            self.human_pan = Some((x as _, y as _));
                        }
                    }
                    Some("pan_end") => self.human_pan = None,
                    _ => {}
                }
            }
        }
        send();
    }
}

pub fn set_agent_control(session_id: SessionID, grant_id: String) {
    let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) else {
        return;
    };
    let mut owner = session.ui_handler.mcp.owner.write().unwrap();
    // A pending grant may have resolved this connection before its last tab closed.
    if crate::flutter::sessions::mcp_session_id(&session.ui_handler.mcp.id.to_string()).is_none() {
        return;
    }
    owner.release_agent(&session);
    if grant_id.is_empty() {
        *owner = InputOwner::default();
        return;
    }
    // Human keys held through the legacy key API are not part of the forwarded
    // event tracker, so release them here. The owner lock is held, so no human
    // send can be recorded between the release and the grant below.
    for key in session.ui_handler.mcp.human_keys.lock().unwrap().drain() {
        session.input_key(&key, false, false, false, false, false, false);
    }
    owner.grant(grant_id, |buttons, pan| {
        // Forwarded human keys are tracked for the tab that has focus, and the
        // grant covers the whole connection. Release them through that tab as
        // long as it shares this connection, so a grant that lands on a sibling
        // tab still cleans up what the focused one left down.
        let current_id = crate::flutter::get_cur_session_id();
        if let Some(current) = crate::flutter::sessions::get_session_by_session_id(&current_id) {
            if current.ui_handler.mcp.id == session.ui_handler.mcp.id {
                crate::keyboard::release_remote_keys_for_session(&current, current_id);
            }
        }
        let swap = session.get_toggle_option("swap-left-right-mouse".to_owned());
        for button in [1, 2, 4, 8, 16] {
            if buttons & button == 0 {
                continue;
            }
            let button = match (swap, button) {
                (true, MOUSE_BUTTON_LEFT) => MOUSE_BUTTON_RIGHT,
                (true, MOUSE_BUTTON_RIGHT) => MOUSE_BUTTON_LEFT,
                _ => button,
            };
            crate::client::send_mouse(
                (button << 3) | MOUSE_TYPE_UP,
                0,
                0,
                false,
                false,
                false,
                false,
                &*session,
            );
        }
        if let Some((x, y)) = pan {
            session.send_touch_pan_event("pan_end", x, y, false, false, false, false);
        }
    });
}

// Hold the connection itself across cleanup; UI tabs can disappear independently.
pub fn release_input(connection_id: &str, grant_id: &str, revoke: bool) {
    let Some(session) = crate::flutter::sessions::mcp_connection(connection_id) else {
        return;
    };
    let mut owner = session.ui_handler.mcp.owner.write().unwrap();
    if !owner.accepts(Some(grant_id)) {
        return;
    }
    owner.release_agent(&session);
    if revoke {
        *owner = InputOwner::default();
    }
}

pub fn close_connection(session: &crate::flutter::FlutterSession) {
    // Finish in-flight native sends and revoke before the connection queues Close.
    let mut owner = session.ui_handler.mcp.owner.write().unwrap();
    owner.release_agent(session);
    session.ui_handler.mcp.human_keys.lock().unwrap().clear();
    *owner = InputOwner::default();
}

pub fn input_key(
    session_id: SessionID,
    grant_id: &str,
    name: &str,
    down: bool,
    press: bool,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) -> String {
    let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) else {
        return "The session was closed.".into();
    };
    let mut owner = session.ui_handler.mcp.owner.write().unwrap();
    if !owner.accepts(Some(grant_id)) {
        return "Control changed; this input was cancelled.".into();
    }
    if !press
        && session.peer_platform() == "Linux"
        && !matches!(name, "VK_SHIFT" | "VK_CONTROL" | "VK_MENU" | "Meta")
    {
        return "Linux peers do not support holding ordinary keys across calls. Use action press."
            .into();
    }
    session.input_key_unswapped(name, down, press, alt, ctrl, shift, command);
    if down && !press {
        owner.agent_keys.insert(name.to_owned());
    } else {
        owner.agent_keys.remove(name);
    }
    String::new()
}

pub fn input_string(session_id: SessionID, grant_id: &str, value: &str) -> String {
    let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) else {
        return "The session was closed.".into();
    };
    let owner = session.ui_handler.mcp.owner.read().unwrap();
    if !owner.accepts(Some(grant_id)) {
        return "Control changed; this input was cancelled.".into();
    }
    session.input_string(value);
    String::new()
}

pub fn with_human_input(session_id: SessionID, send: impl FnOnce()) {
    let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) else {
        return;
    };
    let owner = session.ui_handler.mcp.owner.read().unwrap();
    if owner.accepts(None) {
        send();
    }
}

/// The legacy key API sends a named key instead of a forwarded event, so the
/// handoff cannot find the keys it left down in the event tracker. Record them
/// while the human gate is held, so releasing them at the grant cannot race a
/// human key-down.
pub fn with_human_key_input(
    session_id: SessionID,
    name: &str,
    down: bool,
    press: bool,
    send: impl FnOnce(),
) {
    let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) else {
        return;
    };
    let owner = session.ui_handler.mcp.owner.read().unwrap();
    if !owner.accepts(None) {
        return;
    }
    if !press {
        let mut keys = session.ui_handler.mcp.human_keys.lock().unwrap();
        if down {
            keys.insert(name.to_owned());
        } else {
            keys.remove(name);
        }
    }
    send();
}

pub fn input_os_password(session_id: SessionID, value: String) {
    with_human_input(session_id, || {
        std::thread::spawn(move || {
            // The existing sequence sleeps before typing. Keep the grant out until
            // all of its clicks and keystrokes have been submitted.
            with_human_input(session_id, || {
                if let Some(session) =
                    crate::flutter::sessions::get_session_by_session_id(&session_id)
                {
                    crate::client::_input_os_password(value, true, (*session).clone());
                }
            });
        });
    });
}

pub fn authenticate(
    session_id: SessionID,
    grant_id: &str,
    password: Option<String>,
    two_factor_code: Option<String>,
) -> bool {
    let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) else {
        return false;
    };
    let owner = session.ui_handler.mcp.owner.read().unwrap();
    if !owner.accepts(Some(grant_id)) {
        return false;
    }
    if let Some(password) = password {
        session.login(String::new(), String::new(), password, false);
    } else if let Some(code) = two_factor_code {
        session.send2fa(code, false);
    } else {
        return false;
    }
    true
}

pub fn process_human_key_event(keyboard_mode: &str, event: &rdev::Event, lock_modes: Option<i32>) {
    let session_id = crate::flutter::get_cur_session_id();
    if let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) {
        with_human_input(session_id, || {
            crate::keyboard::client::process_event_with_session(
                keyboard_mode,
                event,
                lock_modes,
                &session,
                session_id,
            );
        });
    } else {
        // Leaving the last session still needs to clear local shortcut/modifier state.
        if !crate::keyboard::shortcuts::try_dispatch(
            Some(&session_id),
            event,
            keyboard_mode,
            String::new,
            |_| {},
        ) && !crate::keyboard::is_long_press(event)
        {
            crate::keyboard::event_to_key_events(
                String::new(),
                event,
                crate::keyboard::get_keyboard_mode_enum(keyboard_mode),
                lock_modes,
            );
        }
    }
}

pub fn send_mouse(
    session_id: SessionID,
    grant_id: Option<&str>,
    mask: i32,
    send: impl FnOnce(),
) -> bool {
    let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) else {
        return false;
    };
    let result = session
        .ui_handler
        .mcp
        .owner
        .write()
        .unwrap()
        .mouse(grant_id, mask, send);
    result
}

pub fn send_human_pointer(session_id: SessionID, msg: &str, send: impl FnOnce()) {
    if let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) {
        session
            .ui_handler
            .mcp
            .owner
            .write()
            .unwrap()
            .pointer(msg, send);
    }
}

pub fn agent_control_grant(session_id: &SessionID) -> String {
    crate::flutter::sessions::get_session_by_session_id(session_id)
        .map(|s| s.ui_handler.mcp.owner.read().unwrap().grant_id.clone())
        .unwrap_or_default()
}

pub fn screenshot_sid(session_id: &SessionID, request_id: &str) -> String {
    format!("{session_id}{REQUEST_SEPARATOR}{request_id}")
}

pub fn is_requested_screenshot(response: &ScreenshotResponse) -> bool {
    response.sid.contains(REQUEST_SEPARATOR)
}

pub fn store_screenshot(response: ScreenshotResponse) {
    let mut screenshots = SCREENSHOTS.lock().unwrap();
    if screenshots.len() >= MAX_SCREENSHOTS {
        screenshots.pop_front();
    }
    screenshots.push_back(response);
}

/// Writes the result for `sid` to `path`. Returns `None` while it has not
/// arrived, otherwise an empty string or the error.
pub fn save_screenshot(sid: &str, path: &str) -> Option<String> {
    let response = {
        let mut screenshots = SCREENSHOTS.lock().unwrap();
        let i = screenshots.iter().position(|r| r.sid == sid)?;
        screenshots.remove(i)?
    };
    if !response.msg.is_empty() {
        return Some(response.msg);
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Some(
        match options
            .open(path)
            .and_then(|mut f| f.write_all(&response.data))
        {
            Ok(()) => String::new(),
            Err(e) => e.to_string(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Data;
    use base::message_proto::{
        key_event::Union as KeyUnion, message::Union as MessageUnion, ControlKey, KeyEvent,
    };
    use hbb_common::tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

    /// Stands in for the peer: every outbound message lands here.
    fn capture(session: &crate::flutter::FlutterSession) -> UnboundedReceiver<Data> {
        let (tx, rx) = unbounded_channel();
        *session.sender.write().unwrap() = Some(tx);
        rx
    }

    fn sent_keys(rx: &mut UnboundedReceiver<Data>) -> Vec<KeyEvent> {
        std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|data| match data {
                Data::Message(msg) => match msg.union {
                    Some(MessageUnion::KeyEvent(evt)) => Some(evt),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    fn control_key(evt: &KeyEvent) -> Option<ControlKey> {
        match evt.union {
            Some(KeyUnion::ControlKey(ck)) => Some(ck.enum_value_or_default()),
            _ => None,
        }
    }

    fn keys_of(keys: &[KeyEvent]) -> Vec<String> {
        keys.iter()
            .map(|k| format!("{:?} down={} press={}", k.union, k.down, k.press))
            .collect()
    }

    fn session() -> (SessionID, crate::flutter::FlutterSession) {
        let id = SessionID::new_v4();
        let session: crate::flutter::FlutterSession = std::sync::Arc::new(Default::default());
        session.lc.write().unwrap().initialize(
            id.to_string(),
            hbb_common::rendezvous_proto::ConnType::DEFAULT_CONN,
            None,
            false,
            None,
            None,
            None,
        );
        crate::flutter::sessions::insert_session(
            id,
            hbb_common::rendezvous_proto::ConnType::DEFAULT_CONN,
            session.clone(),
        );
        (id, session)
    }

    #[test]
    fn grant_waits_for_human_key_submission_and_blocks_later_keys() {
        use std::{sync::mpsc, thread, time::Duration};

        let (session_id, _) = session();
        let (started, received_start) = mpsc::channel();
        let (finish, received_finish) = mpsc::channel();
        let human = thread::spawn(move || {
            with_human_input(session_id, || {
                started.send(()).unwrap();
                received_finish.recv().unwrap();
            });
        });
        received_start.recv_timeout(Duration::from_secs(2)).unwrap();
        let (granted, received_grant) = mpsc::channel();
        let grant = thread::spawn(move || {
            set_agent_control(session_id, "grant".to_owned());
            granted.send(()).unwrap();
        });
        assert!(received_grant
            .recv_timeout(Duration::from_millis(50))
            .is_err());
        finish.send(()).unwrap();
        human.join().unwrap();
        received_grant.recv_timeout(Duration::from_secs(2)).unwrap();
        grant.join().unwrap();

        let mut sent = Vec::new();
        with_human_input(session_id, || sent.push("late human key"));
        assert!(sent.is_empty());
        set_agent_control(session_id, String::new());
        with_human_input(session_id, || sent.push("human key after release"));
        assert_eq!(sent, ["human key after release"]);
        crate::flutter_ffi::session_close(session_id);
    }

    #[test]
    fn human_release_cannot_interrupt_agent_drag() {
        let mut owner = InputOwner::default();
        let mut sent = Vec::new();
        let down = (MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN;
        let up = (MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP;
        assert!(owner.mouse(None, down, || sent.push("human down")));
        owner.grant("grant".to_owned(), |buttons, pan| {
            assert_eq!(buttons, MOUSE_BUTTON_LEFT);
            assert_eq!(pan, None);
            sent.push("handover up");
        });
        assert!(owner.mouse(Some("grant"), down, || sent.push("agent down")));
        assert!(!owner.mouse(None, up, || sent.push("late human up")));
        assert!(!owner.mouse(None, down, || sent.push("new human down")));
        assert!(!owner.mouse(Some("old"), up, || sent.push("stale agent up")));
        assert!(owner.mouse(Some("grant"), up, || sent.push("agent up")));
        assert_eq!(
            sent,
            ["human down", "handover up", "agent down", "agent up"]
        );
    }

    #[test]
    fn handover_ends_human_pan_and_drops_late_pointer_events() {
        let mut owner = InputOwner::default();
        let mut sent = Vec::new();
        let start = r#"{"k":"touch","v":{"t":"pan_start","v":{"x":30,"y":40}}}"#;
        let end = r#"{"k":"touch","v":{"t":"pan_end","v":{"x":30,"y":40}}}"#;
        owner.pointer(start, || sent.push("pan start"));
        owner.grant("grant".to_owned(), |buttons, pan| {
            assert_eq!(buttons, 0);
            assert_eq!(pan, Some((30, 40)));
            sent.push("pan end before grant");
        });
        owner.pointer(end, || sent.push("late pan end"));
        owner.pointer(r#"{"k":"touch","v":{"t":"scale","v":10}}"#, || {
            sent.push("scale")
        });
        assert_eq!(sent, ["pan start", "pan end before grant"]);
    }

    #[test]
    fn closing_session_clears_agent_control() {
        let (session_id, _) = session();
        set_agent_control(session_id, "grant".to_owned());
        assert_eq!(agent_control_grant(&session_id), "grant");
        crate::flutter_ffi::session_close(session_id);
        assert_eq!(agent_control_grant(&session_id), "");
    }

    #[test]
    fn ownership_reads_follow_revoke_and_regrant_without_window_updates() {
        let (session_id, _) = session();
        set_agent_control(session_id, "old".to_owned());
        let old_notice = agent_control_grant(&session_id);
        set_agent_control(session_id, String::new());
        assert_eq!(agent_control_grant(&session_id), "");
        set_agent_control(session_id, "new".to_owned());
        // A window refresh reads current state, even for an old notification.
        assert_ne!(agent_control_grant(&session_id), old_notice);
        assert_eq!(agent_control_grant(&session_id), "new");
        set_agent_control(session_id, String::new());
        crate::flutter_ffi::session_close(session_id);
    }

    #[test]
    fn sibling_windows_share_control_and_survive_one_window_closing() {
        let (a, connection) = session();
        let b = SessionID::new_v4();
        crate::flutter::sessions::insert_session(
            b,
            hbb_common::rendezvous_proto::ConnType::DEFAULT_CONN,
            connection,
        );
        assert_eq!(
            crate::flutter_ffi::session_mcp_connection_id(a).0,
            crate::flutter_ffi::session_mcp_connection_id(b).0
        );
        set_agent_control(a, "grant".into());
        assert_eq!(agent_control_grant(&b), "grant");
        with_human_input(b, || panic!("sibling keyboard must be blocked"));
        assert!(!send_mouse(b, None, MOUSE_TYPE_DOWN, || panic!(
            "sibling mouse must be blocked"
        )));
        crate::flutter_ffi::session_close(a);
        assert_eq!(agent_control_grant(&b), "grant");
        assert!(send_mouse(b, Some("grant"), MOUSE_TYPE_UP, || {}));
        set_agent_control(b, String::new());
        let mut sent = false;
        with_human_input(b, || sent = true);
        assert!(sent);
        crate::flutter_ffi::session_close(b);
    }

    #[test]
    fn cleanup_survives_tab_close_and_cannot_revoke_a_new_grant() {
        let (a, connection) = session();
        let b = SessionID::new_v4();
        let connection_id = connection.ui_handler.mcp.id.to_string();
        crate::flutter::sessions::insert_session(
            b,
            hbb_common::rendezvous_proto::ConnType::DEFAULT_CONN,
            connection.clone(),
        );
        let (sender, mut receiver) = hbb_common::tokio::sync::mpsc::unbounded_channel();
        *connection.sender.write().unwrap() = Some(sender);
        set_agent_control(a, "old".into());
        assert!(send_mouse(
            a,
            Some("old"),
            (MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN,
            || {}
        ));
        assert!(input_key(a, "old", "VK_UP", true, false, false, false, false, false).is_empty());
        receiver.try_recv().unwrap();
        crate::flutter_ffi::session_close(a);
        assert!(receiver.try_recv().is_err());
        release_input(&connection_id, "old", true);
        let crate::client::Data::Message(mouse) = receiver.try_recv().unwrap() else {
            panic!("expected mouse release");
        };
        assert_eq!(
            mouse.mouse_event().mask,
            (MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP
        );
        let crate::client::Data::Message(key) = receiver.try_recv().unwrap() else {
            panic!("expected key release");
        };
        assert!(!key.key_event().down);
        assert_eq!(agent_control_grant(&b), "");
        let mut human_sent = false;
        with_human_input(b, || human_sent = true);
        assert!(human_sent);
        set_agent_control(b, "new".into());
        release_input(&connection_id, "old", true);
        assert!(receiver.try_recv().is_err());
        assert_eq!(agent_control_grant(&b), "new");
        crate::flutter_ffi::session_close(b);
    }

    #[test]
    fn disconnect_releases_keys_before_closing_but_preserves_close_grant() {
        let (id, connection) = session();
        let connection_id = connection.ui_handler.mcp.id.to_string();
        let (sender, mut receiver) = hbb_common::tokio::sync::mpsc::unbounded_channel();
        *connection.sender.write().unwrap() = Some(sender);
        set_agent_control(id, "grant".into());
        assert!(
            input_key(id, "grant", "VK_UP", true, false, false, false, false, false).is_empty()
        );
        receiver.try_recv().unwrap();
        release_input(&connection_id, "grant", false);
        assert_eq!(agent_control_grant(&id), "grant");
        crate::flutter_ffi::session_close(id);
        let crate::client::Data::Message(key) = receiver.try_recv().unwrap() else {
            panic!("expected key release before close");
        };
        assert!(!key.key_event().down);
        assert!(matches!(
            receiver.try_recv().unwrap(),
            crate::client::Data::Close
        ));
    }

    #[test]
    fn native_keyboard_and_text_require_the_matching_owner() {
        let (id, connection) = session();
        let (sender, mut receiver) = hbb_common::tokio::sync::mpsc::unbounded_channel();
        *connection.sender.write().unwrap() = Some(sender);
        set_agent_control(id, "grant".into());
        crate::flutter_ffi::session_input_key(
            id,
            "VK_RETURN".into(),
            false,
            true,
            false,
            false,
            false,
            false,
        );
        crate::flutter_ffi::session_input_string(id, "human".into());
        crate::flutter_ffi::session_input_os_password(id, "password".into());
        crate::flutter_ffi::session_lock_screen(id);
        crate::flutter_ffi::session_ctrl_alt_del(id);
        assert!(!input_key(
            id,
            "stale",
            "VK_RETURN",
            false,
            true,
            false,
            false,
            false,
            false
        )
        .is_empty());
        assert!(!input_string(id, "stale", "stale").is_empty());
        assert!(receiver.try_recv().is_err());
        assert!(input_string(id, "grant", "agent").is_empty());
        let crate::client::Data::Message(text) = receiver.try_recv().unwrap() else {
            panic!("expected text");
        };
        assert_eq!(text.key_event().seq(), "agent");
        set_agent_control(id, String::new());
        crate::flutter_ffi::session_input_string(id, "human".into());
        let crate::client::Data::Message(text) = receiver.try_recv().unwrap() else {
            panic!("expected text");
        };
        assert_eq!(text.key_event().seq(), "human");
        crate::flutter_ffi::session_close(id);
    }

    #[test]
    fn ordinary_close_releases_agent_input_before_disconnect() {
        let (id, connection) = session();
        let (sender, mut receiver) = hbb_common::tokio::sync::mpsc::unbounded_channel();
        *connection.sender.write().unwrap() = Some(sender);
        set_agent_control(id, "grant".into());
        assert!(
            input_key(id, "grant", "VK_UP", true, false, false, false, false, false).is_empty()
        );
        crate::flutter_ffi::session_send_mcp_mouse(
            id,
            "grant".into(),
            r#"{"type":"down","buttons":"left"}"#.into(),
        );
        receiver.try_recv().unwrap();
        receiver.try_recv().unwrap();
        crate::flutter_ffi::session_close(id);
        let crate::client::Data::Message(mouse) = receiver.try_recv().unwrap() else {
            panic!("expected mouse release");
        };
        assert_eq!(
            mouse.mouse_event().mask,
            (MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP
        );
        let crate::client::Data::Message(key) = receiver.try_recv().unwrap() else {
            panic!("expected key release");
        };
        assert!(!key.key_event().down);
        assert!(matches!(
            receiver.try_recv().unwrap(),
            crate::client::Data::Close
        ));
        assert!(!input_string(id, "grant", "late typing").is_empty());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn linux_peer_rejects_held_ordinary_keys_but_accepts_press() {
        let (id, connection) = session();
        connection
            .lc
            .write()
            .unwrap()
            .handle_peer_info(&base::message_proto::PeerInfo {
                platform: "Linux".into(),
                ..Default::default()
            });
        let (sender, mut receiver) = hbb_common::tokio::sync::mpsc::unbounded_channel();
        *connection.sender.write().unwrap() = Some(sender);
        set_agent_control(id, "grant".into());
        for down in [true, false] {
            assert!(
                input_key(id, "grant", "VK_UP", down, false, false, false, false, false)
                    .contains("Linux")
            );
        }
        assert!(receiver.try_recv().is_err());
        assert!(
            input_key(id, "grant", "VK_UP", false, true, false, false, false, false).is_empty()
        );
        let crate::client::Data::Message(key) = receiver.try_recv().unwrap() else {
            panic!("expected key press");
        };
        assert!(key.key_event().press);
        crate::flutter_ffi::session_close(id);
    }

    #[test]
    fn revoked_authentication_cannot_use_a_new_grant() {
        let (id, connection) = session();
        let (sender, mut receiver) = hbb_common::tokio::sync::mpsc::unbounded_channel();
        *connection.sender.write().unwrap() = Some(sender);
        set_agent_control(id, "old".into());
        set_agent_control(id, String::new());
        assert!(!authenticate(id, "old", Some("test".into()), None));
        set_agent_control(id, "new".into());
        assert!(!authenticate(id, "old", Some("test".into()), None));
        assert!(receiver.try_recv().is_err());
        assert!(authenticate(id, "new", Some("test".into()), None));
        assert!(matches!(
            receiver.try_recv().unwrap(),
            crate::client::Data::Login(_)
        ));
        crate::flutter_ffi::session_close(id);
    }

    #[test]
    fn agent_mouse_uses_only_explicit_modifiers() {
        use rdev::{Event, EventType, Key};
        let (id, connection) = session();
        let (sender, mut receiver) = hbb_common::tokio::sync::mpsc::unbounded_channel();
        *connection.sender.write().unwrap() = Some(sender);
        set_agent_control(id, "grant".into());
        let mut event = Event {
            time: std::time::SystemTime::now(),
            unicode: None,
            event_type: EventType::KeyPress(Key::ControlLeft),
            platform_code: 0,
            position_code: 0,
            usb_hid: 0,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            extra_data: 0,
        };
        crate::keyboard::event_to_key_events(
            "Windows".into(),
            &event,
            base::message_proto::KeyboardMode::Legacy,
            None,
        );
        assert!(crate::keyboard::client::get_modifiers_state(false, false, false, false).1);
        for (msg, count) in [
            (r#"{"type":"down","buttons":"left"}"#, 0),
            (r#"{"type":"wheel","y":"1","shift":"true"}"#, 1),
        ] {
            crate::flutter_ffi::session_send_mcp_mouse(id, "grant".into(), msg.into());
            let crate::client::Data::Message(message) = receiver.try_recv().unwrap() else {
                panic!("expected mouse");
            };
            assert_eq!(message.mouse_event().modifiers.len(), count);
            if count == 1 {
                assert_eq!(
                    message.mouse_event().modifiers[0].enum_value_or_default(),
                    base::message_proto::ControlKey::Shift
                );
            }
        }
        event.event_type = EventType::KeyRelease(Key::ControlLeft);
        crate::keyboard::event_to_key_events(
            "Windows".into(),
            &event,
            base::message_proto::KeyboardMode::Legacy,
            None,
        );
        crate::flutter_ffi::session_close(id);
    }

    #[test]
    fn saves_only_the_requested_screenshot() {
        let session_id = SessionID::new_v4();
        let late = screenshot_sid(&session_id, "late");
        let wanted = screenshot_sid(&session_id, "wanted");
        let other = screenshot_sid(&SessionID::new_v4(), "wanted");
        for (sid, data) in [
            (&late, "late image"),
            (&other, "other session"),
            (&wanted, "wanted image"),
        ] {
            let response = ScreenshotResponse {
                sid: sid.clone(),
                data: data.as_bytes().to_vec().into(),
                ..Default::default()
            };
            assert!(is_requested_screenshot(&response));
            store_screenshot(response);
        }
        let path = std::env::temp_dir().join(format!("rustdesk-mcp-{session_id}.png"));
        let path = path.to_string_lossy();

        assert_eq!(save_screenshot(&wanted, &path).as_deref(), Some(""));
        assert_eq!(std::fs::read(&*path).unwrap(), b"wanted image");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&*path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(save_screenshot(&wanted, &path), None);
        std::fs::remove_file(&*path).unwrap();
        assert_eq!(save_screenshot(&other, &path).as_deref(), Some(""));
        assert_eq!(std::fs::read(&*path).unwrap(), b"other session");
        // A pre-existing path must never be overwritten (including symlinks).
        assert!(!save_screenshot(&late, &path).unwrap().is_empty());
        assert_eq!(std::fs::read(&*path).unwrap(), b"other session");
        std::fs::remove_file(&*path).unwrap();
    }

    #[test]
    fn handoff_releases_a_key_held_through_the_legacy_key_api() {
        let (session_id, session) = session();
        let mut rx = capture(&session);
        let human_key = |down: bool, press: bool| {
            crate::flutter_ffi::session_input_key(
                session_id,
                "VK_UP".to_owned(),
                down,
                press,
                false,
                false,
                false,
                false,
            )
        };

        // Held through the legacy API: no forwarded event ever sees this key.
        human_key(true, false);
        let keys = sent_keys(&mut rx);
        assert_eq!(keys.len(), 1, "{:?}", keys_of(&keys));
        assert!(
            control_key(&keys[0]) == Some(ControlKey::UpArrow)
                && keys[0].down
                && !keys[0].press,
            "{:?}",
            keys_of(&keys)
        );

        // The agent takes over; the peer must see the key come up first.
        set_agent_control(session_id, "grant".to_owned());
        let keys = sent_keys(&mut rx);
        assert!(
            keys.iter()
                .any(|k| control_key(k) == Some(ControlKey::UpArrow) && !k.down),
            "no release for the held key: {:?}",
            keys_of(&keys)
        );

        // The human key-up after the grant is dropped, so it cannot arrive
        // after the agent's own input.
        human_key(false, false);
        assert!(sent_keys(&mut rx).is_empty());

        // The agent still gets its input.
        assert!(input_key(session_id, "grant", "VK_DOWN", false, true, false, false, false, false)
            .is_empty());
        assert_eq!(sent_keys(&mut rx).len(), 1);
        set_agent_control(session_id, String::new());
        crate::flutter_ffi::session_close(session_id);
    }

    #[test]
    fn handoff_releases_forwarded_keys_held_by_a_sibling_tab() {
        let (a, connection) = session();
        let b = SessionID::new_v4();
        crate::flutter::sessions::insert_session(
            b,
            hbb_common::rendezvous_proto::ConnType::DEFAULT_CONN,
            connection.clone(),
        );
        // The human types in tab b; the agent grant lands on tab a.
        crate::flutter::set_cur_session_id(b);
        let mut rx = capture(&connection);

        let press = rdev::Event {
            time: std::time::SystemTime::now(),
            unicode: None,
            platform_code: 0,
            position_code: 0,
            event_type: rdev::EventType::KeyPress(rdev::Key::UpArrow),
            usb_hid: 0,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            extra_data: 0,
        };
        let forwarded = crate::keyboard::event_to_key_events(
            "windows".to_owned(),
            &press,
            crate::keyboard::get_keyboard_mode_enum("legacy"),
            None,
        );
        assert!(!forwarded.is_empty(), "the press must be forwardable");

        set_agent_control(a, "grant".to_owned());
        let keys = sent_keys(&mut rx);
        assert!(
            keys.iter()
                .any(|k| control_key(k) == Some(ControlKey::UpArrow) && !k.down),
            "no release through the focused sibling tab: {:?}",
            keys_of(&keys)
        );
        set_agent_control(a, String::new());
        crate::flutter_ffi::session_close(a);
        crate::flutter_ffi::session_close(b);
    }

    #[test]
    fn agent_keys_ignore_the_human_swap_preference() {
        let (session_id, session) = session();
        session
            .lc
            .write()
            .unwrap()
            .set_toggle_option_for_test("allow_swap_key", true);
        let mut rx = capture(&session);
        set_agent_control(session_id, "grant".to_owned());
        sent_keys(&mut rx);

        // The agent asked for Control, so Control goes on the wire.
        assert!(
            input_key(session_id, "grant", "VK_CONTROL", true, false, false, false, false, false)
                .is_empty()
        );
        let keys = sent_keys(&mut rx);
        assert_eq!(keys.len(), 1, "{:?}", keys_of(&keys));
        assert!(
            control_key(&keys[0]) == Some(ControlKey::Control) && keys[0].down,
            "{:?}",
            keys_of(&keys)
        );

        // Revoking releases the key the agent pressed, not its swapped form.
        set_agent_control(session_id, String::new());
        let keys = sent_keys(&mut rx);
        assert!(
            keys.iter()
                .any(|k| control_key(k) == Some(ControlKey::Control) && !k.down),
            "release did not match the pressed key: {:?}",
            keys_of(&keys)
        );
        assert!(
            !keys
                .iter()
                .any(|k| control_key(k) == Some(ControlKey::Meta)),
            "agent input was swapped: {:?}",
            keys_of(&keys)
        );

        // Human input keeps honoring the preference.
        crate::flutter_ffi::session_input_key(
            session_id,
            "VK_CONTROL".to_owned(),
            false,
            true,
            false,
            false,
            false,
            false,
        );
        let keys = sent_keys(&mut rx);
        assert_eq!(keys.len(), 1, "{:?}", keys_of(&keys));
        assert!(
            control_key(&keys[0]) == Some(ControlKey::Meta),
            "human input ignored the swap preference: {:?}",
            keys_of(&keys)
        );
        crate::flutter_ffi::session_close(session_id);
    }
}
