//! The controller's invariants as properties over random sessions.  A scenario
//! test pins one trajectory; these hold whatever the trajectory:
//!
//! 1. viewer isolation: a viewer's private target is a function of its own
//!    replies, timeouts and limit, never of another viewer's (with ABR on, the
//!    shared bitrate state is the one designed input: the frame rate keeps a
//!    floor while the bitrate can still come down);
//! 2. bad evidence never raises anything: a bad reply or a timeout tick keeps or
//!    lowers that viewer's target and the bitrate ratio;
//! 3. lifecycle: a join adds a constraint and a leave removes it, and neither
//!    touches any other viewer's state;
//! 4. evidence ownership: a bitrate cut is asked for by a viewer's own evidence,
//!    by the step that viewer's own evidence calls for, and a newcomer's first
//!    reply does not spend that evidence again;
//! 5. caps: a reply leaves the target within `[MIN_FPS, cap]`, and the stream is
//!    the aggregation of the targets, the caps and the start-up guards;
//! 6. pairing: the late reply of a braked probe does not brake again.
use super::*;

/// xorshift64*, so the tests need no external crate and stay reproducible.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng((seed ^ 0x9E37_79B9_7F4A_7C15).max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
}

#[derive(Debug, Clone, Copy)]
enum Step {
    Reply { id: i32, delay: u32 },
    Timeout { id: i32, elapsed: u128 },
    Wait(u64),
    Tick(usize),
    Cap { id: i32, fps: u32 },
}

const SEEDS: u64 = 150;
const STEPS: usize = 300;

/// Random events for a set of viewers on one link.  A probe that is out stays
/// out until a reply: the connection reports a growing elapsed time every second
/// and the reply that ends the stall carries at least that delay.
struct Driver {
    rng: Rng,
    ids: Vec<i32>,
    base_rtt: u32,
    outstanding: HashMap<i32, u128>,
}

impl Driver {
    fn new(seed: u64, ids: Vec<i32>) -> Self {
        let mut rng = Rng::new(seed);
        let base_rtt = 10 + rng.below(300) as u32;
        Driver {
            rng,
            ids,
            base_rtt,
            outstanding: HashMap::new(),
        }
    }

    fn step(&mut self) -> Step {
        let id = self.ids[self.rng.below(self.ids.len() as u64) as usize];
        let roll = self.rng.below(100);
        match roll {
            0..=64 => {
                let mut delay = if roll < 45 {
                    self.base_rtt + self.rng.below(140) as u32
                } else {
                    self.base_rtt + DELAY_THRESHOLD_150MS + self.rng.below(1500) as u32
                };
                if let Some(elapsed) = self.outstanding.remove(&id) {
                    delay = delay.max(elapsed as u32 + self.rng.below(500) as u32);
                }
                Step::Reply { id, delay }
            }
            65..=74 => {
                let elapsed = match self.outstanding.get(&id) {
                    Some(elapsed) => elapsed + 1000,
                    None => 2001 + self.rng.below(1000) as u128,
                };
                self.outstanding.insert(id, elapsed);
                Step::Timeout { id, elapsed }
            }
            75..=89 => Step::Wait(self.rng.below(1500)),
            90..=96 => Step::Tick(self.rng.below(31) as usize),
            _ => Step::Cap {
                id,
                fps: 1 + self.rng.below(60) as u32,
            },
        }
    }
}

/// What `on_connection_open` inserts, without touching the config store.
fn open(qos: &mut VideoQoS, id: i32) {
    qos.users.insert(
        id,
        UserData {
            joined_at: Some(qos.now()),
            ..Default::default()
        },
    );
}

fn session(abr: bool) -> VideoQoS {
    let mut qos = VideoQoS::default();
    qos.advance_ms(2000);
    qos.abr_config = abr;
    qos.first_reply_adjusts_ratio = true;
    qos.new_display("test".to_owned());
    qos.set_support_changing_quality("test", true);
    qos.store_bitrate(4000);
    qos
}

/// The video loop reports the encoder's bitrate as soon as it applies a ratio.
fn sync_bitrate(qos: &mut VideoQoS) {
    let target = qos.latest_quality().ratio();
    let ratio = qos.ratio();
    qos.store_bitrate((4000.0 * ratio / target) as u32);
}

