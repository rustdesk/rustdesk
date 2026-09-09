use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// Each stream owns its flag so a late callback cannot restart a replacement.
#[derive(Clone, Default)]
pub(super) struct CaptureErrorHandler {
    interrupted: Arc<AtomicBool>,
}

impl CaptureErrorHandler {
    pub(super) fn handle(&self, error: cpal::StreamError) {
        if matches!(error, cpal::StreamError::StreamInterrupted { .. }) {
            // ScreenCaptureKit can stop capture while the remote session stays open.
            // The observed -3821 error does not identify its underlying trigger.
            // https://developer.apple.com/documentation/screencapturekit/scstreamdelegate/stream(_:didstopwitherror:)
            hbb_common::log::error!("Audio capture stream interrupted: {error}");
            self.interrupted.store(true, Ordering::Relaxed);
        } else {
            // Keep frequent sample-buffer errors at the existing trace level.
            hbb_common::log::trace!("an error occurred on stream: {error}");
        }
    }

    pub(super) fn needs_restart(&self) -> bool {
        self.interrupted.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpal::{BackendSpecificError, StreamError};

    fn system_interruption() -> StreamError {
        StreamError::StreamInterrupted {
            err: BackendSpecificError {
                description: "ScreenCaptureKit system-stopped capture (-3821)".to_owned(),
            },
        }
    }

    #[test]
    fn system_interruption_requests_recreation_of_the_active_stream() {
        let errors = CaptureErrorHandler::default();
        let callback = errors.clone();
        assert!(!errors.needs_restart());
        callback.handle(system_interruption());
        assert!(errors.needs_restart());
    }

    #[test]
    fn late_error_from_an_old_stream_does_not_restart_its_replacement() {
        let old_callback = CaptureErrorHandler::default();
        let replacement = CaptureErrorHandler::default();
        old_callback.handle(system_interruption());
        assert!(old_callback.needs_restart());
        assert!(!replacement.needs_restart());
    }

    #[test]
    fn other_backend_errors_do_not_request_recreation() {
        let errors = CaptureErrorHandler::default();
        errors.handle(StreamError::BackendSpecific {
            err: BackendSpecificError {
                description: "A sample buffer could not be read".to_owned(),
            },
        });
        assert!(!errors.needs_restart());
    }
}
