use super::*;

fn learned_baseline(delay: u32) -> RttCalculator {
    let mut rtt = RttCalculator::default();
    for _ in 0..90 {
        rtt.update(delay);
    }
    rtt
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
