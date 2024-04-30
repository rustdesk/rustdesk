use super::*;
use hbb_common::{allow_err, platform::linux::DISTRO};
use scrap::{is_cursor_embedded, set_map_err, Capturer, Display, Frame, TraitCapturer};
use std::io;
use std::process::{Command, Output};

use crate::{
    client::{
        SCRAP_OTHER_VERSION_OR_X11_REQUIRED, SCRAP_UBUNTU_HIGHER_REQUIRED, SCRAP_X11_REQUIRED,
    },
    platform::linux::is_x11,
};

lazy_static::lazy_static! {
    static ref CAP_DISPLAY_INFO: RwLock<u64> = RwLock::new(0);
    static ref LOG_SCRAP_COUNT: Mutex<u32> = Mutex::new(0);
}

pub fn init() {
    set_map_err(map_err_scrap);
}

fn map_err_scrap(err: String) -> io::Error {
    // to-do: Remove this the following log
    log::error!(
        "REMOVE ME ===================================== wayland scrap error {}",
        &err
    );

    // to-do: Handle error better, do not restart server
    if err.starts_with("Did not receive a reply") {
        log::error!("Fatal pipewire error, {}", &err);
        std::process::exit(-1);
    }

    if DISTRO.name.to_uppercase() == "Ubuntu".to_uppercase() {
        if DISTRO.version_id < "21".to_owned() {
            io::Error::new(io::ErrorKind::Other, SCRAP_UBUNTU_HIGHER_REQUIRED)
        } else {
            try_log(&err);
            io::Error::new(io::ErrorKind::Other, err)
        }
    } else {
        try_log(&err);
        if err.contains("org.freedesktop.portal")
            || err.contains("pipewire")
            || err.contains("dbus")
        {
            io::Error::new(io::ErrorKind::Other, SCRAP_OTHER_VERSION_OR_X11_REQUIRED)
        } else {
            io::Error::new(io::ErrorKind::Other, SCRAP_X11_REQUIRED)
        }
    }
}

fn try_log(err: &String) {
    let mut lock_count = LOG_SCRAP_COUNT.lock().unwrap();
    if *lock_count >= 1000000 {
        return;
    }
    if *lock_count % 10000 == 0 {
        log::error!("Failed scrap {}", err);
    }
    *lock_count += 1;
}

struct CapturerPtr(*mut Capturer);

impl Clone for CapturerPtr {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl TraitCapturer for CapturerPtr {
    fn frame<'a>(&'a mut self, timeout: Duration) -> io::Result<Frame<'a>> {
        unsafe { (*self.0).frame(timeout) }
    }
}

struct CapDisplayInfo {
    rects: Vec<((i32, i32), usize, usize)>,
    displays: Vec<DisplayInfo>,
    num: usize,
    primary: usize,
    current: usize,
    capturer: CapturerPtr,
}

#[tokio::main(flavor = "current_thread")]
pub(super) async fn ensure_inited() -> ResultType<()> {
    check_init().await
}

pub(super) fn is_inited() -> Option<Message> {
    if is_x11() {
        None
    } else {
        if *CAP_DISPLAY_INFO.read().unwrap() == 0 {
            let mut msg_out = Message::new();
            let res = MessageBox {
                msgtype: "nook-nocancel-hasclose".to_owned(),
                title: "Wayland".to_owned(),
                text: "Please Select the screen to be shared(Operate on the peer side).".to_owned(),
                link: "".to_owned(),
                ..Default::default()
            };
            msg_out.set_message_box(res);
            Some(msg_out)
        } else {
            None
        }
    }
}

fn get_max_desktop_resolution() -> Option<String> {
    // works with Xwayland
    let output: Output = Command::new("sh")
        .arg("-c")
        .arg("xrandr | awk '/current/ { print $8,$9,$10 }'")
        .output()
        .ok()?;

    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout);
        Some(result.trim().to_string())
    } else {
        None
    }
}

