use crate::rdev::DisplayError;
use std::convert::TryInto;
use winapi::um::winuser::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

pub fn display_size() -> Result<(u64, u64), DisplayError> {
    let w = unsafe {
        GetSystemMetrics(SM_CXSCREEN)
            .try_into()
            .map_err(|_| DisplayError::ConversionError)?
    };
    let h = unsafe {
        GetSystemMetrics(SM_CYSCREEN)
            .try_into()
            .map_err(|_| DisplayError::ConversionError)?
    };
    Ok((w, h))
}
