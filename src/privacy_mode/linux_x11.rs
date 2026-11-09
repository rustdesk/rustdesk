//! Privacy mode for Linux X11 sessions.
//!
//! Like the macOS implementation: the physical monitors are blacked out through the CRTC gamma
//! ramp (the framebuffer, and so the capture, is unaffected) and physical keyboards/mice are
//! grabbed away from the desktop, while remote input (XTEST) keeps working.
//!
//! Like logging on at the console of a Windows machine used over RDP, a key press at the
//! console shows the lock screen there; unlocking it ends the remote session.
//! See `libs/x11_privacy`.

use super::{PrivacyMode, PrivacyModeState, INVALID_PRIVACY_MODE_CONN_ID};
use hbb_common::{anyhow::anyhow, config::Config, log, tokio, ResultType};
use std::{
    sync::atomic::{AtomicI32, Ordering},
    time::Duration,
};
use x11_privacy::{Options, PrivacyGuard};

pub const PRIVACY_MODE_IMPL: &str = "privacy_mode_impl_x11";

/// The connection whose session was taken over at the console, if any.
static TAKEN_OVER_CONN_ID: AtomicI32 = AtomicI32::new(INVALID_PRIVACY_MODE_CONN_ID);

fn state_file() -> std::path::PathBuf {
    Config::path("privacy_mode_x11.state")
}

pub fn is_supported() -> bool {
    crate::platform::linux::is_x11() && x11_privacy::is_supported()
}

/// Undo a privacy mode left on by a crashed server process. Called when `--server` starts.
pub fn recover() {
    match x11_privacy::recover(&state_file()) {
        Ok(true) => log::info!("Restored the console left in privacy mode by a previous run"),
        Ok(false) => {}
        Err(e) => log::error!("Failed to restore the console from privacy mode: {}", e),
    }
}

/// True if the person at the console unlocked the session of `conn_id`. The session must then
/// not be locked again when that connection closes.
pub fn is_taken_over(conn_id: i32) -> bool {
    conn_id != INVALID_PRIVACY_MODE_CONN_ID && TAKEN_OVER_CONN_ID.load(Ordering::SeqCst) == conn_id
}

/// Called when the connection that owns privacy mode closes, before privacy mode is turned off.
/// With "lock after session end", waits until the lock screen is up, so that switching the
/// monitors back on never shows the unlocked desktop.
pub async fn before_turn_off_on_close(conn_id: i32, lock_requested: bool) {
    if !lock_requested
        || is_taken_over(conn_id)
        || !super::is_current_privacy_mode_impl(PRIVACY_MODE_IMPL)
    {
        return;
    }
    let locked =
        tokio::task::spawn_blocking(|| x11_privacy::wait_for_screen_locked(Duration::from_secs(5)))
            .await
            .unwrap_or(false);
    if !locked {
        log::warn!("The screen did not report locked before leaving privacy mode");
    }
}

fn on_takeover(conn_id: i32) {
    log::info!(
        "Session unlocked at the console, closing remote connection {}",
        conn_id
    );
    TAKEN_OVER_CONN_ID.store(conn_id, Ordering::SeqCst);
    // Must not take the privacy mode lock: its holder may be joining the thread we run on.
    for conn in crate::server::AUTHED_CONNS.lock().unwrap().iter() {
        if conn.conn_id == conn_id {
            if let Err(e) = conn.sender.send(crate::ipc::Data::Close) {
                log::error!("Failed to close connection {}: {}", conn_id, e);
            }
        }
    }
}

pub struct PrivacyModeImpl {
    impl_key: String,
    conn_id: i32,
    guard: Option<PrivacyGuard>,
}

impl PrivacyModeImpl {
    pub fn new(impl_key: &str) -> Self {
        Self {
            impl_key: impl_key.to_owned(),
            conn_id: INVALID_PRIVACY_MODE_CONN_ID,
            guard: None,
        }
    }

    fn release(&mut self) -> ResultType<()> {
        let res = match self.guard.take() {
            Some(guard) => guard.turn_off().map_err(|e| anyhow!(e.to_string())),
            None => Ok(()),
        };
        self.conn_id = INVALID_PRIVACY_MODE_CONN_ID;
        res
    }
}

impl PrivacyMode for PrivacyModeImpl {
    fn is_async_privacy_mode(&self) -> bool {
        false
    }

    fn init(&self) -> ResultType<()> {
        Ok(())
    }

    fn clear(&mut self) {
        if let Err(e) = self.release() {
            log::error!("Failed to turn off x11 privacy mode: {}", e);
        }
    }

    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool> {
        if self.check_on_conn_id(conn_id)? {
            return Ok(true);
        }
        TAKEN_OVER_CONN_ID.store(INVALID_PRIVACY_MODE_CONN_ID, Ordering::SeqCst);
        let opts = Options {
            on_takeover: Some(Box::new(move || on_takeover(conn_id))),
            ..Default::default()
        };
        let guard = PrivacyGuard::turn_on(opts, state_file())
            .map_err(|e| anyhow!("Failed to turn on privacy mode: {}", e))?;
        self.guard = Some(guard);
        self.conn_id = conn_id;
        Ok(true)
    }

    fn turn_off_privacy(
        &mut self,
        conn_id: i32,
        _state: Option<PrivacyModeState>,
    ) -> ResultType<()> {
        self.check_off_conn_id(conn_id)?;
        self.release()
    }

    fn pre_conn_id(&self) -> i32 {
        self.conn_id
    }

    fn get_impl_key(&self) -> &str {
        &self.impl_key
    }
}

impl Drop for PrivacyModeImpl {
    fn drop(&mut self) {
        self.clear();
    }
}
