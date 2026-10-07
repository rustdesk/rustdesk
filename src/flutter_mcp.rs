//! Native state behind the MCP server that runs in the Flutter main window.

use crate::flutter_ffi::SessionID;
use base::message_proto::ScreenshotResponse;
use std::{
    collections::{HashSet, VecDeque},
    sync::{Mutex, RwLock},
};

// Joins a session id and a request id into the sid of a screenshot request.
// The peer echoes the sid, so a result is matched with the request that asked
// for it instead of going to the cache the screenshot dialog reads.
const REQUEST_SEPARATOR: char = '#';
// Results nobody collects, e.g. after a timeout, are dropped oldest first.
const MAX_SCREENSHOTS: usize = 4;

lazy_static::lazy_static! {
    static ref AGENT_SESSIONS: RwLock<HashSet<SessionID>> = Default::default();
    static ref SCREENSHOTS: Mutex<VecDeque<ScreenshotResponse>> = Default::default();
}

pub fn set_agent_control(session_id: SessionID, agent: bool) {
    let mut sessions = AGENT_SESSIONS.write().unwrap();
    if agent {
        sessions.insert(session_id);
    } else {
        sessions.remove(&session_id);
    }
}

pub fn is_agent_control(session_id: &SessionID) -> bool {
    AGENT_SESSIONS.read().unwrap().contains(session_id)
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
    Some(match std::fs::write(path, &response.data) {
        Ok(()) => String::new(),
        Err(e) => e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_only_the_requested_screenshot() {
        let session_id = SessionID::new_v4();
        let late = screenshot_sid(&session_id, "late");
        let wanted = screenshot_sid(&session_id, "wanted");
        for (sid, data) in [(&late, "late image"), (&wanted, "wanted image")] {
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
        assert_eq!(save_screenshot(&wanted, &path), None);
        std::fs::remove_file(&*path).unwrap();
    }
}
