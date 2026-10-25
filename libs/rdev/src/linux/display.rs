use crate::linux::common::Display;
use crate::rdev::DisplayError;

pub fn display_size() -> Result<(u64, u64), DisplayError> {
    let display = Display::new().ok_or(DisplayError::NoDisplay)?;
    display.get_size().ok_or(DisplayError::NoDisplay)
}
