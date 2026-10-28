use super::android;
use hbb_common::{anyhow::anyhow, ResultType};
use serde_json::{json, Value};
use std::time::Duration;

const CLOSE_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub(super) struct DocumentDescriptor {
    id: Option<String>,
}

impl DocumentDescriptor {
    pub(super) fn new(token: &Value) -> ResultType<Self> {
        let id = token["descriptor"]
            .as_str()
            .ok_or(anyhow!("Missing document descriptor"))?;
        Ok(Self {
            id: Some(id.to_owned()),
        })
    }

    pub(super) async fn close(mut self, error: &str) -> ResultType<()> {
        let id = self.id.take().ok_or(anyhow!("Document already closed"))?;
        let error = error.to_owned();
        tokio::task::spawn_blocking(move || {
            android::request(json!({
                "op": "close", "descriptor": id, "error": error
            }))
        })
        .await??;
        Ok(())
    }
}

impl Drop for DocumentDescriptor {
    fn drop(&mut self) {
        let Some(id) = self.id.take() else {
            return;
        };
        // Aborted jobs must also release the provider's retained descriptor.
        tokio::task::spawn_blocking(move || {
            if let Err(error) = android::request(json!({
                "op": "close", "descriptor": id, "error": "Document transfer interrupted"
            })) {
                hbb_common::throttled_log!(
                    CLOSE_ERROR_LOG_INTERVAL,
                    warn,
                    "Unable to release document descriptor: {}",
                    error
                );
            }
        });
    }
}
