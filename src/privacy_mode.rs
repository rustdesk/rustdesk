use crate::ui_interface::get_option;
#[cfg(windows)]
use crate::{
    display_service,
    ipc::{connect, Data},
    platform::is_installed,
};
#[cfg(windows)]
use hbb_common::tokio;
use hbb_common::{anyhow::anyhow, bail, lazy_static, tokio::sync::oneshot, ResultType};
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[cfg(windows)]
pub mod win_exclude_from_capture;
#[cfg(windows)]
mod win_input;
#[cfg(windows)]
pub mod win_mag;
#[cfg(windows)]
pub mod win_topmost_window;

#[cfg(windows)]
mod win_virtual_display;
#[cfg(windows)]
pub use win_virtual_display::restore_reg_connectivity;

pub const INVALID_PRIVACY_MODE_CONN_ID: i32 = 0;
pub const OCCUPIED: &'static str = "Privacy occupied by another one.";
pub const TURN_OFF_OTHER_ID: &'static str =
    "Failed to turn off privacy mode that belongs to someone else.";
pub const NO_PHYSICAL_DISPLAYS: &'static str = "no_need_privacy_mode_no_physical_displays_tip";

pub const PRIVACY_MODE_IMPL_WIN_MAG: &str = "privacy_mode_impl_mag";
pub const PRIVACY_MODE_IMPL_WIN_EXCLUDE_FROM_CAPTURE: &str =
    "privacy_mode_impl_exclude_from_capture";
pub const PRIVACY_MODE_IMPL_WIN_VIRTUAL_DISPLAY: &str = "privacy_mode_impl_virtual_display";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum PrivacyModeState {
    OffSucceeded,
    OffByPeer,
    OffUnknown,
}

pub trait PrivacyMode: Sync + Send {
    fn is_async_privacy_mode(&self) -> bool;

    fn init(&self) -> ResultType<()>;
    fn clear(&mut self);
    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool>;
    fn turn_off_privacy(&mut self, conn_id: i32, state: Option<PrivacyModeState>)
        -> ResultType<()>;

    fn pre_conn_id(&self) -> i32;

    fn get_impl_key(&self) -> &str;

    #[inline]
    fn check_on_conn_id(&self, conn_id: i32) -> ResultType<bool> {
        let pre_conn_id = self.pre_conn_id();
        if pre_conn_id == conn_id {
            return Ok(true);
        }
        if pre_conn_id != INVALID_PRIVACY_MODE_CONN_ID {
            bail!(OCCUPIED);
        }
        Ok(false)
    }

    #[inline]
    fn check_off_conn_id(&self, conn_id: i32) -> ResultType<()> {
        let pre_conn_id = self.pre_conn_id();
        if pre_conn_id != INVALID_PRIVACY_MODE_CONN_ID
            && conn_id != INVALID_PRIVACY_MODE_CONN_ID
            && pre_conn_id != conn_id
        {
            bail!(TURN_OFF_OTHER_ID)
        }
        Ok(())
    }
}

lazy_static::lazy_static! {
    pub static ref DEFAULT_PRIVACY_MODE_IMPL: String = {
        #[cfg(windows)]
        {
            if win_exclude_from_capture::is_supported() {
                PRIVACY_MODE_IMPL_WIN_EXCLUDE_FROM_CAPTURE
            } else {
                if display_service::is_privacy_mode_mag_supported() {
                    PRIVACY_MODE_IMPL_WIN_MAG
                } else {
                    if is_installed() {
                        PRIVACY_MODE_IMPL_WIN_VIRTUAL_DISPLAY
                    } else {
                        ""
                    }
                }
            }.to_owned()
        }
        #[cfg(not(windows))]
        {
            "".to_owned()
        }
    };

    static ref PRIVACY_MODE: Arc<Mutex<Option<Box<dyn PrivacyMode>>>> = {
        let mut cur_impl = get_option("privacy-mode-impl-key".to_owned());
        if !get_supported_privacy_mode_impl().iter().any(|(k, _)| k == &cur_impl) {
            cur_impl = DEFAULT_PRIVACY_MODE_IMPL.to_owned();
        }

        let privacy_mode = match PRIVACY_MODE_CREATOR.lock().unwrap().get(&(&cur_impl as &str)) {
            Some(creator) => Some(creator(&cur_impl)),
            None => None,
        };
        Arc::new(Mutex::new(privacy_mode))
    };
}

