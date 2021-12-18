use async_trait::async_trait;
use hbb_common::{
    log,
    tokio::{self, sync::Notify},
    udp::FramedSocket,
    ResultType,
};
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

/// Simple udp server
pub struct Server {
    port: u16,
    exit_notify: Arc<Notify>,
}

pub struct UdpRequest {
    pub data: Vec<u8>,
    pub addr: SocketAddr,
}

type UdpHandler = Box<dyn crate::Handler<UdpRequest>>;

pub struct UdpFnHandler<F>(F);

/// Handlers of udp server.
/// After udp server received data. Command should be parsed.
/// Handler will then be used to process data.
pub struct UdpHandlers {
    handlers: HashMap<Vec<u8>, UdpHandler>,
}

impl Server {
    pub fn new(port: u16, exit_notify: Arc<Notify>) -> Self {
        Self { port, exit_notify }
    }

    /// Start server with the handlers.
    pub async fn start(&self, handlers: UdpHandlers) -> ResultType<()> {
        let exit_notify = self.exit_notify.clone();

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        let mut server = FramedSocket::new(addr).await?;
        log::trace!("succeeded to bind {} for discovery server", addr);

        tokio::spawn(async move {
            let handlers = Arc::new(handlers.handlers);
            loop {
                tokio::select! {
                    _ = exit_notify.notified() => {
                        log::debug!("exit server graceful");
                        break;
                    }
                    n = server.next() => {
                        log::info!("received message");
                        let handlers = handlers.clone();
                        match n {
                            Some(Ok((data, addr))) => {
                                match data.iter().position(|x| x == &crate::CMD_TOKEN) {
                                    Some(p) => {
                                        tokio::spawn(async move {
                                            let cmd = data[0..p].to_vec();
                                            match handlers.get(&cmd) {
                                                Some(h) => {
                                                    let request = UdpRequest {data: data[p+1..].to_vec(), addr};
                                                    if let Err(e) = h.call(request).await {
                                                        log::error!("handle {:?} failed, {}", cmd, e);
                                                    }
                                                }
                                                None => {
                                                    log::warn!("no handler for {:?}", &cmd);
                                                }
                                            }
                                        });
                                    }
                                    None => {
                                        log::error!("failed to parse command token");
                                    }
                                }

                            }
                            Some(Err(e)) => {
                                log::error!("recv error: {}", e)
                            }
                            None => {
                                log::error!("should never reach here");
                            }
                        }
                    }
                }
            }
        });
        Ok(())
    }
}

impl UdpHandlers {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
    /// Insert <cmd, handler> pair.
    ///
    /// # Example
    ///
    /// ```rust
    /// extern crate socket_cs;
    /// use socket_cs::{ResultType, udp::{UdpHandlers, UdpRequest}};
    ///
    /// struct SimpleHandler;
    ///
    /// #[async_trait]
    /// impl crate::Handler<UdpRequest> for SimpleHandler {
    ///     async fn call(&self, _: UdpRequest) -> ResultType<()> {
    ///         Ok(())
    ///     }
    /// }
    /// async fn simple_ignore(_: UdpRequest) -> ResultType<()> {
    ///     Ok(())
    /// }
    /// let handlers = UdpHandlers::new();
    ///
    /// handlers
    /// .handle(b"cmd".to_vec(), Box::new(SimpleHandler))
    /// .handle(b"cmd2".to_vec(), simple_ignore.into());
    ///
    /// ```
    ///
    /// **Notice** Same cmd where override the previous one.
    ///
    pub fn handle(mut self, cmd: Vec<u8>, h: UdpHandler) -> Self {
        self.handlers.insert(cmd, h);
        self
    }
}

/// TODO: more generice Request.
#[async_trait]
impl<F, Fut> crate::Handler<UdpRequest> for UdpFnHandler<F>
where
    Fut: Future<Output = ResultType<()>> + Send,
    F: Fn(UdpRequest) -> Fut + Send + Sync,
{
    async fn call(&self, request: UdpRequest) -> ResultType<()> {
        self.0(request).await
    }
}

impl<F, Fut> From<F> for UdpHandler
where
    Fut: Future<Output = ResultType<()>> + Send,
    F: Fn(UdpRequest) -> Fut + Send + Sync + 'static,
{
    fn from(f: F) -> Self {
        Box::new(UdpFnHandler(f))
    }
}
