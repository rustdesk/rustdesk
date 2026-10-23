//! Privacy mode 2 can't be turned on while the session is locked: Windows refuses
//! `ChangeDisplaySettingsEx` while the secure desktop has the input. A request made then waits
//! here, in its own thread, so the connection never waits on the display operation.

use super::{
    close_other_remote_connections_for_privacy, get_privacy_mode_conn_id, get_supported_impl,
    get_supported_privacy_mode_impl, PrivacyMode, INVALID_PRIVACY_MODE_CONN_ID, PRIVACY_MODE,
    PRIVACY_MODE_CREATOR, PRIVACY_MODE_IMPL_WIN_VIRTUAL_DISPLAY,
};
use crate::{
    common::{make_privacy_mode_msg, make_privacy_mode_msg_with_details},
    platform::{desktop_changed, is_locked},
};
use base::message_proto::{back_notification::PrivacyModeState, Message};
use hbb_common::{anyhow::anyhow, ResultType};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex, MutexGuard, PoisonError, TryLockError,
    },
    thread,
    time::Duration,
};

const POLL: Duration = Duration::from_secs(1);
// Lets a request waiting for the unlock tell that a connection has turned privacy mode on since
// it was queued.
static TURN_ONS: AtomicUsize = AtomicUsize::new(0);
// Polls after the unlock during which a failure caused by the secure desktop is retried.
const SETTLE_POLLS: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Desktop {
    Locked,
    // Unlocked, but the secure desktop still has the input: the switch back from the lock
    // screen is under way, or a UAC prompt is up.
    Secure,
    User,
}

/// A privacy mode 2 request waiting for the session to be unlocked. Dropping it cancels the
/// request.
pub struct WaitingTurnOn {
    impl_key: String,
    conn_id: i32,
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
    cancelled: AtomicBool,
    // Never held during a turn-on, so cancelling doesn't wait for one.
    result: Mutex<Option<ResultType<bool>>>,
    wake: Condvar,
}

/// Returns `true` when this request has to wait for the unlock. It is then made from its own
/// thread once the user's desktop has the input again, and `waiting` holds it until
/// [`WaitingTurnOn::reply`] has the result.
pub fn wait_for_unlock(impl_key: &str, conn_id: i32, waiting: &mut Option<WaitingTurnOn>) -> bool {
    let effective_key = get_supported_impl(impl_key);
    let must_wait = effective_key == PRIVACY_MODE_IMPL_WIN_VIRTUAL_DISPLAY
        && is_supported(&effective_key)
        // Only while privacy mode is off. A request queued behind another connection's could
        // turn it back on after the user at the PC turned it off with Ctrl+P.
        && get_privacy_mode_conn_id().map_or(true, |id| id == INVALID_PRIVACY_MODE_CONN_ID)
        && is_locked();
    // A new request replaces a waiting one.
    *waiting = None;
    if !must_wait {
        return false;
    }
    hbb_common::throttled_log!(
        Duration::from_secs(60),
        info,
        "Privacy mode: the session is locked, waiting for the unlock"
    );
    let turn_ons = TURN_ONS.load(Ordering::SeqCst);
    *waiting = Some(WaitingTurnOn::spawn(
        impl_key.to_owned(),
        conn_id,
        input_desktop,
        move |cancelled| turn_on(&effective_key, conn_id, turn_ons, cancelled),
    ));
    true
}

impl WaitingTurnOn {
    fn spawn(
        impl_key: String,
        conn_id: i32,
        desktop: fn() -> Desktop,
        mut turn_on: impl FnMut(&AtomicBool) -> Option<ResultType<bool>> + Send + 'static,
    ) -> Self {
        let shared = Arc::new(Shared::default());
        let worker = shared.clone();
        thread::spawn(move || {
            let res = turn_on_when_ready(desktop, || turn_on(&worker.cancelled), || worker.wait());
            if let Some(res) = res {
                if let Err(e) = &res {
                    hbb_common::throttled_log!(
                        Duration::from_secs(60),
                        error,
                        "Failed to turn on privacy mode. {}",
                        e
                    );
                }
                *worker.lock() = Some(res);
            }
        });
        Self {
            impl_key,
            conn_id,
            shared,
        }
    }

