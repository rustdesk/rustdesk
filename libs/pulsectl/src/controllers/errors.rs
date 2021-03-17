use std::fmt;

use crate::PulseCtlError;

/// if the error occurs within the Mainloop, we bubble up the error with
/// this conversion
impl From<PulseCtlError> for ControllerError {
    fn from(error: super::errors::PulseCtlError) -> Self {
        ControllerError {
            error: ControllerErrorType::PulseCtlError,
            message: format!("{:?}", error),
        }
    }
}

impl fmt::Debug for ControllerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut error_string = String::new();
        match self.error {
            ControllerErrorType::PulseCtlError => {
                error_string.push_str("PulseCtlError");
            }
            ControllerErrorType::GetInfoError => {
                error_string.push_str("GetInfoError");
            }
        }
        write!(f, "[{}]: {}", error_string, self.message)
    }
}

pub(crate) enum ControllerErrorType {
    PulseCtlError,
    GetInfoError,
}

/// Error thrown while fetching data from pulseaudio,
/// has two variants: PulseCtlError for when PulseAudio returns an error code
/// and GetInfoError when a request for data fails for whatever reason
pub struct ControllerError {
    error: ControllerErrorType,
    message: String,
}

impl ControllerError {
    pub(crate) fn new(err: ControllerErrorType, msg: &str) -> Self {
        ControllerError {
            error: err,
            message: msg.to_string(),
        }
    }
}
