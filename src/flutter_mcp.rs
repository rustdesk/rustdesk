//! Native state behind the MCP server that runs in the Flutter main window.

use crate::flutter_ffi::SessionID;
use base::message_proto::ScreenshotResponse;
use std::{
    collections::{HashMap, VecDeque},
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
    static ref AGENT_SESSIONS: RwLock<HashMap<SessionID, String>> = Default::default();
    static ref SCREENSHOTS: Mutex<VecDeque<ScreenshotResponse>> = Default::default();
}

pub fn set_agent_control(session_id: SessionID, grant_id: String) {
    let mut sessions = AGENT_SESSIONS.write().unwrap();
    if !grant_id.is_empty() {
        sessions.insert(session_id, grant_id);
    } else {
        sessions.remove(&session_id);
    }
}

pub fn is_agent_control(session_id: &SessionID) -> bool {
    AGENT_SESSIONS.read().unwrap().contains_key(session_id)
}

pub fn agent_control_grant(session_id: &SessionID) -> String {
    AGENT_SESSIONS
        .read()
        .unwrap()
        .get(session_id)
        .cloned()
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

    #[test]
    fn closing_session_clears_agent_control() {
        let session_id = SessionID::new_v4();
        set_agent_control(session_id, "grant".to_owned());
        assert!(is_agent_control(&session_id));
        assert_eq!(agent_control_grant(&session_id), "grant");
        crate::flutter_ffi::session_close(session_id);
        assert!(!is_agent_control(&session_id));
        assert_eq!(agent_control_grant(&session_id), "");
    }

    #[test]
    fn ownership_reads_follow_revoke_and_regrant_without_window_updates() {
        let session_id = SessionID::new_v4();
        set_agent_control(session_id, "old".to_owned());
        let old_notice = agent_control_grant(&session_id);
        set_agent_control(session_id, String::new());
        assert!(!is_agent_control(&session_id));
        assert_eq!(agent_control_grant(&session_id), "");
        set_agent_control(session_id, "new".to_owned());
        // A window refresh reads current state, even for an old notification.
        assert_ne!(agent_control_grant(&session_id), old_notice);
        assert_eq!(agent_control_grant(&session_id), "new");
        assert!(is_agent_control(&session_id));
        set_agent_control(session_id, String::new());
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
}
