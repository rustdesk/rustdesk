use crate::ipc::PrivacyModeState;
use hbb_common::{bail, ResultType};

#[cfg(all(windows, feature = "privacy_win_mag"))]
pub mod privacy_win_mag;
pub const WIN_MAG_INJECTED_PROCESS_EXE: &'static str = "RuntimeBroker_rustdesk.exe";

pub const OCCUPIED: &'static str = "Privacy occupied by another one";
pub const TURN_OFF_OTHER_ID: &'static str =
    "Failed to turn off privacy mode that belongs to someone else";
pub const NO_DISPLAYS: &'static str = "No displays";

#[cfg(all(windows, feature = "virtual_display_driver"))]
pub mod privacy_win_idd;

pub trait PrivacyMode {
    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool>;
    fn turn_off_privacy(&mut self, conn_id: i32, state: Option<PrivacyModeState>)
        -> ResultType<()>;

    fn cur_conn_id(&self) -> i32;

    #[inline]
    fn check_on_conn_id(&self, conn_id: i32) -> ResultType<bool> {
        let pre_conn_id = self.cur_conn_id();
        if pre_conn_id == conn_id {
            return Ok(true);
        }
        if pre_conn_id != 0 {
            bail!(OCCUPIED);
        }
        Ok(false)
    }

    #[inline]
    fn check_off_conn_id(&self, conn_id: i32) -> ResultType<()> {
        let pre_conn_id = self.cur_conn_id();
        if pre_conn_id != 0 && conn_id != 0 && pre_conn_id != conn_id {
            bail!(TURN_OFF_OTHER_ID)
        }
        Ok(())
    }
}
