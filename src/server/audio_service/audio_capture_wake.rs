use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    time::Duration,
};
use winapi::{
    shared::{minwindef::FALSE, winerror::WAIT_TIMEOUT},
    um::{
        synchapi::{CreateEventW, SetEvent, WaitForSingleObject},
        winbase::{INFINITE, WAIT_FAILED, WAIT_OBJECT_0},
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

    pub(super) fn wait(&self, timeout: Duration) -> io::Result<()> {
        let milliseconds = u32::try_from(timeout.as_millis())
            .ok()
            .filter(|milliseconds| *milliseconds != INFINITE)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Audio capture wait timeout is too large",
                )
            })?;
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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
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

    fn assert_notification_before_native_wait(submission: Submission) {
        let (mut sender, receiver) = new_pcm_handoff(HANDOFF_CAPACITY, PACKET_SAMPLES).unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            assert!(receiver.is_empty());
            let held = matches!(submission, Submission::Contended)
                .then(|| receiver.handoff.buffers.lock().unwrap());
            entered_tx.send(()).unwrap();
            resume_rx.recv().unwrap();
            drop(held);
            unsafe { WaitForSingleObject(receiver.handoff.wake.event.as_raw_handle() as _, 0) }
        });
        entered_rx.recv_timeout(TEST_TIMEOUT).unwrap();
        let (completed_tx, completed_rx) = mpsc::channel();
        let callback = std::thread::spawn(move || {
            assert_no_allocations(|| match submission {
                Submission::Oversized => sender.submit(&[SAMPLE_VALUE; PACKET_SAMPLES + 1]),
                _ => sender.submit(&PACKET),
            });
            completed_tx.send(()).unwrap();
        });
        let completed = completed_rx.recv_timeout(TEST_TIMEOUT);
        resume_tx.send(()).unwrap();
        callback.join().unwrap();
        let wait_result = worker.join().unwrap();
        assert!(
            completed.is_ok(),
            "{submission:?} notification waited for the worker"
        );
        assert_eq!(
            wait_result, WAIT_OBJECT_0,
            "{submission:?} notification was lost"
        );
    }

    #[test]
    fn callback_notification_finishes_before_worker_enters_native_wait() {
        for submission in [
            Submission::Queued,
            Submission::Oversized,
            Submission::Contended,
        ] {
            assert_notification_before_native_wait(submission);
        }
    }

    #[test]
    fn repeated_notifications_retain_one_signal_until_consumed() {
        let wake = CaptureWake::new().unwrap();
        assert_no_allocations(|| {
            wake.notify().unwrap();
            wake.notify().unwrap();
        });
        assert_eq!(
            unsafe { WaitForSingleObject(wake.event.as_raw_handle() as _, 0) },
            WAIT_OBJECT_0
        );
        assert_eq!(
            unsafe { WaitForSingleObject(wake.event.as_raw_handle() as _, 0) },
            WAIT_TIMEOUT
        );
        wake.notify().unwrap();
        wake.wait(TEST_TIMEOUT).unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(wake.event.as_raw_handle() as _, 0) },
            WAIT_TIMEOUT
        );
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
            let result = unsafe {
                WaitForSingleObject(
                    receiver.handoff.wake.event.as_raw_handle() as _,
                    TEST_TIMEOUT_MILLIS,
                )
            };
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
