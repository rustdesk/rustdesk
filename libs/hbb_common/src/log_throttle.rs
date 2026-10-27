use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Collapses a log site whose call rate is set by someone else — a peer's message rate, or a
/// retry loop — into at most one line per interval.
///
/// Debug output is written to the log file, so a site that fires per received packet lets a
/// peer decide how much a machine writes to disk. Dropping the line entirely instead would
/// hide real faults, so keep one line per interval and carry the count of everything
/// suppressed since the last one.
///
/// Prefer the [`throttled_log!`](crate::throttled_log) macro, which declares the static for
/// you. Reach for this type directly only when the count belongs somewhere other than the end
/// of the line, or when the decision drives more than a log call.
///
/// Declare one per site (they do not share counts):
///
/// ```ignore
/// static DROPPED_ICE: LogThrottle = LogThrottle::new(Duration::from_secs(60));
///
/// if let Some(n) = DROPPED_ICE.due() {
///     log::debug!("dropped {n} ICE candidate(s) with no route");
/// }
/// ```
pub struct LogThrottle {
    interval: Duration,
    state: Mutex<ThrottleState>,
}

struct ThrottleState {
    suppressed: u64,
    last: Option<Instant>,
}

impl LogThrottle {
    pub const fn new(interval: Duration) -> Self {
        Self {
            interval,
            state: Mutex::new(ThrottleState {
                suppressed: 0,
                last: None,
            }),
        }
    }

    /// Record one occurrence. Returns the number of occurrences to report (including this one)
    /// when a line is due, or `None` while still inside the interval.
    ///
    /// The first occurrence after a quiet period always reports, so an isolated fault is not
    /// delayed by the interval.
    pub fn due(&self) -> Option<u64> {
        // A poisoned lock only means some other thread panicked while holding it; the guarded
        // data is two plain counters that are still usable, and going silent for the rest of
        // the process would be worse than a stale count.
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.suppressed += 1;
        // `map_or(true, ..)` rather than clippy's preferred `is_none_or`: that was stabilized in
        // Rust 1.82 and this crate builds on the 1.75 pinned by CI.
        #[allow(clippy::unnecessary_map_or)]
        let due = state
            .last
            .map_or(true, |last| last.elapsed() >= self.interval);
        if !due {
            return None;
        }
        state.last = Some(Instant::now());
        Some(std::mem::replace(&mut state.suppressed, 0))
    }
}

/// Log at most one line per interval from this call site, suffixed with the number of
/// occurrences it stands for.
///
/// Each expansion declares its own hidden static, so two sites never share a count and
/// adding one is a single line:
///
/// ```ignore
/// throttled_log!(Duration::from_secs(5), warn, "rejected ipc peer {peer_pid:?}");
/// ```
///
/// An isolated event logs unchanged; a burst collapses to `... (x47)`. The count includes
/// the occurrence being reported, so it reads as a total rather than as "and N more".
#[macro_export]
macro_rules! throttled_log {
    ($interval:expr, $level:ident, $($arg:tt)+) => {{
        static THROTTLE: $crate::log_throttle::LogThrottle =
            $crate::log_throttle::LogThrottle::new($interval);
        if let Some(n) = THROTTLE.due() {
            if n > 1 {
                $crate::log::$level!("{} (x{})", format_args!($($arg)+), n);
            } else {
                $crate::log::$level!("{}", format_args!($($arg)+));
            }
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two directions of one socket need two throttles: an ICMP error on a connected socket is
    // reported once and cleared, so the steady state alternates (send succeeds, the next recv
    // reports it) and anything shared between them is reset by the succeeding side every cycle.
    #[test]
    fn separate_throttles_do_not_reset_each_other() {
        let send = LogThrottle::new(Duration::from_secs(60));
        let recv = LogThrottle::new(Duration::from_secs(60));
        assert_eq!(recv.due(), Some(1));
        for _ in 0..1_000 {
            // The send side succeeding must not hand the recv side a fresh emit slot.
            assert_eq!(recv.due(), None);
        }
        assert_eq!(
            send.due(),
            Some(1),
            "the other direction keeps its own slot"
        );
    }

    #[test]
    fn first_call_reports_immediately() {
        let t = LogThrottle::new(Duration::from_secs(60));
        assert_eq!(t.due(), Some(1));
    }

    #[test]
    fn calls_inside_the_interval_are_counted_not_reported() {
        let t = LogThrottle::new(Duration::from_secs(60));
        assert_eq!(t.due(), Some(1));
        for _ in 0..100 {
            assert_eq!(t.due(), None);
        }
    }

    #[test]
    fn the_next_due_line_carries_everything_suppressed() {
        let t = LogThrottle::new(Duration::ZERO);
        assert_eq!(t.due(), Some(1));
        // A zero interval is always due, so each call reports exactly itself.
        assert_eq!(t.due(), Some(1));

        let t = LogThrottle::new(Duration::from_millis(30));
        assert_eq!(t.due(), Some(1));
        assert_eq!(t.due(), None);
        assert_eq!(t.due(), None);
        std::thread::sleep(Duration::from_millis(40));
        // The two suppressed calls plus this one.
        assert_eq!(t.due(), Some(3));
    }
}
