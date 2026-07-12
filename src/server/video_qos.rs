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
    response_delayed: bool,
    record: bool,
}

pub struct VideoQoS {
    fps: u32,
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
    Broken = 1000,
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
            fps: FPS,
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
        Duration::from_secs_f32(1. / (self.fps() as f32))
    }

    pub fn fps(&self) -> u32 {
        if self.fps >= MIN_FPS && self.fps <= MAX_FPS {
            self.fps
        } else {
            FPS
        }
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

    pub fn refresh(&mut self, typ: Option<RefreshType>) {
        // fps
        let user_fps = |u: &UserData| {
            // custom_fps
            let mut fps = u.custom_fps.unwrap_or(FPS);
            // auto adjust fps
            if let Some(auto_adjust_fps) = u.auto_adjust_fps {
                if fps == 0 || auto_adjust_fps < fps {
                    fps = auto_adjust_fps;
                }
            }
            // delay
            if let Some(delay) = u.delay {
                fps = match delay.state {
                    DelayState::Normal => fps,
                    DelayState::LowDelay => fps * 3 / 4,
                    DelayState::HighDelay => fps / 2,
                    DelayState::Broken => fps / 4,
                }
            }
            // delay response
            if u.response_delayed {
                if fps > MIN_FPS + 2 {
                    fps = MIN_FPS + 2;
                }
            }
            return fps;
        };
        let mut fps = self
            .users
            .iter()
            .map(|(_, u)| user_fps(u))
            .filter(|u| *u >= MIN_FPS)
            .min()
            .unwrap_or(FPS);
        if fps > MAX_FPS {
            fps = MAX_FPS;
        }
        self.fps = fps;

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
        let state = DelayState::from_delay(delay);
        let debounce = 3;
        if let Some(user) = self.users.get_mut(&id) {
            if let Some(d) = &mut user.delay {
                d.delay = (delay + d.delay) / 2;
                let new_state = DelayState::from_delay(d.delay);
                let slower_than_old_state = new_state as i32 - d.staging_state as i32;
                let slower_than_old_state = if slower_than_old_state > 0 {
                    Some(true)
                } else if slower_than_old_state < 0 {
                    Some(false)
                } else {
                    None
                };
                if d.slower_than_old_state == slower_than_old_state {
                    let old_counter = d.counter;
                    d.counter += delay / 1000 + 1;
                    if old_counter < debounce && d.counter >= debounce {
                        d.counter = 0;
                        d.state = d.staging_state;
                        d.staging_state = new_state;
                    }
                    if d.counter % debounce == 0 {
                        self.refresh(None);
                    }
                } else {
                    d.counter = 0;
                    d.staging_state = new_state;
                    d.slower_than_old_state = slower_than_old_state;
                }
            } else {
                user.delay = Some(Delay {
                    state: DelayState::Normal,
                    staging_state: state,
                    delay,
                    counter: 0,
                    slower_than_old_state: None,
                });
            }
        } else {
            self.users.insert(
                id,
                UserData {
                    delay: Some(Delay {
                        state: DelayState::Normal,
                        staging_state: state,
                        delay,
                        counter: 0,
                        slower_than_old_state: None,
                    }),
                    ..Default::default()
                },
            );
        }
    }

    pub fn user_delay_response_elapsed(&mut self, id: i32, elapsed: u128) {
        if let Some(user) = self.users.get_mut(&id) {
            let old = user.response_delayed;
            user.response_delayed = elapsed > 3000;
            if old != user.response_delayed {
                self.refresh(None);
            }
        }
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
