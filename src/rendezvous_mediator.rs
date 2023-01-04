use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

use uuid::Uuid;

use hbb_common::tcp::FramedStream;
use hbb_common::{
    allow_err,
    anyhow::bail,
    config::{Config, REG_INTERVAL, RENDEZVOUS_PORT, RENDEZVOUS_TIMEOUT},
    futures::future::join_all,
    log,
    protobuf::Message as _,
    rendezvous_proto::*,
    sleep,
    socket_client::{self, is_ipv4},
    tokio::{
        self, select,
        time::{interval, Duration},
    },
    udp::FramedSocket,
    AddrMangle, ResultType,
};

use crate::server::{check_zombie, new as new_server, ServerPtr};

type Message = RendezvousMessage;

lazy_static::lazy_static! {
    static ref SOLVING_PK_MISMATCH: Arc<Mutex<String>> = Default::default();
}
static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
pub struct RendezvousMediator {
    addr: hbb_common::tokio_socks::TargetAddr<'static>,
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
            crate::test_nat_type();
            nat_tested = true;
        }
        if !Config::get_option("stop-service").is_empty() {
            crate::test_rendezvous_server();
        }
        let server_cloned = server.clone();
        tokio::spawn(async move {
            direct_server(server_cloned).await;
        });
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if crate::platform::is_installed() {
            std::thread::spawn(move || {
                allow_err!(super::lan::start_listening());
            });
        }
        loop {
            Config::reset_online();
            if Config::get_option("stop-service").is_empty() {
                if !nat_tested {
                    crate::test_nat_type();
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
            } else {
                server.write().unwrap().close_connections();
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
        let host = crate::check_port(&host, RENDEZVOUS_PORT);
        let (mut socket, addr) = socket_client::new_udp_for(&host, RENDEZVOUS_TIMEOUT).await?;
        let mut rz = Self {
            addr: addr,
            host: host.clone(),
            host_prefix,
            last_id_pk_registry: "".to_owned(),
        };

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
                                    Some(rendezvous_message::Union::RegisterPeerResponse(rpr)) => {
                                        update_latency();
                                        if rpr.request_pk {
                                            log::info!("request_pk received from {}", host);
                                            allow_err!(rz.register_pk(&mut socket).await);
                                            continue;
                                        }
                                    }
                                    Some(rendezvous_message::Union::RegisterPkResponse(rpr)) => {
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
                                    Some(rendezvous_message::Union::PunchHole(ph)) => {
                                        let rz = rz.clone();
                                        let server = server.clone();
                                        tokio::spawn(async move {
                                            allow_err!(rz.handle_punch_hole(ph, server).await);
                                        });
                                    }
                                    Some(rendezvous_message::Union::RequestRelay(rr)) => {
                                        let rz = rz.clone();
                                        let server = server.clone();
                                        tokio::spawn(async move {
                                            allow_err!(rz.handle_request_relay(rr, server).await);
                                        });
                                    }
                                    Some(rendezvous_message::Union::FetchLocalAddr(fla)) => {
                                        let rz = rz.clone();
                                        let server = server.clone();
                                        tokio::spawn(async move {
                                            allow_err!(rz.handle_intranet(fla, server).await);
                                        });
                                    }
                                    Some(rendezvous_message::Union::ConfigureUpdate(cu)) => {
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
                            bail!("Socket receive none. Maybe socks5 server is down.");
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
                    let timeout = (elapsed_resp - last_register_sent.map(|x| x.elapsed().as_millis() as i64).unwrap_or(REG_INTERVAL)) > REG_TIMEOUT;
                    if timeout || elapsed_resp >= REG_INTERVAL {
                        allow_err!(rz.register_peer(&mut socket).await);
                        last_register_sent = now;
                        if timeout {
                            fails += 1;
                            if fails > MAX_FAILS2 {
                                Config::update_latency(&host, -1);
                                old_latency = 0;
                                if last_dns_check.elapsed().as_millis() as i64 > DNS_INTERVAL {
                                    // in some case of network reconnect (dial IP network),
                                    // old UDP socket not work any more after network recover
                                    if let Some((s, addr)) = socket_client::rebind_udp_for(&rz.host).await? {
                                        socket = s;
                                        rz.addr = addr;
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
            rr.socket_addr.into(),
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
            "create_relay requested from {:?}, relay_server: {}, uuid: {}, secure: {}",
            peer_addr,
            relay_server,
            uuid,
            secure,
        );

        let mut socket = socket_client::connect_tcp(&*self.host, RENDEZVOUS_TIMEOUT).await?;

        let mut msg_out = Message::new();
        let mut rr = RelayResponse {
            socket_addr: socket_addr.into(),
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
        crate::create_relay_connection(
            server,
            relay_server,
            uuid,
            peer_addr,
            secure,
            is_ipv4(&self.addr),
        )
        .await;
        Ok(())
    }

    async fn handle_intranet(&self, fla: FetchLocalAddr, server: ServerPtr) -> ResultType<()> {
        let relay_server = self.get_relay_server(fla.relay_server);
        if !is_ipv4(&self.addr) {
            // nat64, go relay directly, because current hbbs will crash if demangle ipv6 address
            let uuid = Uuid::new_v4().to_string();
            return self
                .create_relay(
                    fla.socket_addr.into(),
                    relay_server,
                    uuid,
                    server,
                    true,
                    true,
                )
                .await;
        }
        let peer_addr = AddrMangle::decode(&fla.socket_addr);
        log::debug!("Handle intranet from {:?}", peer_addr);
        let mut socket = socket_client::connect_tcp(&*self.host, RENDEZVOUS_TIMEOUT).await?;
        let local_addr = socket.local_addr();
        let local_addr: SocketAddr =
            format!("{}:{}", local_addr.ip(), local_addr.port()).parse()?;
        let mut msg_out = Message::new();
        msg_out.set_local_addr(LocalAddr {
            id: Config::get_id(),
            socket_addr: AddrMangle::encode(peer_addr).into(),
            local_addr: AddrMangle::encode(local_addr).into(),
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
                .create_relay(
                    ph.socket_addr.into(),
                    relay_server,
                    uuid,
                    server,
                    true,
                    true,
                )
                .await;
        }
        let peer_addr = AddrMangle::decode(&ph.socket_addr);
        log::debug!("Punch hole to {:?}", peer_addr);
        let mut socket = {
            let socket = socket_client::connect_tcp(&*self.host, RENDEZVOUS_TIMEOUT).await?;
            let local_addr = socket.local_addr();
            // key important here for punch hole to tell my gateway incoming peer is safe.
            // it can not be async here, because local_addr can not be reused, we must close the connection before use it again.
            allow_err!(socket_client::connect_tcp_local(peer_addr, Some(local_addr), 30).await);
            socket
        };
        let mut msg_out = Message::new();
        use hbb_common::protobuf::Enum;
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
        let uuid = hbb_common::get_uuid();
        let id = Config::get_id();
        self.last_id_pk_registry = id.clone();
        msg_out.set_register_pk(RegisterPk {
            id,
            uuid: uuid.into(),
            pk: pk.into(),
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
            match hbb_common::tcp::listen_any(port as _).await {
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
                        "Failed to start direct server on port: {}, error: {}",
                        port,
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
                let local_addr = stream
                    .local_addr()
                    .unwrap_or(Config::get_any_listen_addr(true));
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
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if let Ok(Some(mac)) = mac_address::get_mac_address() {
        mac.to_string()
    } else {
        "".to_owned()
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    "".to_owned()
}

#[allow(dead_code)]
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
                    Some(rendezvous_message::Union::PeerDiscovery(p)) => {
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

#[tokio::main(flavor = "current_thread")]
pub async fn query_online_states<F: FnOnce(Vec<String>, Vec<String>)>(ids: Vec<String>, f: F) {
    let test = false;
    if test {
        sleep(1.5).await;
        let mut onlines = ids;
        let offlines = onlines.drain((onlines.len() / 2)..).collect();
        f(onlines, offlines)
    } else {
        let query_begin = Instant::now();
        let query_timeout = std::time::Duration::from_millis(3_000);
        loop {
            if SHOULD_EXIT.load(Ordering::SeqCst) {
                break;
            }
            match query_online_states_(&ids, query_timeout).await {
                Ok((onlines, offlines)) => {
                    f(onlines, offlines);
                    break;
                }
                Err(e) => {
                    log::debug!("{}", &e);
                }
            }

            if query_begin.elapsed() > query_timeout {
                log::debug!("query onlines timeout {:?}", query_timeout);
                break;
            }

            sleep(1.5).await;
        }
    }
}

async fn create_online_stream() -> ResultType<FramedStream> {
    let (rendezvous_server, _servers, _contained) = crate::get_rendezvous_server(1_000).await;
    let tmp: Vec<&str> = rendezvous_server.split(":").collect();
    if tmp.len() != 2 {
        bail!("Invalid server address: {}", rendezvous_server);
    }
    let port: u16 = tmp[1].parse()?;
    if port == 0 {
        bail!("Invalid server address: {}", rendezvous_server);
    }
    let online_server = format!("{}:{}", tmp[0], port - 1);
    socket_client::connect_tcp(online_server, RENDEZVOUS_TIMEOUT).await
}

async fn query_online_states_(
    ids: &Vec<String>,
    timeout: std::time::Duration,
) -> ResultType<(Vec<String>, Vec<String>)> {
    let query_begin = Instant::now();

    let mut msg_out = RendezvousMessage::new();
    msg_out.set_online_request(OnlineRequest {
        id: Config::get_id(),
        peers: ids.clone(),
        ..Default::default()
    });

    loop {
        if SHOULD_EXIT.load(Ordering::SeqCst) {
            // No need to care about onlines
            return Ok((Vec::new(), Vec::new()));
        }

        let mut socket = create_online_stream().await?;
        socket.send(&msg_out).await?;
        match socket.next_timeout(RENDEZVOUS_TIMEOUT).await {
            Some(Ok(bytes)) => {
                if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                    match msg_in.union {
                        Some(rendezvous_message::Union::OnlineResponse(online_response)) => {
                            let states = online_response.states;
                            let mut onlines = Vec::new();
                            let mut offlines = Vec::new();
                            for i in 0..ids.len() {
                                // bytes index from left to right
                                let bit_value = 0x01 << (7 - i % 8);
                                if (states[i / 8] & bit_value) == bit_value {
                                    onlines.push(ids[i].clone());
                                } else {
                                    offlines.push(ids[i].clone());
                                }
                            }
                            return Ok((onlines, offlines));
                        }
                        _ => {
                            // ignore
                        }
                    }
                }
            }
            Some(Err(e)) => {
                log::error!("Failed to receive {e}");
            }
            None => {
                // TODO: Make sure socket closed?
                bail!("Online stream receives None");
            }
        }

        if query_begin.elapsed() > timeout {
            bail!("Try query onlines timeout {:?}", &timeout);
        }

        sleep(300.0).await;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_query_onlines() {
        super::query_online_states(
            vec![
                "152183996".to_owned(),
                "165782066".to_owned(),
                "155323351".to_owned(),
                "460952777".to_owned(),
            ],
            |onlines: Vec<String>, offlines: Vec<String>| {
                println!("onlines: {:?}, offlines: {:?}", &onlines, &offlines);
            },
        );
    }
}
