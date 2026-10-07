use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// Bytes received from the peer, counted before framing.
///
/// A message reaches the application only once it is complete, so a large one — a clipboard image
/// is the case that occurs in practice — leaves the receive loop with nothing to observe for as
/// long as the transfer takes, and a peer that is sending steadily looks identical to one that
/// died. This counter moves on every read, so liveness stays observable across such a message.
///
/// A counter rather than a timestamp on purpose: `Instant::now()` costs an order of magnitude more
/// than a relaxed add, and the reader only ever compares successive samples, so the clock is
/// needed once per sampling tick instead of once per read.
#[derive(Clone, Default)]
pub struct RxProbe(Arc<AtomicU64>);

impl RxProbe {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn add(&self, n: usize) {
        self.0.fetch_add(n as u64, Ordering::Relaxed);
    }

    /// Changes whenever bytes arrived. Compare successive samples; the absolute value and its
    /// wrap-around carry no meaning.
    #[inline]
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_one_counter() {
        let probe = RxProbe::new();
        let cloned = probe.clone();
        assert_eq!(probe.get(), 0);
        cloned.add(7);
        assert_eq!(probe.get(), 7);
        probe.add(3);
        assert_eq!(cloned.get(), 10);
    }
}
