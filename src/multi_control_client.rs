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

// The consumers of this module are the Flutter build (the operate key and the session UI),
// so a build without it legitimately leaves parts of the API unused.
#![cfg_attr(not(feature = "flutter"), allow(dead_code))]

use base::{config::keys, message_proto::MultiControlState};
use hbb_common::config::Config;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

/// The key of one controlling session: what a window borrows belongs to that window, and a
/// process can hold several of them. Kept as text so this module does not depend on the
/// types of a build that has no session layer of its own.
pub type SessionKey = String;

/// Heartbeat cadence of an active borrow; the peer revokes it after 5 s of silence.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);
/// How long a held operate key keeps renewing the borrow without any local input. A window
/// that loses focus never delivers the release, so without this the borrow would never end.
pub const HELD_IDLE_LIMIT: Duration = Duration::from_secs(30);

lazy_static::lazy_static! {
    /// Whether the process already has a thread renewing its borrows.
    static ref HEARTBEATING: AtomicBool = AtomicBool::new(false);
    static ref EPOCH: Instant = Instant::now();
    /// When the user last did something locally, per session, in milliseconds since the
    /// module started. Two windows can hold the operate key for two different peers at
    /// once, so what one of them does is not liveness for the other.
    static ref LAST_LOCAL_INPUT: Mutex<HashMap<SessionKey, u64>> = Mutex::new(HashMap::new());
}

fn now_ms() -> u64 {
    EPOCH.elapsed().as_millis() as u64
}

/// Records that the user did something in this session. Cheap enough for the input path:
/// the heartbeat needs it to tell "still working" from "the window that owned the key is
/// gone", which is what happens when a window loses focus while the key is held.
pub fn touch_local_input(session: &SessionKey) {
    if let Ok(mut last) = LAST_LOCAL_INPUT.lock() {
        // 0 means "nothing recorded yet", so the first input of a process cannot look like it.
        last.insert(session.clone(), now_ms().max(1));
    }
}

fn last_local_input_in(session: &SessionKey) -> u64 {
    LAST_LOCAL_INPUT
        .lock()
        .ok()
        .and_then(|last| last.get(session).copied())
        .unwrap_or(0)
}

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
    /// One entry per controlling session: a process can hold several windows, each with its
    /// own peer, and what one of them borrowed is not what the other one holds.
    static ref ACTIVE: Mutex<HashMap<SessionKey, Active>> = Mutex::new(HashMap::new());
}

