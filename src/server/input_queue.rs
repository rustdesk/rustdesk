use std::{
    sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError},
    time::Duration,
};

// Bound each scan so a producer that never stops cannot starve input delivery.
const MAX_SCAN: usize = 256;

pub(super) struct LatestReceiver<T, F> {
    receiver: Receiver<T>,
    pending: Option<T>,
    can_replace: F,
}

impl<T, F: Fn(&T, &T) -> bool> LatestReceiver<T, F> {
    pub(super) fn new(receiver: Receiver<T>, can_replace: F) -> Self {
        Self {
            receiver,
            pending: None,
            can_replace,
        }
    }

    pub(super) fn recv_timeout(&mut self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        let mut current = match self.pending.take() {
            Some(value) => value,
            None => self.receiver.recv_timeout(timeout)?,
        };
        for _ in 0..MAX_SCAN {
            match self.receiver.try_recv() {
                Ok(next) if (self.can_replace)(&current, &next) => current = next,
                Ok(next) => {
                    // Keep the first barrier for the next call; never reorder a release/key.
                    self.pending = Some(next);
                    break;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn backlog_delivers_latest_position_before_release() {
        let (tx, rx) = channel();
        for x in 0..200 {
            tx.send((0, x)).unwrap();
        }
        tx.send((1, 199)).unwrap();
        drop(tx);
        let mut rx = LatestReceiver::new(rx, |a, b| a.0 == 0 && b.0 == 0);
        assert_eq!(rx.recv_timeout(Duration::ZERO).unwrap(), (0, 199));
        assert_eq!(rx.recv_timeout(Duration::ZERO).unwrap(), (1, 199));
        assert_eq!(
            rx.recv_timeout(Duration::ZERO),
            Err(RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn preserves_buttons_keys_wheel_relative_and_context_changes() {
        // Kind 0 is absolute movement. Other kinds must stay in their original order.
        let input = [
            (1, 0, 0),
            (0, 1, 0),
            (0, 2, 0),
            (2, 2, 0),
            (3, 65, 0),
            (4, 1, 0),
            (5, 7, 0),
            (5, 8, 0),
            (0, 3, 0),
            (0, 4, 1),
            (0, 5, 1),
            (2, 5, 1),
        ];
        let (tx, rx) = channel();
        for event in input {
            tx.send(event).unwrap();
        }
        drop(tx);
        let mut rx = LatestReceiver::new(rx, |a, b| a.0 == 0 && b.0 == 0 && a.2 == b.2);
        let mut output = Vec::new();
        while let Ok(event) = rx.recv_timeout(Duration::ZERO) {
            output.push(event);
        }
        assert_eq!(
            output,
            [
                (1, 0, 0),
                (0, 2, 0),
                (2, 2, 0),
                (3, 65, 0),
                (4, 1, 0),
                (5, 7, 0),
                (5, 8, 0),
                (0, 3, 0),
                (0, 5, 1),
                (2, 5, 1),
            ]
        );
    }

    #[test]
    fn long_burst_keeps_progress_and_final_release() {
        let (tx, rx) = channel();
        for x in 0..10_000 {
            tx.send((0, x)).unwrap();
        }
        tx.send((1, 9_999)).unwrap();
        let mut rx = LatestReceiver::new(rx, |a, b| a.0 == 0 && b.0 == 0);
        let first = rx.recv_timeout(Duration::ZERO).unwrap();
        assert!(first.1 > 0 && first.1 < 10_000);
        let mut last_move = first;
        loop {
            let event = rx.recv_timeout(Duration::ZERO).unwrap();
            if event.0 == 1 {
                assert_eq!(last_move, (0, 9_999));
                assert_eq!(event, (1, 9_999));
                break;
            }
            last_move = event;
        }
        assert_eq!(
            rx.recv_timeout(Duration::ZERO),
            Err(RecvTimeoutError::Timeout)
        );
    }
}