fn apply(qos: &mut VideoQoS, step: Step) {
    match step {
        Step::Reply { id, delay } => qos.user_network_delay(id, delay),
        Step::Timeout { id, elapsed } => qos.user_delay_response_elapsed(id, elapsed),
        Step::Wait(ms) => qos.advance_ms(ms),
        Step::Tick(encoded) => qos.update_display_data("test", encoded),
        Step::Cap { id, fps } => qos.user_custom_fps(id, fps),
    }
    sync_bitrate(qos);
}

/// The viewer's private target, as the controller reads it before a reply.
fn target(qos: &VideoQoS, id: i32) -> u32 {
    let user = &qos.users[&id];
    user.delay.fps.unwrap_or(INIT_FPS.min(user.fps_cap()))
}

fn baseline(qos: &VideoQoS, id: i32) -> Option<u32> {
    qos.users[&id].delay.rtt_calculator.get_rtt()
}

/// Everything the controller keeps about a viewer, for change detection.
fn snapshot(qos: &VideoQoS, id: i32) -> String {
    format!("{:?}", qos.users[&id])
}

/// The aggregation `adjust_fps` is meant to compute: the slowest viewer's target,
/// INIT_FPS for a viewer without a reply or inside its first second, within the
/// lowest cap.
fn expected_stream(qos: &VideoQoS) -> u32 {
    let mut fps = qos
        .users
        .values()
        .map(|u| u.delay.fps.unwrap_or(INIT_FPS))
        .min()
        .unwrap_or(INIT_FPS);
    if qos
        .users
        .values()
        .any(|u| u.joined_at.is_some_and(|j| qos.since(j).as_secs() < 1))
    {
        fps = fps.min(INIT_FPS);
    }
    let cap = qos
        .users
        .values()
        .map(|u| u.fps_cap())
        .min()
        .unwrap_or(FPS);
    fps.clamp(MIN_FPS, cap)
}

// Invariant 2: bad evidence never raises a target or the ratio.
#[test]
fn bad_evidence_never_raises_a_target_or_the_ratio() {
    for seed in 0..SEEDS {
        for abr in [false, true] {
            let mut qos = session(abr);
            let ids: Vec<i32> = (1..=1 + (seed % 3) as i32).collect();
            for id in &ids {
                open(&mut qos, *id);
            }
            let mut driver = Driver::new(seed, ids);
            for step_no in 0..STEPS {
                let step = driver.step();
                let ratio_before = qos.ratio();
                let before = match step {
                    Step::Reply { id, .. } | Step::Timeout { id, .. } => Some((id, target(&qos, id))),
                    _ => None,
                };
                apply(&mut qos, step);
                // Bad by the baseline the controller used for this reply: the reply
                // itself may have relearned it.
                let bad = match step {
                    Step::Reply { id, delay } => baseline(&qos, id)
                        .is_some_and(|base| delay >= base + DELAY_THRESHOLD_150MS),
                    Step::Timeout { .. } => true,
                    _ => false,
                };
                if let (true, Some((id, before))) = (bad, before) {
                    let after = target(&qos, id);
                    assert!(
                        after <= before,
                        "seed {seed} abr {abr} step {step_no} {step:?}: target {before} -> {after}"
                    );
                    assert!(
                        qos.ratio() <= ratio_before,
                        "seed {seed} abr {abr} step {step_no} {step:?}: ratio {ratio_before} -> {}",
                        qos.ratio()
                    );
                }
            }
        }
    }
}

// Invariant 2 and 6: a timeout tick keeps or lowers the target, whatever it is.
#[test]
fn a_timeout_keeps_or_lowers_every_target() {
    for reference in MIN_FPS..=MAX_FPS {
        for elapsed in [2001, 2999, 3000, 3001, 4500, 6001, 9000, 30_000] {
            let mut qos = session(false);
            open(&mut qos, 1);
            qos.user_custom_fps(1, MAX_FPS);
            qos.users.get_mut(&1).unwrap().delay.fps = Some(reference);
            qos.adjust_fps();
            let stream = qos.fps();
            qos.user_delay_response_elapsed(1, elapsed);
            assert!(
                target(&qos, 1) <= reference,
                "{elapsed} ms outstanding at {reference} fps: target {}",
                target(&qos, 1)
            );
            assert!(
                qos.fps() <= stream,
                "{elapsed} ms outstanding at {reference} fps: stream {stream} -> {}",
                qos.fps()
            );
        }
    }
}

