use std::str::FromStr;

use portable_atomic::AtomicUsize;

use super::*;

#[derive(Default)]
struct DummyObserver {
    nclosed: Arc<AtomicUsize>,
    #[allow(clippy::type_complexity)]
    read_ch_tx: Arc<Mutex<Option<mpsc::Sender<Box<dyn Chunk + Send + Sync>>>>>,
}

#[async_trait]
impl ConnObserver for DummyObserver {
    async fn write(&self, c: Box<dyn Chunk + Send + Sync>) -> Result<()> {
        let mut chunk = ChunkUdp::new(c.destination_addr(), c.source_addr());
        chunk.user_data = c.user_data();

        let read_ch_tx = self.read_ch_tx.lock().await;
        if let Some(tx) = &*read_ch_tx {
            tx.send(Box::new(chunk))
                .await
                .map_err(|e| Error::Other(e.to_string()))?;
        }
        Ok(())
    }

    async fn on_closed(&self, _addr: SocketAddr) {
        self.nclosed.fetch_add(1, Ordering::SeqCst);
    }

    fn determine_source_ip(&self, loc_ip: IpAddr, _dst_ip: IpAddr) -> Option<IpAddr> {
        Some(loc_ip)
    }
}

//use std::io::Write;

#[tokio::test]
async fn test_udp_conn_send_to_recv_from() -> Result<()> {
    /*env_logger::Builder::new()
    .format(|buf, record| {
        writeln!(
            buf,
            "{}:{} [{}] {} - {}",
            record.file().unwrap_or("unknown"),
            record.line().unwrap_or(0),
            record.level(),
            chrono::Local::now().format("%H:%M:%S.%6f"),
            record.args()
        )
    })
    .filter(None, log::LevelFilter::Trace)
    .init();*/

    let nclosed = Arc::new(AtomicUsize::new(0));
    let data = b"Hello".to_vec();
    let src_addr = SocketAddr::from_str("127.0.0.1:1234")?;
    let dst_addr = SocketAddr::from_str("127.0.0.1:5678")?;

    let dummy_obs = Arc::new(Mutex::new(DummyObserver::default()));
    let dummy_obs2 = Arc::clone(&dummy_obs);
    let obs = dummy_obs2 as Arc<Mutex<dyn ConnObserver + Send + Sync>>;

    let conn = Arc::new(UdpConn::new(src_addr, None, obs));
    {
        let mut dummy = dummy_obs.lock().await;
        dummy.nclosed = Arc::clone(&nclosed);
        dummy.read_ch_tx = conn.get_inbound_ch();
    }

    let conn_rx = Arc::clone(&conn);
    let data_rx = data.clone();

    let (rcvd_ch_tx, mut rcvd_ch_rx) = mpsc::channel(1);
    let (done_ch_tx, mut done_ch_rx) = mpsc::channel::<()>(1);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 1500];

        loop {
            let (n, addr) = match conn_rx.recv_from(&mut buf).await {
                Ok((n, addr)) => (n, addr),
                Err(err) => {
                    log::debug!("conn closed. exiting the read loop with err {}", err);
                    break;
                }
            };

            log::debug!("read data");
            assert_eq!(data_rx.len(), n, "should match");
            assert_eq!(&data_rx, &buf[..n], "should match");
            log::debug!("dst_addr {} vs add {}", dst_addr, addr);
            assert_eq!(dst_addr.to_string(), addr.to_string(), "should match");
            let _ = rcvd_ch_tx.send(()).await;
        }

        drop(done_ch_tx);
    });

    let n = conn.send_to(&data, dst_addr).await.unwrap();
    assert_eq!(n, data.len(), "should match");

    loop {
        tokio::select! {
            result = rcvd_ch_rx.recv() =>{
                if result.is_some(){
                    log::debug!("closing soon...");
                    conn.close().await?;
                }
            }
            _ = done_ch_rx.recv() => {
                log::debug!("recv done_ch_rx...");
                break;
            }
        }
    }

    assert_eq!(1, nclosed.load(Ordering::SeqCst), "should be closed once");

    Ok(())
}

//use std::io::Write;

#[tokio::test]
async fn test_udp_conn_send_recv() -> Result<()> {
    /*env_logger::Builder::new()
    .format(|buf, record| {
        writeln!(
            buf,
            "{}:{} [{}] {} - {}",
            record.file().unwrap_or("unknown"),
            record.line().unwrap_or(0),
            record.level(),
            chrono::Local::now().format("%H:%M:%S.%6f"),
            record.args()
        )
    })
    .filter(None, log::LevelFilter::Trace)
    .init();*/

    let nclosed = Arc::new(AtomicUsize::new(0));
    let data = b"Hello".to_vec();
    let src_addr = SocketAddr::from_str("127.0.0.1:1234")?;
    let dst_addr = SocketAddr::from_str("127.0.0.1:5678")?;

    let dummy_obs = Arc::new(Mutex::new(DummyObserver::default()));
    let dummy_obs2 = Arc::clone(&dummy_obs);
    let obs = dummy_obs2 as Arc<Mutex<dyn ConnObserver + Send + Sync>>;

    let conn = Arc::new(UdpConn::new(src_addr, Some(dst_addr), obs));
    {
        let mut dummy = dummy_obs.lock().await;
        dummy.nclosed = Arc::clone(&nclosed);
        dummy.read_ch_tx = conn.get_inbound_ch();
    }

    let conn_rx = Arc::clone(&conn);
    let data_rx = data.clone();

    let (rcvd_ch_tx, mut rcvd_ch_rx) = mpsc::channel(1);
    let (done_ch_tx, mut done_ch_rx) = mpsc::channel::<()>(1);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 1500];

        loop {
            let n = match conn_rx.recv(&mut buf).await {
                Ok(n) => n,
                Err(err) => {
                    log::debug!("conn closed. exiting the read loop with err {}", err);
                    break;
                }
            };

            log::debug!("read data");
            assert_eq!(data_rx.len(), n, "should match");
            assert_eq!(&data_rx, &buf[..n], "should match");
            let _ = rcvd_ch_tx.send(()).await;
        }

        drop(done_ch_tx);
    });

    let n = conn.send(&data).await.unwrap();
    assert_eq!(n, data.len(), "should match");

    loop {
        tokio::select! {
            result = rcvd_ch_rx.recv() =>{
                if result.is_some(){
                    log::debug!("closing soon...");
                    conn.close().await?;
                }
            }
            _ = done_ch_rx.recv() => {
                log::debug!("recv done_ch_rx...");
                break;
            }
        }
    }

    assert_eq!(1, nclosed.load(Ordering::SeqCst), "should be closed once");

    Ok(())
}
