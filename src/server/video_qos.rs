use super::*;
use scrap::codec::{Quality, BR_BALANCED, BR_BEST, BR_SPEED};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/*
FPS adjust:
a. new user connected => set to INIT_FPS
b. TestDelay reply => update the user's fps from the excess delay, the reply's delay
   above the baseline this connection has shown so far:
     startup: two consecutive replies with excess < 50 ms permit doubling toward
       the viewer's cap; a higher excess or a brake ends this acceleration;
     excess < DELAY_THRESHOLD_150MS: a good reply; grows the fps, and after a
       reduction returns to the level held before it after two good replies;
     excess >= DELAY_THRESHOLD_150MS: a bad reply; nothing happens until three in a
       row confirm congestion, including after each reduction. FPS drops by a
       fifth at most; a second of excess cannot wait and halves it immediately.
       A recent fast restore also permits halving at 600 ms of excess.
   While the bitrate can still be reduced (ABR) it is reduced first and the fps keeps
   a floor: bitrate-targeted encoders do not send fewer bytes at fewer frames.
c. probe outstanding for more than two seconds => halve the fps for every further
   second, down to MIN_AUTO_FPS and never above the target it found; the late
   reply does not reduce again. Automatic reductions respect this floor unless
   the viewer requested a lower FPS cap.
d. second timeout / TestDelay reply => real fps is the minimum over all users;
   every user starts at INIT_FPS, adapts from its own target and is capped by its
   own limit, never by that minimum or by another user's limit

ratio adjust:
a. user set image quality => update to the maximum ratio of the latest quality
b. 3 seconds timeout => update ratio according to network delay
    When network delay < DELAY_THRESHOLD_150MS, increase ratio, max 150kbps;
    When a user calls for a reduction (two bad replies in a row, or a probe still
    out at the second tick past two seconds), decrease ratio by the step that user's
    own delay and confirmation call for, the most conservative step over all users;
    one slow reply or one short stall does not, and one user's spike is never paired
    with another user's confirmation.
c. confirmed congestion => decrease ratio at once, when the 3 seconds cooldown allows

delay:
    TestDelay shares the video stream, so it measures the queue in front of it rather
    than the path RTT. The baseline starts at the first reply and follows lower
    delays immediately. Old minima expire after 20 fresh replies; a higher window
    minimum is learned gradually only when the recent floor is no longer rising.
    Outstanding-probe checks and their late replies do not age this window.
*/

// Constants
pub const FPS: u32 = 30;
pub const MIN_FPS: u32 = 1;
pub const MAX_FPS: u32 = 120;
pub const INIT_FPS: u32 = 15;
const MIN_AUTO_FPS: u32 = 5;

// Bitrate ratio constants for different quality levels
const BR_MAX: f32 = 40.0; // 2000 * 2 / 100
const BR_MIN: f32 = 0.2;
const BR_MIN_HIGH_RESOLUTION: f32 = 0.1; // For high resolution, BR_MIN is still too high, so we set a lower limit
const MAX_BR_MULTIPLE: f32 = 1.0;

const HISTORY_DELAY_LEN: usize = 2;
const ADJUST_RATIO_INTERVAL: usize = 3; // Adjust quality ratio every 3 seconds
const DYNAMIC_SCREEN_THRESHOLD: usize = 2; // Allow increase quality ratio if encode more than 2 times in one second
const DELAY_THRESHOLD_150MS: u32 = 150; // 150ms is the threshold for good network condition
const RESTORE_GUARD_SAMPLES: u8 = 5; // A restored level that congests this soon is lowered

#[derive(Default, Debug, Clone)]
struct UserDelay {
    stall_ticks: u8, // timer ticks the outstanding probe has been out beyond two seconds
    delay_history: VecDeque<u32>,
    fps: Option<u32>,
    rtt_calculator: RttCalculator,
    quick_increase_fps_count: usize,
    increase_fps_count: usize,
    consecutive_bad_samples: usize,
    fps_bad_samples: u8, // fresh bad replies since the last FPS reduction
    good_samples: usize, // since the last reduction, capped at 3
    replies_after_bitrate_reduction: Option<u8>,
    fps_before_congestion: Option<u32>, // level to return to once replies are good again
    samples_since_restore: Option<u8>,  // set by a restore, cleared once it proved stable
    stall_reference_fps: Option<u32>,   // fps when the outstanding probe passed two seconds
    startup_good_samples: u8,           // u8::MAX permanently ends startup acceleration
}