// Invariant 6: the late reply of a braked probe does not brake again.
#[test]
fn a_late_reply_after_a_brake_does_not_brake_again() {
    for reference in (MIN_FPS + 1..=MAX_FPS).step_by(3) {
        for elapsed in [2001u32, 3001, 4500, 6001, 9000] {
            for abr in [false, true] {
                let mut qos = session(abr);
                open(&mut qos, 1);
                qos.user_custom_fps(1, MAX_FPS);
                for _ in 0..3 {
                    qos.user_network_delay(1, 20);
                }
                qos.users.get_mut(&1).unwrap().delay.fps = Some(reference);
                qos.user_delay_response_elapsed(1, elapsed as u128);
                let braked = target(&qos, 1);
                qos.user_network_delay(1, elapsed + 100);
                assert_eq!(
                    target(&qos, 1),
                    braked,
                    "abr {abr}, {elapsed} ms outstanding at {reference} fps"
                );
            }
        }
    }
}

/// Viewer 1's target after each of its own events, alone or with company whose
/// events are interleaved: a second viewer with its own replies, timeouts, waits
/// and limits, and a third that joins and leaves along the way.  The display
/// timer is left out: on a dynamic screen it raises the ratio off its floor,
/// and the property holds the bitrate state fixed.
fn viewer_one_targets(seed: u64, company: bool, abr: bool, at_floor: bool) -> Vec<u32> {
    let mut own = Driver::new(seed, vec![1]);
    let mut others = Driver::new(seed ^ 0xC0FF_EE, vec![2]);
    let mut qos = session(abr);
    if at_floor {
        qos.ratio = qos.min_ratio();
        sync_bitrate(&mut qos);
        assert!(!qos.can_reduce_bitrate());
    }
    open(&mut qos, 1);
    if company {
        open(&mut qos, 2);
    }
    let no_tick = |step: Step| match step {
        Step::Tick(_) => Step::Wait(1000),
        step => step,
    };
    let mut targets = Vec::new();
    for step_no in 0..STEPS {
        if company {
            for _ in 0..others.rng.below(3) {
                let step = no_tick(others.step());
                apply(&mut qos, step);
            }
            if step_no == STEPS / 3 {
                open(&mut qos, 3);
                others.ids.push(3);
            }
            if step_no == 2 * STEPS / 3 {
                qos.on_connection_close(3);
                others.ids.pop();
            }
        }
        let step = no_tick(own.step());
        apply(&mut qos, step);
        targets.push(target(&qos, 1));
    }
    targets
}

// Invariant 1: another viewer's replies, timeouts, limits, joins and leaves do
// not change a viewer's private target.  With ABR off the bitrate is fixed; with
// ABR on the shared bitrate state is a designed input, so the property is checked
// at the bitrate floor, where it can no longer change.
#[test]
fn a_viewers_target_is_independent_of_other_viewers() {
    for seed in 0..SEEDS {
        for (abr, at_floor) in [(false, false), (true, true)] {
            let alone = viewer_one_targets(seed, false, abr, at_floor);
            let with_company = viewer_one_targets(seed, true, abr, at_floor);
            assert_eq!(
                alone, with_company,
                "seed {seed} abr {abr}: viewer 1's targets differ with company"
            );
        }
    }
}

