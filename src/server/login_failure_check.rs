use crate::AlarmAuditType;
use hbb_common::get_time;
#[cfg(target_os = "windows")]
use hbb_common::tokio::sync::{Mutex as TokioMutex, OwnedMutexGuard};
#[cfg(target_os = "windows")]
use std::sync::Arc;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

const OS_CREDENTIAL_LOGIN_TOTAL_IDLE_RESET_MS: i64 = 120 * 60 * 1_000;
const OS_CREDENTIAL_LOGIN_BACKOFF_BASE_SECONDS: i64 = 15;
const OS_CREDENTIAL_LOGIN_BACKOFF_MAX_SECONDS: i64 = 30 * 60;
const MAX_PASSWORD_LOGIN_FAILURES: u32 = 6;
const PASSWORD_LOGIN_FAILURE_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PasswordLoginLimited {
    pub(crate) report_audit: bool,
}

#[derive(Default)]
struct PasswordFailureWindow {
    expires_at: Option<Instant>,
    failures: u32,
    audit_requested: bool,
}

#[derive(Default)]
struct PasswordLoginLimiter {
    state: Mutex<PasswordFailureWindow>,
}

impl PasswordLoginLimiter {
    fn validate(
        &self,
        now: Instant,
        validate: impl FnOnce() -> bool,
    ) -> Result<bool, PasswordLoginLimited> {
        // Keep verification and failure accounting atomic across connections.
        let mut state = self.state.lock().unwrap();
        if state.expires_at.is_some_and(|expires_at| now >= expires_at) {
            *state = PasswordFailureWindow::default();
        }
        if state.failures > MAX_PASSWORD_LOGIN_FAILURES {
            let report_audit = !state.audit_requested;
            state.audit_requested = true;
            return Err(PasswordLoginLimited { report_audit });
        }
        let valid = validate();
        if valid {
            *state = PasswordFailureWindow::default();
        } else {
            state.failures += 1;
            state
                .expires_at
                .get_or_insert(now + PASSWORD_LOGIN_FAILURE_WINDOW);
        }
        Ok(valid)
    }
}

pub(crate) fn validate_password_with_failure_limit(
    validate: impl FnOnce() -> bool,
) -> Result<bool, PasswordLoginLimited> {
    PASSWORD_LOGIN_LIMITER.validate(Instant::now(), validate)
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum FailureScope {
    Default,
    TerminalOsLogin,
}

pub(crate) struct OsCredentialPolicyDecision {
    pub allowed: bool,
    pub login_error: Option<String>,
    pub audit: Option<AlarmAuditType>,
}

#[derive(Copy, Clone, Debug, Default)]
struct OsCredentialFailureState {
    total_failures: i32,
    backoff_until_ms: Option<i64>,
    last_failure_ms: Option<i64>,
}

lazy_static::lazy_static! {
    static ref PASSWORD_LOGIN_LIMITER: PasswordLoginLimiter = PasswordLoginLimiter::default();
    static ref OS_CREDENTIAL_LOGIN_FAILURE_STATE: Mutex<OsCredentialFailureState> =
        Mutex::new(OsCredentialFailureState::default());
}

#[cfg(target_os = "windows")]
lazy_static::lazy_static! {
    static ref OS_CREDENTIAL_LOGIN_MUTEX: Arc<TokioMutex<()>> = Arc::new(TokioMutex::new(()));
}

fn is_os_credential_scope(scope: FailureScope) -> bool {
    matches!(scope, FailureScope::TerminalOsLogin)
}

fn state_for_os_credential_scope(
    scope: FailureScope,
) -> Option<&'static Mutex<OsCredentialFailureState>> {
    if is_os_credential_scope(scope) {
        Some(&OS_CREDENTIAL_LOGIN_FAILURE_STATE)
    } else {
        None
    }
}

fn backoff_audit_type_for_scope(scope: FailureScope) -> Option<AlarmAuditType> {
    match scope {
        FailureScope::TerminalOsLogin => Some(AlarmAuditType::TerminalOsLoginBackoff),
        FailureScope::Default => None,
    }
}

