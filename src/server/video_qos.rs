use super::*;
use nix::libc::SYS_userfaultfd;
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
const DELAY_LEVEL: (u32, u32, u32) = (280, 600, 4000);

#[derive(Default, Debug)]
struct UserData {
    custom_fps: Option<u32>,
    quality: Option<(i64, Quality)>, // (time, quality)
    delay: Option<u32>,
    long: SampleSmoother<24>,
    short: SampleSmoother<8>,
    record: bool,
}

#[derive(Debug)]
struct SampleSmoother<const N: usize>([u32; N], usize);
impl<const N: usize> Default for SampleSmoother<N> {
    fn default() -> Self {
        Self([0; N], 0)
    }
}

impl<const N: usize> SampleSmoother<N> {
    fn add_sample(&mut self, n: u32) {
        let idx = self.1 % N;
        self.1 += 1;
        self.0[idx] = n;
    }

    fn average(&self) -> f32 {
        let nr = if self.1 >= N { N } else { self.1 };
        if nr == 0 {
            0.0
        } else {
            let n = self.0[0..N].iter().sum::<u32>() as f32;
            n / nr as f32
        }
    }
}

impl UserData {
    fn fps_from_delay(&self, fps: f32, delay: u32) -> f32 {
        let avg_short = self.short.average();
        let avg_long = self.long.average();
        let minfps = 3000.0 / (2.0 * delay as f32);

        let fps = if delay >= DELAY_LEVEL.2 || fps < minfps {
            log::info!(
                "3 ----------------------------------delay {delay}, fps: {fps}, long {avg_long}, short {avg_short}"
            );
            minfps
        } else if delay > DELAY_LEVEL.1 {
            log::info!(
                "2 ----------------------------------delay {delay}, fps: {fps}, long {avg_long}, short {avg_short}"
            );
            minfps + (fps - minfps) * (DELAY_LEVEL.2 - delay) as f32 / DELAY_LEVEL.2 as f32
        } else if delay > DELAY_LEVEL.0 {
            log::info!(
                "1 ----------------------------------delay {delay}, fps: {fps}, long {avg_long}, short {avg_short}"
            );
            fps
        } else {
            log::info!(
                "0 ----------------------------------delay {delay}, fps: {fps}, long {avg_long}, short {avg_short}"
            );
            fps + minfps
        };
        fps.min(MAX_FPS as f32).max(MIN_FPS as f32)
    }

    fn add_sample(&mut self, n: u32) {
        self.short.add_sample(n);
        self.long.add_sample(n);
    }
}

pub struct VideoQoS {
    fps: f32,
    netfps: f32,
    ratio: f32,
    quality: Quality,
    users: HashMap<i32, UserData>,

    timer: Instant,
    sched: u64,
}

impl Default for VideoQoS {
    fn default() -> Self {
        let quality: Quality = Default::default();
        assert!(!matches!(quality, Quality::Custom(_)));

        let mut qos = VideoQoS {
            fps: 0.0,
            netfps: 0.0,

            ratio: VideoQoS::default_ratio(&quality),
            quality: quality,
            users: Default::default(),
            timer: Instant::now(),
            sched: 0,
        };

        let (fps, _) = qos.fps_range_from_quality();
        qos.fps = fps;
        qos.netfps = fps;
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

    fn target_fps(&self, u: &UserData) -> (f32, f32) {
        let mut user_fps = u.custom_fps.unwrap_or_else(|| {
            let (fps, _) = self.fps_range_from_quality();
            fps as u32
        });

        // assure will be user_fps if no delay information
        // why is not self.fps: self.fps is calcated in same way
        // as user_fps and user_fps is newer
        let mut net_fps = user_fps as f32;
        if let Some(delay) = u.delay {
            net_fps = u.fps_from_delay(self.fps, delay);
        }
        (net_fps, f32::min(net_fps, user_fps as f32))
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
    fn default_ratio(q: &Quality) -> f32 {
        match q {
            Quality::Best => MAX,
            Quality::Balanced => Self::BALANCE,
            Quality::Low => MIN,
            Quality::Custom(v) => *v as f32 / 100.0,
        }
    }

    fn bitrate_ratio_from_quality(&self, q: &Quality) -> f32 {
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

    fn allow_change_bps(&self, ratio: f32, inc: bool) -> Option<f32> {
        let bound = match self.quality {
            Quality::Best => {
                if inc {
                    MAX * 1.5
                } else {
                    MAX
                }
            }
            Quality::Balanced => {
                if inc {
                    MAX
                } else {
                    MIN
                }
            }
            Quality::Low => {
                if inc {
                    Self::BALANCE
                } else {
                    MIN
                }
            }
            Quality::Custom(_) => return None,
        };

        if inc && bound > ratio || !inc && bound < ratio {
            Some(bound)
        } else {
            None
        }
    }

    pub fn refresh(&mut self) {
        // will use self.quality in target_fps
        // so updates first
        let q = self
            .users
            .iter()
            .map(|(_, u)| u.quality)
            .filter(|q| *q != None)
            .max_by(|a, b| a.unwrap_or_default().0.cmp(&b.unwrap_or_default().0))
            .unwrap_or_default()
            .unwrap_or_default()
            .1;
        self.ratio = self.bitrate_ratio_from_quality(&q);
        self.quality = q;

        let (mut a, mut b) = (MIN_FPS as f32, MIN_FPS as f32);
        self.users
            .iter()
            .enumerate()
            .map(|(i, (_, u))| {
                let (x, y) = Self::target_fps(self, &u);
                if i == 0 {
                    (a, b) = (x, y);
                } else {
                    a = a.min(x);
                    b = b.min(y);
                }
            })
            .for_each(|_| {});

        if self.fps != b {
            println!("fps {} -----------> {}", self.fps, b);
            self.fps = b;
        }
        self.netfps = a;
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
                    user.add_sample(delay);
                    Some((d + delay) / 2)
                } else {
                    if delay > DELAY_LEVEL.1 {
                        self.refresh();
                    }
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
        let maxfps = self.netfps;
        let now = self.timer.elapsed().as_secs();
        if now >= self.sched {
            self.sched = now + 4;

            if maxfps < fps1 {
                if let Some(r) = self.allow_change_bps(self.ratio, false) {
                    self.ratio -= (self.ratio - r) / 6.0;
                    return Ok(self.ratio);
                }
            } else if maxfps > fps2 {
                if let Some(r) = self.allow_change_bps(self.ratio, true) {
                    self.ratio += (r - self.ratio) / 6.0;
                    return Ok(self.ratio);
                }
            }
        }
        return Err(self.ratio);
    }
}
