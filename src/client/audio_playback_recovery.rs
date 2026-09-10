use super::{AudioBuffer, AudioDecoder, AudioFormat, AudioHandler, MediaData, Mono, Stereo};
use cpal::StreamError;
use crossbeam_queue::SegQueue;
use hbb_common::{log, tokio::time::Instant, ResultType};
use std::{
    sync::{atomic::Ordering, mpsc, Arc},
    time::Duration,
};

const RECOVERY_INTERVAL: Duration = Duration::from_secs(1);
// The pinned WASAPI backend reports this warning but keeps its worker running.
const PRIORITY_WARNING_PREFIX: &str = "SetThreadPriority failed: ";

#[cfg(test)]
#[path = "audio_playback_recovery_state_tests.rs"]
mod state_tests;

#[derive(Clone, Default)]
pub(super) struct PlaybackErrors(Arc<SegQueue<StreamError>>);

impl PlaybackErrors {
    pub(super) fn report(&self, error: StreamError) {
        self.0.push(error);
    }

    pub(super) fn pop(&self) -> Option<StreamError> {
        self.0.pop()
    }

    fn report_pending(&self) -> bool {
        let mut failed = false;
        while let Some(error) = self.pop() {
            if matches!(&error, StreamError::BackendSpecific { err }
                if err.description.starts_with(PRIORITY_WARNING_PREFIX))
            {
                log::warn!("Audio playback nonterminal priority warning: {error}");
            } else {
                log::error!("Audio playback stream failed: {error}");
                failed = true;
            }
        }
        failed
    }
}

#[derive(Default)]
pub(super) struct PlaybackRecovery {
    pub(super) errors: PlaybackErrors,
    format: Option<AudioFormat>,
    pub(super) retry_at: Option<Instant>,
    restart_not_before: Option<Instant>,
    awaiting_callback: bool,
}

impl AudioHandler {
    fn clear_playback_stream(&mut self) {
        // Dropping CPAL may join its worker; run this on the owner, not its callback.
        self.audio_stream = None;
        self.playback_recovery.errors.report_pending();
        self.playback_status.report_errors();
        self.playback_status = Default::default();
        self.audio_buffer = AudioBuffer::default();
        self.audio_resampler = None;
        self.audio_decoder = None;
        self.sample_rate = (0, 0);
        self.channels = 0;
        self.device_channel = 0;
        self.playback_recovery.errors = PlaybackErrors::default();
        self.playback_recovery.awaiting_callback = false;
    }

    pub(super) fn prepare_playback(&mut self, format: &AudioFormat) {
        self.clear_playback_stream();
        self.playback_recovery.format = Some(format.clone());
        self.playback_recovery.retry_at = None;
        self.playback_recovery.restart_not_before = None;
    }

    pub(super) fn finish_playback_start(&mut self, result: ResultType<()>) {
        let retry_at = Instant::now() + RECOVERY_INTERVAL;
        self.playback_recovery.restart_not_before = Some(retry_at);
        match result {
            Ok(()) => {
                self.playback_recovery.retry_at = None;
                self.playback_recovery.awaiting_callback = true;
                log::info!("Audio playback stream opened; waiting for output callback");
            }
            Err(error) => {
                self.clear_playback_stream();
                self.playback_recovery.retry_at = Some(retry_at);
                log::error!(
                    "Audio playback start failed: {error:#}; retrying in {RECOVERY_INTERVAL:?}"
                );
            }
        }
    }

    fn restart_playback(&mut self, format: AudioFormat) -> ResultType<()> {
        let channels = if format.channels > 1 { Stereo } else { Mono };
        let decoder = AudioDecoder::new(format.sample_rate, channels)?;
        let buffer = vec![0.; format.sample_rate as usize * format.channels as usize];
        let channel_count = format.channels as _;
        self.start_audio(format)?;
        self.channels = channel_count;
        self.audio_decoder = Some((decoder, buffer));
        Ok(())
    }

    pub(super) fn recover_playback_with(
        &mut self,
        now: Instant,
        restart: impl FnOnce(&mut Self, AudioFormat) -> ResultType<()>,
    ) {
        if self.playback_recovery.errors.report_pending() {
            self.clear_playback_stream();
            self.playback_recovery.retry_at = Some(
                self.playback_recovery
                    .restart_not_before
                    .map_or(now, |due| due.max(now)),
            );
        }
        if self.playback_recovery.awaiting_callback
            && self.playback_status.ready.load(Ordering::Acquire)
        {
            self.playback_recovery.awaiting_callback = false;
            log::info!("Audio playback output callback started");
        }
        if !self
            .playback_recovery
            .retry_at
            .is_some_and(|due| now >= due)
        {
            return;
        }
        let Some(format) = self.playback_recovery.format.clone() else {
            return;
        };
        log::info!("Recreating audio playback on the current default output device");
        let result = restart(self, format);
        self.finish_playback_start(result);
    }

    pub(super) fn receive_audio(
        &mut self,
        receiver: &mpsc::Receiver<MediaData>,
    ) -> Result<MediaData, mpsc::RecvError> {
        receive_with_recovery(receiver, RECOVERY_INTERVAL, || {
            self.recover_playback_with(Instant::now(), Self::restart_playback);
        })
    }
}

pub(super) fn receive_with_recovery(
    receiver: &mpsc::Receiver<MediaData>,
    interval: Duration,
    mut recover: impl FnMut(),
) -> Result<MediaData, mpsc::RecvError> {
    loop {
        match receiver.recv_timeout(interval) {
            Ok(data) => {
                if !matches!(data, MediaData::AudioFormat(_)) {
                    recover();
                }
                return Ok(data);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => recover(),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(mpsc::RecvError),
        }
    }
}
