use super::*;

fn session(cap: u32, abr: bool) -> VideoQoS {
    let mut qos = super::smoke::session(cap, Quality::Balanced);
    let joined = qos.now();
    qos.users.get_mut(&1).unwrap().joined_at = Some(joined);
    qos.abr_config = abr;
    qos.new_display("startup".to_owned());
    qos.set_support_changing_quality("startup", true);
    qos
}

fn reply(qos: &mut VideoQoS, delay: u32) {
    qos.user_network_delay(1, delay);
    let ratio = qos.ratio();
    qos.store_bitrate((6000.0 * ratio) as u32);
    qos.update_display_data("startup", qos.fps() as usize);
    qos.advance_ms(1000);
}

#[test]
fn clean_startup_reaches_the_cap_without_waiting_for_slow_growth() {
    println!("| cap | base RTT ms | ABR | replies to cap | FPS after each reply |");
    println!("|---|---|---|---|---|");
    for (cap, budget) in [(3, 1), (15, 1), (30, 2), (60, 4), (120, 6)] {
        for base in [10, 150, 300, 600] {
            for abr in [false, true] {
                let mut qos = session(cap, abr);
                qos.advance_ms(base as u64);
                let mut trace = Vec::new();
                for n in 1..=budget {
                    reply(&mut qos, base);
                    let fps = qos.fps();
                    assert!(fps <= cap);
                    if n == 1 {
                        assert!(fps <= INIT_FPS.min(cap), "keep the first-second guard");
                    }
                    trace.push(fps);
                }
                println!("| {cap} | {base} | {abr} | {budget} | {trace:?} |");
                assert_eq!(qos.fps(), cap, "base={base} ABR={abr}: {trace:?}");
                assert_eq!(qos.ratio(), Quality::Balanced.ratio());
            }
        }
    }
}

#[test]
fn an_unclean_startup_reply_disables_faster_growth() {
    for excess in [50, 149, 150, 400] {
        let mut qos = session(120, false);
        reply(&mut qos, 10);
        reply(&mut qos, 10 + excess);
        for _ in 0..15 {
            let before = qos.fps();
            reply(&mut qos, 10);
            assert!(
                qos.fps() <= before + (before / 5).max(6),
                "excess={excess}: startup acceleration restarted: {before} -> {}",
                qos.fps()
            );
        }
    }
}

#[test]
fn constrained_startup_does_not_leave_a_large_queue() {
    use super::sim::{self, EncoderModel};
    for cap in [30, 60, 120] {
        for encoder in [EncoderModel::Cbr, EncoderModel::FixedRate] {
            for seed in sim::SEEDS {
                let mut sc = sim::scenarios()
                    .into_iter()
                    .find(|sc| sc.name == "bandwidth_halved_30")
                    .unwrap();
                sc.limit = cap;
                sc.encoder = encoder;
                sc.seed = seed;
                sc.link.capacity_kbps = vec![(0, 2500.0)];
                let report = sim::run(&sc);
                let startup_queue = report
                    .trace
                    .iter()
                    .filter(|(t, ..)| *t < 15_000)
                    .map(|(_, _, queue, _)| *queue)
                    .max()
                    .unwrap();
                println!("startup limited cap={cap} seed={seed} frame_budget={} peak_queue_ms={startup_queue} queue_p95_ms={} age_p95_ms={}", encoder == EncoderModel::FixedRate, report.queue_p95_ms, report.frame_age_p95_ms);
                assert!(
                    startup_queue < 4000,
                    "cap={cap} seed={seed}: {startup_queue}"
                );
                assert!(report.queue_p95_ms < 3000);
                assert!(report.frame_age_p95_ms < 3000);
                assert!(report
                    .trace
                    .iter()
                    .all(|(_, fps, _, _)| (5..=cap).contains(fps)));
            }
        }
    }
}
