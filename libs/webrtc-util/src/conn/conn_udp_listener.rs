use core::sync::atomic::Ordering;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use portable_atomic::AtomicBool;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch, Mutex};

use super::*;
use crate::error::Error;
use crate::Buffer;

const RECEIVE_MTU: usize = 8192;
const DEFAULT_LISTEN_BACKLOG: usize = 128; // same as Linux default

pub type AcceptFilterFn =
    Box<dyn (Fn(&[u8]) -> Pin<Box<dyn Future<Output = bool> + Send + 'static>>) + Send + Sync>;

type AcceptDoneCh = (mpsc::Receiver<Arc<UdpConn>>, watch::Receiver<()>);

/// listener is used in the [DTLS](https://github.com/webrtc-rs/dtls) and
/// [SCTP](https://github.com/webrtc-rs/sctp) transport to provide a connection-oriented
/// listener over a UDP.
struct ListenerImpl {
    pconn: Arc<dyn Conn + Send + Sync>,
    accepting: Arc<AtomicBool>,
    accept_ch_tx: Arc<Mutex<Option<mpsc::Sender<Arc<UdpConn>>>>>,
    done_ch_tx: Arc<Mutex<Option<watch::Sender<()>>>>,
    ch_rx: Arc<Mutex<AcceptDoneCh>>,
    conns: Arc<Mutex<HashMap<String, Arc<UdpConn>>>>,
}

#[async_trait]
impl Listener for ListenerImpl {
    /// accept waits for and returns the next connection to the listener.
    async fn accept(&self) -> Result<(Arc<dyn Conn + Send + Sync>, SocketAddr)> {
        let (accept_ch_rx, done_ch_rx) = &mut *self.ch_rx.lock().await;

        tokio::select! {
            c = accept_ch_rx.recv() =>{
                if let Some(c) = c{
                    let raddr = c.raddr;
                    Ok((c, raddr))
                }else{
                    Err(Error::ErrClosedListenerAcceptCh)
                }
            }
            _ = done_ch_rx.changed() =>  Err(Error::ErrClosedListener),
        }
    }

    /// close closes the listener.
    /// Any blocked Accept operations will be unblocked and return errors.
    async fn close(&self) -> Result<()> {
        if self.accepting.load(Ordering::SeqCst) {
            self.accepting.store(false, Ordering::SeqCst);
            {
                let mut done_ch = self.done_ch_tx.lock().await;
                done_ch.take();
            }
            {
                let mut accept_ch = self.accept_ch_tx.lock().await;
                accept_ch.take();
            }
        }

        Ok(())
    }

    /// Addr returns the listener's network address.
    async fn addr(&self) -> Result<SocketAddr> {
        self.pconn.local_addr()
    }
}

/// ListenConfig stores options for listening to an address.
#[derive(Default)]
pub struct ListenConfig {
    /// Backlog defines the maximum length of the queue of pending
    /// connections. It is equivalent of the backlog argument of
    /// POSIX listen function.
    /// If a connection request arrives when the queue is full,
    /// the request will be silently discarded, unlike TCP.
    /// Set zero to use default value 128 which is same as Linux default.
    pub backlog: usize,

    /// AcceptFilter determines whether the new conn should be made for
    /// the incoming packet. If not set, any packet creates new conn.
    pub accept_filter: Option<AcceptFilterFn>,
}

pub async fn listen<A: ToSocketAddrs>(laddr: A) -> Result<impl Listener> {
    ListenConfig::default().listen(laddr).await
}

impl ListenConfig {
    /// Listen creates a new listener based on the ListenConfig.
    pub async fn listen<A: ToSocketAddrs>(&mut self, laddr: A) -> Result<impl Listener> {
        if self.backlog == 0 {
            self.backlog = DEFAULT_LISTEN_BACKLOG;
        }

        let pconn = Arc::new(UdpSocket::bind(laddr).await?);
        let (accept_ch_tx, accept_ch_rx) = mpsc::channel(self.backlog);
        let (done_ch_tx, done_ch_rx) = watch::channel(());

        let l = ListenerImpl {
            pconn,
            accepting: Arc::new(AtomicBool::new(true)),
            accept_ch_tx: Arc::new(Mutex::new(Some(accept_ch_tx))),
            done_ch_tx: Arc::new(Mutex::new(Some(done_ch_tx))),
            ch_rx: Arc::new(Mutex::new((accept_ch_rx, done_ch_rx.clone()))),
            conns: Arc::new(Mutex::new(HashMap::new())),
        };

        let pconn = Arc::clone(&l.pconn);
        let accepting = Arc::clone(&l.accepting);
        let accept_filter = self.accept_filter.take();
        let accept_ch_tx = Arc::clone(&l.accept_ch_tx);
        let conns = Arc::clone(&l.conns);
        tokio::spawn(async move {
            ListenConfig::read_loop(
                done_ch_rx,
                pconn,
                accepting,
                accept_filter,
                accept_ch_tx,
                conns,
            )
            .await;
        });

        Ok(l)
    }

    /// read_loop has to tasks:
    /// 1. Dispatching incoming packets to the correct Conn.
    ///    It can therefore not be ended until all Conns are closed.
    /// 2. Creating a new Conn when receiving from a new remote.
    async fn read_loop(
        mut done_ch_rx: watch::Receiver<()>,
        pconn: Arc<dyn Conn + Send + Sync>,
        accepting: Arc<AtomicBool>,
        accept_filter: Option<AcceptFilterFn>,
        accept_ch_tx: Arc<Mutex<Option<mpsc::Sender<Arc<UdpConn>>>>>,
        conns: Arc<Mutex<HashMap<String, Arc<UdpConn>>>>,
    ) {
        let mut buf = vec![0u8; RECEIVE_MTU];

        loop {
            tokio::select! {
                _ = done_ch_rx.changed() => {
                    break;
                }
                result = pconn.recv_from(&mut buf) => {
                    match result {
                        Ok((n, raddr)) => {
                            let udp_conn = match ListenConfig::get_udp_conn(
                                &pconn,
                                &accepting,
                                &accept_filter,
                                &accept_ch_tx,
                                &conns,
                                raddr,
                                &buf[..n],
                            )
                            .await
                            {
                                Ok(conn) => conn,
                                Err(_) => continue,
                            };

                            if let Some(conn) = udp_conn {
                                let _ = conn.buffer.write(&buf[..n]).await;
                            }
                        }
                        Err(err) => {
                            log::warn!("ListenConfig pconn.recv_from error: {}", err);
                            break;
                        }
                    };
                }
            }
        }
    }

    async fn get_udp_conn(
        pconn: &Arc<dyn Conn + Send + Sync>,
        accepting: &Arc<AtomicBool>,
        accept_filter: &Option<AcceptFilterFn>,
        accept_ch_tx: &Arc<Mutex<Option<mpsc::Sender<Arc<UdpConn>>>>>,
        conns: &Arc<Mutex<HashMap<String, Arc<UdpConn>>>>,
        raddr: SocketAddr,
        buf: &[u8],
    ) -> Result<Option<Arc<UdpConn>>> {
        {
            let m = conns.lock().await;
            if let Some(conn) = m.get(raddr.to_string().as_str()) {
                return Ok(Some(conn.clone()));
            }
        }

        if !accepting.load(Ordering::SeqCst) {
            return Err(Error::ErrClosedListener);
        }

        if let Some(f) = accept_filter {
            if !(f(buf).await) {
                return Ok(None);
            }
        }

        let udp_conn = Arc::new(UdpConn::new(Arc::clone(pconn), raddr));
        {
            let accept_ch = accept_ch_tx.lock().await;
            if let Some(tx) = &*accept_ch {
                if tx.try_send(Arc::clone(&udp_conn)).is_err() {
                    return Err(Error::ErrListenQueueExceeded);
                }
            } else {
                return Err(Error::ErrClosedListenerAcceptCh);
            }
        }

        {
            let mut m = conns.lock().await;
            m.insert(raddr.to_string(), Arc::clone(&udp_conn));
        }

        Ok(Some(udp_conn))
    }
}

/// UdpConn augments a connection-oriented connection over a UdpSocket
pub struct UdpConn {
    pconn: Arc<dyn Conn + Send + Sync>,
    raddr: SocketAddr,
    buffer: Buffer,
}

impl UdpConn {
    fn new(pconn: Arc<dyn Conn + Send + Sync>, raddr: SocketAddr) -> Self {
        UdpConn {
            pconn,
            raddr,
            buffer: Buffer::new(0, 0),
        }
    }
}

#[async_trait]
impl Conn for UdpConn {
    async fn connect(&self, addr: SocketAddr) -> Result<()> {
        self.pconn.connect(addr).await
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        Ok(self.buffer.read(buf, None).await?)
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        let n = self.buffer.read(buf, None).await?;
        Ok((n, self.raddr))
    }

    async fn send(&self, buf: &[u8]) -> Result<usize> {
        self.pconn.send_to(buf, self.raddr).await
    }

    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> Result<usize> {
        self.pconn.send_to(buf, target).await
    }

    fn local_addr(&self) -> Result<SocketAddr> {
        self.pconn.local_addr()
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        Some(self.raddr)
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }
}
