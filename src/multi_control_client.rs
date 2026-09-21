//! Controlling-side half of the primary-first multi-controller mode.
//!
//! The controlled side owns the arbitration (see `server/multi_control.rs`); this module
//! only remembers what the peer told us about our role, and turns the local "operate"
//! key into borrow begin/end/heartbeat messages so a helper can hover, open menus and
//! type without asking anybody for permission.
//!
//! The key is opt-in: `multi-control-operate-key` is empty by default, and only a key
//! whose Flutter/Sciter key name matches exactly is swallowed locally instead of being
//! forwarded to the peer.

// The consumers of this module are the Flutter build (the operate key and the session
// UI), so a build without it legitimately leaves parts of the API unused.
#![allow(dead_code)]

use base::{config::keys, message_proto::MultiControlState};
use hbb_common::config::Config;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Heartbeat cadence of an active borrow; the peer revokes it after 5 s of silence.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Active {
    /// The local operate key is currently held.
    held: bool,
    /// Epoch granted by the peer for the current borrow, 0 while unknown.
    epoch: u64,
    /// Whether the peer granted us the real pointer.
    borrowed: bool,
    /// Whether we are the primary controller.
    primary: bool,
    /// Whether the peer runs this mode at all, and whether our input is being accepted.
    enabled: bool,
    keyboard_ok: bool,
    notice: String,
    last_state: Option<Instant>,
}

lazy_static::lazy_static! {
    static ref ACTIVE: Mutex<Active> = Mutex::new(Active::default());
}

/// The key name that starts a borrow, empty when the feature is not configured.
pub fn operate_key() -> String {
    Config::get_option(keys::OPTION_MULTI_CONTROL_OPERATE_KEY)
}

/// Whether this local key event is the operate key instead of remote input.
pub fn is_operate_key(name: &str) -> bool {
    let key = operate_key();
    !key.is_empty() && key == name
}

/// What the peer last told us, for the session UI.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Status {
    pub enabled: bool,
    pub primary: bool,
    pub borrowed_by_me: bool,
    pub borrowed_by_other: bool,
    pub keyboard_ok: bool,
    pub epoch: u64,
    pub notice: String,
}

/// Applies a state message of the controlled side.
pub fn on_state(state: &MultiControlState) -> Status {
    let mut active = ACTIVE.lock().unwrap();
    active.enabled = state.enabled;
    active.primary = state.is_primary;
    active.borrowed = state.borrowed_by_me;
    active.keyboard_ok = state.keyboard_target_confirmed;
    active.epoch = state.epoch;
    active.notice = state.notice.clone();
    active.last_state = Some(Instant::now());
    if !state.borrowed_by_me {
        // The peer took the pointer back: a still held operate key has to ask again
        // instead of reporting the end of a borrow that no longer exists.
        active.held = false;
    }
    Status {
        enabled: state.enabled,
        primary: state.is_primary,
        borrowed_by_me: state.borrowed_by_me,
        borrowed_by_other: state.borrowed_by_other,
        keyboard_ok: state.keyboard_target_confirmed,
        epoch: state.epoch,
        notice: state.notice.clone(),
    }
}

/// The current status, for the session UI.
pub fn status() -> Status {
    status_in(&ACTIVE.lock().unwrap())
}

fn status_in(active: &Active) -> Status {
    Status {
        enabled: active.enabled,
        primary: active.primary,
        borrowed_by_me: active.borrowed,
        borrowed_by_other: false,
        keyboard_ok: active.keyboard_ok,
        epoch: active.epoch,
        notice: active.notice.clone(),
    }
}

/// The status as JSON, so the session UI can show why its input was refused without
/// knowing anything about the borrow protocol.
pub fn status_json() -> String {
    let active = ACTIVE.lock().unwrap();
    let status = status_in(&active);
    serde_json::json!({
        "enabled": status.enabled,
        "primary": status.primary,
        "borrowedByMe": status.borrowed_by_me,
        "keyboardOk": status.keyboard_ok,
        "epoch": status.epoch,
        "notice": status.notice,
        "operateKey": operate_key(),
        "operateKeyHeld": active.held,
    })
    .to_string()
}

/// The epoch to use for end/heartbeat messages, 0 while the peer has not granted one.
pub fn epoch() -> u64 {
    ACTIVE.lock().unwrap().epoch
}

/// Marks the borrow as started or stopped; returns what has to be sent.
pub fn on_operate_key(down: bool) -> Option<(MultiControlBorrowKind, u64)> {
    let mut active = ACTIVE.lock().unwrap();
    if down {
        if active.held {
            // Auto-repeat of a held key must not start a second borrow.
            return None;
        }
        active.held = true;
        return Some((MultiControlBorrowKind::Begin, 0));
    }
    if !active.held {
        return None;
    }
    active.held = false;
    Some((MultiControlBorrowKind::End, active.epoch))
}

