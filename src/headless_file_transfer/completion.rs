use super::HeadlessFileTransferError;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransferCompletion {
    pub(crate) id: i32,
    pub(crate) file_num: i32,
    pub(crate) total_size: u64,
    pub(crate) finished_size: u64,
    #[serde(default)]
    pub(crate) done: bool,
    #[serde(default)]
    pub(crate) error: String,
}

impl TransferCompletion {
    pub(crate) fn parse(job_json: &str) -> Result<Self, HeadlessFileTransferError> {
        let completion = serde_json::from_str::<Self>(job_json).map_err(|_| {
            HeadlessFileTransferError::Protocol("invalid transfer completion".to_owned())
        })?;
        if completion.id <= 0 {
            return Err(HeadlessFileTransferError::Protocol(
                "invalid transfer completion ID".to_owned(),
            ));
        }
        if completion.file_num < 0 {
            return Err(HeadlessFileTransferError::Protocol(
                "invalid transfer completion file number".to_owned(),
            ));
        }
        if completion.finished_size > completion.total_size {
            return Err(HeadlessFileTransferError::Protocol(
                "invalid transfer completion size".to_owned(),
            ));
        }
        Ok(completion)
    }
}

#[cfg(test)]
mod tests {
    use super::TransferCompletion;

    #[test]
    fn parses_completed_job_without_exposing_paths() {
        let completion = TransferCompletion::parse(
            r#"{"id":7,"fileNum":1,"totalSize":42,"finishedSize":42,"done":true,"error":""}"#,
        )
        .unwrap();
        assert_eq!(completion.id, 7);
        assert_eq!(completion.total_size, 42);
        assert!(completion.done);
    }

    #[test]
    fn rejects_missing_or_inconsistent_completion_fields() {
        assert!(TransferCompletion::parse("{}").is_err());
        assert!(TransferCompletion::parse(
            r#"{"id":7,"fileNum":1,"totalSize":42,"finishedSize":43,"done":true,"error":""}"#
        )
        .is_err());
    }
}
