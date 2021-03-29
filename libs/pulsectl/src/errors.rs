use std::fmt;

use pulse::error::{PAErr};

impl From<PAErr> for PulseCtlError {
    fn from(error: PAErr) -> Self {
        PulseCtlError {
            error: PulseCtlErrorType::PulseAudioError,
            message: format!("PulseAudio returned error: {}", error.to_string().unwrap_or("Unknown".to_owned())),
        }
    }
}

impl fmt::Debug for PulseCtlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut error_string = String::new();
        match self.error {
            PulseCtlErrorType::ConnectError => {
                error_string.push_str("ConnectError");
            }
            PulseCtlErrorType::OperationError => {
                error_string.push_str("OperationError");
            }
            PulseCtlErrorType::PulseAudioError => {
                error_string.push_str("PulseAudioError");
            }
        }
        write!(f, "[{}]: {}", error_string, self.message)
    }
}

pub(crate) enum PulseCtlErrorType {
    ConnectError,
    OperationError,
    PulseAudioError,
}

/// Error thrown when PulseAudio throws an error code, there are 3 variants
/// `PulseCtlErrorType::ConnectError` when there's an error establishing a connection
/// `PulseCtlErrorType::OperationError` when the requested operation quis unexpecdatly or is cancelled
/// `PulseCtlErrorType::PulseAudioError` when PulseAudio returns an error code in any circumstance
pub struct PulseCtlError {
    error: PulseCtlErrorType,
    message: String,
}

impl PulseCtlError {
    pub(crate) fn new(err: PulseCtlErrorType, msg: &str) -> Self {
        PulseCtlError {
            error: err,
            message: msg.to_string(),
        }
    }
}