/// Whether a borrow started by the operate key is still waiting for its heartbeat.
pub fn is_held() -> bool {
    ACTIVE.lock().unwrap().held
}

/// The epoch to put into a heartbeat, only while a borrow is held.
pub fn heartbeat_epoch() -> Option<u64> {
    let active = ACTIVE.lock().unwrap();
    (active.held && active.borrowed).then_some(active.epoch)
}

/// Starts a session with no borrowed pointer and the operate key not held: a session that
/// ended while the key was down must not swallow the next press, and must not hand back a
/// borrow the peer no longer knows about.
pub fn reset() {
    *ACTIVE.lock().unwrap() = Active::default();
}

/// The borrow kind to send, kept independent of the generated protobuf enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MultiControlBorrowKind {
    Begin,
    End,
    Heartbeat,
}

impl MultiControlBorrowKind {
    /// The protobuf value of this kind.
    pub fn to_proto(self) -> base::message_proto::multi_control_borrow::Kind {
        use base::message_proto::multi_control_borrow::Kind;
        match self {
            Self::Begin => Kind::KIND_BEGIN,
            Self::End => Kind::KIND_END,
            Self::Heartbeat => Kind::KIND_HEARTBEAT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear() {
        let mut active = ACTIVE.lock().unwrap();
        *active = Active::default();
    }

    #[test]
    fn the_operate_key_is_off_until_it_is_configured() {
        clear();
        assert!(!is_operate_key("RControl"));
        Config::set_option(
            keys::OPTION_MULTI_CONTROL_OPERATE_KEY.to_owned(),
            "RControl".to_owned(),
        );
        assert!(is_operate_key("RControl"));
        assert!(!is_operate_key("LControl"));
        Config::set_option(
            keys::OPTION_MULTI_CONTROL_OPERATE_KEY.to_owned(),
            String::new(),
        );
    }

    #[test]
    fn the_borrow_is_begun_once_and_ended_once() {
        clear();
        assert_eq!(
            on_operate_key(true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        // Auto-repeat while the key is held.
        assert_eq!(on_operate_key(true), None);
        assert_eq!(
            on_operate_key(false),
            Some((MultiControlBorrowKind::End, 0))
        );
        // A release without a press must not talk to the peer.
        assert_eq!(on_operate_key(false), None);
    }

    #[test]
    fn the_granted_epoch_is_used_for_end_and_heartbeat() {
        clear();
        assert_eq!(
            on_operate_key(true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        assert_eq!(heartbeat_epoch(), None);
        let mut state = MultiControlState::new();
        state.enabled = true;
        state.borrowed_by_me = true;
        state.epoch = 7;
        on_state(&state);
        assert_eq!(heartbeat_epoch(), Some(7));
        assert_eq!(
            on_operate_key(false),
            Some((MultiControlBorrowKind::End, 7))
        );
        assert_eq!(heartbeat_epoch(), None);
    }

    #[test]
    fn losing_the_borrow_stops_the_heartbeat_and_drops_the_epoch() {
        clear();
        assert_eq!(
            on_operate_key(true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        let mut state = MultiControlState::new();
        state.borrowed_by_me = true;
        state.epoch = 3;
        on_state(&state);
        // The primary took the pointer back; the peer stops reporting a borrow, so the
        // epoch is not ours to keep and a still held operate key has to ask again.
        let mut revoked = MultiControlState::new();
        revoked.borrowed_by_other = true;
        revoked.notice = "borrow-preempted".to_owned();
        let status = on_state(&revoked);
        assert_eq!(status.notice, "borrow-preempted");
        assert!(!is_held());
        assert_eq!(heartbeat_epoch(), None);
        assert_eq!(epoch(), 0);
    }

    #[test]
    fn a_new_session_does_not_inherit_a_held_operate_key() {
        clear();
        assert_eq!(
            on_operate_key(true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        let mut state = MultiControlState::new();
        state.borrowed_by_me = true;
        state.epoch = 5;
        on_state(&state);
        assert_eq!(heartbeat_epoch(), Some(5));
        // The session ends while the key is still down.
        reset();
        assert!(!is_held());
        assert_eq!(heartbeat_epoch(), None);
        assert_eq!(epoch(), 0);
        // The next press of the new session starts its own borrow instead of being
        // swallowed by the stale state.
        assert_eq!(
            on_operate_key(true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        assert_eq!(
            on_operate_key(false),
            Some((MultiControlBorrowKind::End, 0))
        );
    }

    #[test]
    fn every_borrow_kind_has_a_proto_value() {
        use base::message_proto::multi_control_borrow::Kind;
        assert_eq!(MultiControlBorrowKind::Begin.to_proto(), Kind::KIND_BEGIN);
        assert_eq!(MultiControlBorrowKind::End.to_proto(), Kind::KIND_END);
        assert_eq!(
            MultiControlBorrowKind::Heartbeat.to_proto(),
            Kind::KIND_HEARTBEAT
        );
    }
}