    /// `None` until the request has been made, then the reply for the peer, if there is one.
    /// It doesn't wait for a turn-on under way.
    pub fn reply(&self) -> Option<Option<Message>> {
        let res = match self.shared.result.try_lock() {
            Ok(mut res) => res.take()?,
            Err(TryLockError::Poisoned(e)) => e.into_inner().take()?,
            Err(TryLockError::WouldBlock) => return None,
        };
        Some(reply_for(
            res,
            get_privacy_mode_conn_id() == Some(self.conn_id),
            self.impl_key.clone(),
        ))
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Option<ResultType<bool>>> {
        self.result.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // Waits a poll, or until cancelled. Returns `false` once cancelled.
    fn wait(&self) -> bool {
        let res = self.lock();
        if self.cancelled.load(Ordering::SeqCst) {
            return false;
        }
        let _ = self
            .wake
            .wait_timeout(res, POLL)
            .unwrap_or_else(PoisonError::into_inner);
        !self.cancelled.load(Ordering::SeqCst)
    }
}

// A success is reported only while privacy mode is still on for the connection. If something
// turned it off since (Ctrl+P on the controlled side, say), that has been reported already, and a
// late success would make the peer show privacy mode on.
fn reply_for(res: ResultType<bool>, still_on: bool, impl_key: String) -> Option<Message> {
    Some(match res {
        Ok(true) if !still_on => return None,
        Ok(true) => make_privacy_mode_msg(PrivacyModeState::PrvOnSucceeded, impl_key),
        Ok(false) => make_privacy_mode_msg(PrivacyModeState::PrvOnFailed, impl_key),
        Err(e) => make_privacy_mode_msg_with_details(
            PrivacyModeState::PrvOnFailed,
            e.to_string(),
            impl_key,
        ),
    })
}

impl Drop for WaitingTurnOn {
    fn drop(&mut self) {
        // A turn-on under way runs on. It checked `cancelled` under `PRIVACY_MODE`, which
        // whatever the caller does next (turning privacy mode off, or checking whether it is on)
        // takes too, so that sees how it ended.
        let _res = self.shared.lock();
        self.shared.cancelled.store(true, Ordering::SeqCst);
        self.shared.wake.notify_one();
    }
}

fn is_supported(impl_key: &str) -> bool {
    get_supported_privacy_mode_impl()
        .iter()
        .any(|(k, _)| *k == impl_key)
}

// `None` if the request was cancelled. Otherwise it is made as one that didn't wait would be,
// closing the other remote connections included.
fn turn_on(
    effective_key: &str,
    conn_id: i32,
    turn_ons: usize,
    cancelled: &AtomicBool,
) -> Option<ResultType<bool>> {
    // `turn_on_locked` would fall back to another implementation.
    if !is_supported(effective_key) {
        return Some(Err(anyhow!("Unsupported privacy mode: {}", effective_key)));
    }
    let mut privacy_mode = PRIVACY_MODE.lock().unwrap();
    let res = unless_stale(cancelled, turn_ons, || {
        hbb_common::throttled_log!(
            Duration::from_secs(60),
            info,
            "Privacy mode: the session is unlocked, turning it on"
        );
        turn_on_locked(&mut privacy_mode, effective_key, conn_id)
            .unwrap_or_else(|| Err(anyhow!("Not supported")))
    })?;
    drop(privacy_mode);
    if matches!(res, Ok(true)) {
        note_turn_on();
        close_other_remote_connections_for_privacy(conn_id);
    }
    Some(res)
}

/// Called whenever a connection has turned privacy mode on.
pub fn note_turn_on() {
    TURN_ONS.fetch_add(1, Ordering::SeqCst);
}

// `turn_on_privacy_sync`'s body, for a caller that already holds `PRIVACY_MODE`.
fn turn_on_locked(
    privacy_mode_lock: &mut Option<Box<dyn PrivacyMode>>,
    impl_key: &str,
    conn_id: i32,
) -> Option<ResultType<bool>> {
    // Check or switch privacy mode implementation
    let impl_key = get_supported_impl(impl_key);

    let mut cur_impl_key = "".to_string();
    if let Some(privacy_mode) = privacy_mode_lock.as_ref() {
        cur_impl_key = privacy_mode.get_impl_key().to_string();
        let check_on_conn_id = privacy_mode.check_on_conn_id(conn_id);
        match check_on_conn_id.as_ref() {
            Ok(true) => {
                if cur_impl_key == impl_key {
                    // Same peer, same implementation.
                    return Some(Ok(true));
                } else {
                    // Same peer, switch to new implementation.
                }
            }
            Err(_) => return Some(check_on_conn_id),
            _ => {}
        }
    }

    if cur_impl_key != impl_key {
        if let Some(creator) = PRIVACY_MODE_CREATOR
            .lock()
            .unwrap()
            .get(&(&impl_key as &str))
        {
            if let Some(privacy_mode) = privacy_mode_lock.as_mut() {
                privacy_mode.clear();
            }

            *privacy_mode_lock = Some(creator(&impl_key));
        } else {
            return Some(Err(anyhow!("Unsupported privacy mode: {}", impl_key)));
        }
    }

    // turn on privacy mode
    Some(privacy_mode_lock.as_mut()?.turn_on_privacy(conn_id))
}

// Called under `PRIVACY_MODE`, so no turn-on or turn-off lands between the check and the
// request. If another connection has turned privacy mode on since this request was queued, the
// user at the PC may have turned it off again with Ctrl+P, and this one mustn't undo that.
fn unless_stale(
    cancelled: &AtomicBool,
    turn_ons: usize,
    turn_on: impl FnOnce() -> ResultType<bool>,
) -> Option<ResultType<bool>> {
    if cancelled.load(Ordering::SeqCst) {
        return None;
    }
    if TURN_ONS.load(Ordering::SeqCst) != turn_ons {
        return Some(Err(anyhow!(
            "Privacy mode was turned on by another connection meanwhile"
        )));
    }
    Some(turn_on())
}

// Only meaningful on the waiting thread: a new thread starts on the user's desktop
// (`winsta0\default`), and `desktop_changed()` compares the thread's desktop with the input one.
fn input_desktop() -> Desktop {
    if is_locked() {
        Desktop::Locked
    } else if desktop_changed() {
        Desktop::Secure
    } else {
        Desktop::User
    }
}

/// Makes the request once the user's desktop has the input. A failure is retried only when the
/// secure desktop caused it, and only for `SETTLE_POLLS` polls after the unlock; any other
/// failure is final. Returns `None` when `turn_on` or `wait` reports a cancellation.
fn turn_on_when_ready(
    mut desktop: impl FnMut() -> Desktop,
    mut turn_on: impl FnMut() -> Option<ResultType<bool>>,
    mut wait: impl FnMut() -> bool,
) -> Option<ResultType<bool>> {
    let mut polls_since_unlock = 0;
    loop {
        match desktop() {
            Desktop::Locked => polls_since_unlock = 0,
            seen => {
                let settled = polls_since_unlock >= SETTLE_POLLS;
                if seen == Desktop::User || settled {
                    let res = turn_on()?;
                    if matches!(res, Ok(true)) || settled || desktop() == Desktop::User {
                        return Some(res);
                    }
                }
                polls_since_unlock += 1;
            }
        }
        if !wait() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::anyhow::anyhow;
    use std::sync::mpsc;

    // Each desktop check takes the next entry of `desktops` (the last one repeats) and each
    // turn-on the next of `results` (then success). After `waits` waits the request is
    // cancelled. Returns the outcome and how many turn-ons were attempted.
    fn run(
        desktops: &[Desktop],
        results: Vec<ResultType<bool>>,
        waits: usize,
    ) -> (Option<ResultType<bool>>, usize) {
        let mut desktops = desktops.iter().copied();
        let mut seen = Desktop::Locked;
        let mut results = results.into_iter();
        let mut attempts = 0;
        let mut waited = 0;
        let res = turn_on_when_ready(
            || {
                seen = desktops.next().unwrap_or(seen);
                seen
            },
            || {
                attempts += 1;
                Some(results.next().unwrap_or(Ok(true)))
            },
            || {
                waited += 1;
                waited <= waits
            },
        );
        (res, attempts)
    }

    fn refused_by_the_lock_screen() -> ResultType<bool> {
        Err(anyhow!("Failed ChangeDisplaySettingsEx, ret: -1"))
    }

    #[test]
    fn waits_while_locked_then_turns_on_once() {
        let (res, attempts) = run(&[Desktop::Locked], vec![], 100);
        assert!(res.is_none());
        assert_eq!(attempts, 0);

        let (res, attempts) = run(&[Desktop::Locked, Desktop::Locked, Desktop::User], vec![], 100);
        assert!(matches!(res, Some(Ok(true))));
        assert_eq!(attempts, 1);
    }

    #[test]
    fn retries_a_failure_the_secure_desktop_caused() {
        let (res, attempts) = run(
            &[Desktop::Locked, Desktop::User, Desktop::Secure, Desktop::User],
            vec![refused_by_the_lock_screen()],
            100,
        );
        assert!(matches!(res, Some(Ok(true))));
        assert_eq!(attempts, 2);
    }

    #[test]
    fn stops_at_any_other_failure() {
        let (res, attempts) = run(
            &[Desktop::Locked, Desktop::User],
            vec![Err(anyhow!("No virtual displays."))],
            100,
        );
        assert!(matches!(res, Some(Err(_))));
        assert_eq!(attempts, 1);
    }

    #[test]
    fn a_success_is_not_reported_once_privacy_mode_is_off_again() {
        assert!(reply_for(Ok(true), false, String::new()).is_none());
        assert!(reply_for(Ok(true), true, String::new()).is_some());
        assert!(reply_for(refused_by_the_lock_screen(), false, String::new()).is_some());
    }

    #[test]
    fn gives_up_on_the_secure_desktop_after_a_few_polls() {
        let (res, attempts) = run(
            &[Desktop::Locked, Desktop::Secure],
            vec![refused_by_the_lock_screen()],
            SETTLE_POLLS as usize + 1,
        );
        assert!(matches!(res, Some(Err(_))));
        assert_eq!(attempts, 1);
    }

    #[test]
    fn a_request_queued_before_another_connections_turn_on_is_not_made() {
        let queued = TURN_ONS.load(Ordering::SeqCst);
        TURN_ONS.fetch_add(1, Ordering::SeqCst);
        let res = unless_stale(&AtomicBool::new(false), queued, || panic!("turned on"));
        assert!(matches!(res, Some(Err(_))));
    }

    #[test]
    fn cancelling_does_not_wait_for_a_turn_on_under_way() {
        let (started, turning_on) = mpsc::channel();
        let (release, stalled) = mpsc::channel::<()>();
        let waiting = WaitingTurnOn::spawn(String::new(), 1, || Desktop::User, move |_| {
            let _ = started.send(());
            let _ = stalled.recv();
            Some(Ok(true))
        });
        turning_on.recv().unwrap();
        let (dropped, done) = mpsc::channel();
        thread::spawn(move || {
            drop(waiting);
            let _ = dropped.send(());
        });
        let res = done.recv_timeout(Duration::from_secs(5));
        let _ = release.send(());
        assert!(res.is_ok());
    }
}