pub type PrivacyModeCreator = fn(impl_key: &str) -> Box<dyn PrivacyMode>;
lazy_static::lazy_static! {
    static ref PRIVACY_MODE_CREATOR: Arc<Mutex<HashMap<&'static str, PrivacyModeCreator>>> = {
        #[cfg(not(windows))]
        let map: HashMap<&'static str, PrivacyModeCreator> = HashMap::new();
        #[cfg(windows)]
        let mut map: HashMap<&'static str, PrivacyModeCreator> = HashMap::new();
        #[cfg(windows)]
        {
            if win_exclude_from_capture::is_supported() {
                map.insert(win_exclude_from_capture::PRIVACY_MODE_IMPL, |impl_key: &str| {
                    Box::new(win_exclude_from_capture::PrivacyModeImpl::new(impl_key))
                });
            } else {
                map.insert(win_mag::PRIVACY_MODE_IMPL, |impl_key: &str| {
                    Box::new(win_mag::PrivacyModeImpl::new(impl_key))
                });
            }

            map.insert(win_virtual_display::PRIVACY_MODE_IMPL, |impl_key: &str| {
                    Box::new(win_virtual_display::PrivacyModeImpl::new(impl_key))
                });
        }
        Arc::new(Mutex::new(map))
    };
}

#[inline]
pub fn init() -> Option<ResultType<()>> {
    Some(PRIVACY_MODE.lock().unwrap().as_ref()?.init())
}

#[inline]
pub fn clear() -> Option<()> {
    Some(PRIVACY_MODE.lock().unwrap().as_mut()?.clear())
}

#[inline]
pub fn switch(impl_key: &str) {
    let mut privacy_mode_lock = PRIVACY_MODE.lock().unwrap();
    if let Some(privacy_mode) = privacy_mode_lock.as_ref() {
        if privacy_mode.get_impl_key() == impl_key {
            return;
        }
    }

    if let Some(creator) = PRIVACY_MODE_CREATOR.lock().unwrap().get(impl_key) {
        *privacy_mode_lock = Some(creator(impl_key));
    }
}

fn get_supported_impl(impl_key: &str) -> String {
    let supported_impls = get_supported_privacy_mode_impl();
    if supported_impls.iter().any(|(k, _)| k == &impl_key) {
        return impl_key.to_owned();
    };
    // TODO: Is it a good idea to use fallback here? Because user do not know the fallback.
    // fallback
    let mut cur_impl = get_option("privacy-mode-impl-key".to_owned());
    if !get_supported_privacy_mode_impl()
        .iter()
        .any(|(k, _)| k == &cur_impl)
    {
        // fallback
        cur_impl = DEFAULT_PRIVACY_MODE_IMPL.to_owned();
    }
    cur_impl
}

pub async fn turn_on_privacy(impl_key: &str, conn_id: i32) -> Option<ResultType<bool>> {
    if is_async_privacy_mode() {
        turn_on_privacy_async(impl_key.to_string(), conn_id).await
    } else {
        turn_on_privacy_sync(impl_key, conn_id)
    }
}

#[inline]
fn is_async_privacy_mode() -> bool {
    PRIVACY_MODE
        .lock()
        .unwrap()
        .as_ref()
        .map_or(false, |m| m.is_async_privacy_mode())
}

#[inline]
async fn turn_on_privacy_async(impl_key: String, conn_id: i32) -> Option<ResultType<bool>> {
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        let res = turn_on_privacy_sync(&impl_key, conn_id);
        let _ = tx.send(res);
    });
    // Wait at most 7.5 seconds for the result.
    // Because it may take a long time to turn on the privacy mode with amyuni idd.
    // Some laptops may take time to plug in a virtual display.
    match hbb_common::timeout(7500, rx).await {
        Ok(res) => match res {
            Ok(res) => res,
            Err(e) => Some(Err(anyhow!(e.to_string()))),
        },
        Err(e) => Some(Err(anyhow!(e.to_string()))),
    }
}

