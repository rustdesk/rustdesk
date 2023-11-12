#[cfg(windows)]
use crate::ipc::{connect, Data};
use crate::{ipc::PrivacyModeState, ui_interface::get_option};
#[cfg(windows)]
use hbb_common::tokio;
use hbb_common::{bail, lazy_static, ResultType};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[cfg(windows)]
mod win_input;

#[cfg(windows)]
pub mod win_mag;

#[cfg(all(windows, feature = "virtual_display_driver"))]
mod win_virtual_display;
#[cfg(all(windows, feature = "virtual_display_driver"))]
pub use win_virtual_display::restore_reg_connectivity;

pub const INVALID_PRIVACY_MODE_CONN_ID: i32 = 0;
pub const OCCUPIED: &'static str = "Privacy occupied by another one";
pub const TURN_OFF_OTHER_ID: &'static str =
    "Failed to turn off privacy mode that belongs to someone else";
pub const NO_DISPLAYS: &'static str = "No displays";

#[cfg(windows)]
pub const PRIVACY_MODE_IMPL_WIN_MAG: &str = win_mag::PRIVACY_MODE_IMPL;

#[cfg(all(windows, feature = "virtual_display_driver"))]
pub const PRIVACY_MODE_IMPL_WIN_VIRTUAL_DISPLAY: &str = win_virtual_display::PRIVACY_MODE_IMPL;

#[cfg(windows)]
pub const DEFAULT_PRIVACY_MODE_IMPL: &str = PRIVACY_MODE_IMPL_WIN_MAG;
#[cfg(not(windows))]
pub const DEFAULT_PRIVACY_MODE_IMPL: &str = "";

pub trait PrivacyMode: Sync + Send {
    fn init(&self) -> ResultType<()>;
    fn clear(&mut self);
    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool>;
    fn turn_off_privacy(&mut self, conn_id: i32, state: Option<PrivacyModeState>)
        -> ResultType<()>;

    fn pre_conn_id(&self) -> i32;

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
    static ref CUR_PRIVACY_MODE_IMPL: Arc<Mutex<String>> = {
        let mut cur_impl = get_option("privacy-mode-impl".to_owned());
        if !get_supported_privacy_mode_impl().iter().any(|(k, _)| k == &cur_impl) {
            cur_impl = DEFAULT_PRIVACY_MODE_IMPL.to_owned();
        }
        Arc::new(Mutex::new(cur_impl))
    };
    static ref PRIVACY_MODE: Arc<Mutex<Option<Box<dyn PrivacyMode>>>> = {
        let cur_impl = (*CUR_PRIVACY_MODE_IMPL.lock().unwrap()).clone();
        let privacy_mode = match PRIVACY_MODE_CREATOR.lock().unwrap().get(&(&cur_impl as &str)) {
            Some(creator) => Some(creator()),
            None => None,
        };
        Arc::new(Mutex::new(privacy_mode))
    };
}

pub type PrivacyModeCreator = fn() -> Box<dyn PrivacyMode>;
lazy_static::lazy_static! {
    static ref PRIVACY_MODE_CREATOR: Arc<Mutex<HashMap<&'static str, PrivacyModeCreator>>> = {
        #[cfg(not(windows))]
        let map: HashMap<&'static str, PrivacyModeCreator> = HashMap::new();
        #[cfg(windows)]
        let mut map: HashMap<&'static str, PrivacyModeCreator> = HashMap::new();
        #[cfg(windows)]
        {
            map.insert(win_mag::PRIVACY_MODE_IMPL, || {
                    Box::new(win_mag::PrivacyModeImpl::default())
                });

            #[cfg(feature = "virtual_display_driver")]
            map.insert(win_virtual_display::PRIVACY_MODE_IMPL, || {
                    Box::new(win_virtual_display::PrivacyModeImpl::default())
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
pub fn switch(impl_method: &str) {
    let mut cur_impl_lock = CUR_PRIVACY_MODE_IMPL.lock().unwrap();
    if *cur_impl_lock == impl_method {
        return;
    }
    if let Some(creator) = PRIVACY_MODE_CREATOR.lock().unwrap().get(impl_method) {
        *PRIVACY_MODE.lock().unwrap() = Some(creator());
        *cur_impl_lock = impl_method.to_owned();
    }
}

#[inline]
pub fn turn_on_privacy(conn_id: i32) -> Option<ResultType<bool>> {
    Some(
        PRIVACY_MODE
            .lock()
            .unwrap()
            .as_mut()?
            .turn_on_privacy(conn_id),
    )
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
    ms_timeout: u64,
) -> ResultType<()> {
    let mut c = connect(ms_timeout, "_cm").await?;
    c.send(&Data::PrivacyModeState((conn_id, state))).await
}

pub fn get_supported_privacy_mode_impl() -> Vec<(&'static str, &'static str)> {
    #[cfg(target_os = "windows")]
    {
        let mut vec_impls = Vec::new();
        vec_impls.push((PRIVACY_MODE_IMPL_WIN_MAG, "privacy_mode_impl_mag_tip"));
        #[cfg(feature = "virtual_display_driver")]
        if crate::platform::windows::is_installed() {
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
pub fn is_current_privacy_mode_impl(impl_method: &str) -> bool {
    *CUR_PRIVACY_MODE_IMPL.lock().unwrap() == impl_method
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
    if is_current_privacy_mode_impl(PRIVACY_MODE_IMPL_WIN_MAG) {
        crate::video_service::test_create_capturer(privacy_mode_id, display_idx, timeout_millis)
    } else {
        "".to_owned()
    }
}
