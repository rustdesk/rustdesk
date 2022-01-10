use async_trait::async_trait;
pub use hbb_common::ResultType;
pub mod discovery;
pub mod udp;

const CMD_TOKEN: u8 = '\n' as u8;

/// Use tower::Service may be better ?
#[async_trait]
pub trait Handler<Request>: Send + Sync {
    async fn call(&self, request: Request) -> ResultType<()>;
}
