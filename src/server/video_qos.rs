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
const MAX: f32 = 1.0;

#[derive(Default, Debug, Copy, Clone)]
struct UserData {
    custom_fps: Option<u32>,
    quality: Option<(i64, Quality)>, // (time, quality)
    delay: Option<u32>,
    record: bool,
}

impl UserData {
    fn fps_from_delay(&self, fps: f32, delay: u32) -> f32 {
        let minfps = 3000.0 / (2.0 * delay as f32);
        println!("minfps {minfps}, delay: {delay}, fps: {fps}");

        let fps = if delay > 800 {
            (fps - minfps).max(minfps)
        } else if delay > 600 {
            fps
        } else {
            fps + minfps
        };
        fps.min(MAX_FPS as f32).max(MIN_FPS as f32)
    }
}

pub struct VideoQoS {
    adjust_bps: bool,
    adjust_fps: bool,

    fps: f32,
    ratio: f32,
    quality: Quality,
    users: HashMap<i32, UserData>,

    timer: Instant,
    sched: u64,
}

impl Default for VideoQoS {
    fn default() -> Self {
        let mut qos = VideoQoS {
            adjust_bps: true,
            adjust_fps: true,
            fps: FPS as f32 / 2.0,
            ratio: VideoQoS::default_ratio(&Quality::Balanced),
            quality: Default::default(),
            users: Default::default(),

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

    pub fn record(&self) -> bool {
        self.users.iter().any(|u| u.1.record)
    }

    pub fn target_fps(fps: f32, u: &UserData) -> f32 {
        let mut net_fps = fps;
        if let Some(delay) = u.delay {
            net_fps = u.fps_from_delay(net_fps, delay);
        }

        let mut user_fps = u.custom_fps.unwrap_or(0);
        if user_fps > 0 {
            f32::min(net_fps, user_fps as f32)
        } else {
            net_fps
        }
    }

    fn fps_range_from_quality(&self) -> (f32, f32) {
        let best_fps = FPS as f32;
        let worst_fps = 1.0;
        match self.quality {
            Quality::Best => (2.0 * best_fps / 3.0, best_fps),
            Quality::Balanced => (1.0 * best_fps / 3.0, 2.0 * best_fps / 3.0),
            Quality::Low => (worst_fps, 1.0 * best_fps / 3.0),
            ref custom => {
                let fps = self.bitrate_ratio_from_quality(custom) * best_fps;
                let fps = fps.max(worst_fps).min(best_fps);
                (fps, fps)
            }
        }
    }

    fn fps_from_user(&self) -> (f32, f32) {
        let fps = self
            .users
            .iter()
            .map(|(_, u)| u.custom_fps.unwrap_or(0))
            .min()
            .unwrap_or(0);
        if fps > 0 {
            let fps = fps as f32;
            return (fps, fps);
        }
        self.fps_range_from_quality()
    }

    const BALANCE: f32 = 0.67;
    pub fn default_ratio(q: &Quality) -> f32 {
        match q {
            Quality::Best => MAX,
            Quality::Balanced => Self::BALANCE,
            Quality::Low => MIN,
            Quality::Custom(v) => *v as f32 / 100.0,
        }
    }

    pub fn bitrate_ratio_from_quality(&self, q: &Quality) -> f32 {
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
            Quality::Custom(v) => *v as f32 / 100.0,
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
            .map(|(_, u)| Self::target_fps(self.fps, &u))
            .reduce(f32::min)
            .unwrap_or(MIN_FPS as f32);
        if self.fps != fps {
            // if self.fps > fps + 1.0 || fps > self.fps + 1.0 {
            println!("fps {} -----------> {}", self.fps, fps);
            // }
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
        self.quality = q;
    }

    pub fn user_custom_fps(&mut self, id: i32, fps: u32) {
        let fps = if fps < MIN_FPS {
            MIN_FPS
        } else if fps > MAX_FPS {
            MAX_FPS
        } else {
            fps
        };

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
        self.sched_adjust();
    }

    pub fn user_image_quality(&mut self, id: i32, image_quality: i32) {
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
        self.sched_adjust();
    }

    pub fn user_network_delay(&mut self, id: i32, delay: u32, complete: bool) {
        log::info!("g:::::::::::::::::::::::::::::::::::::::::::::::::::: complete {complete} get delay {delay}");
        let delay = delay.max(1);

        if let Some(user) = self.users.get_mut(&id) {
            user.delay = if let Some(d) = user.delay {
                if complete || delay > d {
                    Some((d + delay) / 2)
                } else {
                    return;
                }
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

    fn sched_adjust(&mut self) {
        self.sched = 1;
    }

    pub fn calc_prefered_quality(&mut self) -> Result<f32, f32> {
        if self.sched == 0 {
            self.refresh();
        }

        let (fps1, fps2) = self.fps_from_user();
        let current_fps = self.fps();
        let now = self.timer.elapsed().as_secs();

        if now >= self.sched && self.adjust_bps {
            self.sched = now + 12;

            if current_fps < fps1 && self.ratio > MIN {
                self.ratio -= (self.ratio - MIN) / 12.0;
                return Ok(self.ratio);
            } else if current_fps > fps2 && self.ratio < MAX {
                self.ratio += (MAX - self.ratio) / 12.0;
                return Ok(self.ratio);
            }
        }
        return Err(self.ratio);
    }
}
