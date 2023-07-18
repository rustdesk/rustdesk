use super::*;
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

#[derive(Default, Debug)]
struct UserData {
    full_speed_fps: Option<u32>,
    custom_fps: Option<u32>,
    quality: Option<(i32, i64)>,             // (quality, time)
    delay: Option<(DelayState, u32, usize)>, // (state, ms, counter)
}

pub struct VideoQoS {
    width: u32,
    height: u32,
    fps: u32,
    target_bitrate: u32,
    updated: bool,
    users: HashMap<i32, UserData>,
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum DelayState {
    Normal = 0,
    LowDelay = 200,
    HighDelay = 500,
    Broken = 1000,
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
            width: 0,
            height: 0,
            target_bitrate: 0,
            updated: false,
            users: Default::default(),
        }
    }
}

impl VideoQoS {
    pub fn set_size(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
    }

    pub fn spf(&self) -> Duration {
        Duration::from_secs_f32(1. / (self.fps as f32))
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn bitrate(&self) -> u32 {
        self.target_bitrate
    }

    pub fn check_if_updated(&mut self) -> bool {
        if self.updated {
            self.updated = false;
            return true;
        }
        return false;
    }

    pub fn abr_enabled() -> bool {
        "N" != Config::get_option("enable-abr")
    }

    pub fn refresh(&mut self) {
        let mut updated = false;
        // fps
        let user_fps = |u: &UserData| {
            // full_speed_fps
            let mut fps = u.full_speed_fps.unwrap_or_default() * 9 / 10;
            // custom_fps
            if let Some(custom_fps) = u.custom_fps {
                if fps == 0 || custom_fps < fps {
                    fps = custom_fps;
                }
            }
            // delay
            if let Some(delay) = u.delay {
                fps = match delay.0 {
                    DelayState::Normal => fps,
                    DelayState::LowDelay => fps,
                    DelayState::HighDelay => fps / 2,
                    DelayState::Broken => fps / 4,
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
        if fps != self.fps {
            self.fps = fps;
            updated = true;
        }

        // quality
        // latest image quality
        let latest = self
            .users
            .iter()
            // .map(|(_, u)| u.quality)
            .filter(|u| u.1.quality != None)
            .max_by(|u1, u2| {
                u1.1.quality
                    .unwrap_or_default()
                    .1
                    .cmp(&u2.1.quality.unwrap_or_default().1)
            });
        let quality = if let Some((id, data)) = latest {
            let mut quality = data.quality.unwrap_or_default().0;
            if quality <= 0 {
                quality = ImageQuality::Balanced.as_percent() as _;
            }
            // use latest's delay for quality
            if Self::abr_enabled() {
                if let Some(Some((delay, _, _))) = self.users.get(id).map(|u| u.delay) {
                    quality = match delay {
                        DelayState::Normal => quality,
                        DelayState::LowDelay => std::cmp::min(quality, 50),
                        DelayState::HighDelay => std::cmp::min(quality, 25),
                        DelayState::Broken => 10,
                    };
                }
            }
            quality
        } else {
            ImageQuality::Balanced.as_percent() as _
        };
        // bitrate
        #[allow(unused_mut)]
        let mut base_bitrate = ((self.width * self.height) / 800) as u32;
        if base_bitrate == 0 {
            base_bitrate = 1920 * 1080 / 800;
        }
        #[cfg(target_os = "android")]
        {
            // fix when android screen shrinks
            let fix = scrap::Display::fix_quality() as u32;
            log::debug!("Android screen, fix quality:{}", fix);
            base_bitrate = base_bitrate * fix;
        }
        let target_bitrate = base_bitrate * quality as u32 / 100;
        if self.target_bitrate != target_bitrate {
            self.target_bitrate = target_bitrate;
            updated = true;
        }

        self.updated = updated;
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
        self.refresh();
    }

    pub fn user_full_speed_fps(&mut self, id: i32, full_speed_fps: u32) {
        if let Some(user) = self.users.get_mut(&id) {
            user.full_speed_fps = Some(full_speed_fps);
        } else {
            self.users.insert(
                id,
                UserData {
                    full_speed_fps: Some(full_speed_fps),
                    ..Default::default()
                },
            );
        }
        self.refresh();
    }

    pub fn user_image_quality(&mut self, id: i32, image_quality: i32) {
        let convert_quality = |q: i32| -> i32 {
            if q == ImageQuality::Balanced.value() {
                100 * 2 / 3
            } else if q == ImageQuality::Low.value() {
                100 / 2
            } else if q == ImageQuality::Best.value() {
                100
            } else {
                (q >> 8 & 0xFF) * 2
            }
        };

        let quality = Some((convert_quality(image_quality), hbb_common::get_time()));
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
        self.refresh();
    }

    pub fn user_network_delay(&mut self, id: i32, delay: u32) {
        let mut refresh = true;
        let state = DelayState::from_delay(delay);
        if let Some(user) = self.users.get_mut(&id) {
            if let Some((old_state, old_delay, mut counter)) = user.delay {
                let new_delay = (delay + old_delay) / 2;
                let new_state = DelayState::from_delay(new_delay);
                if old_state == new_state {
                    counter += 1;
                } else {
                    counter = 0;
                }
                let debounce = 3;
                refresh = counter == debounce;
                user.delay = Some((new_state, new_delay, counter));
            } else {
                user.delay = Some((state, delay, 0));
            }
        } else {
            self.users.insert(
                id,
                UserData {
                    delay: Some((state, delay, 0)),
                    ..Default::default()
                },
            );
        }
        if refresh {
            self.refresh();
        }
    }

    pub fn on_connection_close(&mut self, id: i32) {
        self.users.remove(&id);
        self.refresh();
    }
}
