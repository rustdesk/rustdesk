use super::*;

pub(super) fn session(fps: u32, quality: Quality) -> VideoQoS {
    let mut qos = VideoQoS {
        fps: INIT_FPS.min(fps),
        abr_config: false,
        ..Default::default()
    };
    qos.advance_ms(2000);
    qos.users.insert(
        1,
        UserData {
            custom_fps: Some(fps),
            quality: Some((0, quality)),
            ..Default::default()
        },
    );
    qos
}

#[test]
fn smoke_capacity_with_short_stalls() {
    for limit in [30, 60] {
        // Probes run at one second cadence; sweep the stall phase so no alignment hides.
        for phase in (0..1000).step_by(50) {
            let mut qos = session(limit, Quality::Balanced);
            for _ in 0..90 {
                qos.user_network_delay(1, 10);
            }
            let capacity = limit as f64 * 2.0;
            let mut queue = 0.0_f64;
            let mut probe: Option<(u32, f64, Option<u32>)> = None;
            let mut max_delay = 0;
            for now in (0..120_000).step_by(10) {
                qos.advance_ms(10);
                // A 700 ms pause affects video and probes on the same FIFO link.
                let available = if (now + phase) % 6000 < 700 {
                    0.0
                } else {
                    capacity
                };
                queue = (queue + (qos.fps() as f64 - available) * 0.01).max(0.0);
                if let Some((_, remaining, reply_at)) = probe.as_mut() {
                    *remaining -= available * 0.01;
                    if available > 0.0 && *remaining <= 0.0 && reply_at.is_none() {
                        *reply_at = Some(now + 10);
                    }
                }
                if let Some((sent, _, Some(reply_at))) = probe {
                    if now >= reply_at {
                        max_delay = max_delay.max(now - sent);
                        qos.user_network_delay(1, now - sent);
                        probe = None;
                    }
                }
                if now % 1000 == 0 {
                    if probe.is_none() {
                        probe = Some((now, queue, None));
                    }
                    qos.user_delay_response_elapsed(1, (now - probe.unwrap().0) as u128);
                }
                assert_eq!(qos.fps(), limit, "limit={limit}, phase={phase}, time={now}");
            }
            println!("healthy FIFO link: limit={limit}, phase={phase}, max_delay_ms={max_delay}, final_fps={}", qos.fps());
        }
    }
}
