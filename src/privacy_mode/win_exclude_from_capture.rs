use hbb_common::platform::windows::is_windows_version_or_greater;

pub use super::win_topmost_window::PrivacyModeImpl;

pub(super) const PRIVACY_MODE_IMPL: &str = "privacy_mode_impl_exclude_from_capture";

pub(super) fn is_supported() -> bool {
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity
    // https://en.wikipedia.org/wiki/Windows_10_version_history
    is_windows_version_or_greater(10, 0, 19041, 0, 0)
}
