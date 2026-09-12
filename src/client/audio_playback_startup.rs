use super::{AudioHandler, Instant, Ordering, ResultType};
use hbb_common::log;

impl AudioHandler {
    pub(in crate::client) fn cancel_pending_playback(&mut self) {
        if let Some(mut pending) = self.playback_recovery.pending_output.take() {
            pending.audio_stream = None;
            pending.playback_recovery.report_pending();
            pending.playback_status.report_errors();
        }
    }

    pub(in crate::client) fn finish_playback_replacement(
        &mut self,
        result: ResultType<()>,
        previous: Option<Self>,
    ) {
        self.finish_playback_start(result);
        let Some(mut previous) = previous else {
            return;
        };
        previous.audio_decoder = self.audio_decoder.take();
        let candidate = std::mem::replace(self, previous);
        self.playback_recovery.pending_output = Some(Box::new(candidate));
        log::info!("Audio playback replacement pending; continuing on the compatible output");
        self.recover_playback_with(Instant::now(), Self::restart_playback);
    }

    pub(super) fn resolve_pending_playback(&mut self) -> Option<bool> {
        let mut candidate = self.playback_recovery.pending_output.take()?;
        let candidate_failed = candidate.playback_recovery.report_pending();
        let previous_failed = self.playback_recovery.report_pending();
        if candidate_failed {
            self.playback_recovery.restart_not_before =
                candidate.playback_recovery.restart_not_before;
            candidate.audio_stream = None;
            candidate.playback_recovery.report_pending();
            candidate.playback_status.report_errors();
            if !previous_failed {
                log::error!("Audio playback replacement failed before startup confirmation; keeping the existing compatible stream");
            }
            return Some(previous_failed);
        }
        if candidate.playback_status.ready.load(Ordering::Acquire) || previous_failed {
            candidate.audio_decoder = self.audio_decoder.take();
            self.audio_stream = None;
            self.playback_recovery.report_pending();
            self.playback_status.report_errors();
            *self = *candidate;
            return Some(false);
        }
        self.playback_recovery.pending_output = Some(candidate);
        // A second active-queue read could discard a healthy pending candidate.
        Some(false)
    }
}
