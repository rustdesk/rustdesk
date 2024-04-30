use super::win_topmost_window::PRIVACY_WINDOW_NAME;
use hbb_common::{bail, log, ResultType};
use std::time::Instant;

pub use super::win_topmost_window::PrivacyModeImpl;

pub(super) const PRIVACY_MODE_IMPL: &str = "privacy_mode_impl_mag";

pub fn create_capturer(
    privacy_mode_id: i32,
    origin: (i32, i32),
    width: usize,
    height: usize,
) -> ResultType<Option<scrap::CapturerMag>> {
    if !super::is_current_privacy_mode_impl(PRIVACY_MODE_IMPL) {
        return Ok(None);
    }

    match scrap::CapturerMag::new(origin, width, height) {
        Ok(mut c1) => {
            let mut ok = false;
            let check_begin = Instant::now();
            while check_begin.elapsed().as_secs() < 5 {
                match c1.exclude("", PRIVACY_WINDOW_NAME) {
                    Ok(false) => {
                        ok = false;
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                    Err(e) => {
                        bail!(
                            "Failed to exclude privacy window {} - {}, err: {}",
                            "",
                            PRIVACY_WINDOW_NAME,
                            e
                        );
                    }
                    _ => {
                        ok = true;
                        break;
                    }
                }
            }
            if !ok {
                bail!(
                    "Failed to exclude privacy window {} - {} ",
                    "",
                    PRIVACY_WINDOW_NAME
                );
            }
            log::debug!("Create magnifier capture for {}", privacy_mode_id);
            Ok(Some(c1))
        }
        Err(e) => {
            bail!(format!("Failed to create magnifier capture {}", e));
        }
    }
}