fn turn_on_privacy_sync(impl_key: &str, conn_id: i32) -> Option<ResultType<bool>> {
    // Check if privacy mode is already on or occupied by another one
    let mut privacy_mode_lock = PRIVACY_MODE.lock().unwrap();

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

#[inline]
pub fn turn_off_privacy(conn_id: i32, state: Option<PrivacyModeState>) -> Option<ResultType<()>> {
    Some(
        PRIVACY_MODE
            .lock()
            .unwrap()
            .as_mut()?
            .turn_off_privacy(conn_id, state),
    )
}

#[inline]
pub fn check_on_conn_id(conn_id: i32) -> Option<ResultType<bool>> {
    Some(
        PRIVACY_MODE
            .lock()
            .unwrap()
            .as_ref()?
            .check_on_conn_id(conn_id),
    )
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn set_privacy_mode_state(
    conn_id: i32,
    state: PrivacyModeState,
    impl_key: String,
    ms_timeout: u64,
) -> ResultType<()> {
    let mut c = connect(ms_timeout, "_cm").await?;
    c.send(&Data::PrivacyModeState((conn_id, state, impl_key)))
        .await
}

pub fn get_supported_privacy_mode_impl() -> Vec<(&'static str, &'static str)> {
    #[cfg(target_os = "windows")]
    {
        let mut vec_impls = Vec::new();

        if win_exclude_from_capture::is_supported() {
            vec_impls.push((
                PRIVACY_MODE_IMPL_WIN_EXCLUDE_FROM_CAPTURE,
                "privacy_mode_impl_mag_tip",
            ));
        } else {
            if display_service::is_privacy_mode_mag_supported() {
                vec_impls.push((PRIVACY_MODE_IMPL_WIN_MAG, "privacy_mode_impl_mag_tip"));
            }
        }

        if is_installed() && crate::platform::windows::is_self_service_running() {
            vec_impls.push((
                PRIVACY_MODE_IMPL_WIN_VIRTUAL_DISPLAY,
                "privacy_mode_impl_virtual_display_tip",
            ));
        }

        vec_impls
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

#[inline]
pub fn get_cur_impl_key() -> Option<String> {
    PRIVACY_MODE
        .lock()
        .unwrap()
        .as_ref()
        .map(|pm| pm.get_impl_key().to_owned())
}

#[inline]
pub fn is_current_privacy_mode_impl(impl_key: &str) -> bool {
    PRIVACY_MODE
        .lock()
        .unwrap()
        .as_ref()
        .map(|pm| pm.get_impl_key() == impl_key)
        .unwrap_or(false)
}

#[inline]
#[cfg(not(windows))]
pub fn check_privacy_mode_err(
    _privacy_mode_id: i32,
    _display_idx: usize,
    _timeout_millis: u64,
) -> String {
    "".to_owned()
}

#[inline]
#[cfg(windows)]
pub fn check_privacy_mode_err(
    privacy_mode_id: i32,
    display_idx: usize,
    timeout_millis: u64,
) -> String {
    // win magnifier implementation requires a test of creating a capturer.
    if is_current_privacy_mode_impl(PRIVACY_MODE_IMPL_WIN_MAG) {
        crate::video_service::test_create_capturer(privacy_mode_id, display_idx, timeout_millis)
    } else {
        "".to_owned()
    }
}

#[inline]
pub fn is_privacy_mode_supported() -> bool {
    !DEFAULT_PRIVACY_MODE_IMPL.is_empty()
}

#[inline]
pub fn get_privacy_mode_conn_id() -> Option<i32> {
    PRIVACY_MODE
        .lock()
        .unwrap()
        .as_ref()
        .map(|pm| pm.pre_conn_id())
}

#[inline]
pub fn is_in_privacy_mode() -> bool {
    PRIVACY_MODE
        .lock()
        .unwrap()
        .as_ref()
        .map(|pm| pm.pre_conn_id() != INVALID_PRIVACY_MODE_CONN_ID)
        .unwrap_or(false)
}
