use super::*;
use scrap::codec::Quality;
use std::time::{Duration, Instant};
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

const MIN: f32 = 0.5;
const MAX: f32 = 1.5;

#[derive(Default, Debug, Copy, Clone)]
struct UserData {
    auto_adjust_fps: Option<u32>, // reserve for compatibility
    custom_fps: Option<u32>,
    quality: Option<(i64, Quality)>, // (time, quality)
    delay: Option<u32>,
    record: bool,
}

pub struct VideoQoS {
    adjust_bps: bool,
    adjust_fps: bool,

    fps: f32,
    ratio: f32,
    quality: Quality,
    users: HashMap<i32, UserData>,
    bitrate_store: u32,

    timer: Instant,
    sched: u64,
}

impl Default for VideoQoS {
    fn default() -> Self {
        let mut qos = VideoQoS {
            adjust_bps: true,
            adjust_fps: true,
            fps: FPS as f32,
            ratio: VideoQoS::default_ratio(&Quality::Balanced),
            quality: Default::default(),
            users: Default::default(),
            bitrate_store: 0,

            timer: Instant::now(),
            sched: 0,
        };

        assert!(!matches!(qos.quality, Quality::Custom(_)));
        if qos.quality == Quality::Best {
            qos.adjust_fps = false;
        } else if qos.quality == Quality::Low {
            qos.adjust_bps = false;
        }
        qos
    }
}

impl VideoQoS {
    pub fn spf(&self) -> Duration {
        Duration::from_secs_f32(1. / self.fps())
    }

    pub fn fps(&self) -> f32 {
        assert!(self.fps > 0.0);
        self.fps
    }

    pub fn quality(&self) -> Quality {
        // self.ratio
        // HACK
        Quality::Balanced
    }

    pub fn record(&self) -> bool {
        self.users.iter().any(|u| u.1.record)
    }

    pub fn fps_from_user_honor_server<const B: bool>(u: &UserData) -> f32 {
        let mut net_fps = FPS as f32;
        if B {
            if let Some(delay) = u.delay {
                net_fps = f32::min(2000.0 / delay as f32, net_fps);
            }
        }

        let mut user_fps = u.custom_fps.unwrap_or(0);
        if let Some(auto_adjust_fps) = u.auto_adjust_fps {
            if user_fps == 0 || user_fps > auto_adjust_fps {
                user_fps = auto_adjust_fps;
            }
        }

        // HACK
        user_fps = 25;

        if user_fps == 0 {
            user_fps = FPS;
        }

        if B {
            f32::min(net_fps, user_fps as f32)
        } else {
            user_fps as f32
        }
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

    const BALANCE: f32 = 0.67;
    pub fn default_ratio(q: &Quality) -> f32 {
        match q {
            Quality::Best => MAX,
            Quality::Balanced => Self::BALANCE,
            Quality::Low => MIN,
            Quality::Custom(v) => (v + 100) as f32 / 200.0,
        }
    }

    pub fn bitrate_ratio_from_quality(&mut self, q: &Quality) -> f32 {
        if self.quality == *q {
            return self.ratio;
        }

        match q {
            Quality::Best => MAX,
            Quality::Balanced => {
                if self.ratio == MIN || self.ratio == MAX {
                    Self::BALANCE
                } else {
                    self.ratio
                }
            }
            Quality::Low => MIN,
            Quality::Custom(v) => (v + 100) as f32 / 200.0,
        }
    }

    fn allow_adjust_from_quality(&mut self, q: &Quality) {
        match q {
            Quality::Best => {
                self.adjust_bps = false;
                self.adjust_fps = true;
            }

            Quality::Balanced => {
                self.adjust_bps = true;
                self.adjust_fps = true;
            }

            Quality::Low => {
                self.adjust_bps = true;
                self.adjust_fps = false;
            }

            Quality::Custom(v) => {
                self.adjust_bps = false;
                self.adjust_fps = false;
            }
        }
    }

    pub fn refresh(&mut self) {
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

        let q = self
            .users
            .iter()
            .map(|(_, u)| u.quality)
            .filter(|q| *q != None)
            .max_by(|a, b| a.unwrap_or_default().0.cmp(&b.unwrap_or_default().0))
            .unwrap_or_default()
            .unwrap_or_default()
            .1;

        self.allow_adjust_from_quality(&q);
        self.ratio = self.bitrate_ratio_from_quality(&q);
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
        self.refresh();
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
        self.refresh();
    }

    pub fn user_network_delay(&mut self, id: i32, delay: u32) {
        println!("g::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::::get delay {delay}");
        let delay = delay.max(1);

        if let Some(user) = self.users.get_mut(&id) {
            user.delay = if let Some(d) = user.delay {
                Some((d + delay) / 2)
            } else {
                Some(delay)
            }
        } else {
            self.users.insert(
                id,
                UserData {
                    delay: Some(delay),
                    ..Default::default()
                },
            );
        }
        self.refresh();
    }

    pub fn user_record(&mut self, id: i32, v: bool) {
        if let Some(user) = self.users.get_mut(&id) {
            user.record = v;
        }
    }

    pub fn on_connection_close(&mut self, id: i32) {
        self.users.remove(&id);
        self.refresh();
    }

    pub fn prefered_quality(&mut self) -> Option<Quality> {
        let target_fps = self.fps_from_user();
        let current_fps = self.fps();
        let target = self.timer.elapsed().as_secs() + 12;

        if self.sched > target {
            self.sched = target;

            if current_fps < target_fps {
                if self.ratio > MIN {
                    self.ratio -= (self.ratio - MIN) / 12.0;
                    // HACK
                    return Some(Quality::Custom(self.ratio as u32));
                }
            } else if current_fps > target_fps {
                if self.ratio < MAX {
                    self.ratio += (MAX - self.ratio) / 12.0;
                    // HACK
                    return Some(Quality::Custom(self.ratio as u32));
                }
            }
        } else if self.sched == 0 {
            self.sched = target;
        }
        return None;
    }
}
