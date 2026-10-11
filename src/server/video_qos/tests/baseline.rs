use super::*;

fn learned_baseline(delay: u32) -> RttCalculator {
    let mut rtt = RttCalculator::default();
    for _ in 0..90 {
        rtt.update(delay);
    }
    rtt
}

#[test]
fn old_minimum_expires_after_a_stable_path_change() {
    for (old, new) in [(10, 310), (10, 500), (159, 500)] {
        let mut rtt = learned_baseline(old);
        for i in 0..40 {
            let before = rtt.get_rtt().unwrap();
            rtt.update(new + i % 5 * 10);
            let after = rtt.get_rtt().unwrap();
            assert!(after <= before + 50, "limit the cost of relearning");
            if i < 10 {
                assert_eq!(after, old, "a short burst must not replace the baseline");
            }
        }
        assert_eq!(rtt.get_rtt(), Some(new), "{old} -> {new}");
        rtt.update(old);
        assert_eq!(rtt.get_rtt(), Some(old), "a lower delay is direct evidence");
    }
}

#[test]
fn rising_delay_is_not_learned_as_a_new_baseline() {
    for step in [2, 10, 50] {
        let mut rtt = learned_baseline(10);
        for i in 0..120 {
            rtt.update(200 + i * step);
            assert_eq!(rtt.get_rtt(), Some(10), "rising by {step} ms per reply");
        }
    }
}

#[test]
fn intermittent_spikes_do_not_raise_the_baseline() {
    let mut rtt = learned_baseline(159);
    for delay in [159, 900, 500, 159, 350].repeat(30) {
        rtt.update(delay);
        assert_eq!(rtt.get_rtt(), Some(159));
    }
}

#[test]
fn pending_probes_and_late_replies_do_not_age_the_baseline() {
    let mut qos = stable_qos();
    for elapsed in (2001..122_001).step_by(1000) {
        qos.user_delay_response_elapsed(1, elapsed);
        assert_eq!(qos.users[&1].delay.rtt_calculator.get_rtt(), Some(10));
    }
    qos.user_network_delay(1, 122_000);
    assert_eq!(qos.users[&1].delay.rtt_calculator.get_rtt(), Some(10));
    for _ in 0..30 {
        qos.user_delay_response_elapsed(1, 2500);
        qos.user_network_delay(1, 2600);
    }
    assert_eq!(qos.users[&1].delay.rtt_calculator.get_rtt(), Some(10));
    assert_eq!(qos.fps(), 5);
}

#[test]
fn stable_path_change_recovers_without_reconnecting() {
    println!("| ABR | path delay | seconds to 30 FPS | final baseline | final FPS |");
    println!("|---|---:|---:|---:|---:|");
    for abr in [false, true] {
        for delay in [310, 410, 500] {
            let mut qos = super::smoke::session(FPS, Quality::Balanced);
            qos.abr_config = abr;
            qos.new_display("baseline".to_owned());
            qos.set_support_changing_quality("baseline", true);
            let second = |qos: &mut VideoQoS, delay| {
                qos.advance_ms(1000);
                let bitrate = (6000.0 * qos.ratio()) as u32;
                qos.store_bitrate(bitrate);
                qos.user_network_delay(1, delay);
                let bitrate = (6000.0 * qos.ratio()) as u32;
                qos.store_bitrate(bitrate);
                qos.update_display_data("baseline", qos.fps() as usize);
            };
            for _ in 0..90 {
                second(&mut qos, 10);
            }
            let mut first_recovered = None;
            for s in 1..=90 {
                second(&mut qos, delay);
                if s >= 10 && qos.fps() == FPS && first_recovered.is_none() {
                    first_recovered = Some(s);
                }
                if s >= 40 {
                    assert_eq!(qos.fps(), FPS, "stay recovered: ABR={abr}, delay={delay}");
                }
            }
            let base = qos.users[&1].delay.rtt_calculator.get_rtt().unwrap();
            println!(
                "| {abr} | 10 -> {delay} ms | {first_recovered:?} | {base} | {} |",
                qos.fps()
            );
            assert!(first_recovered.is_some_and(|s| s <= 30));
            assert_eq!(base, delay);
            second(&mut qos, 10);
            assert_eq!(qos.users[&1].delay.rtt_calculator.get_rtt(), Some(10));
        }
    }
}
