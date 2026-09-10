use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
};
use winapi::{
    shared::{minwindef::FALSE, winerror::WAIT_TIMEOUT},
    um::{
        synchapi::{CreateEventW, SetEvent, WaitForSingleObject},
        winbase::{WAIT_FAILED, WAIT_OBJECT_0},
    },
};

pub(super) struct CaptureWake {
    event: OwnedHandle,
}

impl CaptureWake {
    pub(super) fn new() -> io::Result<Self> {
        let event = unsafe { CreateEventW(std::ptr::null_mut(), FALSE, FALSE, std::ptr::null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: CreateEventW returned a valid, exclusively owned event handle.
        Ok(Self {
            event: unsafe { OwnedHandle::from_raw_handle(event as _) },
        })
    }

    pub(super) fn notify(&self) -> io::Result<()> {
        // SetEvent retains a signal without a waiter. Rust 1.75's Windows 7
        // Thread::unpark instead waits for the worker via keyed events.
        if unsafe { SetEvent(self.event.as_raw_handle() as _) } == FALSE {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn wait(&self, milliseconds: u32) -> io::Result<()> {
        match unsafe { WaitForSingleObject(self.event.as_raw_handle() as _, milliseconds) } {
            WAIT_OBJECT_0 | WAIT_TIMEOUT => Ok(()),
            WAIT_FAILED => Err(io::Error::last_os_error()),
            result => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Unexpected audio capture wait result: {result}"),
            )),
        }
    }
}

impl super::CapturePcmHandoff {
    pub(super) fn notify(&self) {
        if let Err(error) = self.wake.notify() {
            hbb_common::log::error!("Failed to notify audio capture worker: {error}");
        }
    }
}

impl super::CapturePcmSender {
    pub(super) fn wake(&self) {
        self.handoff.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::super::{new_pcm_handoff, CaptureEncoderWorker};
    use super::*;
    use crate::audio_resampler::allocation_tests::assert_no_allocations;
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc,
        },
        time::Duration,
    };

    const PACKET_SAMPLES: usize = 4;
    const HANDOFF_CAPACITY: usize = 1;
    const TEST_TIMEOUT: Duration = Duration::from_secs(2);
    const TEST_TIMEOUT_MILLIS: u32 = TEST_TIMEOUT.as_millis() as u32;
    const SAMPLE_VALUE: f32 = 0.25;
    const PACKET: [f32; PACKET_SAMPLES] = [SAMPLE_VALUE; PACKET_SAMPLES];

    #[derive(Clone, Copy, Debug)]
    enum Submission {
        Queued,
        Oversized,
        Contended,
    }

    fn wait_result(wake: &CaptureWake, milliseconds: u32) -> u32 {
        unsafe { WaitForSingleObject(wake.event.as_raw_handle() as _, milliseconds) }
    }

    #[test]
    fn callback_notification_precedes_wait_and_retains_coalesced_signals() {
        for submission in [
            Submission::Queued,
            Submission::Oversized,
            Submission::Contended,
        ] {
            let (mut sender, receiver) = new_pcm_handoff(HANDOFF_CAPACITY, PACKET_SAMPLES).unwrap();
            assert!(receiver.is_empty());
            let held = matches!(submission, Submission::Contended)
                .then(|| receiver.handoff.buffers.lock().unwrap());
            let (completed_tx, completed_rx) = mpsc::channel();
            let callback = std::thread::spawn(move || {
                assert_no_allocations(|| match submission {
                    Submission::Oversized => sender.submit(&[SAMPLE_VALUE; PACKET_SAMPLES + 1]),
                    _ => sender.submit(&PACKET),
                });
                completed_tx.send(()).unwrap();
            });
            assert!(
                completed_rx.recv_timeout(TEST_TIMEOUT).is_ok(),
                "{submission:?} notification waited for the receiver"
            );
            callback.join().unwrap();
            drop(held);
            let wake = &receiver.handoff.wake;
            assert_eq!(wait_result(wake, 0), WAIT_OBJECT_0);
            assert_no_allocations(|| {
                wake.notify().unwrap();
                wake.notify().unwrap();
            });
            assert_eq!(wait_result(wake, 0), WAIT_OBJECT_0);
            assert_eq!(wait_result(wake, 0), WAIT_TIMEOUT);
            wake.notify().unwrap();
            wake.wait(TEST_TIMEOUT_MILLIS).unwrap();
            assert_eq!(wait_result(wake, 0), WAIT_TIMEOUT);
        }
    }

    #[test]
    fn shutdown_wakes_an_idle_capture_worker() {
        let (sender, receiver) = new_pcm_handoff(HANDOFF_CAPACITY, PACKET_SAMPLES).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (completed_tx, completed_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            entered_tx.send(()).unwrap();
            let result = wait_result(&receiver.handoff.wake, TEST_TIMEOUT_MILLIS);
            completed_tx
                .send((result, worker_stop.load(Ordering::Acquire)))
                .unwrap();
        });
        let worker = CaptureEncoderWorker {
            stop,
            handle: Some(handle),
            handoff: sender.handoff.clone(),
        };
        entered_rx.recv_timeout(TEST_TIMEOUT).unwrap();
        drop(worker);
        assert_eq!(
            completed_rx.recv_timeout(TEST_TIMEOUT).unwrap(),
            (WAIT_OBJECT_0, true)
        );
    }
}
