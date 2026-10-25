// OpenHarmony platform support for the controller (主控端) build.
//
// Mirrors the Android client's platform surface: the session engine only needs
// a handful of functions; everything desktop-specific (input grabbing, screen
// capture, service install) lives behind cfg gates that exclude
// `target_env = "ohos"`.

/// Android-compatible sample rate constant referenced by shared code paths.
pub const PA_SAMPLE_RATE: u32 = 48000;

/// Placeholder username; the real identity comes from the ArkTS layer via
/// `DEVICE_NAME` (same as Android).
pub fn get_active_username() -> String {
    "ohos".into()
}

#[derive(Default)]
pub struct WakeLock;

impl WakeLock {
    pub fn new(_tag: &str) -> Self {
        // Audio/screen wakelocks are managed by the ArkTS layer
        // (`window.setWindowKeepScreenOn`) on OpenHarmony.
        Self
    }
}

#[inline]
pub fn is_prelogin() -> bool {
    false
}
