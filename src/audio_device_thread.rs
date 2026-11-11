//! One process-lifetime thread for audio device queries (CPAL device enumeration).
//!
//! Problem: on Windows, CPAL keeps one process-global `IMMDeviceEnumerator` (`ENUMERATOR`, a `OnceLock` in
//! `cpal::host::wasapi::device`), created on the first thread that queries a device. That thread's COM apartment is
//! initialized by a CPAL `thread_local!` guard that calls `CoUninitialize()` when the thread exits. When a short-lived
//! thread is the first to query and no other thread of the process holds COM initialized (the `--server` process),
//! its exit unloads `MMDevAPI.dll`; the next `default_output_device()` calls through the stale vtable and the process
//! ends with `0xc0000005`.
//!
//! Solution: every device query that does not run on a thread living as long as the process MUST go through `run()`,
//! which executes it on the `audio-device-query` thread. That thread never exits, so its COM apartment, and an
//! `ENUMERATOR` created on it, stay valid for the life of the process.

use hbb_common::log;
use std::{
    sync::{mpsc, Mutex},
    time::Duration,
};

type Job = Box<dyn FnOnce() + Send>;

/// Longest wait for one query; a device driver that hangs MUST NOT block the caller indefinitely.
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);

lazy_static::lazy_static! {
    static ref JOBS: Mutex<Option<mpsc::Sender<Job>>> = Mutex::new(None);
}

fn sender() -> Option<mpsc::Sender<Job>> {
    let mut lock = JOBS.lock().unwrap();
    if lock.is_none() {
        let (tx, rx) = mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("audio-device-query".to_owned())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    job();
                }
            });
        match spawned {
            Ok(_) => *lock = Some(tx),
            Err(e) => {
                log::error!("Failed to start the audio device query thread: {e}");
                return None;
            }
        }
    }
    lock.clone()
}

/// Runs `f` on the `audio-device-query` thread and returns its result; `None` when the thread is unavailable or `f`
/// does not finish within `QUERY_TIMEOUT`.
pub fn run<R, F>(f: F) -> Option<R>
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let (result_tx, result_rx) = mpsc::sync_channel::<R>(1);
    let job: Job = Box::new(move || {
        let _ = result_tx.send(f());
    });
    sender()?.send(job).ok()?;
    match result_rx.recv_timeout(QUERY_TIMEOUT) {
        Ok(r) => Some(r),
        Err(e) => {
            log::error!("Audio device query failed: {e}");
            None
        }
    }
}
