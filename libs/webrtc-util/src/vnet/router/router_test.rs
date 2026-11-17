use portable_atomic::{AtomicI32, AtomicUsize};

use super::*;

const MARGIN: Duration = Duration::from_millis(18);
const DEMO_IP: &str = "1.2.3.4";

struct DummyNic {
    net: Net,
    on_inbound_chunk_handler: u16,
    cbs0: AtomicI32,
    done_ch_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    delay_res: Arc<Mutex<Vec<Duration>>>,
    npkts: i32,
}

impl Default for DummyNic {
    fn default() -> Self {
        DummyNic {
            net: Net::Ifs(vec![]),
            on_inbound_chunk_handler: 0,
            cbs0: AtomicI32::new(0),
            done_ch_tx: Arc::new(Mutex::new(None)),
            delay_res: Arc::new(Mutex::new(vec![])),
            npkts: 0,
        }
    }
}

#[async_trait]
impl Nic for DummyNic {
    async fn get_interface(&self, ifc_name: &str) -> Option<Interface> {
        self.net.get_interface(ifc_name).await
    }

    async fn add_addrs_to_interface(&mut self, ifc_name: &str, addrs: &[IpNet]) -> Result<()> {
        let nic = self.net.get_nic()?;
        let mut net = nic.lock().await;
        net.add_addrs_to_interface(ifc_name, addrs).await
    }

    async fn set_router(&self, r: Arc<Mutex<Router>>) -> Result<()> {
        let nic = self.net.get_nic()?;
        let net = nic.lock().await;
        net.set_router(r).await
    }

    async fn on_inbound_chunk(&self, c: Box<dyn Chunk + Send + Sync>) {
        log::debug!("received: {}", c);
        match self.on_inbound_chunk_handler {
            0 => {
                self.cbs0.fetch_add(1, Ordering::SeqCst);
            }
            1 => {
                let mut done_ch_tx = self.done_ch_tx.lock().await;
                done_ch_tx.take();
            }
            2 => {
                let delay = SystemTime::now()
                    .duration_since(c.get_timestamp())
                    .unwrap_or(Duration::from_secs(0));
                {
                    let mut delay_res = self.delay_res.lock().await;
                    delay_res.push(delay);
                }

                let n = self.cbs0.fetch_add(1, Ordering::SeqCst);
                if n >= self.npkts - 1 {
                    let mut done_ch_tx = self.done_ch_tx.lock().await;
                    done_ch_tx.take();
                }
            }
            3 => {
                // echo the chunk
                let mut echo = c.clone_to();
                let result = echo.set_source_addr(&c.destination_addr().to_string());
                assert!(result.is_ok(), "should succeed");
                let result = echo.set_destination_addr(&c.source_addr().to_string());
                assert!(result.is_ok(), "should succeed");

                log::debug!("wan.push being called..");
                if let Net::VNet(vnet) = &self.net {
                    let net = vnet.lock().await;
                    let vi = net.vi.lock().await;
                    if let Some(r) = &vi.router {
                        let wan = r.lock().await;
                        wan.push(echo).await;
                    }
                }
                log::debug!("wan.push called!");
            }
            _ => {}
        };
    }

    async fn get_static_ips(&self) -> Vec<IpAddr> {
        let nic = match self.net.get_nic() {
            Ok(nic) => nic,
            Err(_) => return vec![],
        };
        let net = nic.lock().await;
        net.get_static_ips().await
    }
}

async fn get_ipaddr(nic: &Arc<Mutex<dyn Nic + Send + Sync>>) -> Result<IpAddr> {
    let n = nic.lock().await;
    let eth0 = n.get_interface("eth0").await.ok_or(Error::ErrNoInterface)?;
    let addrs = eth0.addrs();
    if addrs.is_empty() {
        Err(Error::ErrNoAddressAssigned)
    } else {
        Ok(addrs[0].addr())
    }
}