impl UserDelay {
    fn add_delay(&mut self, delay: u32) {
        if self.delay_history.len() >= HISTORY_DELAY_LEN {
            self.delay_history.pop_front();
        }
        self.delay_history.push_back(delay);
    }

    fn limit_fps_change(
        &mut self,
        current_fps: u32,
        fps: u32,
        delay: u32,
        bitrate_first: bool,
        braked: bool,
    ) -> u32 {
        // A spike stays in the average for several samples; confirm congestion with fresh samples.
        let delay = delay.saturating_sub(self.rtt_calculator.get_rtt().unwrap_or_default());
        if let Some(samples) = self.samples_since_restore.as_mut() {
            *samples = samples.saturating_add(1);
        }
        if delay < DELAY_THRESHOLD_150MS {
            self.consecutive_bad_samples = 0;
            self.fps_bad_samples = 0;
            self.replies_after_bitrate_reduction = None;
            self.good_samples = (self.good_samples + 1).min(3);
            return self.recover(current_fps, fps);
        }
        self.consecutive_bad_samples = (self.consecutive_bad_samples + 1).min(3);
        self.fps_bad_samples = (self.fps_bad_samples + 1).min(3);
        if let Some(replies) = self.replies_after_bitrate_reduction.as_mut() {
            *replies = (*replies + 1).min(2);
        }
        let failed_restore = delay >= 600
            && self
                .samples_since_restore
                .is_some_and(|samples| samples <= RESTORE_GUARD_SAMPLES);
        // A level that congests right after being restored is not the level to return to.
        if self
            .samples_since_restore
            .is_some_and(|samples| samples <= RESTORE_GUARD_SAMPLES)
            && (failed_restore || self.consecutive_bad_samples >= 3)
        {
            self.fps_before_congestion = Some(current_fps - current_fps / 4);
            self.samples_since_restore = None;
        }
        // The timeout brake already reduced for the probe this reply answers.
        if fps >= current_fps || braked {
            return current_fps;
        }
        // An extra second of delay cannot wait for another confirmation.
        if !failed_restore
            && delay < 1000
            && (self.fps_bad_samples < 3
                || (bitrate_first && self.replies_after_bitrate_reduction.unwrap_or_default() < 2))
        {
            return current_fps;
        }
        // A fast restore probes capacity. Roll it back promptly if the queue grows
        // again, rather than waiting through another ordinary confirmation window.
        let divisor = if delay >= 1000 || failed_restore {
            2
        } else {
            5
        };
        self.on_reduction(current_fps);
        fps.max(current_fps.saturating_sub((current_fps / divisor).max(1)))
    }

    // Fresh low-delay replies permit recovery even while the average contains a spike:
    // a little at first, then back to the level held before congestion.
    fn recover(&mut self, current_fps: u32, fps: u32) -> u32 {
        let gradual = current_fps + (current_fps / 10).max(1);
        let level = self
            .fps_before_congestion
            .filter(|level| *level > current_fps);
        match (self.good_samples, level) {
            (2 | 3, Some(level)) => {
                self.fps_before_congestion = None;
                self.samples_since_restore = Some(0);
                fps.max(level)
            }
            (3, None) => {
                self.fps_before_congestion = None;
                fps.max(current_fps + (current_fps / 5).max(2))
            }
            _ => gradual,
        }
    }

    fn accelerate_startup(
        &mut self,
        current_fps: u32,
        fps: u32,
        cap: u32,
        delay: u32,
        braked: bool,
    ) -> u32 {
        if self.startup_good_samples == u8::MAX {
            return fps;
        }
        let excess = delay.saturating_sub(self.rtt_calculator.get_rtt().unwrap_or_default());
        // A low-load sample does not establish capacity: require two clean replies
        // per step and abandon startup probing on the first sign of queue growth.
        if braked || excess >= 50 || current_fps >= cap {
            self.startup_good_samples = u8::MAX;
            return fps;
        }
        self.startup_good_samples += 1;
        if self.startup_good_samples < 2 {
            return fps;
        }
        self.startup_good_samples = 0;
        let accelerated = fps.max(current_fps.saturating_mul(2)).min(cap);
        if accelerated >= cap {
            self.startup_good_samples = u8::MAX;
        }
        accelerated
    }

    // The first reduction of an episode remembers the level to return to.
    fn on_reduction(&mut self, current_fps: u32) {
        self.startup_good_samples = u8::MAX;
        self.good_samples = 0;
        self.fps_bad_samples = 0;
        if self.fps_before_congestion.is_none() {
            self.fps_before_congestion = Some(current_fps);
        }
    }

