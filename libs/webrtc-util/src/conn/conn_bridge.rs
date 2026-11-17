use std::collections::VecDeque;
use std::io::{Error, ErrorKind};
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use bytes::Bytes;
use portable_atomic::AtomicUsize;
use tokio::sync::{mpsc, Mutex};
use tokio::time::Duration;

use super::*;

const TICK_WAIT: Duration = Duration::from_micros(10);

/// BridgeConn is a Conn that represents an endpoint of the bridge.
struct BridgeConn {
    br: Arc<Bridge>,
    id: usize,
    rd_rx: Mutex<mpsc::Receiver<Bytes>>,
    loss_chance: u8,
}

#[async_trait]
impl Conn for BridgeConn {
    async fn connect(&self, _addr: SocketAddr) -> Result<()> {
        Err(Error::new(ErrorKind::Other, "Not applicable").into())
    }

    async fn recv(&self, b: &mut [u8]) -> Result<usize> {
        let mut rd_rx = self.rd_rx.lock().await;
        let v = match rd_rx.recv().await {
            Some(v) => v,
            None => return Err(Error::new(ErrorKind::UnexpectedEof, "Unexpected EOF").into()),
        };
        let l = std::cmp::min(v.len(), b.len());
        b[..l].copy_from_slice(&v[..l]);
        Ok(l)
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        let n = self.recv(buf).await?;
        Ok((n, SocketAddr::from_str("0.0.0.0:0")?))
    }

    async fn send(&self, b: &[u8]) -> Result<usize> {
        if rand::random::<u8>() % 100 < self.loss_chance {
            return Ok(b.len());
        }

        self.br.push(b, self.id).await
    }

    async fn send_to(&self, _buf: &[u8], _target: SocketAddr) -> Result<usize> {
        Err(Error::new(ErrorKind::Other, "Not applicable").into())
    }

    fn local_addr(&self) -> Result<SocketAddr> {
        Err(Error::new(ErrorKind::AddrNotAvailable, "Addr Not Available").into())
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        None
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }
}

pub type FilterCbFn = Box<dyn Fn(&Bytes) -> bool + Send + Sync>;

/// Bridge represents a network between the two endpoints.
#[derive(Default)]
pub struct Bridge {
    drop_nwrites: [AtomicUsize; 2],
    reorder_nwrites: [AtomicUsize; 2],

    stack: [Mutex<VecDeque<Bytes>>; 2],
    queue: [Mutex<VecDeque<Bytes>>; 2],

    wr_tx: [Option<mpsc::Sender<Bytes>>; 2],
    filter_cb: [Option<FilterCbFn>; 2],
}

impl Bridge {
    pub fn new(
        loss_chance: u8,
        filter_cb0: Option<FilterCbFn>,
        filter_cb1: Option<FilterCbFn>,
    ) -> (Arc<Bridge>, impl Conn, impl Conn) {
        let (wr_tx0, rd_rx0) = mpsc::channel(1024);
        let (wr_tx1, rd_rx1) = mpsc::channel(1024);

        let br = Arc::new(Bridge {
            wr_tx: [Some(wr_tx0), Some(wr_tx1)],
            filter_cb: [filter_cb0, filter_cb1],
            ..Default::default()
        });
        let conn0 = BridgeConn {
            br: Arc::clone(&br),
            id: 0,
            rd_rx: Mutex::new(rd_rx0),
            loss_chance,
        };
        let conn1 = BridgeConn {
            br: Arc::clone(&br),
            id: 1,
            rd_rx: Mutex::new(rd_rx1),
            loss_chance,
        };

        (br, conn0, conn1)
    }

    /// Len returns number of queued packets.
    #[allow(clippy::len_without_is_empty)]
    pub async fn len(&self, id: usize) -> usize {
        let q = self.queue[id].lock().await;
        q.len()
    }

    pub async fn push(&self, b: &[u8], id: usize) -> Result<usize> {
        // Push rate should be limited as same as Tick rate.
        // Otherwise, queue grows too fast on free running Write.
        tokio::time::sleep(TICK_WAIT).await;

        let d = Bytes::from(b.to_vec());
        if self.drop_nwrites[id].load(Ordering::SeqCst) > 0 {
            self.drop_nwrites[id].fetch_sub(1, Ordering::SeqCst);
        } else if self.reorder_nwrites[id].load(Ordering::SeqCst) > 0 {
            let mut stack = self.stack[id].lock().await;
            stack.push_back(d);
            if self.reorder_nwrites[id].fetch_sub(1, Ordering::SeqCst) == 1 {
                let ok = inverse(&mut stack);
                if ok {
                    let mut queue = self.queue[id].lock().await;
                    queue.append(&mut stack);
                }
            }
        } else if let Some(filter_cb) = &self.filter_cb[id] {
            if filter_cb(&d) {
                let mut queue = self.queue[id].lock().await;
                queue.push_back(d);
            }
        } else {
            //log::debug!("queue [{}] enter lock", id);
            let mut queue = self.queue[id].lock().await;
            queue.push_back(d);
            //log::debug!("queue [{}] exit lock", id);
        }

        Ok(b.len())
    }

    /// Reorder inverses the order of packets currently in the specified queue.
    pub async fn reorder(&self, id: usize) -> bool {
        let mut queue = self.queue[id].lock().await;
        inverse(&mut queue)
    }

    /// Drop drops the specified number of packets from the given offset index
    /// of the specified queue.
    pub async fn drop_offset(&self, id: usize, offset: usize, n: usize) {
        let mut queue = self.queue[id].lock().await;
        queue.drain(offset..offset + n);
    }

    /// drop_next_nwrites drops the next n packets that will be written
    /// to the specified queue.
    pub fn drop_next_nwrites(&self, id: usize, n: usize) {
        self.drop_nwrites[id].store(n, Ordering::SeqCst);
    }

    /// reorder_next_nwrites drops the next n packets that will be written
    /// to the specified queue.
    pub fn reorder_next_nwrites(&self, id: usize, n: usize) {
        self.reorder_nwrites[id].store(n, Ordering::SeqCst);
    }

    pub async fn clear(&self) {
        for id in 0..2 {
            let mut queue = self.queue[id].lock().await;
            queue.clear();
        }
    }

    /// Tick attempts to hand a packet from the queue for each directions, to readers,
    /// if there are waiting on the queue. If there's no reader, it will return
    /// immediately.
    pub async fn tick(&self) -> usize {
        let mut n = 0;

        for id in 0..2 {
            let mut queue = self.queue[id].lock().await;
            if let Some(d) = queue.pop_front() {
                n += 1;
                if let Some(wr_tx) = &self.wr_tx[1 - id] {
                    let _ = wr_tx.send(d).await;
                }
            }
        }

        n
    }

    /// Process repeats tick() calls until no more outstanding packet in the queues.
    pub async fn process(&self) {
        loop {
            tokio::time::sleep(TICK_WAIT).await;
            self.tick().await;
            if self.len(0).await == 0 && self.len(1).await == 0 {
                break;
            }
        }
    }
}

pub(crate) fn inverse(s: &mut VecDeque<Bytes>) -> bool {
    if s.len() < 2 {
        return false;
    }

    let (mut i, mut j) = (0, s.len() - 1);
    while i < j {
        s.swap(i, j);
        i += 1;
        j -= 1;
    }

    true
}
