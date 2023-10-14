use super::*;
#[cfg(all(windows, feature = "virtual_display_driver"))]
use crate::virtual_display_manager;
#[cfg(windows)]
use hbb_common::get_version_number;
use hbb_common::protobuf::MessageField;
use scrap::Display;

pub const NAME: &'static str = "display";

struct ChangedResolution {
    original: (i32, i32),
    changed: (i32, i32),
}

lazy_static::lazy_static! {
    static ref IS_CAPTURER_MAGNIFIER_SUPPORTED: bool = is_capturer_mag_supported();
    static ref CHANGED_RESOLUTIONS: Arc<RwLock<HashMap<String, ChangedResolution>>> = Default::default();
    // Initial primary display index.
    // It should only be updated when the rustdesk server is started, and should not be updated when displays changed.
    pub static ref PRIMARY_DISPLAY_IDX: usize = get_primary();
}

#[inline]
pub fn set_last_changed_resolution(display_name: &str, original: (i32, i32), changed: (i32, i32)) {
    let mut lock = CHANGED_RESOLUTIONS.write().unwrap();
    match lock.get_mut(display_name) {
        Some(res) => res.changed = changed,
        None => {
            lock.insert(
                display_name.to_owned(),
                ChangedResolution { original, changed },
            );
        }
    }
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn reset_resolutions() {
    for (name, res) in CHANGED_RESOLUTIONS.read().unwrap().iter() {
        let (w, h) = res.original;
        if let Err(e) = crate::platform::change_resolution(name, w as _, h as _) {
            log::error!(
                "Failed to reset resolution of display '{}' to ({},{}): {}",
                name,
                w,
                h,
                e
            );
        }
    }
}

#[inline]
fn is_capturer_mag_supported() -> bool {
    #[cfg(windows)]
    return scrap::CapturerMag::is_supported();
    #[cfg(not(windows))]
    false
}

#[inline]
pub fn capture_cursor_embedded() -> bool {
    scrap::is_cursor_embedded()
}

#[inline]
pub fn is_privacy_mode_supported() -> bool {
    #[cfg(windows)]
    return *IS_CAPTURER_MAGNIFIER_SUPPORTED
        && get_version_number(&crate::VERSION) > get_version_number("1.1.9");
    #[cfg(not(windows))]
    return false;
}

#[derive(Default)]
struct StateDisplay {
    synced_displays: Vec<DisplayInfo>,
}

impl super::service::Reset for StateDisplay {
    fn reset(&mut self) {
        self.synced_displays.clear();
    }
}

pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), false);
    GenericService::repeat::<StateDisplay, _, _>(&svc.clone(), 300, run);
    svc.sp
}

fn check_get_displays_changed_msg(last_synced_displays: &mut Vec<DisplayInfo>) -> Option<Message> {
    let displays = try_get_displays().ok()?;
    if displays.len() == last_synced_displays.len() {
        return None;
    }

    // Display to DisplayInfo
    let displays = to_display_info(&displays);
    if last_synced_displays.len() == 0 {
        *last_synced_displays = displays;
        None
    } else {
        let mut pi = PeerInfo {
            ..Default::default()
        };
        pi.displays = displays.clone();
        pi.current_display = 0;
        let mut msg_out = Message::new();
        msg_out.set_peer_info(pi);
        *last_synced_displays = displays;
        Some(msg_out)
    }
}

#[cfg(all(windows, feature = "virtual_display_driver"))]
pub fn try_plug_out_virtual_display() {
    let _res = virtual_display_manager::plug_out_headless();
}

fn run(sp: EmptyExtraFieldService, state: &mut StateDisplay) -> ResultType<()> {
    if let Some(msg_out) = check_get_displays_changed_msg(&mut state.synced_displays) {
        sp.send(msg_out);
        log::info!("Displays changed");
    }
    Ok(())
}

#[inline]
pub(super) fn get_original_resolution(
    display_name: &str,
    w: usize,
    h: usize,
) -> MessageField<Resolution> {
    #[cfg(all(windows, feature = "virtual_display_driver"))]
    let is_virtual_display = crate::virtual_display_manager::is_virtual_display(&display_name);
    #[cfg(not(all(windows, feature = "virtual_display_driver")))]
    let is_virtual_display = false;
    Some(if is_virtual_display {
        Resolution {
            width: 0,
            height: 0,
            ..Default::default()
        }
    } else {
        let mut changed_resolutions = CHANGED_RESOLUTIONS.write().unwrap();
        let (width, height) = match changed_resolutions.get(display_name) {
            Some(res) => {
                if res.changed.0 != w as i32 || res.changed.1 != h as i32 {
                    // If the resolution is changed by third process, remove the record in changed_resolutions.
                    changed_resolutions.remove(display_name);
                    (w as _, h as _)
                } else {
                    res.original
                }
            }
            None => (w as _, h as _),
        };
        Resolution {
            width,
            height,
            ..Default::default()
        }
    })
    .into()
}

pub fn to_display_info(all: &Vec<Display>) -> Vec<DisplayInfo> {
    all.iter()
        .map(|d| {
            let display_name = d.name();
            let original_resolution = get_original_resolution(&display_name, d.width(), d.height());
            DisplayInfo {
                x: d.origin().0 as _,
                y: d.origin().1 as _,
                width: d.width() as _,
                height: d.height() as _,
                name: display_name,
                online: d.is_online(),
                cursor_embedded: false,
                original_resolution,
                ..Default::default()
            }
        })
        .collect::<Vec<DisplayInfo>>()
}

pub fn is_inited_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    if !scrap::is_x11() {
        return super::wayland::is_inited();
    }
    None
}

pub async fn get_displays() -> ResultType<Vec<DisplayInfo>> {
    #[cfg(target_os = "linux")]
    {
        if !scrap::is_x11() {
            return super::wayland::get_displays().await;
        }
    }
    Ok(to_display_info(&try_get_displays()?))
}

#[inline]
pub fn get_primary() -> usize {
    #[cfg(target_os = "linux")]
    {
        if !scrap::is_x11() {
            return match super::wayland::get_primary() {
                Ok(n) => n,
                Err(_) => 0,
            };
        }
    }

    try_get_displays().map(|d| get_primary_2(&d)).unwrap_or(0)
}

#[inline]
pub fn get_primary_2(all: &Vec<Display>) -> usize {
    all.iter().position(|d| d.is_primary()).unwrap_or(0)
}

#[inline]
#[cfg(all(windows, feature = "virtual_display_driver"))]
fn no_displays(displays: &Vec<Display>) -> bool {
    let display_len = displays.len();
    if display_len == 0 {
        true
    } else if display_len == 1 {
        let display = &displays[0];
        let dummy_display_side_max_size = 800;
        display.width() <= dummy_display_side_max_size
            && display.height() <= dummy_display_side_max_size
    } else {
        false
    }
}

#[inline]
#[cfg(not(all(windows, feature = "virtual_display_driver")))]
pub fn try_get_displays() -> ResultType<Vec<Display>> {
    Ok(Display::all()?)
}

#[cfg(all(windows, feature = "virtual_display_driver"))]
pub fn try_get_displays() -> ResultType<Vec<Display>> {
    let mut displays = Display::all()?;
    if no_displays(&displays) {
        log::debug!("no displays, create virtual display");
        if let Err(e) = virtual_display_manager::plug_in_headless() {
            log::error!("plug in headless failed {}", e);
        } else {
            displays = Display::all()?;
        }
    }
    Ok(displays)
}
