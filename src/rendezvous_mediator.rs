use crate::server::{check_zombie, new as new_server, ServerPtr};
use hbb_common::{
    allow_err,
    anyhow::bail,
    config::{self, Config, REG_INTERVAL, RENDEZVOUS_PORT, RENDEZVOUS_TIMEOUT},
    futures::future::join_all,
    log,
    protobuf::Message as _,
    rendezvous_proto::*,
    sleep, socket_client,
    tokio::{
        self, select,
        time::{interval, Duration},
    },
    udp::FramedSocket,
    AddrMangle, IntoTargetAddr, ResultType, TargetAddr,
};
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use uuid::Uuid;

type Message = RendezvousMessage;

lazy_static::lazy_static! {
    static ref SOLVING_PK_MISMATCH: Arc<Mutex<String>> = Default::default();
}
static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
pub struct RendezvousMediator {
    addr: TargetAddr<'static>,
    host: String,
    host_prefix: String,
    last_id_pk_registry: String,
}

impl RendezvousMediator {
    pub fn restart() {
        SHOULD_EXIT.store(true, Ordering::SeqCst);
        log::info!("server restart");
    }

    pub async fn start_all() {
        let mut nat_tested = false;
        check_zombie();
        let server = new_server();
        if Config::get_nat_type() == NatType::UNKNOWN_NAT as i32 {
            crate::common::test_nat_type();
            nat_tested = true;
        }
        let server_cloned = server.clone();
        tokio::spawn(async move {
            direct_server(server_cloned).await;
        });
        if crate::platform::is_installed() {
            std::thread::spawn(move || {
                allow_err!(lan_discovery());
            });
        }
        loop {
            Config::reset_online();
            if Config::get_option("stop-service").is_empty() {
                if !nat_tested {
                    crate::common::test_nat_type();
                    nat_tested = true;
                }
                let mut futs = Vec::new();
                let servers = Config::get_rendezvous_servers();
                SHOULD_EXIT.store(false, Ordering::SeqCst);
                for host in servers.clone() {
                    let server = server.clone();
                    futs.push(tokio::spawn(async move {
                        allow_err!(Self::start(server, host).await);
                        // SHOULD_EXIT here is to ensure once one exits, the others also exit.
                        SHOULD_EXIT.store(true, Ordering::SeqCst);
                    }));
                }
                join_all(futs).await;
            }
            sleep(1.).await;
        }
    }