    // Bitrate is cut on confirmation only: two bad replies in a row, or a probe still
    // outstanding at the second tick past two seconds.  One slow reply or one short
    // stall is jitter, and a static screen would never earn the cut back.
    fn needs_bitrate_reduction(&self) -> bool {
        self.consecutive_bad_samples >= 2 || self.stall_ticks >= 2
    }

    // The bitrate step this viewer's own evidence calls for, None when it calls for
    // none.  Severity and confirmation come from the same viewer; the controller
    // never pairs one viewer's spike with another viewer's confirmation.
    fn ratio_reduction(&self) -> Option<f32> {
        if !self.needs_bitrate_reduction() {
            return None;
        }
        let excess = self.avg_delay();
        let confirmed = self.consecutive_bad_samples >= 3;
        Some(if excess < 200 {
            0.95
        } else if excess < 300 {
            0.9
        } else if excess < 500 {
            if confirmed {
                0.7
            } else {
                0.85
            }
        } else if confirmed {
            0.5
        } else {
            0.8
        })
    }

    // Average delay above the baseline: what the queue adds on top of the path itself.
    fn avg_delay(&self) -> u32 {
        if self.delay_history.is_empty() {
            return DELAY_THRESHOLD_150MS;
        }
        let avg_delay = self.delay_history.iter().sum::<u32>() / self.delay_history.len() as u32;
        avg_delay.saturating_sub(self.rtt_calculator.get_rtt().unwrap_or_default())
    }
}

// User session data structure
#[derive(Default, Debug, Clone)]
struct UserData {
    auto_adjust_fps: Option<u32>, // reserve for compatibility
    custom_fps: Option<u32>,
    quality: Option<(i64, Quality)>, // (time, quality)
    delay: UserDelay,
    record: bool,
    joined_at: Option<Instant>, // set by on_connection_open; the start-up guard's clock
}

impl UserData {
    // The frame rate this viewer asked for, from its custom or auto-adjust limit.
    fn fps_cap(&self) -> u32 {
        let mut fps = self.custom_fps.unwrap_or(FPS);
        if let Some(auto_adjust_fps) = self.auto_adjust_fps {
            if fps == 0 || auto_adjust_fps < fps {
                fps = auto_adjust_fps;
            }
        }
        fps.clamp(MIN_FPS, MAX_FPS)
    }
}

#[derive(Default, Debug, Clone)]
struct DisplayData {
    send_counter: usize, // Number of times encode during period
    support_changing_quality: bool,
}

// Main QoS controller structure
pub struct VideoQoS {
    fps: u32,
    ratio: f32,
    users: HashMap<i32, UserData>,
    displays: HashMap<String, DisplayData>,
    bitrate_store: u32,
    adjust_ratio_instant: Instant,
    abr_config: bool,
    first_reply_adjusts_ratio: bool, // false on Linux, where it can create vaapi twice
    #[cfg(test)]
    test_now: Option<Instant>,
}

impl Default for VideoQoS {
    fn default() -> Self {
        VideoQoS {
            fps: FPS,
            ratio: BR_BALANCED,
            users: Default::default(),
            displays: Default::default(),
            bitrate_store: 0,
            adjust_ratio_instant: Instant::now(),
            abr_config: true,
            first_reply_adjusts_ratio: !cfg!(target_os = "linux"),
            #[cfg(test)]
            test_now: None,
        }
    }
}

// Clock; tests drive a virtual clock so timing is deterministic.
impl VideoQoS {
    fn now(&self) -> Instant {
        #[cfg(test)]
        if let Some(now) = self.test_now {
            return now;
        }
        Instant::now()
    }

    fn since(&self, instant: Instant) -> Duration {
        self.now().saturating_duration_since(instant)
    }

    #[cfg(test)]
    fn advance_ms(&mut self, ms: u64) {
        self.test_now = Some(self.now() + Duration::from_millis(ms));
    }
}

// Basic functionality
impl VideoQoS {
    // Calculate seconds per frame based on current FPS
    pub fn spf(&self) -> Duration {
        Duration::from_secs_f32(1. / (self.fps() as f32))
    }

    // Get current FPS within valid range
    pub fn fps(&self) -> u32 {
        let fps = self.fps;
        if fps >= MIN_FPS && fps <= MAX_FPS {
            fps
        } else {
            FPS
        }
    }

    // Store bitrate for later use
    pub fn store_bitrate(&mut self, bitrate: u32) {
        self.bitrate_store = bitrate;
    }

