use super::*;
use hbb_common::allow_err;
use scrap::{Capturer, Display, Frame, TraitCapturer};
use std::io::Result;

lazy_static::lazy_static! {
    static ref CAP_DISPLAY_INFO: RwLock<u64> = RwLock::new(0);
}

struct CapturerPtr(*mut Capturer);

impl Clone for CapturerPtr {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl TraitCapturer for CapturerPtr {
    fn frame<'a>(&'a mut self, timeout: Duration) -> Result<Frame<'a>> {
        unsafe { (*self.0).frame(timeout) }
    }

    fn set_use_yuv(&mut self, use_yuv: bool) {
        unsafe {
            (*self.0).set_use_yuv(use_yuv);
        }
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

async fn check_init() -> ResultType<()> {
    if !scrap::is_x11() {
        let mut minx = 0;
        let mut maxx = 0;
        let mut miny = 0;
        let mut maxy = 0;

        if *CAP_DISPLAY_INFO.read().unwrap() == 0 {
            let mut lock = CAP_DISPLAY_INFO.write().unwrap();
            if *lock == 0 {
                let all = Display::all()?;
                let num = all.len();
                let (primary, displays) = super::video_service::get_displays_2(&all);

                let mut rects: Vec<((i32, i32), usize, usize)> = Vec::new();
                for d in &all {
                    rects.push((d.origin(), d.width(), d.height()));
                }

                let (ndisplay, current, display) =
                    super::video_service::get_current_display_2(all)?;
                let (origin, width, height) = (display.origin(), display.width(), display.height());
                log::debug!(
                    "#displays={}, current={}, origin: {:?}, width={}, height={}, cpus={}/{}",
                    ndisplay,
                    current,
                    &origin,
                    width,
                    height,
                    num_cpus::get_physical(),
                    num_cpus::get(),
                );

                minx = origin.0;
                maxx = origin.0 + width as i32;
                miny = origin.1;
                maxy = origin.1 + height as i32;

                let capturer = Box::into_raw(Box::new(
                    Capturer::new(display, true).with_context(|| "Failed to create capturer")?,
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
                "send uinput resolution: ({}, {}), ({}, {})",
                minx,
                maxx,
                miny,
                maxy
            );
            allow_err!(input_service::set_uinput_resolution(minx, maxx, miny, maxy).await);
            allow_err!(input_service::set_uinput().await);
        }
    }
    Ok(())
}

pub fn clear() {
    if scrap::is_x11() {
        return;
    }

    let mut lock = CAP_DISPLAY_INFO.write().unwrap();
    if *lock != 0 {
        unsafe {
            let cap_display_info = Box::from_raw(*lock as *mut CapDisplayInfo);
            let _ = Box::from_raw(cap_display_info.capturer.0);
        }
        *lock = 0;
    }
}

pub(super) async fn get_displays() -> ResultType<(usize, Vec<DisplayInfo>)> {
    check_init().await?;
    let addr = *CAP_DISPLAY_INFO.read().unwrap();
    if addr != 0 {
        let cap_display_info: *const CapDisplayInfo = addr as _;
        unsafe {
            let cap_display_info = &*cap_display_info;
            let primary = cap_display_info.primary;
            let displays = cap_display_info.displays.clone();
            Ok((primary, displays))
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

pub(super) fn get_display_num() -> ResultType<usize> {
    let addr = *CAP_DISPLAY_INFO.read().unwrap();
    if addr != 0 {
        let cap_display_info: *const CapDisplayInfo = addr as _;
        unsafe {
            let cap_display_info = &*cap_display_info;
            Ok(cap_display_info.num)
        }
    } else {
        bail!("Failed to get capturer display info");
    }
}

pub(super) fn get_capturer() -> ResultType<super::video_service::CapturerInfo> {
    if scrap::is_x11() {
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
                _captuerer_privacy_mode_id: 0,
                capturer: Box::new(cap_display_info.capturer.clone()),
            })
        }
    } else {
        bail!("Failed to get capturer display info");
    }
}
