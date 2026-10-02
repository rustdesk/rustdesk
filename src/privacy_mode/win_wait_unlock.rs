//! Privacy mode 2 can't be turned on while the session is locked: Windows refuses
//! `ChangeDisplaySettingsEx` while the secure desktop has the input. A request made then waits
//! here, in its own thread, so the connection never waits on the display operation.

use super::{
    get_privacy_mode_conn_id, get_supported_impl, get_supported_privacy_mode_impl,
    turn_on_privacy_sync, PRIVACY_MODE_IMPL_WIN_VIRTUAL_DISPLAY,
};
use crate::{
    common::{make_privacy_mode_msg, make_privacy_mode_msg_with_details},
    platform::{desktop_changed, is_locked},
};
use base::message_proto::{back_notification::PrivacyModeState, Message};
use hbb_common::{anyhow::anyhow, log, ResultType};
use std::{
    sync::{Arc, Condvar, Mutex, PoisonError, TryLockError},
    thread,
    time::Duration,
};

const POLL: Duration = Duration::from_secs(1);
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
    shared: Arc<(Mutex<State>, Condvar)>,
}

#[derive(Default)]
struct State {
    cancelled: bool,
    result: Option<ResultType<bool>>,
}

/// Returns `true` when this request has to wait for the unlock. It is then made from its own
/// thread once the user's desktop has the input again, and `waiting` holds it until
/// [`WaitingTurnOn::reply`] has the result.
pub fn wait_for_unlock(impl_key: &str, conn_id: i32, waiting: &mut Option<WaitingTurnOn>) -> bool {
    let effective_key = get_supported_impl(impl_key);
    let must_wait = effective_key == PRIVACY_MODE_IMPL_WIN_VIRTUAL_DISPLAY
        && get_supported_privacy_mode_impl()
            .iter()
            .any(|(k, _)| *k == effective_key)
        && get_privacy_mode_conn_id() != Some(conn_id)
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
    *waiting = Some(WaitingTurnOn::start(
        impl_key.to_owned(),
        effective_key,
        conn_id,
    ));
    true
}

impl WaitingTurnOn {
    fn start(impl_key: String, effective_key: String, conn_id: i32) -> Self {
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let worker = shared.clone();
        thread::spawn(move || {
            let (lock, wake) = &*worker;
            // Held except while waiting, so a cancellation never lands during a turn-on.
            let guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
            if guard.cancelled {
                return;
            }
            let mut state = Some(guard);
            let res = turn_on_when_ready(
                input_desktop,
                || {
                    log::info!("Privacy mode: the session is unlocked, turning it on");
                    turn_on_privacy_sync(&effective_key, conn_id)
                        .unwrap_or_else(|| Err(anyhow!("Not supported")))
                },
                || {
                    let Some(guard) = state.take() else {
                        return false;
                    };
                    let (guard, _) = wake
                        .wait_timeout(guard, POLL)
                        .unwrap_or_else(PoisonError::into_inner);
                    let cancelled = guard.cancelled;
                    state = Some(guard);
                    !cancelled
                },
            );
            if let (Some(res), Some(mut guard)) = (res, state) {
                if let Err(e) = &res {
                    log::error!("Failed to turn on privacy mode. {}", e);
                }
                guard.result = Some(res);
            }
        });
        Self { impl_key, shared }
    }

    /// The reply for the peer once the request has been made, `None` until then. It never
    /// blocks, even while the turn-on is under way.
    pub fn reply(&self) -> Option<Message> {
        let mut state = match self.shared.0.try_lock() {
            Ok(state) => state,
            Err(TryLockError::Poisoned(e)) => e.into_inner(),
            Err(TryLockError::WouldBlock) => return None,
        };
        let impl_key = self.impl_key.clone();
        Some(match state.result.take()? {
            Ok(true) => make_privacy_mode_msg(PrivacyModeState::PrvOnSucceeded, impl_key),
            Ok(false) => make_privacy_mode_msg(PrivacyModeState::PrvOnFailed, impl_key),
            Err(e) => make_privacy_mode_msg_with_details(
                PrivacyModeState::PrvOnFailed,
                e.to_string(),
                impl_key,
            ),
        })
    }
}

impl Drop for WaitingTurnOn {
    fn drop(&mut self) {
        let (lock, wake) = &*self.shared;
        // Waits out a turn-on under way, so that what the caller does next (turning privacy
        // mode off, or checking whether it is on) sees how it ended.
        lock.lock().unwrap_or_else(PoisonError::into_inner).cancelled = true;
        wake.notify_one();
    }
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
/// failure is final. Returns `None` when `wait` reports a cancellation.
fn turn_on_when_ready(
    mut desktop: impl FnMut() -> Desktop,
    mut turn_on: impl FnMut() -> ResultType<bool>,
    mut wait: impl FnMut() -> bool,
) -> Option<ResultType<bool>> {
    let mut polls_since_unlock = 0;
    loop {
        match desktop() {
            Desktop::Locked => polls_since_unlock = 0,
            seen => {
                let settled = polls_since_unlock >= SETTLE_POLLS;
                if seen == Desktop::User || settled {
                    let res = turn_on();
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
                results.next().unwrap_or(Ok(true))
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
    fn gives_up_on_the_secure_desktop_after_a_few_polls() {
        let (res, attempts) = run(
            &[Desktop::Locked, Desktop::Secure],
            vec![refused_by_the_lock_screen()],
            SETTLE_POLLS as usize + 1,
        );
        assert!(matches!(res, Some(Err(_))));
        assert_eq!(attempts, 1);
    }
}
