use crate::server::{check_zombie, new as new_server, ServerPtr};
use hbb_common::{
    allow_err,
    config::{Config, RENDEZVOUS_PORT, RENDEZVOUS_TIMEOUT},
    futures::future::join_all,
    log,
    protobuf::Message as _,
    rendezvous_proto::*,
    sleep,
    tcp::FramedStream,
    tokio::{
        self, select,
        time::{interval, Duration},
    },
    udp::FramedSocket,
    AddrMangle, ResultType,
};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::SystemTime,
};
use uuid::Uuid;

type Message = RendezvousMessage;

lazy_static::lazy_static! {
    pub static ref SOLVING_PK_MISMATCH: Arc<Mutex<String>> = Default::default();
}

#[derive(Clone)]
pub struct RendezvousMediator {
    addr: SocketAddr,
    host: String,
    host_prefix: String,
    rendezvous_servers: Vec<String>,
    last_id_pk_registery: String,
}

impl RendezvousMediator {
    pub async fn start_all() {
        let mut nat_tested = false;
        check_zombie();
        let server = new_server();
        if Config::get_nat_type() == NatType::UNKNOWN_NAT as i32 {
            crate::common::test_nat_type();
            nat_tested = true;
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
                for host in servers.clone() {
                    let server = server.clone();
                    let servers = servers.clone();
                    futs.push(tokio::spawn(async move {
                        allow_err!(Self::start(server, host, servers).await);
                    }));
                }
                join_all(futs).await;
            }
            sleep(1.).await;
        }
    }

    pub async fn start(
        server: ServerPtr,
        host: String,
        rendezvous_servers: Vec<String>,
    ) -> ResultType<()> {
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
            addr: Config::get_any_listen_addr(),
            host: host.clone(),
            host_prefix,
            rendezvous_servers,
            last_id_pk_registery: "".to_owned(),
        };
        allow_err!(rz.dns_check());
        let mut socket = FramedSocket::new(Config::get_any_listen_addr()).await?;
        const TIMER_OUT: Duration = Duration::from_secs(1);
        let mut timer = interval(TIMER_OUT);
        let mut last_timer = SystemTime::UNIX_EPOCH;
        const REG_INTERVAL: i64 = 12_000;
        const REG_TIMEOUT: i64 = 3_000;
        const MAX_FAILS1: i64 = 3;
        const MAX_FAILS2: i64 = 6;
        const DNS_INTERVAL: i64 = 60_000;
        let mut fails = 0;
        let mut last_register_resp = SystemTime::UNIX_EPOCH;
        let mut last_register_sent = SystemTime::UNIX_EPOCH;
        let mut last_dns_check = SystemTime::UNIX_EPOCH;
        let mut old_latency = 0;
        let mut ema_latency = 0;
        loop {
            let mut update_latency = || {
                last_register_resp = SystemTime::now();
                fails = 0;
                let mut latency = last_register_resp
                    .duration_since(last_register_sent)
                    .map(|d| d.as_micros() as i64)
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
                Some(Ok((bytes, _))) = socket.next() => {
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
                                Config::set_option("rendezvous-servers".to_owned(), cu.rendezvous_servers.join(","));
                                Config::set_serial(cu.serial);
                            }
                            _ => {}
                        }
                    } else {
                        log::debug!("Non-protobuf message bytes received: {:?}", bytes);
                    }
                },
                _ = timer.tick() => {
                    if Config::get_rendezvous_servers() != rz.rendezvous_servers {
                        break;
                    }
                    if !Config::get_option("stop-service").is_empty() {
                        break;
                    }
                    if rz.addr.port() == 0 {
                        allow_err!(rz.dns_check());
                        if rz.addr.port() == 0 {
                            continue;
                        } else {
                            // have to do this for osx, to avoid "Can't assign requested address"
                            // when socket created before OS network ready
                            socket = FramedSocket::new(Config::get_any_listen_addr()).await?;
                        }
                    }
                    let now = SystemTime::now();
                    if now.duration_since(last_timer).map(|d| d < TIMER_OUT).unwrap_or(false) {
                        // a workaround of tokio timer bug
                        continue;
                    }
                    last_timer = now;
                    let elapsed_resp = now.duration_since(last_register_resp).map(|d| d.as_millis() as i64).unwrap_or(REG_INTERVAL);
                    let timeout = last_register_sent.duration_since(last_register_resp).map(|d| d.as_millis() as i64).unwrap_or(0) >= REG_TIMEOUT;
                    if timeout || elapsed_resp >= REG_INTERVAL {
                        allow_err!(rz.register_peer(&mut socket).await);
                        last_register_sent = now;
                        if timeout {
                            fails += 1;
                            if fails > MAX_FAILS2 {
                                Config::update_latency(&host, -1);
                                old_latency = 0;
                                if now.duration_since(last_dns_check).map(|d| d.as_millis() as i64).unwrap_or(0) > DNS_INTERVAL {
                                    if let Ok(_) = rz.dns_check() {
                                        // in some case of network reconnect (dial IP network),
                                        // old UDP socket not work any more after network recover
                                        socket = FramedSocket::new(Config::get_any_listen_addr()).await?;
                                    }
                                    last_dns_check = now;
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

    fn dns_check(&mut self) -> ResultType<()> {
        self.addr = hbb_common::to_socket_addr(&crate::check_port(&self.host, RENDEZVOUS_PORT))?;
        log::debug!("Lookup dns of {}", self.host);
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
        let mut socket =
            FramedStream::new(self.addr, Config::get_any_listen_addr(), RENDEZVOUS_TIMEOUT).await?;
        let mut msg_out = Message::new();
        let mut rr = RelayResponse {
            socket_addr,
            ..Default::default()
        };
        if initiate {
            rr.uuid = uuid.clone();
            rr.relay_server = relay_server.clone();
            rr.uuid = uuid.clone();
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
        let (mut socket, port) = {
            let socket =
                FramedStream::new(self.addr, Config::get_any_listen_addr(), RENDEZVOUS_TIMEOUT)
                    .await?;
            let port = socket.get_ref().local_addr()?.port();
            (socket, port)
        };
        let local_addr = socket.get_ref().local_addr()?;
        let local_addr: SocketAddr = format!("{}:{}", local_addr.ip(), port).parse()?;
        let mut msg_out = Message::new();
        let mut relay_server = Config::get_option("relay-server");
        if relay_server.is_empty() {
            relay_server = fla.relay_server;
        }
        msg_out.set_local_addr(LocalAddr {
            id: Config::get_id(),
            socket_addr: AddrMangle::encode(peer_addr),
            local_addr: AddrMangle::encode(local_addr),
            relay_server,
            ..Default::default()
        });
        let bytes = msg_out.write_to_bytes()?;
        socket.send_raw(bytes).await?;
        crate::accept_connection(server.clone(), socket, peer_addr, true).await;
        Ok(())
    }

    async fn handle_punch_hole(&self, ph: PunchHole, server: ServerPtr) -> ResultType<()> {
        let mut relay_server = Config::get_option("relay-server");
        if relay_server.is_empty() {
            relay_server = ph.relay_server;
        }
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
            let socket =
                FramedStream::new(self.addr, Config::get_any_listen_addr(), RENDEZVOUS_TIMEOUT)
                    .await?;
            allow_err!(FramedStream::new(peer_addr, socket.get_ref().local_addr()?, 300).await);
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
        self.last_id_pk_registery = id.clone();
        msg_out.set_register_pk(RegisterPk {
            id,
            uuid,
            pk,
            ..Default::default()
        });
        socket.send(&msg_out, self.addr).await?;
        Ok(())
    }

    async fn handle_uuid_mismatch(&mut self, socket: &mut FramedSocket) -> ResultType<()> {
        if self.last_id_pk_registery != Config::get_id() {
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
        socket.send(&msg_out, self.addr).await?;
        Ok(())
    }
}