#[test]
fn test_router_standalone_cidr_parsing() -> Result<()> {
    let r = Router::new(RouterConfig {
        cidr: "1.2.3.0/24".to_string(),
        ..Default::default()
    })?;

    assert_eq!(r.ipv4net.addr().to_string(), "1.2.3.0", "ip should match");
    assert_eq!(
        r.ipv4net.netmask().to_string(),
        "255.255.255.0",
        "mask should match"
    );

    Ok(())
}

#[tokio::test]
async fn test_router_standalone_assign_ip_address() -> Result<()> {
    let r = Router::new(RouterConfig {
        cidr: "1.2.3.0/24".to_string(),
        ..Default::default()
    })?;

    let mut ri = r.router_internal.lock().await;
    for i in 1..255 {
        let ip = match ri.assign_ip_address()? {
            IpAddr::V4(ip) => ip.octets().to_vec(),
            IpAddr::V6(ip) => ip.octets().to_vec(),
        };
        assert_eq!(ip[0], 1_u8, "should match");
        assert_eq!(ip[1], 2_u8, "should match");
        assert_eq!(ip[2], 3_u8, "should match");
        assert_eq!(ip[3], i as u8, "should match");
    }

    let result = ri.assign_ip_address();
    assert!(result.is_err(), "assign_ip_address should fail");

    Ok(())
}

#[tokio::test]
async fn test_router_standalone_add_net() -> Result<()> {
    let wan = Arc::new(Mutex::new(Router::new(RouterConfig {
        cidr: "1.2.3.0/24".to_string(),
        ..Default::default()
    })?));

    let net = Net::new(Some(NetConfig::default()));

    let nic = net.get_nic()?;

    {
        let mut w = wan.lock().await;
        w.add_net(Arc::clone(&nic)).await?;
    }

    let n = nic.lock().await;
    n.set_router(Arc::clone(&wan)).await?;

    let eth0 = n.get_interface("eth0").await;
    assert!(eth0.is_some(), "should succeed");
    if let Some(eth0) = eth0 {
        let addrs = eth0.addrs();
        assert_eq!(addrs.len(), 1, "should match");
        assert_eq!(addrs[0].to_string(), "1.2.3.1/24", "should match");
        assert_eq!(addrs[0].addr().to_string(), "1.2.3.1", "should match");
    }

    Ok(())
}

#[tokio::test]
async fn test_router_standalone_routing() -> Result<()> {
    let wan = Arc::new(Mutex::new(Router::new(RouterConfig {
        cidr: "1.2.3.0/24".to_string(),
        ..Default::default()
    })?));

    let (done_ch_tx, mut done_ch_rx) = mpsc::channel(1);
    let mut done_ch_tx = Some(done_ch_tx);

    let mut nics = vec![];
    let mut ips = vec![];
    for i in 0..2 {
        let dn = DummyNic {
            net: Net::new(Some(NetConfig::default())),
            on_inbound_chunk_handler: i,
            ..Default::default()
        };
        if i == 1 {
            let mut done_ch = dn.done_ch_tx.lock().await;
            *done_ch = done_ch_tx.take();
        }
        let nic = Arc::new(Mutex::new(dn));

        {
            let n = Arc::clone(&nic) as Arc<Mutex<dyn Nic + Send + Sync>>;
            let mut w = wan.lock().await;
            w.add_net(n).await?;
        }
        {
            let n = nic.lock().await;
            n.set_router(Arc::clone(&wan)).await?;
        }

        {
            // Now, eth0 must have one address assigned
            let n = nic.lock().await;
            if let Some(eth0) = n.get_interface("eth0").await {
                let addrs = eth0.addrs();
                assert_eq!(addrs.len(), 1, "should match");
                ips.push(SocketAddr::new(addrs[0].addr(), 1111 * (i + 1)));
            }
        }

        nics.push(nic);
    }

    {
        let c = Box::new(ChunkUdp::new(ips[0], ips[1]));

        let mut r = wan.lock().await;
        r.start().await?;
        r.push(c).await;
    }

    let _ = done_ch_rx.recv().await;

    {
        let mut r = wan.lock().await;
        r.stop().await?;
    }

    {
        let n = nics[0].lock().await;
        assert_eq!(n.cbs0.load(Ordering::SeqCst), 0, "should be zero");
    }

    Ok(())
}