    // Get stored bitrate
    pub fn bitrate(&self) -> u32 {
        self.bitrate_store
    }

    // Get current bitrate ratio with bounds checking
    pub fn ratio(&mut self) -> f32 {
        if self.ratio < BR_MIN_HIGH_RESOLUTION || self.ratio > BR_MAX {
            self.ratio = BR_BALANCED;
        }
        self.ratio
    }

    // Check if any user is in recording mode
    pub fn record(&self) -> bool {
        self.users.iter().any(|u| u.1.record)
    }

    pub fn set_support_changing_quality(&mut self, video_service_name: &str, support: bool) {
        if let Some(display) = self.displays.get_mut(video_service_name) {
            display.support_changing_quality = support;
        }
    }

    // Check if variable bitrate encoding is supported and enabled
    pub fn in_vbr_state(&self) -> bool {
        self.abr_config && self.displays.iter().all(|e| e.1.support_changing_quality)
    }
}

// User session management
impl VideoQoS {
    // Initialize new user session
    pub fn on_connection_open(&mut self, id: i32) {
        let user = UserData {
            joined_at: Some(self.now()),
            ..Default::default()
        };
        self.users.insert(id, user);
        self.abr_config = Config::get_option("enable-abr") != "N";
    }

    // Clean up user session
    pub fn on_connection_close(&mut self, id: i32) {
        self.users.remove(&id);
        if self.users.is_empty() {
            *self = Default::default();
            return;
        }
        // The stream follows the remaining viewers at once; a departed viewer's
        // start-up guard left with its entry.
        self.adjust_fps();
    }

    pub fn user_custom_fps(&mut self, id: i32, fps: u32) {
        if fps < MIN_FPS || fps > MAX_FPS {
            return;
        }
        if let Some(user) = self.users.get_mut(&id) {
            user.custom_fps = Some(fps);
        }
    }

    pub fn user_auto_adjust_fps(&mut self, id: i32, fps: u32) {
        if fps < MIN_FPS || fps > MAX_FPS {
            return;
        }
        if let Some(user) = self.users.get_mut(&id) {
            user.auto_adjust_fps = Some(fps);
        }
    }

    pub fn user_image_quality(&mut self, id: i32, image_quality: i32) {
        let convert_quality = |q: i32| -> Quality {
            if q == ImageQuality::Balanced.value() {
                Quality::Balanced
            } else if q == ImageQuality::Low.value() {
                Quality::Low
            } else if q == ImageQuality::Best.value() {
                Quality::Best
            } else {
                let b = ((q >> 8 & 0xFFF) * 2) as f32 / 100.0;
                Quality::Custom(b.clamp(BR_MIN, BR_MAX))
            }
        };

        let quality = Some((hbb_common::get_time(), convert_quality(image_quality)));
        if let Some(user) = self.users.get_mut(&id) {
            user.quality = quality;
            // update ratio directly
            self.ratio = self.latest_quality().ratio();
        }
    }

    pub fn user_record(&mut self, id: i32, v: bool) {
        if let Some(user) = self.users.get_mut(&id) {
            user.record = v;
        }
    }