fn os_credential_login_backoff_seconds(total_failures: i32) -> i64 {
    if total_failures <= 2 {
        return 0;
    }
    let exp = (total_failures - 3).min(7);
    let seconds = OS_CREDENTIAL_LOGIN_BACKOFF_BASE_SECONDS * (1_i64 << exp);
    seconds.min(OS_CREDENTIAL_LOGIN_BACKOFF_MAX_SECONDS)
}

fn normalize_backoff(state: &mut OsCredentialFailureState, now_ms: i64) {
    if let Some(until_ms) = state.backoff_until_ms {
        if until_ms <= now_ms {
            state.backoff_until_ms = None;
        }
    }
}

fn reset_totals_on_idle(state: &mut OsCredentialFailureState, now_ms: i64) {
    if let Some(last_ms) = state.last_failure_ms {
        if now_ms.saturating_sub(last_ms) >= OS_CREDENTIAL_LOGIN_TOTAL_IDLE_RESET_MS {
            state.total_failures = 0;
            state.backoff_until_ms = None;
            state.last_failure_ms = None;
        }
    }
}

fn allow_decision() -> OsCredentialPolicyDecision {
    OsCredentialPolicyDecision {
        allowed: true,
        login_error: None,
        audit: None,
    }
}

fn block_decision(
    login_error: String,
    alarm_type: Option<AlarmAuditType>,
) -> OsCredentialPolicyDecision {
    OsCredentialPolicyDecision {
        allowed: false,
        login_error: Some(login_error),
        audit: alarm_type,
    }
}

pub(crate) fn evaluate_os_credential_policy(
    scope: FailureScope,
    now_ms: i64,
) -> OsCredentialPolicyDecision {
    if !is_os_credential_scope(scope) {
        return allow_decision();
    }
    let Some(state_mutex) = state_for_os_credential_scope(scope) else {
        return allow_decision();
    };
    let mut state = state_mutex.lock().unwrap();
    reset_totals_on_idle(&mut state, now_ms);
    normalize_backoff(&mut state, now_ms);

    if let Some(until_ms) = state.backoff_until_ms {
        let remaining_ms = (until_ms - now_ms).max(0);
        let remaining_seconds = ((remaining_ms + 999) / 1_000).max(1);
        let seconds_label = if remaining_seconds == 1 {
            "second"
        } else {
            "seconds"
        };
        block_decision(
            format!(
                "Please try again in {} {}.",
                remaining_seconds, seconds_label
            ),
            backoff_audit_type_for_scope(scope),
        )
    } else {
        allow_decision()
    }
}

pub(crate) fn record_os_credential_failure(scope: FailureScope) {
    if !is_os_credential_scope(scope) {
        return;
    }
    let Some(state_mutex) = state_for_os_credential_scope(scope) else {
        return;
    };
    let mut state = state_mutex.lock().unwrap();
    let now_ms = get_time();
    reset_totals_on_idle(&mut state, now_ms);
    normalize_backoff(&mut state, now_ms);
    state.total_failures = state.total_failures.saturating_add(1);
    state.last_failure_ms = Some(now_ms);
    let backoff_seconds = os_credential_login_backoff_seconds(state.total_failures);
    if backoff_seconds > 0 {
        state.backoff_until_ms = Some(now_ms + backoff_seconds * 1_000);
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn try_acquire_os_credential_login_gate() -> Result<OwnedMutexGuard<()>, ()> {
    OS_CREDENTIAL_LOGIN_MUTEX
        .clone()
        .try_lock_owned()
        .map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_os_credential_failure_state(scope: FailureScope) {
        if let Some(state_mutex) = state_for_os_credential_scope(scope) {
            *state_mutex.lock().unwrap() = OsCredentialFailureState::default();
        }
    }

    #[test]
    fn os_credential_policy_prioritizes_backoff() {
        clear_os_credential_failure_state(FailureScope::TerminalOsLogin);
        let now_ms = get_time();
        for _ in 0..3 {
            record_os_credential_failure(FailureScope::TerminalOsLogin);
        }
        let decision = evaluate_os_credential_policy(FailureScope::TerminalOsLogin, now_ms);
        assert!(!decision.allowed);
        assert!(decision.login_error.is_some());
        clear_os_credential_failure_state(FailureScope::TerminalOsLogin);
    }
}