//use std::io::Write;

#[tokio::test]
async fn test_router_standalone_add_chunk_filter() -> Result<()> {
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

    let wan = Arc::new(Mutex::new(Router::new(RouterConfig {
        cidr: "1.2.3.0/24".to_string(),
        ..Default::default()
    })?));

    let mut nics = vec![];
    let mut ips = vec![];
    for i in 0..2 {
        let dn = DummyNic {
            net: Net::new(Some(NetConfig::default())),
            on_inbound_chunk_handler: 0,
            ..Default::default()
        };
        let nic = Arc::new(Mutex::new(dn));

        {
            let n = Arc::clone(&nic) as Arc<Mutex<dyn Nic + Send + Sync>>;
            let mut w = wan.lock().await;
            w.add_net(n).await?;
        }
        {
            let n = nic.lock().await;
            n.set_router(Arc::clone(&wan)).await?;
        }

        {
            // Now, eth0 must have one address assigned
            let n = nic.lock().await;
            if let Some(eth0) = n.get_interface("eth0").await {
                let addrs = eth0.addrs();
                assert_eq!(addrs.len(), 1, "should match");
                ips.push(SocketAddr::new(addrs[0].addr(), 1111 * (i + 1)));
            }
        }

        nics.push(nic);
    }

    // this creates a filter that block the first chunk
    let make_filter_fn = |name: String| {
        let n = AtomicUsize::new(0);
        Box::new(move |c: &(dyn Chunk + Send + Sync)| -> bool {
            let m = n.fetch_add(1, Ordering::SeqCst);
            let pass = m > 0;
            if pass {
                log::debug!("{}: {} passed {}", m, name, c);
            } else {
                log::debug!("{}: {} blocked {}", m, name, c);
            }
            pass
        })
    };

    {
        let mut r = wan.lock().await;
        r.add_chunk_filter(make_filter_fn("filter1".to_owned()))
            .await;
        r.add_chunk_filter(make_filter_fn("filter2".to_owned()))
            .await;
        r.start().await?;

        // send 3 packets
        for i in 0..3u8 {
            let mut c = ChunkUdp::new(ips[0], ips[1]);
            c.user_data = vec![i]; // 1-byte seq num
            r.push(Box::new(c)).await;
        }
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let mut r = wan.lock().await;
        r.stop().await?;
    }

    {
        let n = nics[0].lock().await;
        assert_eq!(n.cbs0.load(Ordering::SeqCst), 0, "should be zero");
    }

    {
        let n = nics[1].lock().await;
        assert_eq!(n.cbs0.load(Ordering::SeqCst), 1, "should be one");
    }

    Ok(())
}