    pub fn user_network_delay(&mut self, id: i32, delay: u32) {
        let target_ratio = self.latest_quality().ratio();
        // Fewer frames only save bytes with encoders that size frames for a fixed rate;
        // bitrate-targeted encoders keep the bitrate, so the bitrate has to come down first.
        let bitrate_first = self.can_reduce_bitrate();

        // For bad network, small fps means quick reaction and high quality
        let (min_fps, normal_fps) = if target_ratio >= BR_BEST {
            (8, 16)
        } else if target_ratio >= BR_BALANCED {
            (10, 20)
        } else {
            (12, 24)
        };

        // Calculate minimum acceptable delay-fps product
        let dividend_ms = DELAY_THRESHOLD_150MS * min_fps;

        let mut adjust_ratio = false;
        let mut reduce_bitrate = false;
        if let Some(user) = self.users.get_mut(&id) {
            let delay = delay.max(10);
            // The reply closes the outstanding probe, braked or not.
            user.delay.stall_ticks = 0;
            let braked = user.delay.stall_reference_fps.take().is_some();
            let old_avg_delay = user.delay.avg_delay();
            if !braked {
                user.delay.rtt_calculator.update(delay);
            }
            user.delay.add_delay(delay);
            let mut avg_delay = user.delay.avg_delay();
            avg_delay = avg_delay.max(10);
            // Each viewer adapts from its own target, starts at INIT_FPS and is capped
            // by its own limit.  The stream follows the slowest viewer in adjust_fps;
            // neither that minimum nor another viewer's limit feeds back into it.
            let user_cap = user.fps_cap();
            let current_fps = user.delay.fps.unwrap_or(INIT_FPS.min(user_cap));
            let mut fps = current_fps;

            // Adaptive FPS adjustment based on network delay:
            if avg_delay < 50 {
                user.delay.quick_increase_fps_count += 1;
                let mut step = if fps < normal_fps { 1 } else { 0 };
                if user.delay.quick_increase_fps_count >= 3 {
                    // After 3 consecutive good samples, increase more aggressively
                    user.delay.quick_increase_fps_count = 0;
                    step = 5;
                }
                fps = min_fps.max(fps + step);
            } else if avg_delay < 100 {
                let step = if avg_delay < old_avg_delay {
                    if fps < normal_fps {
                        1
                    } else {
                        0
                    }
                } else {
                    0
                };
                fps = min_fps.max(fps + step);
            } else if avg_delay < DELAY_THRESHOLD_150MS {
                fps = min_fps.max(fps);
            } else {
                let devide_fps = ((fps as f32) / (avg_delay as f32 / DELAY_THRESHOLD_150MS as f32))
                    .ceil() as u32;
                if avg_delay < 200 {
                    fps = min_fps.max(devide_fps);
                } else if avg_delay < 300 {
                    fps = min_fps.min(devide_fps);
                } else if avg_delay < 600 {
                    fps = dividend_ms / avg_delay;
                } else {
                    fps = (dividend_ms / avg_delay).min(devide_fps);
                }
            }

            if avg_delay < DELAY_THRESHOLD_150MS {
                user.delay.increase_fps_count += 1;
            } else {
                user.delay.increase_fps_count = 0;
            }
            if user.delay.increase_fps_count >= 3 {
                // After 3 stable samples, try increasing FPS
                user.delay.increase_fps_count = 0;
                fps += 1;
            }

            // Reset quick increase counter if network condition worsens
            if avg_delay > 50 {
                user.delay.quick_increase_fps_count = 0;
            }

            if bitrate_first {
                // While the bitrate can still come down, the frame rate keeps its floor.
                fps = fps.max(min_fps);
            }
            fps = fps.max(MIN_AUTO_FPS.min(user_cap));
            fps = user
                .delay
                .limit_fps_change(current_fps, fps, delay, bitrate_first, braked);
            fps = user
                .delay
                .accelerate_startup(current_fps, fps, user_cap, delay, braked);
            reduce_bitrate = bitrate_first
                && user.delay.needs_bitrate_reduction()
                && user.delay.replies_after_bitrate_reduction.is_none();
            fps = fps.clamp(MIN_FPS, user_cap);
            // first network delay message
            adjust_ratio = user.delay.fps.is_none();
            user.delay.fps = Some(fps);
            let base = user.delay.rtt_calculator.get_rtt().unwrap_or_default();
            log::debug!(
                "qos_trace t={} id={id} delay={delay} base={base} excess={} avg={avg_delay} bad={} good={} braked={braked} fps={fps} ratio={:.3} reduce_bitrate={reduce_bitrate}",
                hbb_common::get_time(),
                delay.saturating_sub(base),
                user.delay.consecutive_bad_samples,
                user.delay.good_samples,
                self.ratio,
            );
        }
        self.adjust_fps();
        // A viewer's first reply is one more trigger of the periodic adjustment and
        // keeps its cooldown: a viewer joining right after a cut must not spend the
        // other viewers' evidence a second time.
        if adjust_ratio
            && self.first_reply_adjusts_ratio
            && self.since(self.adjust_ratio_instant).as_secs() >= ADJUST_RATIO_INTERVAL as u64
        {
            self.adjust_ratio(false);
        }
        if reduce_bitrate
            && self.since(self.adjust_ratio_instant).as_secs() >= ADJUST_RATIO_INTERVAL as u64
        {
            self.adjust_ratio(false);
        }
    }

