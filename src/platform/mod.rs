#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(windows)]
pub use windows::*;

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "macos")]
pub mod delegate;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(all(target_os = "linux", feature = "linux_headless"))]
#[cfg(not(any(feature = "flatpak", feature = "appimage")))]
pub mod linux_desktop_manager;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use hbb_common::{message_proto::CursorData, ResultType};
#[cfg(not(any(target_os = "macos", target_os = "android", target_os = "ios")))]
const SERVICE_INTERVAL: u64 = 300;

pub fn is_xfce() -> bool {
    #[cfg(target_os = "linux")]
    {
        return std::env::var_os("XDG_CURRENT_DESKTOP") == Some(std::ffi::OsString::from("XFCE"));
    }
    #[cfg(not(target_os = "linux"))]
    {
        return false;
    }
}

pub fn breakdown_callback() {
    #[cfg(target_os = "linux")]
    crate::input_service::clear_remapped_keycode();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    crate::input_service::release_device_modifiers();
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn change_resolution(name: &str, width: usize, height: usize) -> ResultType<()> {
    let cur_resolution = current_resolution(name)?;
    // For MacOS
    // to-do: Make sure the following comparison works. 
    // For Linux
    // Just run "xrandr", dpi may not be taken into consideration.
    // For Windows
    // dmPelsWidth and dmPelsHeight is the same to width and height
    // Because this process is running in dpi awareness mode.
    if cur_resolution.width as usize == width && cur_resolution.height as usize == height {
        return Ok(());
    }
    hbb_common::log::warn!("Change resolution of '{}' to ({},{})", name, width, height);
    change_resolution_directly(name, width, height)
}

// Android
#[cfg(target_os = "android")]
pub fn get_active_username() -> String {
    // TODO
    "android".into()
}

#[cfg(target_os = "android")]
pub const PA_SAMPLE_RATE: u32 = 48000;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cursor_data() {
        for _ in 0..30 {
            if let Some(hc) = get_cursor().unwrap() {
                let cd = get_cursor_data(hc).unwrap();
                repng::encode(
                    std::fs::File::create("cursor.png").unwrap(),
                    cd.width as _,
                    cd.height as _,
                    &cd.colors[..],
                )
                .unwrap();
            }
            #[cfg(target_os = "macos")]
            macos::is_process_trusted(false);
        }
    }
    #[test]
    fn test_get_cursor_pos() {
        for _ in 0..30 {
            assert!(!get_cursor_pos().is_none());
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn test_resolution() {
        let name = r"\\.\DISPLAY1";
        println!("current:{:?}", current_resolution(name));
        println!("change:{:?}", change_resolution(name, 2880, 1800));
        println!("resolutions:{:?}", resolutions(name));
    }
}
