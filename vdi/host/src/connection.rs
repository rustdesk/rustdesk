use hbb_common::{message_proto::*, tokio, ResultType};
pub use tokio::sync::{mpsc, Mutex};
pub struct Connection {
    pub tx: mpsc::UnboundedSender<Message>,
}

impl Connection {
    pub async fn on_message(&mut self, message: Message) -> ResultType<bool> {
        Ok(true)
    }
}