    pub fn user_delay_response_elapsed(&mut self, id: i32, elapsed: u128) {
        let Some(user) = self.users.get_mut(&id) else {
            return;
        };
        if elapsed <= 2000 {
            return;
        }
        user.delay.stall_ticks = user.delay.stall_ticks.saturating_add(1);
        user.delay.add_delay(elapsed as u32);
        // Halve for every second the probe stays out beyond the first: two seconds
        // halve, three quarter, and so on down to the floor.
        let reference = match user.delay.stall_reference_fps {
            Some(reference) => reference,
            None => {
                let reference = user.delay.fps.unwrap_or(INIT_FPS.min(user.fps_cap()));
                user.delay.stall_reference_fps = Some(reference);
                user.delay.on_reduction(reference);
                reference
            }
        };
        let divisor = 1u32 << ((elapsed / 1000) as u32).saturating_sub(1).min(5);
        let user_cap = user.fps_cap();
        // The floor is a floor, not a lift: a target already below it stays.
        let current = user.delay.fps.unwrap_or(reference);
        let fps = (reference / divisor)
            .clamp(MIN_AUTO_FPS.min(user_cap), user_cap)
            .min(current);
        user.delay.fps = Some(fps);
        log::debug!(
            "qos_trace t={} id={id} timeout={elapsed} fps={fps}",
            hbb_common::get_time()
        );
        self.adjust_fps();
    }
}

// Common adjust functions
impl VideoQoS {
    pub fn new_display(&mut self, video_service_name: String) {
        self.displays
            .insert(video_service_name, DisplayData::default());
    }

    pub fn remove_display(&mut self, video_service_name: &str) {
        self.displays.remove(video_service_name);
    }

    pub fn update_display_data(&mut self, video_service_name: &str, send_counter: usize) {
        if let Some(display) = self.displays.get_mut(video_service_name) {
            display.send_counter += send_counter;
        }
        self.adjust_fps();
        let abr_enabled = self.in_vbr_state();
        if abr_enabled {
            if self.since(self.adjust_ratio_instant).as_secs() >= ADJUST_RATIO_INTERVAL as u64 {
                let dynamic_screen = self
                    .displays
                    .iter()
                    .any(|d| d.1.send_counter >= ADJUST_RATIO_INTERVAL * DYNAMIC_SCREEN_THRESHOLD);
                self.adjust_ratio(dynamic_screen);
            }
        } else {
            self.ratio = self.latest_quality().ratio();
        }
    }

    #[inline]
    fn highest_fps(&self) -> u32 {
        self.users
            .values()
            .map(|u| u.fps_cap())
            .min()
            .unwrap_or(FPS)
            .clamp(MIN_FPS, MAX_FPS)
    }

    // Get latest quality settings from all users
    pub fn latest_quality(&self) -> Quality {
        self.users
            .iter()
            .map(|(_, u)| u.quality)
            .filter(|q| *q != None)
            .max_by(|a, b| a.unwrap_or_default().0.cmp(&b.unwrap_or_default().0))
            .flatten()
            .unwrap_or((0, Quality::Balanced))
            .1
    }

    // Lowest ratio the latest quality allows: keeps about 1Mbps at high resolutions.
    fn min_ratio(&self) -> f32 {
        let current_bitrate = self.bitrate();
        let ratio_1mbps = if current_bitrate > 0 {
            Some((self.ratio * 1000.0 / current_bitrate as f32).max(BR_MIN_HIGH_RESOLUTION))
        } else {
            None
        };
        match self.latest_quality() {
            Quality::Best => {
                let mut min = BR_BEST / 2.5;
                if let Some(ratio_1mbps) = ratio_1mbps {
                    if min > ratio_1mbps {
                        min = ratio_1mbps;
                    }
                }
                min.max(BR_MIN)
            }
            Quality::Balanced => {
                let mut min = (BR_BALANCED / 2.0).min(0.4);
                if let Some(ratio_1mbps) = ratio_1mbps {
                    if min > ratio_1mbps {
                        min = ratio_1mbps;
                    }
                }
                min.max(BR_MIN_HIGH_RESOLUTION)
            }
            Quality::Low | Quality::Custom(_) => BR_MIN_HIGH_RESOLUTION,
        }
    }

    // Whether congestion can still be answered with a lower bitrate.  Within two
    // percent of the floor another step is not worth waiting a cooldown for.
    fn can_reduce_bitrate(&self) -> bool {
        self.in_vbr_state() && !self.displays.is_empty() && self.ratio > self.min_ratio() * 1.02
    }

    // Every ratio adjustment starts a new window for the dynamic screen counters.
    fn reset_send_counters(&mut self) {
        self.displays.values_mut().for_each(|d| d.send_counter = 0);
    }

