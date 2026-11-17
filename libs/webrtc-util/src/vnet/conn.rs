#[cfg(test)]
mod conn_test;

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use async_trait::async_trait;
use portable_atomic::AtomicBool;
use tokio::sync::{mpsc, Mutex};

use crate::conn::Conn;
use crate::error::*;
use crate::sync::RwLock;
use crate::vnet::chunk::{Chunk, ChunkUdp};

const MAX_READ_QUEUE_SIZE: usize = 1024;

/// vNet implements this
#[async_trait]
pub(crate) trait ConnObserver {
    async fn write(&self, c: Box<dyn Chunk + Send + Sync>) -> Result<()>;
    async fn on_closed(&self, addr: SocketAddr);
    fn determine_source_ip(&self, loc_ip: IpAddr, dst_ip: IpAddr) -> Option<IpAddr>;
}

pub(crate) type ChunkChTx = mpsc::Sender<Box<dyn Chunk + Send + Sync>>;

/// UDPConn is the implementation of the Conn and PacketConn interfaces for UDP network connections.
/// compatible with net.PacketConn and net.Conn
pub(crate) struct UdpConn {
    loc_addr: SocketAddr,
    rem_addr: RwLock<Option<SocketAddr>>,
    read_ch_tx: Arc<Mutex<Option<ChunkChTx>>>,
    read_ch_rx: Mutex<mpsc::Receiver<Box<dyn Chunk + Send + Sync>>>,
    closed: AtomicBool,
    obs: Weak<Mutex<dyn ConnObserver + Send + Sync>>,
}

impl UdpConn {
    pub(crate) fn new(
        loc_addr: SocketAddr,
        rem_addr: Option<SocketAddr>,
        obs: Arc<Mutex<dyn ConnObserver + Send + Sync>>,
    ) -> Self {
        let (read_ch_tx, read_ch_rx) = mpsc::channel(MAX_READ_QUEUE_SIZE);

        let weak_obs = Arc::downgrade(&obs);
        UdpConn {
            loc_addr,
            rem_addr: RwLock::new(rem_addr),
            read_ch_tx: Arc::new(Mutex::new(Some(read_ch_tx))),
            read_ch_rx: Mutex::new(read_ch_rx),
            closed: AtomicBool::new(false),
            obs: weak_obs,
        }
    }

    pub(crate) fn get_inbound_ch(&self) -> Arc<Mutex<Option<ChunkChTx>>> {
        Arc::clone(&self.read_ch_tx)
    }
}

#[async_trait]
impl Conn for UdpConn {
    async fn connect(&self, addr: SocketAddr) -> Result<()> {
        self.rem_addr.write().replace(addr);

        Ok(())
    }
    async fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        let (n, _) = self.recv_from(buf).await?;
        Ok(n)
    }

    /// recv_from reads a packet from the connection,
    /// copying the payload into p. It returns the number of
    /// bytes copied into p and the return address that
    /// was on the packet.
    /// It returns the number of bytes read (0 <= n <= len(p))
    /// and any error encountered. Callers should always process
    /// the n > 0 bytes returned before considering the error err.
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        let mut read_ch = self.read_ch_rx.lock().await;
        let rem_addr = *self.rem_addr.read();
        while let Some(chunk) = read_ch.recv().await {
            let user_data = chunk.user_data();
            let n = std::cmp::min(buf.len(), user_data.len());
            buf[..n].copy_from_slice(&user_data[..n]);
            let addr = chunk.source_addr();
            {
                if let Some(rem_addr) = &rem_addr {
                    if &addr != rem_addr {
                        continue; // discard (shouldn't happen)
                    }
                }
            }
            return Ok((n, addr));
        }

        Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "Connection Aborted").into())
    }

    async fn send(&self, buf: &[u8]) -> Result<usize> {
        let rem_addr = *self.rem_addr.read();
        if let Some(rem_addr) = rem_addr {
            self.send_to(buf, rem_addr).await
        } else {
            Err(Error::ErrNoRemAddr)
        }
    }

    /// send_to writes a packet with payload p to addr.
    /// send_to can be made to time out and return
    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> Result<usize> {
        let obs = self.obs.upgrade().ok_or_else(|| Error::ErrVnetDisabled)?;

        let src_ip = {
            let obs = obs.lock().await;
            match obs.determine_source_ip(self.loc_addr.ip(), target.ip()) {
                Some(ip) => ip,
                None => return Err(Error::ErrLocAddr),
            }
        };

        let src_addr = SocketAddr::new(src_ip, self.loc_addr.port());

        let mut chunk = ChunkUdp::new(src_addr, target);
        chunk.user_data = buf.to_vec();
        {
            let c: Box<dyn Chunk + Send + Sync> = Box::new(chunk);
            let obs = obs.lock().await;
            obs.write(c).await?
        }

        Ok(buf.len())
    }

    fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.loc_addr)
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        *self.rem_addr.read()
    }

    async fn close(&self) -> Result<()> {
        let obs = self.obs.upgrade().ok_or_else(|| Error::ErrVnetDisabled)?;

        if self.closed.load(Ordering::SeqCst) {
            return Err(Error::ErrAlreadyClosed);
        }
        self.closed.store(true, Ordering::SeqCst);
        {
            let mut reach_ch = self.read_ch_tx.lock().await;
            reach_ch.take();
        }
        {
            let obs = obs.lock().await;
            obs.on_closed(self.loc_addr).await;
        }

        Ok(())
    }

    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }
}
