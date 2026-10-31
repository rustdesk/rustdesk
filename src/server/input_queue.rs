#[cfg(not(any(target_os = "android", target_os = "ios")))]
use super::MessageInput;
use std::{
    sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError},
    time::Duration,
};

// Bound each scan so a producer that never stops cannot starve input delivery.
const MAX_SCAN: usize = 256;

/// Keeps Windows mouse coalescing within the same input context and movement type.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) fn coalesce_mouse_moves(
    receiver: Receiver<MessageInput>,
) -> LatestReceiver<MessageInput, impl Fn(&MessageInput, &MessageInput) -> bool> {
    LatestReceiver::new(receiver, |current, next| match (current, next) {
        (MessageInput::Mouse(current), MessageInput::Mouse(next)) => {
            current.simulate
                && next.simulate
                && !current.show_cursor
                && !next.show_cursor
                && current.conn_id == next.conn_id
                && current.username == next.username
                && current.argb == next.argb
                && current.msg.mask & crate::input::MOUSE_TYPE_MASK == crate::input::MOUSE_TYPE_MOVE
                && current.msg.mask == next.msg.mask
                && current.msg.modifiers == next.msg.modifiers
        }
        _ => false,
    })
}

pub(super) struct LatestReceiver<T, F> {
    receiver: Receiver<T>,
    pending: Option<T>,
    can_replace: F,
}

impl<T, F: Fn(&T, &T) -> bool> LatestReceiver<T, F> {
    /// `can_replace` must accept only events where `next` supersedes `current`.
    pub(super) fn new(receiver: Receiver<T>, can_replace: F) -> Self {
        Self {
            receiver,
            pending: None,
            can_replace,
        }
    }

    /// Waits only for the first event, then scans queued events without blocking.
    /// The first non-replaceable event is retained for the next call.
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

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn real_mouse_inputs_coalesce_without_crossing_barriers() {
        use super::super::InputMouse;
        use crate::input::*;
        use base::message_proto::{ControlKey, MouseEvent};

        let mouse = |x| InputMouse {
            msg: MouseEvent {
                x,
                mask: MOUSE_TYPE_MOVE,
                ..Default::default()
            },
            conn_id: 1,
            username: "test".into(),
            argb: 0,
            simulate: true,
            show_cursor: false,
        };
        let changes: &[fn(&mut InputMouse)] = &[
            |_| {},
            |m| m.msg.mask = MOUSE_TYPE_DOWN,
            |m| m.msg.mask = MOUSE_TYPE_UP,
            |m| m.msg.mask = MOUSE_TYPE_WHEEL,
            |m| m.msg.mask = MOUSE_TYPE_MOVE_RELATIVE,
            |m| m.simulate = false,
            |m| m.show_cursor = true,
            |m| m.conn_id += 1,
            |m| m.username.push('2'),
            |m| m.argb = 1,
            |m| m.msg.mask |= MOUSE_BUTTON_LEFT << 3,
            |m| m.msg.modifiers.push(ControlKey::Shift.into()),
        ];
        for (case, change) in changes.iter().enumerate() {
            for change_first in [false, true] {
                let mut first = mouse(1);
                let mut second = mouse(2);
                change(if change_first {
                    &mut first
                } else {
                    &mut second
                });
                let (tx, rx) = channel();
                tx.send(MessageInput::Mouse(first)).unwrap();
                tx.send(MessageInput::Mouse(second)).unwrap();
                drop(tx);
                let mut rx = coalesce_mouse_moves(rx);
                let mut positions = Vec::new();
                while let Ok(MessageInput::Mouse(event)) = rx.recv_timeout(Duration::ZERO) {
                    positions.push(event.msg.x);
                }
                let expected = if case == 0 { vec![2] } else { vec![1, 2] };
                assert_eq!(
                    positions, expected,
                    "case {case}, change_first {change_first}"
                );
            }
        }
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