async fn delay_sub_test(title: String, min_delay: Duration, max_jitter: Duration) -> Result<()> {
    let wan = Arc::new(Mutex::new(Router::new(RouterConfig {
        cidr: "1.2.3.0/24".to_string(),
        min_delay,
        max_jitter,
        ..Default::default()
    })?));

    let npkts = 1;
    let (done_ch_tx, mut done_ch_rx) = mpsc::channel(1);
    let mut done_ch_tx = Some(done_ch_tx);

    let mut nics = vec![];
    let mut ips = vec![];
    for i in 0..2 {
        let mut dn = DummyNic {
            net: Net::new(Some(NetConfig::default())),
            on_inbound_chunk_handler: 0,
            ..Default::default()
        };
        if i == 1 {
            dn.on_inbound_chunk_handler = 2;
            dn.npkts = npkts;

            let mut done_ch = dn.done_ch_tx.lock().await;
            *done_ch = done_ch_tx.take();
        }
        let nic = Arc::new(Mutex::new(dn));

        {
            let n = Arc::clone(&nic) as Arc<Mutex<dyn Nic + Send + Sync>>;
            let mut w = wan.lock().await;
            w.add_net(n).await?;
        }
        {
            let n = nic.lock().await;
            n.set_router(Arc::clone(&wan)).await?;
        }

        {
            // Now, eth0 must have one address assigned
            let n = nic.lock().await;
            if let Some(eth0) = n.get_interface("eth0").await {
                let addrs = eth0.addrs();
                assert_eq!(addrs.len(), 1, "should match");
                ips.push(SocketAddr::new(addrs[0].addr(), 1111 * (i + 1)));
            }
        }

        nics.push(nic);
    }

    {
        let mut r = wan.lock().await;
        r.start().await?;

        for _ in 0..npkts {
            let c = Box::new(ChunkUdp::new(ips[0], ips[1]));
            r.push(c).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    let _ = done_ch_rx.recv().await;

    {
        let mut r = wan.lock().await;
        r.stop().await?;
    }

    // Validate the amount of delays
    {
        let n = nics[1].lock().await;
        let delay_res = n.delay_res.lock().await;
        for d in &*delay_res {
            log::info!("min delay : {:?}", min_delay);
            log::info!("max jitter: {:?}", max_jitter);
            log::info!("actual delay: {:?}", d);
            assert!(*d >= min_delay, "{title} should delay {d:?} >= 20ms");
            assert!(
                *d <= (min_delay + max_jitter + MARGIN),
                "{title} should delay {d:?} <= minDelay + maxJitter",
            );
            // Note: actual delay should be within 30ms but giving a 8ms
            // MARGIN for possible extra delay
            // (e.g. wakeup delay, debug logs, etc)
        }
    }

    Ok(())
}

//use std::io::Write;
#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_router_delay() -> Result<()> {
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

    delay_sub_test(
        "Delay only".to_owned(),
        Duration::from_millis(20),
        Duration::from_millis(0),
    )
    .await?;
    delay_sub_test(
        "Jitter only".to_owned(),
        Duration::from_millis(0),
        Duration::from_millis(10),
    )
    .await?;
    delay_sub_test(
        "Delay and Jitter".to_owned(),
        Duration::from_millis(20),
        Duration::from_millis(10),
    )
    .await?;

    Ok(())
}

//use std::io::Write;

#[tokio::test]
async fn test_router_one_child() -> Result<()> {
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

    let (done_ch_tx, mut done_ch_rx) = mpsc::channel(1);
    let mut done_ch_tx = Some(done_ch_tx);

    let mut rs = vec![];
    let mut nics = vec![];
    let mut ips = vec![];
    for i in 0..2 {
        let r = Arc::new(Mutex::new(Router::new(RouterConfig {
            cidr: if i == 0 {
                "1.2.3.0/24".to_owned()
            } else {
                "192.168.0.0/24".to_owned()
            },
            ..Default::default()
        })?));

        let mut dn = DummyNic {
            net: Net::new(Some(NetConfig::default())),
            on_inbound_chunk_handler: i,
            ..Default::default()
        };
        if i == 1 {
            let mut done_ch = dn.done_ch_tx.lock().await;
            *done_ch = done_ch_tx.take();
        } else {
            dn.on_inbound_chunk_handler = 3;
        }
        let nic = Arc::new(Mutex::new(dn));

        {
            let n = Arc::clone(&nic) as Arc<Mutex<dyn Nic + Send + Sync>>;
            let mut w = r.lock().await;
            w.add_net(n).await?;
        }
        {
            let n = nic.lock().await;
            n.set_router(Arc::clone(&r)).await?;
        }

        {
            let n = Arc::clone(&nic) as Arc<Mutex<dyn Nic + Send + Sync>>;
            let ip = get_ipaddr(&n).await?;
            ips.push(ip);
        }

        nics.push(nic);
        rs.push(r);
    }

    {
        let child = Arc::clone(&rs[1]);
        let mut wan = rs[0].lock().await;
        wan.add_router(child).await?;
    }
    {
        let parent = Arc::clone(&rs[0]);
        let lan = rs[1].lock().await;
        lan.set_router(parent).await?;
    }

    {
        let mut wan = rs[0].lock().await;
        wan.start().await?;
    }

    {
        let c = Box::new(ChunkUdp::new(
            SocketAddr::new(ips[1], 1234), //lanIP
            SocketAddr::new(ips[0], 5678), //wanIP
        ));
        log::debug!("sending {}", c);
        let lan = rs[1].lock().await;
        lan.push(c).await;
    }

    log::debug!("waiting done_ch_rx");
    let _ = done_ch_rx.recv().await;

    {
        let mut wan = rs[0].lock().await;
        wan.stop().await?;
    }

    Ok(())
}

#[test]
fn test_router_static_ips_more_than_one() -> Result<()> {
    let lan = Router::new(RouterConfig {
        cidr: "192.168.0.0/24".to_owned(),
        static_ips: vec![
            "1.2.3.1".to_owned(),
            "1.2.3.2".to_owned(),
            "1.2.3.3".to_owned(),
        ],
        ..Default::default()
    })?;

    assert_eq!(lan.static_ips.len(), 3, "should be 3");
    assert_eq!(lan.static_ips[0].to_string(), "1.2.3.1", "should match");
    assert_eq!(lan.static_ips[1].to_string(), "1.2.3.2", "should match");
    assert_eq!(lan.static_ips[2].to_string(), "1.2.3.3", "should match");

    Ok(())
}

#[test]
fn test_router_static_ips_static_ip_local_ip_mapping() -> Result<()> {
    let lan = Router::new(RouterConfig {
        cidr: "192.168.0.0/24".to_owned(),
        static_ips: vec![
            "1.2.3.1/192.168.0.1".to_owned(),
            "1.2.3.2/192.168.0.2".to_owned(),
            "1.2.3.3/192.168.0.3".to_owned(),
        ],
        ..Default::default()
    })?;

    assert_eq!(lan.static_ips.len(), 3, "should be 3");
    assert_eq!(lan.static_ips[0].to_string(), "1.2.3.1", "should match");
    assert_eq!(lan.static_ips[1].to_string(), "1.2.3.2", "should match");
    assert_eq!(lan.static_ips[2].to_string(), "1.2.3.3", "should match");

    assert_eq!(3, lan.static_local_ips.len(), "should be 3");
    let local_ips = ["192.168.0.1", "192.168.0.2", "192.168.0.3"];
    let ips = ["1.2.3.1", "1.2.3.2", "1.2.3.3"];
    for i in 0..3 {
        let ext_ipstr = ips[i];
        if let Some(loc_ip) = lan.static_local_ips.get(ext_ipstr) {
            assert_eq!(local_ips[i], loc_ip.to_string(), "should match");
        } else {
            panic!("should have the external IP");
        }
    }

    // bad local IP
    let result = Router::new(RouterConfig {
        cidr: "192.168.0.0/24".to_owned(),
        static_ips: vec![
            "1.2.3.1/192.168.0.1".to_owned(),
            "1.2.3.2/bad".to_owned(), // <-- invalid local IP
        ],
        ..Default::default()
    });
    assert!(result.is_err(), "should fail");

    // local IP out of CIDR
    let result = Router::new(RouterConfig {
        cidr: "192.168.0.0/24".to_owned(),
        static_ips: vec![
            "1.2.3.1/192.168.0.1".to_owned(),
            "1.2.3.2/172.16.1.2".to_owned(), // <-- out of CIDR
        ],
        ..Default::default()
    });
    assert!(result.is_err(), "should fail");

    // num of local IPs mismatch
    let result = Router::new(RouterConfig {
        cidr: "192.168.0.0/24".to_owned(),
        static_ips: vec![
            "1.2.3.1/192.168.0.1".to_owned(),
            "1.2.3.2".to_owned(), // <-- lack of local IP
        ],
        ..Default::default()
    });
    assert!(result.is_err(), "should fail");

    Ok(())
}

#[tokio::test]
async fn test_router_static_ips_1to1_nat() -> Result<()> {
    let wan = Arc::new(Mutex::new(Router::new(RouterConfig {
        cidr: "0.0.0.0/0".to_owned(),
        ..Default::default()
    })?));

    let lan = Arc::new(Mutex::new(Router::new(RouterConfig {
        cidr: "192.168.0.0/24".to_owned(),
        static_ips: vec![
            "1.2.3.1/192.168.0.1".to_owned(),
            "1.2.3.2/192.168.0.2".to_owned(),
            "1.2.3.3/192.168.0.3".to_owned(),
        ],
        nat_type: Some(NatType {
            mode: NatMode::Nat1To1,
            ..Default::default()
        }),
        ..Default::default()
    })?));

    {
        let mut w = wan.lock().await;
        w.add_router(Arc::clone(&lan)).await?;
    }
    {
        let n = lan.lock().await;
        n.set_router(Arc::clone(&wan)).await?;
    }

    {
        let l = lan.lock().await;
        let ri = l.router_internal.lock().await;

        assert_eq!(ri.nat.mapped_ips.len(), 3, "should be 3");
        assert_eq!(ri.nat.mapped_ips[0].to_string(), "1.2.3.1", "should match");
        assert_eq!(ri.nat.mapped_ips[1].to_string(), "1.2.3.2", "should match");
        assert_eq!(ri.nat.mapped_ips[2].to_string(), "1.2.3.3", "should match");

        assert_eq!(3, ri.nat.local_ips.len(), "should be 3");
        assert_eq!(
            ri.nat.local_ips[0].to_string(),
            "192.168.0.1",
            "should match"
        );
        assert_eq!(
            ri.nat.local_ips[1].to_string(),
            "192.168.0.2",
            "should match"
        );
        assert_eq!(
            ri.nat.local_ips[2].to_string(),
            "192.168.0.3",
            "should match"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_router_failures_stop() -> Result<()> {
    let mut r = Router::new(RouterConfig {
        cidr: "1.2.3.0/24".to_owned(),
        ..Default::default()
    })?;

    let result = r.stop().await;
    assert!(result.is_err(), "should fail");

    Ok(())
}

#[tokio::test]
async fn test_router_failures_add_net() -> Result<()> {
    let wan = Arc::new(Mutex::new(Router::new(RouterConfig {
        cidr: "1.2.3.0/24".to_owned(),
        ..Default::default()
    })?));

    let net = Net::new(Some(NetConfig {
        static_ips: vec![
            "5.6.7.8".to_owned(), // out of parent router'c CIDR
        ],
        ..Default::default()
    }));

    {
        let nic = net.get_nic()?;
        let mut w = wan.lock().await;
        let result = w.add_net(nic).await;
        assert!(result.is_err(), "should fail");
    }

    Ok(())
}

#[tokio::test]
async fn test_router_failures_add_router() -> Result<()> {
    let r1 = Arc::new(Mutex::new(Router::new(RouterConfig {
        cidr: "1.2.3.0/24".to_owned(),
        ..Default::default()
    })?));

    let r2 = Arc::new(Mutex::new(Router::new(RouterConfig {
        cidr: "192.168.0.0/24".to_owned(),
        static_ips: vec![
            "5.6.7.8".to_owned(), // out of parent router'c CIDR
        ],
        ..Default::default()
    })?));

    {
        let mut r = r1.lock().await;
        let result = r.add_router(Arc::clone(&r2)).await;
        assert!(result.is_err(), "should fail");
    }

    Ok(())
}
