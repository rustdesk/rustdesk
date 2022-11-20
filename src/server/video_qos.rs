use super::*;
use std::time::Duration;
pub const FPS: u8 = 30;
pub const MIN_FPS: u8 = 10;
pub const MAX_FPS: u8 = 120;
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

pub struct VideoQoS {
    width: u32,
    height: u32,
    user_image_quality: u32,
    current_image_quality: u32,
    enable_abr: bool,
    pub current_delay: u32,
    pub fps: u8, // abr
    pub user_fps: u8,
    pub target_bitrate: u32, // abr
    updated: bool,
    state: DelayState,
    debounce_count: u32,
}

#[derive(PartialEq, Debug)]
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
            user_fps: FPS,
            user_image_quality: ImageQuality::Balanced.as_percent(),
            current_image_quality: ImageQuality::Balanced.as_percent(),
            enable_abr: false,
            width: 0,
            height: 0,
            current_delay: 0,
            target_bitrate: 0,
            updated: false,
            state: DelayState::Normal,
            debounce_count: 0,
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

    pub fn spf(&mut self) -> Duration {
        if self.fps < MIN_FPS || self.fps > MAX_FPS {
            self.fps = self.base_fps();
        }
        Duration::from_secs_f32(1. / (self.fps as f32))
    }

    fn base_fps(&self) -> u8 {
        if self.user_fps >= MIN_FPS && self.user_fps <= MAX_FPS {
            return self.user_fps;
        }
        return FPS;
    }

    // update_network_delay periodically
    // decrease the bitrate when the delay gets bigger
    pub fn update_network_delay(&mut self, delay: u32) {
        if self.current_delay.eq(&0) {
            self.current_delay = delay;
            return;
        }

        self.current_delay = delay / 2 + self.current_delay / 2;
        log::trace!(
            "VideoQoS update_network_delay:{}, {}, state:{:?}",
            self.current_delay,
            delay,
            self.state,
        );

        // ABR
        if !self.enable_abr {
            return;
        }
        let current_state = DelayState::from_delay(self.current_delay);
        if current_state != self.state && self.debounce_count > 5 {
            log::debug!(
                "VideoQoS state changed:{:?} -> {:?}",
                self.state,
                current_state
            );
            self.state = current_state;
            self.debounce_count = 0;
            self.refresh_quality();
        } else {
            self.debounce_count += 1;
        }
    }

    fn refresh_quality(&mut self) {
        match self.state {
            DelayState::Normal => {
                self.fps = self.base_fps();
                self.current_image_quality = self.user_image_quality;
            }
            DelayState::LowDelay => {
                self.fps = self.base_fps();
                self.current_image_quality = std::cmp::min(self.user_image_quality, 50);
            }
            DelayState::HighDelay => {
                self.fps = self.base_fps() / 2;
                self.current_image_quality = std::cmp::min(self.user_image_quality, 25);
            }
            DelayState::Broken => {
                self.fps = self.base_fps() / 4;
                self.current_image_quality = 10;
            }
        }
        let _ = self.generate_bitrate().ok();
        self.updated = true;
    }

    // handle image_quality change from peer
    pub fn update_image_quality(&mut self, image_quality: i32) {
        if image_quality == ImageQuality::Low.value()
            || image_quality == ImageQuality::Balanced.value()
            || image_quality == ImageQuality::Best.value()
        {
            // not custom
            self.user_fps = FPS;
            self.fps = FPS;
        }
        let image_quality = Self::convert_quality(image_quality) as _;
        if self.current_image_quality != image_quality {
            self.current_image_quality = image_quality;
            let _ = self.generate_bitrate().ok();
            self.updated = true;
        }

        self.user_image_quality = self.current_image_quality;
    }

    pub fn update_user_fps(&mut self, fps: u8) {
        if fps >= MIN_FPS && fps <= MAX_FPS {
            if self.user_fps != fps {
                self.user_fps = fps;
                self.fps = fps;
                self.updated = true;
            }
        }
    }

    pub fn generate_bitrate(&mut self) -> ResultType<u32> {
        // https://www.nvidia.com/en-us/geforce/guides/broadcasting-guide/
        if self.width == 0 || self.height == 0 {
            bail!("Fail to generate_bitrate, width or height is not set");
        }
        if self.current_image_quality == 0 {
            self.current_image_quality = ImageQuality::Balanced.as_percent();
        }

        let base_bitrate = ((self.width * self.height) / 800) as u32;

        #[cfg(target_os = "android")]
        {
            // fix when andorid screen shrinks
            let fix = scrap::Display::fix_quality() as u32;
            log::debug!("Android screen, fix quality:{}", fix);
            let base_bitrate = base_bitrate * fix;
            self.target_bitrate = base_bitrate * self.current_image_quality / 100;
            Ok(self.target_bitrate)
        }
        #[cfg(not(target_os = "android"))]
        {
            self.target_bitrate = base_bitrate * self.current_image_quality / 100;
            Ok(self.target_bitrate)
        }
    }

    pub fn check_if_updated(&mut self) -> bool {
        if self.updated {
            self.updated = false;
            return true;
        }
        return false;
    }

    pub fn reset(&mut self) {
        *self = Default::default();
    }

    pub fn check_abr_config(&mut self) -> bool {
        self.enable_abr = "N" != Config::get_option("enable-abr");
        self.enable_abr
    }

    pub fn convert_quality(q: i32) -> i32 {
        if q == ImageQuality::Balanced.value() {
            100 * 2 / 3
        } else if q == ImageQuality::Low.value() {
            100 / 2
        } else if q == ImageQuality::Best.value() {
            100
        } else {
            (q >> 8 & 0xFF) * 2
        }
    }
}
