use hbb_common::ResultType;

pub fn init() {
    // Initialize iOS-specific components
    #[cfg(feature = "flutter")]
    {
        log::info!("Initializing iOS platform");
    }
}

pub fn get_display_server() -> String {
    "iOS".to_string()
}

pub fn is_installed() -> bool {
    // iOS apps are always "installed" via App Store or TestFlight
    true
}

pub fn get_active_display() -> String {
    "iOS Display".to_string()
}

pub fn get_display_names() -> Vec<String> {
    vec!["iOS Screen".to_string()]
}

pub fn is_root() -> bool {
    // iOS apps run in sandbox, never root
    false
}

pub fn check_super_user_permission() -> ResultType<bool> {
    // iOS doesn't have super user concept
    Ok(false)
}

pub fn elevate(cmd: &str) -> ResultType<bool> {
    // iOS doesn't support elevation
    Ok(false)
}

pub fn run_as_user(arg: Vec<&str>) -> ResultType<()> {
    // iOS apps always run as current user
    Ok(())
}

pub fn get_app_name() -> String {
    "RustDesk".to_string()
}

pub fn is_prelogin() -> bool {
    false
}

pub fn is_can_screen_recording() -> bool {
    // Check if screen recording permission is granted
    // This would need to be implemented with iOS-specific APIs
    true
}

pub fn is_installed_daemon(prompt: bool) -> bool {
    false
}

pub fn is_login_screen() -> bool {
    false
}

pub fn lock_screen() {
    // Cannot lock screen on iOS from app
}

pub fn is_screen_locked() -> bool {
    false
}

pub fn switch_display(display: &str) {
    // iOS only has one display
}

pub fn is_text_control_key(key: &enigo::Key) -> bool {
    matches!(
        key,
        enigo::Key::Return
            | enigo::Key::Space
            | enigo::Key::Delete
            | enigo::Key::Backspace
            | enigo::Key::LeftArrow
            | enigo::Key::RightArrow
            | enigo::Key::UpArrow
            | enigo::Key::DownArrow
            | enigo::Key::End
            | enigo::Key::Home
    )
}

#[inline]
pub fn is_x11() -> bool {
    false
}

#[inline]
pub fn is_wayland() -> bool {
    false
}

pub fn is_permission_granted() -> bool {
    // This would check ReplayKit permissions
    true
}

pub fn request_permission() -> bool {
    // This would request ReplayKit permissions
    true
}