/// Runs `f` on the state of one session, creating it on first use.
fn with_active<T>(session: &SessionKey, f: impl FnOnce(&mut Active) -> T) -> T {
    let mut all = ACTIVE.lock().unwrap();
    f(all.entry(session.clone()).or_default())
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
pub fn on_state(session: &SessionKey, state: &MultiControlState) -> Status {
    with_active(session, |active| {
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
    })
}

/// Marks the borrow as started or stopped; returns what has to be sent.
pub fn on_operate_key(session: &SessionKey, down: bool) -> Option<(MultiControlBorrowKind, u64)> {
    with_active(session, |active| {
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
        // The pointer is handed back right here, so nothing is left to renew until the peer
        // reports a borrow again.
        active.borrowed = false;
        let epoch = active.epoch;
        active.epoch = 0;
        Some((MultiControlBorrowKind::End, epoch))
    })
}

/// What each session still has to renew: the message that keeps its borrow alive.
///
/// Every borrow a session holds has to be renewed, not only the one begun by the operate
/// key: the peer gives up a borrow that stops speaking, and a click that borrowed the
/// pointer is a borrow like any other.
pub fn heartbeat_due() -> Vec<(SessionKey, MultiControlBorrowKind, u64)> {
    let at_ms = now_ms();
    let mut all = ACTIVE.lock().unwrap();
    let mut due = Vec::new();
    for (session, active) in all.iter_mut() {
        if let Some(epoch) = heartbeat_in(active, at_ms, last_local_input_in(session)) {
            due.push((session.clone(), MultiControlBorrowKind::Heartbeat, epoch));
        }
    }
    due
}

fn heartbeat_in(active: &mut Active, at_ms: u64, last_input_ms: u64) -> Option<u64> {
    if active.epoch == 0 || (!active.held && !active.borrowed) {
        return None;
    }
    if active.held {
        let limit = HELD_IDLE_LIMIT.as_millis() as u64;
        // No local input recorded yet means "just now": the key press that started the
        // borrow is itself local input, so this can only happen before the input path
        // starts reporting, and guessing "idle" there would drop a fresh borrow.
        if last_input_ms != 0 && at_ms.saturating_sub(last_input_ms) >= limit {
            // Nothing happened in this window for a long time: the release of the operate
            // key was never delivered (a window that lost focus does not get one), so the
            // renewing stops here and the peer's lease gives the pointer back.
            active.held = false;
            active.borrowed = false;
            active.epoch = 0;
            return None;
        }
    }
    Some(active.epoch)
}

/// Claims the one heartbeat thread of this process; false when it is already running.
pub fn claim_heartbeat() -> bool {
    !HEARTBEATING.swap(true, Ordering::SeqCst)
}

/// Lets a later borrow start a heartbeat thread again.
pub fn release_heartbeat() {
    HEARTBEATING.store(false, Ordering::SeqCst);
}

/// The end message for the borrow this session still holds, if any; the state is cleared, so
/// a session that ends does not hand back a borrow twice.
pub fn end_for_close(session: &SessionKey) -> Option<(MultiControlBorrowKind, u64)> {
    with_active(session, |active| {
        let pending = active.held || active.borrowed;
        let epoch = active.epoch;
        *active = Active::default();
        pending.then_some((MultiControlBorrowKind::End, epoch))
    })
}

/// Starts a session with no borrowed pointer and the operate key not held: a session that
/// ended while the key was down must not swallow the next press, and must not hand back a
/// borrow the peer no longer knows about.
pub fn reset(session: &SessionKey) {
    with_active(session, |active| *active = Active::default());
    if let Ok(mut last) = LAST_LOCAL_INPUT.lock() {
        last.remove(session);
    }
}

/// Forgets a session that is gone, so a process that opens many of them does not keep
/// collecting their state.
pub fn forget(session: &SessionKey) {
    if let Ok(mut all) = ACTIVE.lock() {
        all.remove(session);
    }
    if let Ok(mut last) = LAST_LOCAL_INPUT.lock() {
        last.remove(session);
    }
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

    fn sk() -> SessionKey {
        "session".to_owned()
    }

    /// The renewals of the test's session, as `(kind, epoch)`.
    fn renewals() -> Vec<(MultiControlBorrowKind, u64)> {
        heartbeat_due()
            .into_iter()
            .filter(|(session, _, _)| session == &sk())
            .map(|(_, kind, epoch)| (kind, epoch))
            .collect()
    }

    fn clear() {
        let mut active = ACTIVE.lock().unwrap();
        active.clear();
        LAST_LOCAL_INPUT.lock().unwrap().clear();
    }

    /// The state of this module is process-global, so its tests take turns and start from
    /// a clean one.
    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        lazy_static::lazy_static! {
            static ref TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());
        }
        let guard = TESTS.lock().unwrap_or_else(|err| err.into_inner());
        clear();
        guard
    }

    #[test]
    fn a_held_key_stops_renewing_when_nothing_happens_locally() {
        let _guard = exclusive();
        assert_eq!(
            on_operate_key(&sk(), true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        let mut state = MultiControlState::new();
        state.borrowed_by_me = true;
        state.epoch = 9;
        on_state(&sk(), &state);
        // The user is working on this side: the borrow is renewed.
        touch_local_input(&sk());
        assert_eq!(renewals(), vec![(MultiControlBorrowKind::Heartbeat, 9)]);
        // The window that owned the key lost focus and never delivered its release: after
        // the local idle limit the renewal stops, so the peer's lease ends the borrow
        // instead of leaving the pointer borrowed forever.
        let limit = HELD_IDLE_LIMIT.as_millis() as u64;
        let last = last_local_input_in(&sk());
        let mut all = ACTIVE.lock().unwrap();
        let active = all.get_mut(&sk()).expect("the session state");
        assert_eq!(heartbeat_in(active, last + limit, last), None);
        assert!(!active.held);
        assert_eq!(active.epoch, 0);
    }

    #[test]
    fn two_windows_do_not_share_a_borrow() {
        let _guard = exclusive();
        let (one, two) = ("one".to_owned(), "two".to_owned());
        // Both windows hold the operate key for their own peer.
        assert_eq!(
            on_operate_key(&one, true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        assert_eq!(
            on_operate_key(&two, true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        let mut state = MultiControlState::new();
        state.borrowed_by_me = true;
        state.epoch = 4;
        on_state(&one, &state);
        // Only the window whose peer granted the borrow has something to renew.
        let due = heartbeat_due();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0, one);
        assert_eq!(due[0].2, 4);
        // Ending the borrow of one window leaves the other one alone.
        assert_eq!(
            on_operate_key(&one, false),
            Some((MultiControlBorrowKind::End, 4))
        );
        // Its own liveness is not the other window's liveness either.
        assert_eq!(last_local_input_in(&two), 0);
        touch_local_input(&one);
        assert_ne!(last_local_input_in(&one), 0);
        assert_eq!(last_local_input_in(&two), 0);
    }

    #[test]
    fn the_operate_key_is_off_until_it_is_configured() {
        let _guard = exclusive();
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
        let _guard = exclusive();
        assert_eq!(
            on_operate_key(&sk(), true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        // Auto-repeat while the key is held.
        assert_eq!(on_operate_key(&sk(), true), None);
        assert_eq!(
            on_operate_key(&sk(), false),
            Some((MultiControlBorrowKind::End, 0))
        );
        // A release without a press must not talk to the peer.
        assert_eq!(on_operate_key(&sk(), false), None);
    }

    #[test]
    fn the_granted_epoch_is_used_for_end_and_heartbeat() {
        let _guard = exclusive();
        assert_eq!(
            on_operate_key(&sk(), true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        assert!(renewals().is_empty());
        let mut state = MultiControlState::new();
        state.enabled = true;
        state.borrowed_by_me = true;
        state.epoch = 7;
        on_state(&sk(), &state);
        assert_eq!(renewals(), vec![(MultiControlBorrowKind::Heartbeat, 7)]);
        assert_eq!(
            on_operate_key(&sk(), false),
            Some((MultiControlBorrowKind::End, 7))
        );
        // The borrow is over, so there is nothing left to renew.
        assert!(renewals().is_empty());
    }

    #[test]
    fn a_click_borrow_is_renewed_too() {
        let _guard = exclusive();
        // Nothing is held locally: the peer gave this side the pointer because of a click,
        // and it still has to be renewed, or the peer takes it back mid-operation.
        let mut state = MultiControlState::new();
        state.borrowed_by_me = true;
        state.epoch = 4;
        on_state(&sk(), &state);
        assert_eq!(renewals(), vec![(MultiControlBorrowKind::Heartbeat, 4)]);
        // A borrow the peer has not granted yet has no epoch to renew.
        clear();
        assert_eq!(
            on_operate_key(&sk(), true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        assert!(renewals().is_empty());
    }

    #[test]
    fn losing_the_borrow_stops_the_heartbeat_and_drops_the_epoch() {
        let _guard = exclusive();
        assert_eq!(
            on_operate_key(&sk(), true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        let mut state = MultiControlState::new();
        state.borrowed_by_me = true;
        state.epoch = 3;
        on_state(&sk(), &state);
        // The primary took the pointer back; the peer stops reporting a borrow, so the
        // epoch is not ours to keep and a still held operate key has to ask again.
        let mut revoked = MultiControlState::new();
        revoked.borrowed_by_other = true;
        revoked.notice = "borrow-preempted".to_owned();
        let status = on_state(&sk(), &revoked);
        assert_eq!(status.notice, "borrow-preempted");
        assert!(status.epoch == 0);
        assert!(renewals().is_empty());
        // The key is still down, but the peer already took the pointer back: asking again
        // is a fresh begin and never a late release of a borrow that is gone.
        assert_eq!(on_operate_key(&sk(), false), None);
        assert_eq!(
            on_operate_key(&sk(), true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
    }

    #[test]
    fn closing_a_session_hands_a_borrow_back_once() {
        let _guard = exclusive();
        assert_eq!(
            on_operate_key(&sk(), true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        let mut state = MultiControlState::new();
        state.borrowed_by_me = true;
        state.epoch = 5;
        on_state(&sk(), &state);
        assert_eq!(
            end_for_close(&sk()),
            Some((MultiControlBorrowKind::End, 5)),
            "the peer must be told, or it keeps the pointer for a session that is gone"
        );
        // Nothing is left to hand back, and nothing of that session is left behind.
        assert_eq!(end_for_close(&sk()), None);
        forget(&sk());
        assert!(ACTIVE.lock().unwrap().is_empty());
        assert!(LAST_LOCAL_INPUT.lock().unwrap().is_empty());
    }

    #[test]
    fn a_new_session_does_not_inherit_a_held_operate_key() {
        let _guard = exclusive();
        assert_eq!(
            on_operate_key(&sk(), true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        let mut state = MultiControlState::new();
        state.borrowed_by_me = true;
        state.epoch = 5;
        on_state(&sk(), &state);
        assert_eq!(renewals(), vec![(MultiControlBorrowKind::Heartbeat, 5)]);
        // The session ends while the key is still down.
        reset(&sk());
        assert!(renewals().is_empty());
        assert_eq!(end_for_close(&sk()), None);
        // The next press of the new session starts its own borrow instead of being
        // swallowed by the stale state.
        assert_eq!(
            on_operate_key(&sk(), true),
            Some((MultiControlBorrowKind::Begin, 0))
        );
        assert_eq!(
            on_operate_key(&sk(), false),
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