    pub async fn start(server: ServerPtr, host: String) -> ResultType<()> {
        log::info!("start rendezvous mediator of {}", host);
        let host_prefix: String = host
            .split(".")
            .next()
            .map(|x| {
                if x.parse::<i32>().is_ok() {
                    host.clone()
                } else {
                    x.to_string()
                }
            })
            .unwrap_or(host.to_owned());
        let mut rz = Self {
            addr: Config::get_any_listen_addr().into_target_addr()?,
            host: host.clone(),
            host_prefix,
            last_id_pk_registry: "".to_owned(),
        };

        rz.addr = socket_client::get_target_addr(&crate::check_port(&host, RENDEZVOUS_PORT))?;
        let any_addr = Config::get_any_listen_addr();
        let mut socket = socket_client::new_udp(any_addr, RENDEZVOUS_TIMEOUT).await?;

        const TIMER_OUT: Duration = Duration::from_secs(1);
        let mut timer = interval(TIMER_OUT);
        let mut last_timer: Option<Instant> = None;
        const REG_TIMEOUT: i64 = 3_000;
        const MAX_FAILS1: i64 = 3;
        const MAX_FAILS2: i64 = 6;
        const DNS_INTERVAL: i64 = 60_000;
        let mut fails = 0;
        let mut last_register_resp: Option<Instant> = None;
        let mut last_register_sent: Option<Instant> = None;
        let mut last_dns_check = Instant::now();
        let mut old_latency = 0;
        let mut ema_latency = 0;
        loop {
            let mut update_latency = || {
                last_register_resp = Some(Instant::now());
                fails = 0;
                let mut latency = last_register_sent
                    .map(|x| x.elapsed().as_micros() as i64)
                    .unwrap_or(0);
                if latency < 0 || latency > 1_000_000 {
                    return;
                }
                if ema_latency == 0 {
                    ema_latency = latency;
                } else {
                    ema_latency = latency / 30 + (ema_latency * 29 / 30);
                    latency = ema_latency;
                }
                let mut n = latency / 5;
                if n < 3000 {
                    n = 3000;
                }
                if (latency - old_latency).abs() > n || old_latency <= 0 {
                    Config::update_latency(&host, latency);
                    log::debug!("Latency of {}: {}ms", host, latency as f64 / 1000.);
                    old_latency = latency;
                }
            };
            select! {
                n = socket.next() => {
                    match n {
                        Some(Ok((bytes, _))) => {
                            if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                                match msg_in.union {
                                    Some(rendezvous_message::Union::register_peer_response(rpr)) => {
                                        update_latency();
                                        if rpr.request_pk {
                                            log::info!("request_pk received from {}", host);
                                            allow_err!(rz.register_pk(&mut socket).await);
                                            continue;
                                        }
                                    }
                                    Some(rendezvous_message::Union::register_pk_response(rpr)) => {
                                        update_latency();
                                        match rpr.result.enum_value_or_default() {
                                            register_pk_response::Result::OK => {
                                                Config::set_key_confirmed(true);
                                                Config::set_host_key_confirmed(&rz.host_prefix, true);
                                                *SOLVING_PK_MISMATCH.lock().unwrap() = "".to_owned();
                                            }
                                            register_pk_response::Result::UUID_MISMATCH => {
                                                allow_err!(rz.handle_uuid_mismatch(&mut socket).await);
                                            }
                                            _ => {}
                                        }
                                    }
                                    Some(rendezvous_message::Union::punch_hole(ph)) => {
                                        let rz = rz.clone();
                                        let server = server.clone();
                                        tokio::spawn(async move {
                                            allow_err!(rz.handle_punch_hole(ph, server).await);
                                        });
                                    }
                                    Some(rendezvous_message::Union::request_relay(rr)) => {
                                        let rz = rz.clone();
                                        let server = server.clone();
                                        tokio::spawn(async move {
                                            allow_err!(rz.handle_request_relay(rr, server).await);
                                        });
                                    }
                                    Some(rendezvous_message::Union::fetch_local_addr(fla)) => {
                                        let rz = rz.clone();
                                        let server = server.clone();
                                        tokio::spawn(async move {
                                            allow_err!(rz.handle_intranet(fla, server).await);
                                        });
                                    }
                                    Some(rendezvous_message::Union::configure_update(cu)) => {
                                        let v0 = Config::get_rendezvous_servers();
                                        Config::set_option("rendezvous-servers".to_owned(), cu.rendezvous_servers.join(","));
                                        Config::set_serial(cu.serial);
                                        if v0 != Config::get_rendezvous_servers() {
                                            Self::restart();
                                        }
                                    }
                                    _ => {}
                                }
                            } else {
                                log::debug!("Non-protobuf message bytes received: {:?}", bytes);
                            }
                        },
                        Some(Err(e)) => bail!("Failed to receive next {}", e),  // maybe socks5 tcp disconnected
                        None => {
                            // unreachable!()
                        },
                    }
                },
                _ = timer.tick() => {
                    if SHOULD_EXIT.load(Ordering::SeqCst) {
                        break;
                    }
                    let now = Some(Instant::now());
                    if last_timer.map(|x| x.elapsed() < TIMER_OUT).unwrap_or(false) {
                        // a workaround of tokio timer bug
                        continue;
                    }
                    last_timer = now;
                    let elapsed_resp = last_register_resp.map(|x| x.elapsed().as_millis() as i64).unwrap_or(REG_INTERVAL);
                    let timeout = (last_register_sent.map(|x| x.elapsed().as_millis() as i64).unwrap_or(REG_INTERVAL) - elapsed_resp) > REG_TIMEOUT;
                    if timeout || elapsed_resp >= REG_INTERVAL {
                        allow_err!(rz.register_peer(&mut socket).await);
                        last_register_sent = now;
                        if timeout {
                            fails += 1;
                            if fails > MAX_FAILS2 {
                                Config::update_latency(&host, -1);
                                old_latency = 0;
                                if last_dns_check.elapsed().as_millis() as i64 > DNS_INTERVAL {
                                    rz.addr = socket_client::get_target_addr(&crate::check_port(&host, RENDEZVOUS_PORT))?;
                                    // in some case of network reconnect (dial IP network),
                                    // old UDP socket not work any more after network recover
                                    if let Some(s) = socket_client::rebind_udp(any_addr).await? {
                                        socket = s;
                                    }
                                    last_dns_check = Instant::now();
                                }
                            } else if fails > MAX_FAILS1 {
                                Config::update_latency(&host, 0);
                                old_latency = 0;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_request_relay(&self, rr: RequestRelay, server: ServerPtr) -> ResultType<()> {
        self.create_relay(
            rr.socket_addr,
            rr.relay_server,
            rr.uuid,
            server,
            rr.secure,
            false,
        )
        .await
    }

    async fn create_relay(
        &self,
        socket_addr: Vec<u8>,
        relay_server: String,
        uuid: String,
        server: ServerPtr,
        secure: bool,
        initiate: bool,
    ) -> ResultType<()> {
        let peer_addr = AddrMangle::decode(&socket_addr);
        log::info!(
            "create_relay requested from from {:?}, relay_server: {}, uuid: {}, secure: {}",
            peer_addr,
            relay_server,
            uuid,
            secure,
        );

        let mut socket = socket_client::connect_tcp(
            self.addr.to_owned(),
            Config::get_any_listen_addr(),
            RENDEZVOUS_TIMEOUT,
        )
        .await?;

        let mut msg_out = Message::new();
        let mut rr = RelayResponse {
            socket_addr,
            version: crate::VERSION.to_owned(),
            ..Default::default()
        };
        if initiate {
            rr.uuid = uuid.clone();
            rr.relay_server = relay_server.clone();
            rr.set_id(Config::get_id());
        }
        msg_out.set_relay_response(rr);
        socket.send(&msg_out).await?;
        crate::create_relay_connection(server, relay_server, uuid, peer_addr, secure).await;
        Ok(())
    }

    async fn handle_intranet(&self, fla: FetchLocalAddr, server: ServerPtr) -> ResultType<()> {
        let peer_addr = AddrMangle::decode(&fla.socket_addr);
        log::debug!("Handle intranet from {:?}", peer_addr);
        let mut socket = socket_client::connect_tcp(
            self.addr.to_owned(),
            Config::get_any_listen_addr(),
            RENDEZVOUS_TIMEOUT,
        )
        .await?;
        let local_addr = socket.local_addr();
        let local_addr: SocketAddr =
            format!("{}:{}", local_addr.ip(), local_addr.port()).parse()?;
        let mut msg_out = Message::new();
        let relay_server = self.get_relay_server(fla.relay_server);
        msg_out.set_local_addr(LocalAddr {
            id: Config::get_id(),
            socket_addr: AddrMangle::encode(peer_addr),
            local_addr: AddrMangle::encode(local_addr),
            relay_server,
            version: crate::VERSION.to_owned(),
            ..Default::default()
        });
        let bytes = msg_out.write_to_bytes()?;
        socket.send_raw(bytes).await?;
        crate::accept_connection(server.clone(), socket, peer_addr, true).await;
        Ok(())
    }

    async fn handle_punch_hole(&self, ph: PunchHole, server: ServerPtr) -> ResultType<()> {
        let relay_server = self.get_relay_server(ph.relay_server);
        if ph.nat_type.enum_value_or_default() == NatType::SYMMETRIC
            || Config::get_nat_type() == NatType::SYMMETRIC as i32
        {
            let uuid = Uuid::new_v4().to_string();
            return self
                .create_relay(ph.socket_addr, relay_server, uuid, server, true, true)
                .await;
        }
        let peer_addr = AddrMangle::decode(&ph.socket_addr);
        log::debug!("Punch hole to {:?}", peer_addr);
        let mut socket = {
            let socket = socket_client::connect_tcp(
                self.addr.to_owned(),
                Config::get_any_listen_addr(),
                RENDEZVOUS_TIMEOUT,
            )
            .await?;
            let local_addr = socket.local_addr();
            allow_err!(socket_client::connect_tcp(peer_addr, local_addr, 300).await);
            socket
        };
        let mut msg_out = Message::new();
        use hbb_common::protobuf::ProtobufEnum;
        let nat_type = NatType::from_i32(Config::get_nat_type()).unwrap_or(NatType::UNKNOWN_NAT);
        msg_out.set_punch_hole_sent(PunchHoleSent {
            socket_addr: ph.socket_addr,
            id: Config::get_id(),
            relay_server,
            nat_type: nat_type.into(),
            version: crate::VERSION.to_owned(),
            ..Default::default()
        });
        let bytes = msg_out.write_to_bytes()?;
        socket.send_raw(bytes).await?;
        crate::accept_connection(server.clone(), socket, peer_addr, true).await;
        Ok(())
    }

    async fn register_pk(&mut self, socket: &mut FramedSocket) -> ResultType<()> {
        let mut msg_out = Message::new();
        let pk = Config::get_key_pair().1;
        let uuid = if let Ok(id) = machine_uid::get() {
            log::info!("machine uid: {}", id);
            id.into()
        } else {
            pk.clone()
        };
        let id = Config::get_id();
        self.last_id_pk_registry = id.clone();
        msg_out.set_register_pk(RegisterPk {
            id,
            uuid,
            pk,
            ..Default::default()
        });
        socket.send(&msg_out, self.addr.to_owned()).await?;
        Ok(())
    }

    async fn handle_uuid_mismatch(&mut self, socket: &mut FramedSocket) -> ResultType<()> {
        if self.last_id_pk_registry != Config::get_id() {
            return Ok(());
        }
        {
            let mut solving = SOLVING_PK_MISMATCH.lock().unwrap();
            if solving.is_empty() || *solving == self.host {
                log::info!("UUID_MISMATCH received from {}", self.host);
                Config::set_key_confirmed(false);
                Config::update_id();
                *solving = self.host.clone();
            } else {
                return Ok(());
            }
        }
        self.register_pk(socket).await
    }

    async fn register_peer(&mut self, socket: &mut FramedSocket) -> ResultType<()> {
        if !SOLVING_PK_MISMATCH.lock().unwrap().is_empty() {
            return Ok(());
        }
        if !Config::get_key_confirmed() || !Config::get_host_key_confirmed(&self.host_prefix) {
            log::info!(
                "register_pk of {} due to key not confirmed",
                self.host_prefix
            );
            return self.register_pk(socket).await;
        }
        let id = Config::get_id();
        log::trace!(
            "Register my id {:?} to rendezvous server {:?}",
            id,
            self.addr,
        );
        let mut msg_out = Message::new();
        let serial = Config::get_serial();
        msg_out.set_register_peer(RegisterPeer {
            id,
            serial,
            ..Default::default()
        });
        socket.send(&msg_out, self.addr.to_owned()).await?;
        Ok(())
    }

    fn get_relay_server(&self, provided_by_rendzvous_server: String) -> String {
        let mut relay_server = Config::get_option("relay-server");
        if relay_server.is_empty() {
            relay_server = provided_by_rendzvous_server;
        }
        if relay_server.is_empty() {
            if self.host.contains(":") {
                let tmp: Vec<&str> = self.host.split(":").collect();
                if tmp.len() == 2 {
                    let port: u16 = tmp[1].parse().unwrap_or(0);
                    relay_server = format!("{}:{}", tmp[0], port + 1);
                }
            } else {
                relay_server = self.host.clone();
            }
        }
        relay_server
    }
}

fn get_direct_port() -> i32 {
    let mut port = Config::get_option("direct-access-port")
        .parse::<i32>()
        .unwrap_or(0);
    if port <= 0 {
        port = RENDEZVOUS_PORT + 2;
    }
    port
}

async fn direct_server(server: ServerPtr) {
    let mut listener = None;
    let mut port = 0;
    loop {
        let disabled = Config::get_option("direct-server").is_empty();
        if !disabled && listener.is_none() {
            port = get_direct_port();
            let addr = format!("0.0.0.0:{}", port);
            match hbb_common::tcp::new_listener(&addr, false).await {
                Ok(l) => {
                    listener = Some(l);
                    log::info!(
                        "Direct server listening on: {:?}",
                        listener.as_ref().unwrap().local_addr()
                    );
                }
                Err(err) => {
                    // to-do: pass to ui
                    log::error!(
                        "Failed to start direct server on : {}, error: {}",
                        addr,
                        err
                    );
                    loop {
                        if port != get_direct_port() {
                            break;
                        }
                        sleep(1.).await;
                    }
                }
            }
        }
        if let Some(l) = listener.as_mut() {
            if disabled || port != get_direct_port() {
                log::info!("Exit direct access listen");
                listener = None;
                continue;
            }
            if let Ok(Ok((stream, addr))) = hbb_common::timeout(1000, l.accept()).await {
                stream.set_nodelay(true).ok();
                log::info!("direct access from {}", addr);
                let local_addr = stream.local_addr().unwrap_or(Config::get_any_listen_addr());
                let server = server.clone();
                tokio::spawn(async move {
                    allow_err!(
                        crate::server::create_tcp_connection(
                            server,
                            hbb_common::Stream::from(stream, local_addr),
                            addr,
                            false,
                        )
                        .await
                    );
                });
            } else {
                sleep(0.1).await;
            }
        } else {
            sleep(1.).await;
        }
    }
}

#[inline]
pub fn get_broadcast_port() -> u16 {
    (RENDEZVOUS_PORT + 3) as _
}

pub fn get_mac() -> String {
    if let Ok(Some(mac)) = mac_address::get_mac_address() {
        mac.to_string()
    } else {
        "".to_owned()
    }
}

fn lan_discovery() -> ResultType<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], get_broadcast_port()));
    let socket = std::net::UdpSocket::bind(addr)?;
    socket.set_read_timeout(Some(std::time::Duration::from_millis(1000)))?;
    log::info!("lan discovery listener started");
    loop {
        let mut buf = [0; 2048];
        if let Ok((len, addr)) = socket.recv_from(&mut buf) {
            if let Ok(msg_in) = Message::parse_from_bytes(&buf[0..len]) {
                match msg_in.union {
                    Some(rendezvous_message::Union::peer_discovery(p)) => {
                        if p.cmd == "ping" {
                            let mut msg_out = Message::new();
                            let peer = PeerDiscovery {
                                cmd: "pong".to_owned(),
                                mac: get_mac(),
                                id: Config::get_id(),
                                hostname: whoami::hostname(),
                                username: crate::platform::get_active_username(),
                                platform: whoami::platform().to_string(),
                                ..Default::default()
                            };
                            msg_out.set_peer_discovery(peer);
                            socket.send_to(&msg_out.write_to_bytes()?, addr).ok();
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

pub fn discover() -> ResultType<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 0));
    let socket = std::net::UdpSocket::bind(addr)?;
    socket.set_broadcast(true)?;
    let mut msg_out = Message::new();
    let peer = PeerDiscovery {
        cmd: "ping".to_owned(),
        ..Default::default()
    };
    msg_out.set_peer_discovery(peer);
    let maddr = SocketAddr::from(([255, 255, 255, 255], get_broadcast_port()));
    socket.send_to(&msg_out.write_to_bytes()?, maddr)?;
    log::info!("discover ping sent");
    let mut last_recv_time = Instant::now();
    let mut last_write_time = Instant::now();
    let mut last_write_n = 0;
    // to-do: load saved peers, and update incrementally (then we can see offline)
    let mut peers = Vec::new();
    let mac = get_mac();
    socket.set_read_timeout(Some(std::time::Duration::from_millis(10)))?;
    loop {
        let mut buf = [0; 2048];
        if let Ok((len, _)) = socket.recv_from(&mut buf) {
            if let Ok(msg_in) = Message::parse_from_bytes(&buf[0..len]) {
                match msg_in.union {
                    Some(rendezvous_message::Union::peer_discovery(p)) => {
                        last_recv_time = Instant::now();
                        if p.cmd == "pong" {
                            if p.mac != mac {
                                peers.push((p.id, p.username, p.hostname, p.platform));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if last_write_time.elapsed().as_millis() > 300 && last_write_n != peers.len() {
            config::LanPeers::store(serde_json::to_string(&peers)?);
            last_write_time = Instant::now();
            last_write_n = peers.len();
        }
        if last_recv_time.elapsed().as_millis() > 3_000 {
            break;
        }
    }
    log::info!("discover ping done");
    config::LanPeers::store(serde_json::to_string(&peers)?);
    Ok(())
}