    // Adjust quality ratio based on network delay and screen changes
    fn adjust_ratio(&mut self, dynamic_screen: bool) {
        if !self.in_vbr_state() {
            return;
        }
        // Get maximum delay from all users
        let max_delay = self.users.iter().map(|u| u.1.delay.avg_delay()).max();
        let Some(max_delay) = max_delay else {
            return;
        };
        // Each viewer judges its own delay; the stream takes the most conservative
        // step any viewer asks for.
        let reduction = self
            .users
            .values()
            .filter_map(|u| u.delay.ratio_reduction())
            .reduce(f32::min);
        if reduction.is_none() && max_delay >= DELAY_THRESHOLD_150MS {
            // Elevated but unconfirmed: no change, and no cooldown either, so a
            // confirmation on the next reply is acted on at once.
            self.reset_send_counters();
            return;
        }

        let target_ratio = self.latest_quality().ratio();
        let current_ratio = self.ratio;
        let current_bitrate = self.bitrate();

        // Calculate ratio for adding 150kbps bandwidth
        let ratio_add_150kbps = if current_bitrate > 0 {
            Some((current_bitrate + 150) as f32 * current_ratio / current_bitrate as f32)
        } else {
            None
        };

        let min = self.min_ratio();
        let max = target_ratio * MAX_BR_MULTIPLE;

        let mut v = current_ratio;

        // Three bad replies in a row confirm congestion; with a bitrate-targeted
        // encoder the bitrate is then the only thing that drains the queue, so it
        // comes down hard.  Increases need every viewer below the threshold.
        if let Some(factor) = reduction {
            v = current_ratio * factor;
        } else if max_delay < 50 {
            if dynamic_screen {
                v = current_ratio * 1.15;
            }
        } else if max_delay < 100 {
            if dynamic_screen {
                v = current_ratio * 1.1;
            }
        } else if dynamic_screen {
            v = current_ratio * 1.05;
        }

        // Limit quality increase rate for better stability
        if let Some(ratio_add_150kbps) = ratio_add_150kbps {
            if v > ratio_add_150kbps
                && ratio_add_150kbps > current_ratio
                && current_ratio >= BR_SPEED
            {
                v = ratio_add_150kbps;
            }
        }

        if reduction.is_some() {
            for user in self.users.values_mut() {
                if user.delay.needs_bitrate_reduction()
                    && user.delay.replies_after_bitrate_reduction.is_none()
                {
                    // One outstanding probe may have started before the bitrate change.
                    user.delay.replies_after_bitrate_reduction =
                        Some(if v.clamp(min, max) < current_ratio {
                            0
                        } else {
                            2
                        });
                }
            }
        }
        self.ratio = v.clamp(min, max);
        self.reset_send_counters();
        self.adjust_ratio_instant = self.now();
    }

    // Adjust fps based on network delay and user response time
    fn adjust_fps(&mut self) {
        let highest_fps = self.highest_fps();
        // Get minimum fps from all users
        let mut fps = self
            .users
            .iter()
            .map(|u| u.1.delay.fps.unwrap_or(INIT_FPS))
            .min()
            .unwrap_or(INIT_FPS);

        // Every viewer inside its first second keeps the stream at INIT_FPS to
        // ensure stability; each viewer carries its own start-up clock.
        if self.users.values().any(|u| {
            u.joined_at
                .is_some_and(|joined| self.since(joined).as_secs() < 1)
        }) {
            fps = fps.min(INIT_FPS);
        }

        // Ensure fps stays within valid range
        self.fps = fps.clamp(MIN_FPS, highest_fps);
    }
}

#[derive(Default, Debug, Clone)]
struct RttCalculator {
    baseline: Option<u32>,
    samples: VecDeque<u32>,
}

impl RttCalculator {
    const WINDOW_SAMPLES: usize = 20;
    const MAX_INCREASE_MS: u32 = 50;

    pub fn update(&mut self, delay: u32) {
        if self.samples.len() >= Self::WINDOW_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(delay);
        let baseline = self.baseline.unwrap_or(delay).min(delay);
        self.baseline = Some(baseline);

        if self.samples.len() < Self::WINDOW_SAMPLES {
            return;
        }
        let half = Self::WINDOW_SAMPLES / 2;
        let older_min = self.samples.iter().take(half).min().copied();
        let recent_min = self.samples.iter().skip(half).min().copied();
        let (Some(older_min), Some(recent_min)) = (older_min, recent_min) else {
            return;
        };
        // A rising floor can be a growing queue. Allow 10 ms of probe granularity,
        // but wait for it to settle before forgetting the old baseline.
        if recent_min > older_min.saturating_add(10) {
            return;
        }
        let rise = older_min.min(recent_min).saturating_sub(baseline);
        if rise > 0 {
            self.baseline = Some(baseline + (rise / 2).clamp(1, Self::MAX_INCREASE_MS));
        }
    }

