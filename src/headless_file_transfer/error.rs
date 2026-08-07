#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HeadlessFileTransferError {
    Internal(String),
    Usage(String),
    LocalPrecondition(String),
    Authentication(String),
    Connection(String),
    Transfer(String),
    DestinationExists(String),
    Protocol(String),
    Interrupted,
    Terminated,
}

impl HeadlessFileTransferError {
    pub(crate) const fn status(&self) -> i32 {
        match self {
            Self::Internal(_) => 1,
            Self::Usage(_) => 2,
            Self::LocalPrecondition(_) => 3,
            Self::Authentication(_) => 4,
            Self::Connection(_) | Self::Protocol(_) => 5,
            Self::Transfer(_) => 6,
            Self::DestinationExists(_) => 7,
            Self::Interrupted => 130,
            Self::Terminated => 143,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HeadlessFileTransferError;

    #[test]
    fn maps_headless_file_transfer_failures_to_stable_exit_statuses() {
        let cases = [
            (HeadlessFileTransferError::Internal("internal".into()), 1),
            (HeadlessFileTransferError::Usage("usage".into()), 2),
            (
                HeadlessFileTransferError::LocalPrecondition("local".into()),
                3,
            ),
            (
                HeadlessFileTransferError::Authentication("authentication".into()),
                4,
            ),
            (
                HeadlessFileTransferError::Connection("connection".into()),
                5,
            ),
            (HeadlessFileTransferError::Transfer("transfer".into()), 6),
            (
                HeadlessFileTransferError::DestinationExists("destination".into()),
                7,
            ),
            (HeadlessFileTransferError::Protocol("protocol".into()), 5),
            (HeadlessFileTransferError::Interrupted, 130),
            (HeadlessFileTransferError::Terminated, 143),
        ];

        for (error, status) in cases {
            assert_eq!(error.status(), status);
        }
    }
}
