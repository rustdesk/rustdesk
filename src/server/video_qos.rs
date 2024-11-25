use super::*;
use scrap::codec::Quality;
use std::time::Duration;
pub const FPS: u32 = 30;
pub const MIN_FPS: u32 = 1;
pub const MAX_FPS: u32 = 120;
trait Percent {
    fn as_percent(&self) -> u32;
}

impl Percent for ImageQuality {
    fn as_percent(&self) -> u32 {
        match self {
            ImageQuality::NotSet => 0,
            ImageQuality::Low => 50,
            ImageQuality::Balanced => 66,
            ImageQuality::Best => 100,
        }
    }
}

#[derive(Default, Debug, Copy, Clone)]
struct Delay {
    state: DelayState,
    staging_state: DelayState,
    delay: u32,
    counter: u32,
    slower_than_old_state: Option<bool>,
}

#[derive(Default, Debug, Copy, Clone)]
struct UserData {
    auto_adjust_fps: Option<u32>, // reserve for compatibility
    custom_fps: Option<u32>,
    quality: Option<(i64, Quality)>, // (time, quality)
    delay: Option<Delay>,
    record: bool,
}

pub struct VideoQoS {
    fps: f32,
    quality: Quality,
    users: HashMap<i32, UserData>,
    bitrate_store: u32,
    support_abr: HashMap<usize, bool>,
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum DelayState {
    Normal = 0,
    LowDelay = 200,
    HighDelay = 500,
    Broken = 3000,
}

impl Default for DelayState {
    fn default() -> Self {
        DelayState::Normal
    }
}

impl DelayState {
    fn from_delay(delay: u32) -> Self {
        if delay > DelayState::Broken as u32 {
            DelayState::Broken
        } else if delay > DelayState::HighDelay as u32 {
            DelayState::HighDelay
        } else if delay > DelayState::LowDelay as u32 {
            DelayState::LowDelay
        } else {
            DelayState::Normal
        }
    }
}

impl Default for VideoQoS {
    fn default() -> Self {
        VideoQoS {
            fps: FPS as f32,
            quality: Default::default(),
            users: Default::default(),
            bitrate_store: 0,
            support_abr: Default::default(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RefreshType {
    SetImageQuality,
}

impl VideoQoS {
    pub fn spf(&self) -> Duration {
        Duration::from_secs_f32(1. / self.fps())
    }

    pub fn fps(&self) -> f32 {
        assert!(self.fps > 0.0);
        self.fps
    }

    pub fn store_bitrate(&mut self, bitrate: u32) {
        self.bitrate_store = bitrate;
    }

    pub fn bitrate(&self) -> u32 {
        self.bitrate_store
    }

    pub fn quality(&self) -> Quality {
        self.quality
    }

    pub fn record(&self) -> bool {
        self.users.iter().any(|u| u.1.record)
    }

    pub fn set_support_abr(&mut self, display_idx: usize, support: bool) {
        self.support_abr.insert(display_idx, support);
    }

    pub fn in_vbr_state(&self) -> bool {
        Config::get_option("enable-abr") != "N" && self.support_abr.iter().all(|e| *e.1)
    }

    pub fn fps_from_user_honor_server<const B: bool>(u: &UserData) -> f32 {
        let mut new_fps = 0.0;
        let mut b = B;

        if b {
            if let Some(delay) = u.delay {
                new_fps = 2000.0 / delay.delay as f32;
            } else {
                b = false;
            }
        }

        if !b {
            let mut client_fps = u.custom_fps.unwrap_or(0);
            if let Some(auto_adjust_fps) = u.auto_adjust_fps {
                if client_fps == 0 || client_fps > auto_adjust_fps {
                    client_fps = auto_adjust_fps;
                }
                new_fps = client_fps as f32;
            }
        }
        return new_fps;
    }

    pub fn fps_from_user(&self) -> f32 {
        self.users
            .iter()
            .map(|(_, u)| Self::fps_from_user_honor_server::<false>(u))
            .reduce(f32::min)
            .unwrap_or(FPS as f32)
            .max(MIN_FPS as f32)
            .min(MAX_FPS as f32)
    }

    pub fn refresh(&mut self, typ: Option<RefreshType>) {
        let fps = self
            .users
            .iter()
            .map(|(_, u)| Self::fps_from_user_honor_server::<true>(u))
            .reduce(f32::min)
            .unwrap_or(FPS as f32)
            .min(FPS as f32);

        if self.fps != fps {
            if self.fps > fps + 1.0 || fps > self.fps + 1.0 {
                println!("fps {} -----------> {}", self.fps, fps);
            }
            self.fps = fps;
        }

        // quality
        // latest image quality
        let latest_quality = self
            .users
            .iter()
            .map(|(_, u)| u.quality)
            .filter(|q| *q != None)
            .max_by(|a, b| a.unwrap_or_default().0.cmp(&b.unwrap_or_default().0))
            .unwrap_or_default()
            .unwrap_or_default()
            .1;
        let mut quality = latest_quality;

        // network delay
        let abr_enabled = self.in_vbr_state();
        if abr_enabled && typ != Some(RefreshType::SetImageQuality) {
            // max delay
            let delay = self
                .users
                .iter()
                .map(|u| u.1.delay)
                .filter(|d| d.is_some())
                .max_by(|a, b| {
                    (a.unwrap_or_default().state as u32).cmp(&(b.unwrap_or_default().state as u32))
                });
            let delay = delay.unwrap_or_default().unwrap_or_default().state;
            if delay != DelayState::Normal {
                match self.quality {
                    Quality::Best => {
                        quality = if delay == DelayState::Broken {
                            Quality::Low
                        } else {
                            Quality::Balanced
                        };
                    }
                    Quality::Balanced => {
                        quality = Quality::Low;
                    }
                    Quality::Low => {
                        quality = Quality::Low;
                    }
                    Quality::Custom(b) => match delay {
                        DelayState::LowDelay => {
                            quality =
                                Quality::Custom(if b >= 150 { 100 } else { std::cmp::min(50, b) });
                        }
                        DelayState::HighDelay => {
                            quality =
                                Quality::Custom(if b >= 100 { 50 } else { std::cmp::min(25, b) });
                        }
                        DelayState::Broken => {
                            quality =
                                Quality::Custom(if b >= 50 { 25 } else { std::cmp::min(10, b) });
                        }
                        DelayState::Normal => {}
                    },
                }
            } else {
                match self.quality {
                    Quality::Low => {
                        if latest_quality == Quality::Best {
                            quality = Quality::Balanced;
                        }
                    }
                    Quality::Custom(current_b) => {
                        if let Quality::Custom(latest_b) = latest_quality {
                            if current_b < latest_b / 2 {
                                quality = Quality::Custom(latest_b / 2);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        self.quality = quality;
    }

    pub fn user_custom_fps(&mut self, id: i32, fps: u32) {
        if fps < MIN_FPS {
            return;
        }
        if let Some(user) = self.users.get_mut(&id) {
            user.custom_fps = Some(fps);
        } else {
            self.users.insert(
                id,
                UserData {
                    custom_fps: Some(fps),
                    ..Default::default()
                },
            );
        }
        self.refresh(None);
    }

    pub fn user_auto_adjust_fps(&mut self, id: i32, fps: u32) {
        if let Some(user) = self.users.get_mut(&id) {
            user.auto_adjust_fps = Some(fps);
        } else {
            self.users.insert(
                id,
                UserData {
                    auto_adjust_fps: Some(fps),
                    ..Default::default()
                },
            );
        }
        self.refresh(None);
    }

    pub fn user_image_quality(&mut self, id: i32, image_quality: i32) {
        // https://github.com/rustdesk/rustdesk/blob/d716e2b40c38737f1aa3f16de0dec67394a6ac68/src/server/video_service.rs#L493
        let convert_quality = |q: i32| {
            if q == ImageQuality::Balanced.value() {
                Quality::Balanced
            } else if q == ImageQuality::Low.value() {
                Quality::Low
            } else if q == ImageQuality::Best.value() {
                Quality::Best
            } else {
                let mut b = (q >> 8 & 0xFFF) * 2;
                b = std::cmp::max(b, 20);
                b = std::cmp::min(b, 8000);
                Quality::Custom(b as u32)
            }
        };

        let quality = Some((hbb_common::get_time(), convert_quality(image_quality)));
        if let Some(user) = self.users.get_mut(&id) {
            user.quality = quality;
        } else {
            self.users.insert(
                id,
                UserData {
                    quality,
                    ..Default::default()
                },
            );
        }
        self.refresh(Some(RefreshType::SetImageQuality));
    }

    pub fn user_network_delay(&mut self, id: i32, delay: u32) {
        let delay = delay.max(1);
        let state = DelayState::from_delay(delay);

        fn delay_with(delay: u32, state: DelayState) -> Delay {
            Delay {
                state,
                staging_state: state,
                delay,
                counter: 0,
                slower_than_old_state: None,
            }
        }

        if let Some(user) = self.users.get_mut(&id) {
            if let Some(d) = &mut user.delay {
                d.delay = (d.delay + delay) / 2;
            } else {
                user.delay = Some(delay_with(delay, state));
            }
        } else {
            self.users.insert(
                id,
                UserData {
                    delay: Some(delay_with(delay, state)),
                    ..Default::default()
                },
            );
        }
        self.refresh(None);
    }

    pub fn user_record(&mut self, id: i32, v: bool) {
        if let Some(user) = self.users.get_mut(&id) {
            user.record = v;
        }
    }

    pub fn on_connection_close(&mut self, id: i32) {
        self.users.remove(&id);
        self.refresh(None);
    }
}