pub(super) async fn check_init() -> ResultType<()> {
    if !is_x11() {
        let mut minx = 0;
        let mut maxx = 0;
        let mut miny = 0;
        let mut maxy = 0;

        if *CAP_DISPLAY_INFO.read().unwrap() == 0 {
            let mut lock = CAP_DISPLAY_INFO.write().unwrap();
            if *lock == 0 {
                let mut all = Display::all()?;
                let num = all.len();
                let primary = super::display_service::get_primary_2(&all);
                let current = primary;
                super::display_service::check_update_displays(&all);
                let mut displays = super::display_service::get_sync_displays();
                for display in displays.iter_mut() {
                    display.cursor_embedded = is_cursor_embedded();
                }

                let mut rects: Vec<((i32, i32), usize, usize)> = Vec::new();
                for d in &all {
                    rects.push((d.origin(), d.width(), d.height()));
                }

                let display = all.remove(current);
                let (origin, width, height) = (display.origin(), display.width(), display.height());
                log::debug!(
                    "#displays={}, current={}, origin: {:?}, width={}, height={}, cpus={}/{}",
                    num,
                    current,
                    &origin,
                    width,
                    height,
                    num_cpus::get_physical(),
                    num_cpus::get(),
                );

                let (max_width, max_height) = match get_max_desktop_resolution() {
                    Some(result) if !result.is_empty() => {
                        let resolution: Vec<&str> = result.split(" ").collect();
                        let w: i32 = resolution[0].parse().unwrap_or(origin.0 + width as i32);
                        let h: i32 = resolution[2]
                            .trim_end_matches(",")
                            .parse()
                            .unwrap_or(origin.1 + height as i32);
                        if w < origin.0 + width as i32 || h < origin.1 + height as i32 {
                            (origin.0 + width as i32, origin.1 + height as i32)
                        }
                        else{
                            (w, h)
                        }
                    }
                    _ => (origin.0 + width as i32, origin.1 + height as i32),
                };

                minx = 0;
                maxx = max_width;
                miny = 0;
                maxy = max_height;

                let capturer = Box::into_raw(Box::new(
                    Capturer::new(display).with_context(|| "Failed to create capturer")?,
                ));
                let capturer = CapturerPtr(capturer);
                let cap_display_info = Box::into_raw(Box::new(CapDisplayInfo {
                    rects,
                    displays,
                    num,
                    primary,
                    current,
                    capturer,
                }));
                *lock = cap_display_info as _;
            }
        }

        if minx != maxx && miny != maxy {
            log::info!(
                "update mouse resolution: ({}, {}), ({}, {})",
                minx,
                maxx,
                miny,
                maxy
            );
            allow_err!(input_service::update_mouse_resolution(minx, maxx, miny, maxy).await);
        }
    }
    Ok(())
}

pub(super) async fn get_displays() -> ResultType<Vec<DisplayInfo>> {
    check_init().await?;
    let addr = *CAP_DISPLAY_INFO.read().unwrap();
    if addr != 0 {
        let cap_display_info: *const CapDisplayInfo = addr as _;
        unsafe {
            let cap_display_info = &*cap_display_info;
            Ok(cap_display_info.displays.clone())
        }
    } else {
        bail!("Failed to get capturer display info");
    }
}

pub(super) fn get_primary() -> ResultType<usize> {
    let addr = *CAP_DISPLAY_INFO.read().unwrap();
    if addr != 0 {
        let cap_display_info: *const CapDisplayInfo = addr as _;
        unsafe {
            let cap_display_info = &*cap_display_info;
            Ok(cap_display_info.primary)
        }
    } else {
        bail!("Failed to get capturer display info");
    }
}

pub fn clear() {
    if is_x11() {
        return;
    }
    let mut write_lock = CAP_DISPLAY_INFO.write().unwrap();
    if *write_lock != 0 {
        let cap_display_info: *mut CapDisplayInfo = *write_lock as _;
        unsafe {
            let _box_capturer = Box::from_raw((*cap_display_info).capturer.0);
            let _box_cap_display_info = Box::from_raw(cap_display_info);
            *write_lock = 0;
        }
    }
}

pub(super) fn get_capturer() -> ResultType<super::video_service::CapturerInfo> {
    if is_x11() {
        bail!("Do not call this function if not wayland");
    }
    let addr = *CAP_DISPLAY_INFO.read().unwrap();
    if addr != 0 {
        let cap_display_info: *const CapDisplayInfo = addr as _;
        unsafe {
            let cap_display_info = &*cap_display_info;
            let rect = cap_display_info.rects[cap_display_info.current];
            Ok(super::video_service::CapturerInfo {
                origin: rect.0,
                width: rect.1,
                height: rect.2,
                ndisplay: cap_display_info.num,
                current: cap_display_info.current,
                privacy_mode_id: 0,
                _capturer_privacy_mode_id: 0,
                capturer: Box::new(cap_display_info.capturer.clone()),
            })
        }
    } else {
        bail!("Failed to get capturer display info");
    }
}

pub fn common_get_error() -> String {
    if DISTRO.name.to_uppercase() == "Ubuntu".to_uppercase() {
        if DISTRO.version_id < "21".to_owned() {
            return "".to_owned();
        }
    } else {
        // to-do: check other distros
    }
    return "".to_owned();
}