    pub fn get_rtt(&self) -> Option<u32> {
        self.baseline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stable_qos() -> VideoQoS {
        let mut qos = VideoQoS::default();
        qos.advance_ms(2000);
        qos.users.insert(1, UserData::default());
        for _ in 0..12 {
            qos.user_network_delay(1, 10);
        }
        assert_eq!(qos.fps(), FPS);
        qos
    }

    #[test]
    fn isolated_delay_spike_does_not_lower_fps() {
        let mut qos = stable_qos();
        for delay in [800, 10, 10, 10] {
            qos.user_network_delay(1, delay);
            assert_eq!(qos.fps(), FPS);
        }
    }

    #[test]
    fn occasional_spikes_do_not_accumulate_congestion() {
        let mut qos = stable_qos();
        for delay in [800, 10, 10].repeat(20) {
            qos.user_network_delay(1, delay);
            assert_eq!(qos.fps(), FPS);
        }
    }

    #[test]
    fn sustained_delay_reduces_fps_gradually() {
        let mut qos = stable_qos();
        for expected_fps in [30, 30, 24, 24, 24, 20] {
            qos.user_network_delay(1, 800);
            assert_eq!(qos.fps(), expected_fps);
        }
    }

    #[test]
    fn delay_history_keeps_two_samples() {
        let mut delay = UserDelay::default();
        for sample in [1, 2, 3] {
            delay.add_delay(sample);
        }
        assert_eq!(delay.delay_history.len(), HISTORY_DELAY_LEN);
    }

    #[test]
    fn response_timeout_halves_fps_for_each_second_outstanding() {
        let mut qos = stable_qos();
        for (elapsed, expected) in [(2001, 15), (3001, 7), (4001, 5), (5001, 5), (6001, 5)] {
            qos.user_delay_response_elapsed(1, elapsed);
            assert_eq!(qos.fps(), expected, "{elapsed} ms outstanding");
        }
    }

    #[test]
    fn severe_delay_does_not_wait_for_another_reply() {
        let mut qos = stable_qos();
        qos.user_network_delay(1, 1200);
        assert_eq!(qos.fps(), 15);
    }

    #[test]
    fn response_timeout_recovers_in_two_good_replies() {
        let mut qos = stable_qos();
        qos.user_delay_response_elapsed(1, 3000);
        assert_eq!(qos.fps(), 7);
        qos.user_network_delay(1, 3200);
        assert_eq!(
            qos.fps(),
            7,
            "the late reply belongs to the stall that was braked"
        );
        qos.user_delay_response_elapsed(1, 0);
        qos.user_network_delay(1, 10);
        assert_eq!(
            qos.fps(),
            8,
            "one good reply must not restore the full frame rate"
        );
        qos.user_network_delay(1, 10);
        assert_eq!(
            qos.fps(),
            FPS,
            "the second good reply restores the frame rate"
        );
        qos.user_network_delay(1, 10);
        assert_eq!(qos.fps(), FPS, "the third keeps the restored frame rate");
    }

    #[test]
    fn restore_aims_lower_after_a_restore_that_congested() {
        let mut qos = stable_qos();
        for _ in 0..2 {
            qos.user_network_delay(1, 1200);
        }
        assert_eq!(qos.fps(), 8);
        qos.user_network_delay(1, 10);
        qos.user_network_delay(1, 10);
        assert_eq!(qos.fps(), FPS);
        // The restored level congests at once, so the next restore aims lower.
        for _ in 0..3 {
            qos.user_network_delay(1, 400);
        }
        assert!(qos.fps() < FPS);
        qos.user_network_delay(1, 10);
        qos.user_network_delay(1, 10);
        assert!(
            qos.fps() < FPS,
            "no return to the level that failed: {}",
            qos.fps()
        );
        qos.user_network_delay(1, 10);
        assert_eq!(qos.fps(), FPS, "ordinary recovery can still reach the cap");
    }

    #[test]
    fn custom_fps_limit_applies_during_delay_spike() {
        let mut qos = stable_qos();
        qos.user_custom_fps(1, 12);
        qos.user_network_delay(1, 800);
        assert_eq!(qos.fps(), 12);
    }

    mod adaptation;
    mod baseline;
    mod invariants;
    mod jitter;
    mod recovery;
    mod robustness;
    mod sim;
    mod smoke;
    mod startup;
}