// Invariant 3: a join adds a constraint, a leave removes it, and neither touches
// another viewer's state.
#[test]
fn joins_and_leaves_only_change_the_aggregation() {
    for seed in 0..SEEDS {
        for abr in [false, true] {
            let mut qos = session(abr);
            open(&mut qos, 1);
            open(&mut qos, 2);
            let mut driver = Driver::new(seed, vec![1, 2]);
            let mut next_id = 3;
            let mut present: Vec<i32> = Vec::new();
            for step_no in 0..STEPS {
                let step = driver.step();
                apply(&mut qos, step);
                if driver.rng.chance(10) {
                    let others: Vec<i32> = qos.users.keys().copied().collect();
                    let before: Vec<String> = others.iter().map(|id| snapshot(&qos, *id)).collect();
                    qos.adjust_fps();
                    let stream = qos.fps();
                    open(&mut qos, next_id);
                    present.push(next_id);
                    driver.ids.push(next_id);
                    next_id += 1;
                    qos.adjust_fps();
                    assert!(
                        qos.fps() <= stream,
                        "seed {seed} abr {abr} step {step_no}: a join raised the stream {stream} -> {}",
                        qos.fps()
                    );
                    assert_eq!(qos.fps(), expected_stream(&qos));
                    let after: Vec<String> = others.iter().map(|id| snapshot(&qos, *id)).collect();
                    assert_eq!(
                        before, after,
                        "seed {seed} abr {abr} step {step_no}: a join changed a viewer"
                    );
                } else if !present.is_empty() && driver.rng.chance(10) {
                    let leaving = present.remove(driver.rng.below(present.len() as u64) as usize);
                    driver.ids.retain(|id| *id != leaving);
                    driver.outstanding.remove(&leaving);
                    let others: Vec<i32> = qos.users.keys().copied().filter(|id| *id != leaving).collect();
                    let before: Vec<String> = others.iter().map(|id| snapshot(&qos, *id)).collect();
                    qos.on_connection_close(leaving);
                    let after: Vec<String> = others.iter().map(|id| snapshot(&qos, *id)).collect();
                    assert_eq!(
                        before, after,
                        "seed {seed} abr {abr} step {step_no}: a leave changed a viewer"
                    );
                    assert_eq!(
                        qos.fps(),
                        expected_stream(&qos),
                        "seed {seed} abr {abr} step {step_no}: the stream after a leave"
                    );
                }
            }
        }
    }
}

// Invariant 4: the ratio comes down only when a viewer's own evidence asks for
// it, by that viewer's own step, and a newcomer's first reply does not spend the
// evidence again.
#[test]
fn a_bitrate_cut_is_owned_by_a_viewers_evidence() {
    let mut cuts = 0;
    for seed in 0..SEEDS {
        let mut qos = session(true);
        let ids: Vec<i32> = (1..=1 + (seed % 3) as i32).collect();
        for id in &ids {
            open(&mut qos, *id);
        }
        let mut driver = Driver::new(seed, ids);
        for step_no in 0..STEPS {
            let step = driver.step();
            let before = qos.ratio();
            apply(&mut qos, step);
            let after = qos.ratio();
            if after >= before {
                continue;
            }
            cuts += 1;
            let asked: Vec<f32> = qos
                .users
                .values()
                .filter_map(|u| u.delay.ratio_reduction())
                .collect();
            assert!(
                !asked.is_empty(),
                "seed {seed} step {step_no} {step:?}: a cut nobody asked for"
            );
            let deepest = asked.iter().copied().fold(1.0_f32, f32::min);
            assert!(
                after >= before * deepest * 0.999,
                "seed {seed} step {step_no} {step:?}: cut {before} -> {after}, deepest step asked {deepest}"
            );
            // A newcomer replying inside the cooldown finds the evidence spent.
            open(&mut qos, 99);
            qos.advance_ms(driver.rng.below(2900));
            qos.user_network_delay(99, driver.base_rtt);
            assert_eq!(
                qos.ratio(),
                after,
                "seed {seed} step {step_no}: a newcomer's first reply spent the evidence again"
            );
            qos.on_connection_close(99);
        }
    }
    assert!(cuts > SEEDS as usize, "only {cuts} cuts across {SEEDS} sessions");
}

// Invariant 5: a reply leaves the target within its cap, and the stream is the
// aggregation of targets, caps and start-up guards after every decision.
#[test]
fn targets_stay_within_caps_and_the_stream_is_their_aggregation() {
    for seed in 0..SEEDS {
        for abr in [false, true] {
            let mut qos = session(abr);
            let ids: Vec<i32> = (1..=1 + (seed % 3) as i32).collect();
            for id in &ids {
                open(&mut qos, *id);
            }
            let mut driver = Driver::new(seed, ids);
            for step_no in 0..STEPS {
                let step = driver.step();
                apply(&mut qos, step);
                match step {
                    Step::Reply { id, .. } => {
                        let cap = qos.users[&id].fps_cap();
                        let t = target(&qos, id);
                        assert!(
                            (MIN_FPS..=cap).contains(&t),
                            "seed {seed} abr {abr} step {step_no} {step:?}: target {t} outside [{MIN_FPS}, {cap}]"
                        );
                    }
                    Step::Wait(_) | Step::Cap { .. } => continue,
                    _ => {}
                }
                assert_eq!(
                    qos.fps(),
                    expected_stream(&qos),
                    "seed {seed} abr {abr} step {step_no} {step:?}: the stream is not the aggregation"
                );
            }
        }
    }
}
