use std::time::{Duration, Instant};

pub(crate) struct Window<const N: usize> {
    started: Instant,
    samples: u64,
    completed: u64,
    total: [Duration; N],
    maximum: [Duration; N],
}

pub(crate) struct Summary<const N: usize> {
    pub samples: u64,
    pub seconds: f64,
    pub completed_per_second: f64,
    pub average_ms: [f64; N],
    pub maximum_ms: [f64; N],
}

impl<const N: usize> Window<N> {
    pub fn new(now: Instant) -> Self {
        Self {
            started: now,
            samples: 0,
            completed: 0,
            total: [Duration::ZERO; N],
            maximum: [Duration::ZERO; N],
        }
    }

    pub fn record(
        &mut self,
        now: Instant,
        stages: [Duration; N],
        completed: bool,
    ) -> Option<Summary<N>> {
        self.samples += 1;
        self.completed += u64::from(completed);
        for (index, elapsed) in stages.into_iter().enumerate() {
            self.total[index] += elapsed;
            self.maximum[index] = self.maximum[index].max(elapsed);
        }
        let seconds = now.duration_since(self.started).as_secs_f64();
        if seconds < 1.0 {
            return None;
        }
        let summary = Summary {
            samples: self.samples,
            seconds,
            completed_per_second: self.completed as f64 / seconds,
            average_ms: self
                .total
                .map(|total| total.as_secs_f64() * 1000.0 / self.samples as f64),
            maximum_ms: self.maximum.map(|maximum| maximum.as_secs_f64() * 1000.0),
        };
        *self = Self::new(now);
        Some(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_stage_averages_peaks_and_completed_rate_once_per_window() {
        let start = Instant::now();
        let mut window = Window::<2>::new(start);
        assert!(window
            .record(
                start + Duration::from_millis(500),
                [Duration::from_millis(4), Duration::from_millis(10)],
                true,
            )
            .is_none());
        let summary = window
            .record(
                start + Duration::from_secs(1),
                [Duration::from_millis(8), Duration::from_millis(2)],
                false,
            )
            .unwrap();
        assert_eq!(summary.samples, 2);
        assert_eq!(summary.seconds, 1.0);
        assert_eq!(summary.completed_per_second, 1.0);
        assert_eq!(summary.average_ms, [6.0, 6.0]);
        assert_eq!(summary.maximum_ms, [8.0, 10.0]);
        let next = window
            .record(start + Duration::from_secs(2), [Duration::ZERO; 2], true)
            .unwrap();
        assert_eq!(next.samples, 1);
        assert_eq!(next.completed_per_second, 1.0);
        assert_eq!(next.average_ms, [0.0; 2]);
        assert_eq!(next.maximum_ms, [0.0; 2]);
    }

    #[test]
    fn sparse_samples_use_elapsed_wall_time_without_catch_up_reports() {
        let start = Instant::now();
        let mut window = Window::<1>::new(start);
        let now = start + Duration::from_secs(4);
        let summary = window
            .record(now, [Duration::from_millis(8)], true)
            .unwrap();
        assert_eq!(summary.seconds, 4.0);
        assert_eq!(summary.completed_per_second, 0.25);
        assert_eq!(summary.average_ms, [8.0]);
        assert!(window.record(now, [Duration::ZERO], true).is_none());
    }
}
