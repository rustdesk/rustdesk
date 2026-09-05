use std::{
    collections::{hash_map::RandomState, HashMap, VecDeque},
    hash::BuildHasher,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

use uuid::Uuid;

use hbb_common::{
    allow_err,
    anyhow::{self, bail},
    config::{
        self, keys::*, option2bool, use_ws, Config, CONNECT_TIMEOUT, REG_INTERVAL, RENDEZVOUS_PORT,
    },
    futures::future::join_all,
    log,
    protobuf::Message as _,
    rendezvous_proto::*,
    sleep,
    socket_client::{self, connect_tcp, is_ipv4, new_direct_udp_for, new_udp_for},
    tokio::{
        self, select,
        sync::{mpsc, Mutex},
        time::interval,
    },
    udp::FramedSocket,
    webrtc::WebRTCStream,
    AddrMangle, IntoTargetAddr, ResultType, Stream, TargetAddr,
};

use crate::{
    check_port,
    server::{check_zombie, new as new_server, ConnectionMeta, ServerPtr},
};

type Message = RendezvousMessage;

fn connection_meta(
    control_permissions: Option<ControlPermissions>,
    controlled_context: Option<ControlledContext>,
) -> ConnectionMeta {
    ConnectionMeta {
        control_permissions,
        controlled_context,
    }
}

lazy_static::lazy_static! {
    static ref SOLVING_PK_MISMATCH: Mutex<String> = Default::default();
    static ref LAST_MSG: Mutex<(SocketAddr, Instant)> = Mutex::new((SocketAddr::new([0; 4].into(), 0), Instant::now()));
    static ref LAST_RELAY_MSG: Mutex<(SocketAddr, Instant)> = Mutex::new((SocketAddr::new([0; 4].into(), 0), Instant::now()));
    static ref WEBRTC_ICE_TXS: Mutex<HashMap<String, IceRoute>> = Default::default();
    static ref ICE_DIGEST_STATE: RandomState = Default::default();
}
/// Remote ICE candidates buffered per session while the answerer applies them. Same depth as the
/// controller's own buffer (`Client::MAX_PENDING_WEBRTC_ICE`), though that one evicts its oldest
/// where a full channel here refuses the newest.
const MAX_PENDING_REMOTE_ICE: usize = 64;
/// Queued candidates remembered so the controller's re-send is skipped instead of taking a slot
/// of its own. Far more than an honest peer gathers, at eight bytes each.
const ICE_DEDUP_WINDOW: usize = 256;
// The rendezvous ICE route is reachable without a prior punch and the peer decides how many
// candidates it sends, so these sites would let someone else set how much this machine writes to
// its log file. One line a minute each, carrying the suppressed count.
const ICE_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
static UNKNOWN_ICE_SESSION_LOG: hbb_common::log_throttle::LogThrottle =
    hbb_common::log_throttle::LogThrottle::new(ICE_LOG_INTERVAL);
static REJECTED_REMOTE_ICE_LOG: hbb_common::log_throttle::LogThrottle =
    hbb_common::log_throttle::LogThrottle::new(ICE_LOG_INTERVAL);
static FULL_ICE_QUEUE_LOG: hbb_common::log_throttle::LogThrottle =
    hbb_common::log_throttle::LogThrottle::new(ICE_LOG_INTERVAL);

struct IceRoute {
    tx: mpsc::Sender<String>,
    recent: VecDeque<u64>,
}

impl IceRoute {
    fn new(tx: mpsc::Sender<String>) -> Self {
        Self {
            tx,
            recent: VecDeque::new(),
        }
    }

    /// Keeps `queue` the only way onto the channel, so nothing reaches it unrecorded.
    fn is_same_channel(&self, other: &mpsc::Sender<String>) -> bool {
        self.tx.same_channel(other)
    }

    /// Skip the controller's re-send of a candidate already queued: the ICE agent that dedups
    /// repeats is downstream of this queue, so the copy would spend a slot of its own.
    /// False means the candidate was dropped.
    fn queue(&mut self, candidate: String) -> bool {
        let digest = ICE_DIGEST_STATE.hash_one(candidate.as_str());
        if self.recent.contains(&digest) {
            // Only honest about the drop if the route is still alive to have taken it.
            return !self.tx.is_closed();
        }
        // Recorded once queued, never before: a refused candidate stays repairable by the re-send.
        if self.tx.try_send(candidate).is_err() {
            return false;
        }
        if self.recent.len() >= ICE_DEDUP_WINDOW {
            self.recent.pop_front();
        }
        self.recent.push_back(digest);
        true
    }
}

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);
static MANUAL_RESTARTED: AtomicBool = AtomicBool::new(false);
static SENT_REGISTER_PK: AtomicBool = AtomicBool::new(false);
pub(crate) static NEEDS_DEPLOY: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "android")]
static NOTIFIED_NEEDS_DEPLOY: AtomicBool = AtomicBool::new(false);
// register_pk retry interval (ms) when device is awaiting deployment
const DEPLOY_RETRY_INTERVAL: i64 = 30_000;
lazy_static::lazy_static! {
    static ref LAST_NOT_DEPLOYED_REGISTER: Mutex<Option<Instant>> = Mutex::new(None);
}

// Single source of truth for the "awaiting deployment" backoff. The server has
// already told us this device is not in its db; until the operator runs
// `rustdesk --deploy --token <api_token>` there is no point re-running the
// register path more often than DEPLOY_RETRY_INTERVAL. Gating in the timer
// loops (rather than only inside register_pk) also avoids the
// last_register_sent / fails / latency / UDP-rebind churn the loop would
// otherwise spin on while no response ever comes back.
async fn deploy_register_throttled() -> bool {
    if !NEEDS_DEPLOY.load(Ordering::SeqCst) {
        return false;
    }
    LAST_NOT_DEPLOYED_REGISTER
        .lock()
        .await
        .map(|t| (t.elapsed().as_millis() as i64) < DEPLOY_RETRY_INTERVAL)
        .unwrap_or(false)
}

#[cfg(target_os = "android")]
fn notify_android_needs_deploy() {
    if NOTIFIED_NEEDS_DEPLOY.load(Ordering::SeqCst) {
        return;
    }
    let event = serde_json::json!({ "name": "android_needs_deploy" }).to_string();
    if matches!(
        crate::flutter::push_global_event(crate::flutter::APP_TYPE_MAIN, event),
        Some(true)
    ) {
        NOTIFIED_NEEDS_DEPLOY.store(true, Ordering::SeqCst);
    }
}

#[cfg(target_os = "android")]
pub(crate) fn reset_needs_deploy_notification() {
    NEEDS_DEPLOY.store(false, Ordering::SeqCst);
    NOTIFIED_NEEDS_DEPLOY.store(false, Ordering::SeqCst);
}

#[derive(Clone)]
pub struct RendezvousMediator {
    addr: TargetAddr<'static>,
    host: String,
    host_prefix: String,
    keep_alive: i32,
}

impl RendezvousMediator {
    pub fn restart() {
        SHOULD_EXIT.store(true, Ordering::SeqCst);
        MANUAL_RESTARTED.store(true, Ordering::SeqCst);
        log::info!("server restart");
    }

    pub async fn start_all() {
        crate::test_nat_type();
        if config::is_outgoing_only() {
            loop {
                sleep(1.).await;
            }
        }
        crate::hbbs_http::sync::start();
        #[cfg(target_os = "windows")]
        if crate::platform::is_installed() && crate::is_server() {
            crate::updater::start_auto_update();
        }
        check_zombie();
        let server = new_server();
        if config::option2bool("stop-service", &Config::get_option("stop-service")) {
            crate::test_rendezvous_server();
        }
        let server_cloned = server.clone();
        tokio::spawn(async move {
            direct_server(server_cloned).await;
        });
        #[cfg(target_os = "android")]
        let start_lan_listening = true;
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let start_lan_listening = crate::platform::is_installed();
        if start_lan_listening {
            std::thread::spawn(move || {
                allow_err!(super::lan::start_listening());
            });
        }
        scrap::codec::test_av1();
        *LAST_NOT_DEPLOYED_REGISTER.lock().await = None;
        loop {
            let timeout = Arc::new(RwLock::new(CONNECT_TIMEOUT));
            let conn_start_time = Instant::now();
            *SOLVING_PK_MISMATCH.lock().await = "".to_owned();
            if !config::option2bool("stop-service", &Config::get_option("stop-service"))
                && !crate::platform::installing_service()
            {
                let mut futs = Vec::new();
                let servers = Config::get_rendezvous_servers();
                SHOULD_EXIT.store(false, Ordering::SeqCst);
                MANUAL_RESTARTED.store(false, Ordering::SeqCst);
                for host in servers.clone() {
                    let server = server.clone();
                    let timeout = timeout.clone();
                    futs.push(tokio::spawn(async move {
                        if let Err(err) = Self::start(server, host).await {
                            let err = format!("rendezvous mediator error: {err}");
                            // When user reboot, there might be below error, waiting too long
                            // (CONNECT_TIMEOUT 18s) will make user think there is bug
                            if err.contains("10054") || err.contains("11001") {
                                // No such host is known. (os error 11001)
                                // An existing connection was forcibly closed by the remote host. (os error 10054): also happens for UDP
                                *timeout.write().unwrap() = 3000;
                            }
                            log::error!("{err}");
                        }
                        // SHOULD_EXIT here is to ensure once one exits, the others also exit.
                        SHOULD_EXIT.store(true, Ordering::SeqCst);
                    }));
                }
                join_all(futs).await;
            } else {
                server.write().unwrap().close_connections();
            }
            Config::reset_online();
            let timeout = *timeout.read().unwrap();
            if !MANUAL_RESTARTED.load(Ordering::SeqCst) {
                let elapsed = conn_start_time.elapsed().as_millis() as u64;
                if elapsed < timeout {
                    sleep(((timeout - elapsed) / 1000) as _).await;
                }
            } else {
                // https://github.com/rustdesk/rustdesk/issues/12233
                sleep(0.033).await;
            }
        }
    }

    fn get_host_prefix(host: &str) -> String {
        host.split(".")
            .next()
            .map(|x| {
                if x.parse::<i32>().is_ok() {
                    host.to_owned()
                } else {
                    x.to_owned()
                }
            })
            .unwrap_or(host.to_owned())
    }

    pub async fn start_udp(server: ServerPtr, host: String) -> ResultType<()> {
        let host = check_port(&host, RENDEZVOUS_PORT);
        log::info!("start udp: {host}");
        let (mut socket, mut addr) = new_udp_for(&host, CONNECT_TIMEOUT).await?;
        let mut rz = Self {
            addr: addr.clone(),
            host: host.clone(),
            host_prefix: Self::get_host_prefix(&host),
            keep_alive: crate::DEFAULT_KEEP_ALIVE,
        };

        let mut timer = crate::rustdesk_interval(interval(crate::TIMER_OUT));
        const MIN_REG_TIMEOUT: i64 = 3_000;
        const MAX_REG_TIMEOUT: i64 = 30_000;
        let mut reg_timeout = MIN_REG_TIMEOUT;
        const MAX_FAILS1: i64 = 2;
        const MAX_FAILS2: i64 = 4;
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
                reg_timeout = MIN_REG_TIMEOUT;
                let mut latency = last_register_sent
                    .map(|x| x.elapsed().as_micros() as i64)
                    .unwrap_or(0);
                last_register_sent = None;
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
                            if let Ok(msg) = Message::parse_from_bytes(&bytes) {
                                rz.handle_resp(msg.union, Sink::Framed(&mut socket, &addr), &server, &mut update_latency).await?;
                            } else {
                                log::debug!("Non-protobuf message bytes received: {:?}", bytes);
                            }
                        },
                        Some(Err(e)) => bail!("Failed to receive next: {}", e),  // maybe socks5 tcp disconnected
                        None => {
                            bail!("Socket receive none. Maybe socks5 server is down.");
                        },
                    }
                },
                _ = timer.tick() => {
                    if SHOULD_EXIT.load(Ordering::SeqCst) {
                        break;
                    }
                    // The server already told us this device is not deployed. Skip
                    // the whole register / fails / latency / UDP-rebind path until
                    // DEPLOY_RETRY_INTERVAL elapses, otherwise the loop spins every
                    // few seconds (log spam + misapplied network-recovery rebind)
                    // until the operator runs `rustdesk --deploy`.
                    if deploy_register_throttled().await {
                        continue;
                    }
                    let now = Some(Instant::now());
                    let expired = last_register_resp.map(|x| x.elapsed().as_millis() as i64 >= REG_INTERVAL).unwrap_or(true);
                    let timeout = last_register_sent.map(|x| x.elapsed().as_millis() as i64 >= reg_timeout).unwrap_or(false);
                    // temporarily disable exponential backoff for android before we add wakeup trigger to force connect in android
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    if crate::using_public_server() { // only turn on this for public server, may help DDNS self-hosting user.
                        if timeout && reg_timeout < MAX_REG_TIMEOUT {
                            reg_timeout += MIN_REG_TIMEOUT;
                        }
                    }
                    if timeout || (last_register_sent.is_none() && expired) {
                        if timeout {
                            fails += 1;
                            if fails >= MAX_FAILS2 {
                                Config::update_latency(&host, -1);
                                old_latency = 0;
                                if last_dns_check.elapsed().as_millis() as i64 > DNS_INTERVAL {
                                    // in some case of network reconnect (dial IP network),
                                    // old UDP socket not work any more after network recover
                                    if let Some((s, new_addr)) = socket_client::rebind_udp_for(&rz.host).await? {
                                        socket = s;
                                        rz.addr = new_addr.clone();
                                        addr = new_addr;
                                    }
                                    last_dns_check = Instant::now();
                                }
                            } else if fails >= MAX_FAILS1 {
                                Config::update_latency(&host, 0);
                                old_latency = 0;
                            }
                        }
                        rz.register_peer(Sink::Framed(&mut socket, &addr)).await?;
                        last_register_sent = now;
                    }
                }
            }
        }
        Ok(())
    }

    #[inline]
    async fn handle_resp(
        &mut self,
        msg: Option<rendezvous_message::Union>,
        sink: Sink<'_>,
        server: &ServerPtr,
        update_latency: &mut impl FnMut(),
    ) -> ResultType<()> {
        match msg {
            Some(rendezvous_message::Union::RegisterPeerResponse(rpr)) => {
                update_latency();
                if rpr.request_pk {
                    log::info!("request_pk received from {}", self.host);
                    self.register_pk(sink).await?;
                }
            }
            Some(rendezvous_message::Union::RegisterPkResponse(rpr)) => {
                update_latency();
                match rpr.result.enum_value() {
                    Ok(register_pk_response::Result::OK) => {
                        Config::set_key_confirmed(true);
                        Config::set_host_key_confirmed(&self.host_prefix, true);
                        *SOLVING_PK_MISMATCH.lock().await = "".to_owned();
                        NEEDS_DEPLOY.store(false, Ordering::SeqCst);
                        #[cfg(target_os = "android")]
                        reset_needs_deploy_notification();
                    }
                    Ok(register_pk_response::Result::UUID_MISMATCH) => {
                        self.handle_uuid_mismatch(sink).await?;
                    }
                    Ok(register_pk_response::Result::NOT_DEPLOYED) => {
                        if !NEEDS_DEPLOY.load(Ordering::SeqCst) {
                            log::warn!("Server requires deployment. Run `rustdesk --deploy --token <api_token>` on this device.");
                        }
                        NEEDS_DEPLOY.store(true, Ordering::SeqCst);
                        // Clear key_confirmed so the UI reflects the truth: this device is
                        // not currently registered. Covers the case where an online device
                        // was deleted by an admin while running.
                        Config::set_key_confirmed(false);
                        Config::set_host_key_confirmed(&self.host_prefix, false);
                        #[cfg(target_os = "android")]
                        notify_android_needs_deploy();
                    }
                    _ => {
                        log::error!("unknown RegisterPkResponse");
                    }
                }
                if rpr.keep_alive > 0 {
                    self.keep_alive = rpr.keep_alive * 1000;
                    log::info!("keep_alive: {}ms", self.keep_alive);
                }
            }
            Some(rendezvous_message::Union::PunchHole(ph)) => {
                let rz = self.clone();
                let server = server.clone();
                tokio::spawn(async move {
                    allow_err!(rz.handle_punch_hole(ph, server).await);
                });
            }
            Some(rendezvous_message::Union::RequestRelay(rr)) => {
                let rz = self.clone();
                let server = server.clone();
                tokio::spawn(async move {
                    allow_err!(rz.handle_request_relay(rr, server).await);
                });
            }
            Some(rendezvous_message::Union::FetchLocalAddr(fla)) => {
                let rz = self.clone();
                let server = server.clone();
                tokio::spawn(async move {
                    allow_err!(rz.handle_intranet(fla, server).await);
                });
            }
            Some(rendezvous_message::Union::IceCandidate(ice)) => {
                let queued = {
                    let mut txs = WEBRTC_ICE_TXS.lock().await;
                    txs.get_mut(&ice.session_key)
                        .map(|route| route.queue(ice.candidate))
                };
                match queued {
                    Some(false) => {
                        if let Some(n) = FULL_ICE_QUEUE_LOG.due() {
                            log::debug!("dropped {} ICE candidate(s): queue full or closed", n);
                        }
                    }
                    None => {
                        if let Some(n) = UNKNOWN_ICE_SESSION_LOG.due() {
                            log::debug!(
                                "dropped {} ICE candidate(s) for unknown WebRTC session key, last: {}",
                                n,
                                ice.session_key
                            );
                        }
                    }
                    _ => {}
                }
            }
            Some(rendezvous_message::Union::ConfigureUpdate(cu)) => {
                let v0 = Config::get_rendezvous_servers();
                Config::set_option(
                    "rendezvous-servers".to_owned(),
                    cu.rendezvous_servers.join(","),
                );
                Config::set_serial(cu.serial);
                if v0 != Config::get_rendezvous_servers() {
                    Self::restart();
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn start_tcp(server: ServerPtr, host: String) -> ResultType<()> {
        let host = check_port(&host, RENDEZVOUS_PORT);
        log::info!("start tcp: {}", hbb_common::websocket::check_ws(&host));
        let mut conn = connect_tcp(host.clone(), CONNECT_TIMEOUT).await?;
        let key = crate::get_key(true).await;
        crate::secure_tcp(&mut conn, &key).await?;
        let mut rz = Self {
            addr: conn.local_addr().into_target_addr()?,
            host: host.clone(),
            host_prefix: Self::get_host_prefix(&host),
            keep_alive: crate::DEFAULT_KEEP_ALIVE,
        };
        let mut timer = crate::rustdesk_interval(interval(crate::TIMER_OUT));
        let mut last_register_sent: Option<Instant> = None;
        let mut last_recv_msg = Instant::now();
        // we won't support connecting to multiple rendzvous servers any more, so we can use a global variable here.
        Config::set_host_key_confirmed(&rz.host_prefix, false);
        loop {
            let mut update_latency = || {
                let latency = last_register_sent
                    .map(|x| x.elapsed().as_micros() as i64)
                    .unwrap_or(0);
                Config::update_latency(&host, latency);
                log::debug!("Latency of {}: {}ms", host, latency as f64 / 1000.);
            };
            select! {
                res = conn.next() => {
                    last_recv_msg = Instant::now();
                    let bytes = res.ok_or_else(|| anyhow::anyhow!("Rendezvous connection is reset by the peer"))??;
                    if bytes.is_empty() {
                        // After fixing frequent register_pk, for websocket, nginx need to set proxy_read_timeout to more than 60 seconds, eg: 120s
                        // https://serverfault.com/questions/1060525/why-is-my-websocket-connection-gets-closed-in-60-seconds
                        conn.send_bytes(bytes::Bytes::new()).await?;
                        continue; // heartbeat
                    }
                    let msg = Message::parse_from_bytes(&bytes)?;
                    rz.handle_resp(msg.union, Sink::Stream(&mut conn), &server, &mut update_latency).await?
                }
                _ = timer.tick() => {
                    if SHOULD_EXIT.load(Ordering::SeqCst) {
                        break;
                    }
                    // https://www.emqx.com/en/blog/mqtt-keep-alive
                    if last_recv_msg.elapsed().as_millis() as u64 > rz.keep_alive as u64 * 3 / 2 {
                        bail!("Rendezvous connection is timeout");
                    }
                    if (!Config::get_key_confirmed() ||
                        !Config::get_host_key_confirmed(&rz.host_prefix)) &&
                        last_register_sent.map(|x| x.elapsed().as_millis() as i64).unwrap_or(REG_INTERVAL) >= REG_INTERVAL {
                        rz.register_pk(Sink::Stream(&mut conn)).await?;
                        last_register_sent = Some(Instant::now());
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn start(server: ServerPtr, host: String) -> ResultType<()> {
        log::info!("start rendezvous mediator of {}", host);
        //If the investment agent type is http or https, then tcp forwarding is enabled.
        if (cfg!(debug_assertions) && option_env!("TEST_TCP").is_some())
            || Config::is_proxy()
            || use_ws()
            || crate::is_udp_disabled()
        {
            Self::start_tcp(server, host).await
        } else {
            Self::start_udp(server, host).await
        }
    }

    async fn handle_request_relay(&self, rr: RequestRelay, server: ServerPtr) -> ResultType<()> {
        let addr = AddrMangle::decode(&rr.socket_addr);
        let last = *LAST_RELAY_MSG.lock().await;
        *LAST_RELAY_MSG.lock().await = (addr, Instant::now());
        // skip duplicate relay request messages
        if last.0 == addr && last.1.elapsed().as_millis() < 100 {
            return Ok(());
        }
        let meta = connection_meta(
            rr.control_permissions.into_option(),
            rr.controlled_context.into_option(),
        );

        self.create_relay(
            rr.socket_addr.into(),
            rr.relay_server,
            rr.uuid,
            server,
            rr.secure,
            false,
            Default::default(),
            String::new(),
            meta,
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
        socket_addr_v6: bytes::Bytes,
        webrtc_sdp_answer: String,
        meta: ConnectionMeta,
    ) -> ResultType<()> {
        let peer_addr = AddrMangle::decode(&socket_addr);
        log::info!(
            "create_relay requested from {:?}, relay_server: {}, uuid: {}, secure: {}",
            peer_addr,
            relay_server,
            uuid,
            secure,
        );

        let mut socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;

        let mut msg_out = Message::new();
        let mut rr = RelayResponse {
            socket_addr: socket_addr.into(),
            version: crate::VERSION.to_owned(),
            socket_addr_v6,
            webrtc_sdp_answer,
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
            meta,
        )
        .await;
        Ok(())
    }

    async fn handle_intranet(&self, fla: FetchLocalAddr, server: ServerPtr) -> ResultType<()> {
        let addr = AddrMangle::decode(&fla.socket_addr);
        let last = *LAST_MSG.lock().await;
        *LAST_MSG.lock().await = (addr, Instant::now());
        // skip duplicate punch hole messages
        if last.0 == addr && last.1.elapsed().as_millis() < 100 {
            return Ok(());
        }
        let peer_addr_v6 = hbb_common::AddrMangle::decode(&fla.socket_addr_v6);
        let relay_server = self.get_relay_server(fla.relay_server.clone());
        let relay = use_ws() || Config::is_proxy();
        let mut socket_addr_v6 = Default::default();
        let meta = connection_meta(
            fla.control_permissions.clone().into_option(),
            fla.controlled_context.clone().into_option(),
        );
        if peer_addr_v6.port() > 0 && !relay {
            socket_addr_v6 = start_ipv6(peer_addr_v6, addr, server.clone(), meta.clone()).await;
        }
        if is_ipv4(&self.addr) && !relay && !config::is_disable_tcp_listen() {
            if let Err(err) = self
                .handle_intranet_(
                    fla.clone(),
                    server.clone(),
                    relay_server.clone(),
                    socket_addr_v6.clone(),
                    meta.clone(),
                )
                .await
            {
                log::debug!("Failed to handle intranet: {:?}, will try relay", err);
            } else {
                return Ok(());
            }
        }
        let uuid = Uuid::new_v4().to_string();
        self.create_relay(
            fla.socket_addr.into(),
            relay_server,
            uuid,
            server,
            true,
            true,
            socket_addr_v6,
            String::new(),
            meta,
        )
        .await
    }

    async fn handle_intranet_(
        &self,
        fla: FetchLocalAddr,
        server: ServerPtr,
        relay_server: String,
        socket_addr_v6: bytes::Bytes,
        meta: ConnectionMeta,
    ) -> ResultType<()> {
        let peer_addr = AddrMangle::decode(&fla.socket_addr);
        log::debug!("Handle intranet from {:?}", peer_addr);
        let mut socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
        let local_addr = socket.local_addr();
        // we saw invalid local_addr while using proxy, local_addr.ip() == "::1"
        let local_addr: SocketAddr =
            format!("{}:{}", local_addr.ip(), local_addr.port()).parse()?;
        let mut msg_out = Message::new();
        msg_out.set_local_addr(LocalAddr {
            id: Config::get_id(),
            socket_addr: AddrMangle::encode(peer_addr).into(),
            local_addr: AddrMangle::encode(local_addr).into(),
            relay_server,
            version: crate::VERSION.to_owned(),
            socket_addr_v6,
            ..Default::default()
        });
        let bytes = msg_out.write_to_bytes()?;
        socket.send_raw(bytes).await?;
        crate::accept_connection(server.clone(), socket, peer_addr, true, meta).await;
        Ok(())
    }

    /// Build the WebRTC answerer for a punch-hole offer and return the SDP answer that rides in
    /// the punch reply (PunchHoleSent / RelayResponse).
    ///
    /// Awaited inline on the punch-reply path, which only holds because everything here is local
    /// (pc + keygen + SDP; trickle means the answer carries no candidates). Keep network I/O out
    /// — connection setup belongs in the detached task below.
    async fn spawn_webrtc_answerer(
        &self,
        ph: &PunchHole,
        relay_only_ice: bool,
        server: ServerPtr,
        peer_addr: SocketAddr,
        meta: ConnectionMeta,
    ) -> ResultType<String> {
        let mut stream =
            WebRTCStream::new(&ph.webrtc_sdp_offer, relay_only_ice, CONNECT_TIMEOUT).await?;
        let answer = stream.local_endpoint().to_owned();
        let session_key = stream.session_key().to_owned();
        let return_route = ph.socket_addr.clone();

        // A duplicate PunchHole (the offerer re-sends the same request across punch attempts)
        // resolves to the SESSIONS-cached stream. `take_local_ice_rx` yields the receiver
        // exactly once per stream instance, so `None` here means an answerer was already
        // spawned for this offer: return the (identical) cached answer without spawning a
        // second connect task. Otherwise two `create_tcp_connection` tasks would detach and
        // read the same data channel, interleaving the handshake and corrupting the session.
        let Some(mut local_ice_rx) = stream.take_local_ice_rx() else {
            return Ok(answer);
        };

        // Bounded: how many candidates arrive is the sender's choice, while draining one costs a
        // JSON parse and the ICE agent's lock, so an unbounded queue lets whoever can reach this
        // session's route grow it without limit inside a long-lived service process. A full queue
        // drops the newest candidate, and the controller re-sends it once — the digests beside the
        // sender are what keep that re-send from spending a slot of its own.
        let (remote_ice_tx, mut remote_ice_rx) = mpsc::channel::<String>(MAX_PENDING_REMOTE_ICE);
        let own_ice_tx = remote_ice_tx.clone();
        WEBRTC_ICE_TXS
            .lock()
            .await
            .insert(session_key.clone(), IceRoute::new(remote_ice_tx));

        let stream_for_remote_ice = stream.clone();
        tokio::spawn(async move {
            while let Some(candidate) = remote_ice_rx.recv().await {
                if let Err(err) = stream_for_remote_ice.add_remote_ice_candidate(&candidate).await
                {
                    if let Some(n) = REJECTED_REMOTE_ICE_LOG.due() {
                        log::warn!(
                            "failed to add {} remote WebRTC ICE candidate(s), last: {}",
                            n,
                            err
                        );
                    }
                }
            }
        });

        {
            let host = self.host.clone();
            let socket_addr = return_route.clone();
            let session_key_for_ice = session_key.clone();
            tokio::spawn(async move {
                // Candidates ride a dedicated TCP connection to the rendezvous server, like
                // the answer, NOT the mediator channel: that channel is UDP in the default
                // setup, and target deployments front hbbs with websocket/TCP only, where
                // its UDP port is unreachable. The server keeps candidate-carrying TCP
                // connections open, so one lazily-opened connection serves the whole
                // trickle, and TCP reliability replaces the old 400ms duplicate re-send
                // (the controller keeps its own re-send for the server->peer UDP downlink).
                let mut conn = None;
                while let Some(candidate) = local_ice_rx.recv().await {
                    let mut msg = Message::new();
                    msg.set_ice_candidate(IceCandidate {
                        socket_addr: socket_addr.clone(),
                        session_key: session_key_for_ice.clone(),
                        candidate,
                        ..Default::default()
                    });
                    // One reconnect attempt per candidate: the first send after an hbbs
                    // restart or an idle-killed connection fails on the stale stream.
                    for _ in 0..2 {
                        if conn.is_none() {
                            match connect_tcp(&*host, CONNECT_TIMEOUT).await {
                                Ok(s) => conn = Some(s),
                                Err(err) => {
                                    log::warn!(
                                        "failed to connect for WebRTC ICE candidate: {}",
                                        err
                                    );
                                    break;
                                }
                            }
                        }
                        if let Some(s) = conn.as_mut() {
                            match s.send(&msg).await {
                                Ok(()) => break,
                                Err(err) => {
                                    log::debug!(
                                        "WebRTC ICE candidate send failed, reconnecting: {}",
                                        err
                                    );
                                    conn = None;
                                }
                            }
                        }
                    }
                }
            });
        }

        let session_key_for_cleanup = session_key.clone();
        tokio::spawn(async move {
            let result = stream.wait_connected(CONNECT_TIMEOUT).await;
            // Only evict our own route. The key is the offer's DTLS fingerprint, identical across
            // the controller's punch retries, so a retry that built a fresh answerer has already
            // replaced this entry — removing it blindly would delete the live session's sender and
            // leave it receiving no candidates at all.
            {
                let mut txs = WEBRTC_ICE_TXS.lock().await;
                if txs
                    .get(&session_key_for_cleanup)
                    .is_some_and(|route| route.is_same_channel(&own_ice_tx))
                {
                    txs.remove(&session_key_for_cleanup);
                }
            }
            if let Err(err) = result {
                log::warn!("webrtc wait_connected failed: {}", err);
                // Release the pc now rather than waiting for the ICE agent to time out into a
                // terminal state (~30s); this also drops the SESSIONS entry promptly.
                stream.close().await;
                return;
            }
            // create_tcp_connection takes ownership of the stream; keep a handle to close the pc
            // once the session returns. It runs the whole session and returns Ok on normal end,
            // Err on setup failure — either way the pc must be closed, else it lingers forever in
            // SESSIONS (its state handler only fires on a terminal ICE state, which a cleanly
            // closed session may never reach) leaking the pc, channels, and socket fds.
            let stream_for_cleanup = stream.clone();
            if let Err(err) = crate::server::create_tcp_connection(
                server,
                Stream::WebRTC(stream),
                peer_addr,
                true,
                meta,
            )
            .await
            {
                log::warn!("failed to create WebRTC server connection: {}", err);
            }
            stream_for_cleanup.close().await;
        });

        Ok(answer)
    }

    async fn handle_punch_hole(&self, ph: PunchHole, server: ServerPtr) -> ResultType<()> {
        let mut peer_addr = AddrMangle::decode(&ph.socket_addr);
        let last = *LAST_MSG.lock().await;
        *LAST_MSG.lock().await = (peer_addr, Instant::now());
        // skip duplicate punch hole messages
        if last.0 == peer_addr && last.1.elapsed().as_millis() < 100 {
            return Ok(());
        }
        let peer_addr_v6 = hbb_common::AddrMangle::decode(&ph.socket_addr_v6);
        let local_proxy = use_ws() || Config::is_proxy();
        let relay = local_proxy || ph.force_relay;
        let mut socket_addr_v6 = Default::default();
        let meta = connection_meta(
            ph.control_permissions.clone().into_option(),
            ph.controlled_context.clone().into_option(),
        );
        // The controller's force_relay alone does not say whether ICE must be Relay-only; its
        // offer envelope does. `ice_policy: "all"` means the relay was forced by the transport
        // (ws), so answer with full ICE and let a direct pair form.
        let webrtc_relay_only =
            ph.force_relay && !WebRTCStream::endpoint_declares_all_ice(&ph.webrtc_sdp_offer);
        // No enable-webrtc check here: it is LocalConfig, which the UI process writes and never
        // syncs over IPC, so this (server) process would read the private-server default of "N"
        // and refuse to answer in exactly the self-hosted deployments the transport is for.
        // A proxy still rules it out — ICE would bypass it and leak the real IP.
        let webrtc_viable = !ph.webrtc_sdp_offer.is_empty()
            && !Config::is_proxy()
            && (!webrtc_relay_only || WebRTCStream::has_turn_server());
        let webrtc_sdp_answer = if webrtc_viable {
            self.spawn_webrtc_answerer(
                &ph,
                webrtc_relay_only,
                server.clone(),
                peer_addr,
                meta.clone(),
            )
            .await
            .unwrap_or_else(|err| {
                log::warn!("failed to create WebRTC answer: {}", err);
                String::new()
            })
        } else {
            String::new()
        };
        if peer_addr_v6.port() > 0 && !relay {
            socket_addr_v6 =
                start_ipv6(peer_addr_v6, peer_addr, server.clone(), meta.clone()).await;
        }
        let relay_server = self.get_relay_server(ph.relay_server);
        // for ensure, websocket go relay directly
        // A symmetric NAT relays the legacy transports but deliberately not WebRTC: the answer
        // built above rides along on the relay request, and ICE probes the candidate pairs rather
        // than trusting this classification, so a direct WebRTC pair can still form on a
        // connection this branch has already called relay-only. Do not gate the answerer on
        // nat_type to make the two agree.
        if ph.nat_type.enum_value() == Ok(NatType::SYMMETRIC)
            || Config::get_nat_type() == NatType::SYMMETRIC as i32
            || relay
            || (config::is_disable_tcp_listen() && ph.udp_port <= 0)
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
                    socket_addr_v6.clone(),
                    webrtc_sdp_answer.clone(),
                    meta,
                )
                .await;
        }
        use hbb_common::protobuf::Enum;
        let nat_type = NatType::from_i32(Config::get_nat_type()).unwrap_or(NatType::UNKNOWN_NAT);
        let msg_punch = PunchHoleSent {
            socket_addr: ph.socket_addr,
            id: Config::get_id(),
            relay_server,
            nat_type: nat_type.into(),
            version: crate::VERSION.to_owned(),
            socket_addr_v6,
            webrtc_sdp_answer,
            ..Default::default()
        };
        if ph.udp_port > 0 {
            peer_addr.set_port(ph.udp_port as u16);
            self.punch_udp_hole(peer_addr, server, msg_punch, meta)
                .await?;
            return Ok(());
        }
        if !ph.webrtc_sdp_offer.is_empty() {
            // Return the answer over its own short-lived TCP connection rather than the mediator
            // channel: that channel is UDP by default, and hbbs applies UDP-punch semantics
            // (source-address observation) to a PunchHoleSent that arrives on it. No TCP punch
            // is made — the controller keeps its request socket for trickled ICE.
            let mut msg_out = Message::new();
            msg_out.set_punch_hole_sent(msg_punch);
            let mut socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
            socket.send(&msg_out).await?;
            return Ok(());
        }
        log::debug!("Punch tcp hole to {:?}", peer_addr);
        let mut socket = {
            let socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
            let local_addr = socket.local_addr();
            // key important here for punch hole to tell my gateway incoming peer is safe.
            // Awaited rather than spawned so the mapping exists before `PunchHoleSent` goes out;
            // `local_addr` itself is shared, not exclusive - every socket here binds it with the
            // reuse flags `new_socket` sets.
            allow_err!(socket_client::connect_tcp_local(peer_addr, Some(local_addr), 30).await);
            socket
        };
        let mut msg_out = Message::new();
        msg_out.set_punch_hole_sent(msg_punch);
        let bytes = msg_out.write_to_bytes()?;
        socket.send_raw(bytes).await?;
        let local_addr = socket.local_addr();
        // The listener inside takes this address over, so the mediator's socket goes first.
        drop(socket);
        punch_tcp_until_connected(server, peer_addr, local_addr, meta).await;
        Ok(())
    }

    async fn punch_udp_hole(
        &self,
        peer_addr: SocketAddr,
        server: ServerPtr,
        msg_punch: PunchHoleSent,
        meta: ConnectionMeta,
    ) -> ResultType<()> {
        let mut msg_out = Message::new();
        msg_out.set_punch_hole_sent(msg_punch);
        let (socket, addr) = new_direct_udp_for(&self.host).await?;
        let data = msg_out.write_to_bytes()?;
        socket.send_to(&data, addr).await?;
        let socket_cloned = socket.clone();
        tokio::spawn(async move {
            for _ in 0..2 {
                let tm = (hbb_common::time_based_rand() % 20 + 10) as f32 / 1000.;
                hbb_common::sleep(tm).await;
                socket.send_to(&data, addr).await.ok();
            }
        });
        udp_nat_listen(socket_cloned.clone(), peer_addr, peer_addr, server, meta).await?;
        Ok(())
    }

    async fn register_pk(&mut self, socket: Sink<'_>) -> ResultType<()> {
        // Throttle register_pk when the device is awaiting deployment: server
        // already told us we're not in its db; sending more often than every
        // DEPLOY_RETRY_INTERVAL ms is wasted traffic until the operator runs
        // `rustdesk --deploy --token <api_token>`.
        if NEEDS_DEPLOY.load(Ordering::SeqCst) {
            let mut last = LAST_NOT_DEPLOYED_REGISTER.lock().await;
            if let Some(t) = *last {
                if (t.elapsed().as_millis() as i64) < DEPLOY_RETRY_INTERVAL {
                    return Ok(());
                }
            }
            *last = Some(Instant::now());
        } else {
            *LAST_NOT_DEPLOYED_REGISTER.lock().await = None;
        }
        let mut msg_out = Message::new();
        let pk = Config::get_key_pair().1;
        let uuid = hbb_common::get_uuid();
        let id = Config::get_id();
        msg_out.set_register_pk(RegisterPk {
            id,
            uuid: uuid.into(),
            pk: pk.into(),
            no_register_device: Config::no_register_device(),
            ..Default::default()
        });
        socket.send(&msg_out).await?;
        SENT_REGISTER_PK.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn handle_uuid_mismatch(&mut self, socket: Sink<'_>) -> ResultType<()> {
        {
            let mut solving = SOLVING_PK_MISMATCH.lock().await;
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

    async fn register_peer(&mut self, socket: Sink<'_>) -> ResultType<()> {
        let solving = SOLVING_PK_MISMATCH.lock().await;
        if !(solving.is_empty() || *solving == self.host) {
            return Ok(());
        }
        drop(solving);
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
        socket.send(&msg_out).await?;
        Ok(())
    }

    fn get_relay_server(&self, provided_by_rendezvous_server: String) -> String {
        let mut relay_server = Config::get_option("relay-server");
        if relay_server.is_empty() {
            relay_server = provided_by_rendezvous_server;
        }
        if relay_server.is_empty() {
            relay_server = crate::increase_port(&self.host, 1);
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
        let disabled = !option2bool(
            OPTION_DIRECT_SERVER,
            &Config::get_option(OPTION_DIRECT_SERVER),
        ) || option2bool("stop-service", &Config::get_option("stop-service"));
        if !disabled && listener.is_none() {
            port = get_direct_port();
            match hbb_common::tcp::listen_any(port as _).await {
                Ok(l) => {
                    listener = Some(l);
                    log::info!(
                        "Direct server listening on: {:?}",
                        listener.as_ref().map(|l| l.local_addr())
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
                            ConnectionMeta::default(), // Direct connections don't have server-side user context.
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

enum Sink<'a> {
    Framed(&'a mut FramedSocket, &'a TargetAddr<'a>),
    Stream(&'a mut Stream),
}

impl Sink<'_> {
    async fn send(self, msg: &Message) -> ResultType<()> {
        match self {
            Sink::Framed(socket, addr) => socket.send(msg, addr.to_owned()).await,
            Sink::Stream(stream) => stream.send(msg).await,
        }
    }
}

async fn start_ipv6(
    peer_addr_v6: SocketAddr,
    peer_addr_v4: SocketAddr,
    server: ServerPtr,
    meta: ConnectionMeta,
) -> bytes::Bytes {
    crate::test_ipv6().await;
    if let Some((socket, local_addr_v6)) = crate::get_ipv6_socket().await {
        let server = server.clone();
        tokio::spawn(async move {
            allow_err!(
                udp_nat_listen(socket.clone(), peer_addr_v6, peer_addr_v4, server, meta).await
            );
        });
        return local_addr_v6;
    }
    Default::default()
}

async fn udp_nat_listen(
    socket: Arc<tokio::net::UdpSocket>,
    peer_addr: SocketAddr,
    peer_addr_v4: SocketAddr,
    server: ServerPtr,
    meta: ConnectionMeta,
) -> ResultType<()> {
    let tm = Instant::now();
    let socket_cloned = socket.clone();
    let func = async {
        socket.connect(peer_addr).await?;
        let init_packet = crate::punch_udp(socket.clone(), true).await?;
        let stream = crate::kcp_stream::KcpStream::accept(
            socket,
            Duration::from_millis(CONNECT_TIMEOUT as _),
            init_packet,
        )
        .await?;
        crate::server::create_tcp_connection(server, stream.1, peer_addr_v4, true, meta).await?;
        Ok(())
    };
    func.await.map_err(|e: anyhow::Error| {
        anyhow::anyhow!(
            "Stop listening on {:?} for remote {peer_addr} with KCP, {:?} elapsed: {e}",
            socket_cloned.local_addr(),
            tm.elapsed()
        )
    })?;
    Ok(())
}

/// Where the repeats start, and the factor they slow by. The controller's SYN arrives once, at an
/// instant we are never told, inside a window we are not told either: `Client::connect` sizes its
/// dial only after our PunchHoleSent, from its own rendezvous time and the direct failures it has
/// recorded for us - `CONNECT_TIMEOUT` between two known-asymmetric NATs that never failed, as
/// little as a second once one has. So the repeats cover our own ceiling instead, `CONNECT_TIMEOUT`,
/// which is as long as the accept below has always been willing to take a connection, and back
/// off across it: dense at the start, where every window begins and the short ones end, sparse
/// afterwards, which is `punch_udp`'s shape for the same reason.
const PUNCH_INTERVAL: f32 = 0.15;
const PUNCH_BACKOFF: f32 = 1.5;
const PUNCH_MAX_INTERVAL: f32 = 2.0;
/// How long a punch in flight may run past the deadline, and the only timer it runs on. A punch
/// is cancel-safe while it is still in SYN_SENT and not once the controller's SYN has crossed it:
/// the socket is then half way through a handshake, and dropping it there cuts the connection the
/// controller is opening - which its `connect` has already returned, so that attempt fails
/// outright rather than falling back to relay. A timer cannot tell the two states apart, so no
/// punch is cut on a schedule of its own, and none needs to be. A gateway that answers with RST
/// fails the connect at once, and the loop punches again. One that drops the SYN in silence
/// leaves the socket in SYN_SENT, where it holds the mapping open and the kernel re-sends the
/// SYN, and any SYN of the controller's that arrives crosses it - a second punch has nothing to
/// add. That leaves the deadline, and this much past it lets a crossing begun just before it
/// complete; Windows gives a SYN up at about 21s anyway.
const PUNCH_GRACE: u64 = 3000;

/// The punch above leaves before hbbs has told the controller where to dial, so it is never in
/// flight at the same time as the controller's SYN: it opens our NAT, meets nothing, and a gateway
/// that answers it with RST takes the mapping down with it - leaving the listener below waiting on
/// a hole that no longer exists. Punching again across the window in which the controller dials
/// rebuilds it, and once the controller sits in SYN_SENT one of those punches meets its SYN and
/// completes as a simultaneous open: a second way in, which a single punch never had.
async fn punch_tcp_until_connected(
    server: ServerPtr,
    peer_addr: SocketAddr,
    local_addr: SocketAddr,
    meta: ConnectionMeta,
) {
    use hbb_common::tcp::new_listener;
    // Shadows the module's `std::time::Instant`: the deadline is held against tokio's sleeps and
    // timeouts, so it runs on their clock.
    use hbb_common::tokio::time::Instant;

    // Not fatal on its own - the punch below can still meet the controller's SYN without it, and
    // that half is the one a listener the OS refused to bind could not have covered anyway.
    let listener = match new_listener(local_addr, true).await {
        Ok(listener) => {
            log::info!("Server listening on: {local_addr}");
            Some(listener)
        }
        Err(err) => {
            log::warn!("Failed to listen on {local_addr} after punching: {err}");
            None
        }
    };
    // Bounds both halves: the punch keeps the mapping open only while the accept is still
    // willing to take a connection through it.
    let until = Instant::now() + Duration::from_millis(CONNECT_TIMEOUT);
    let punch = punch_until(until, peer_addr, |ms| {
        socket_client::connect_tcp_local(peer_addr, Some(local_addr), ms)
    });
    let Some(listener) = listener else {
        if let Some(stream) = punch.await {
            serve_punched(server, stream, peer_addr, meta).await;
        }
        return;
    };
    // Accepting in a loop, not once: a transient `accept` error must not spend the whole window
    // the controller still has to arrive in.
    let accept = async {
        loop {
            let left = until.saturating_duration_since(Instant::now()).as_millis() as u64;
            if left == 0 {
                break;
            }
            match hbb_common::timeout(left, listener.accept()).await {
                // Not filtered by address, as `accept_connection` never did: hbbs saw the
                // controller through one mapping and a NAT that pools its external addresses may
                // dial us from another, and what keeps `meta`'s control permissions from a second
                // peer is the handshake, plus that exactly one connection is ever served.
                Ok(Ok(accepted)) => return Some(accepted),
                Ok(Err(err)) => {
                    log::warn!("Failed to accept from {peer_addr}: {err}");
                    // One that persists - EMFILE, say - would otherwise spin here for the window.
                    sleep(1.).await;
                }
                Err(_) => break,
            }
        }
        log::info!("Nothing connected to the hole punched to {peer_addr}");
        None
    };
    // Only the accept races the punch. Racing `accept_connection` instead would race the whole
    // session it goes on to run, so a punch landing mid-session would tear that session down.
    //
    // Whichever arrives first is the one connection this request produces. Serving the loser too
    // would give a second peer the control permissions hbbs granted for this one controller, and
    // no test on the connection itself can tell the two apart before `create_tcp_connection` has
    // spoken to it - so the invariant is kept here, by there being no second serve.
    let punched = select! {
        // Both ready at once is two connections, not one seen twice - a crossing carries the
        // punch's four-tuple, which the listener never matches - and the punch is the one kept:
        // it is known to have met something at the address hbbs gave, where the accept takes
        // any address, and dropping it would reset the connection the controller is opening.
        biased;
        Some(stream) = punch => stream,
        Some((stream, addr)) = accept => {
            return accept_punched_connection(server, stream, addr, meta).await;
        }
        else => return,
    };
    serve_punched(server, punched, peer_addr, meta).await;
}

/// The repeats of `punch_tcp_until_connected`, over any punch rather than `connect_tcp_local`
/// alone, so that a test can run the schedule against a paused clock - which no socket can be.
async fn punch_until<T, F, Fut>(
    until: tokio::time::Instant,
    peer_addr: SocketAddr,
    mut punch: F,
) -> Option<T>
where
    F: FnMut(u64) -> Fut,
    Fut: std::future::Future<Output = ResultType<T>>,
{
    use hbb_common::tokio::time::Instant;

    let mut interval = PUNCH_INTERVAL;
    let mut round = 0;
    loop {
        // The deadline decides whether another punch starts, never how long one already in
        // flight may take: that one runs to PUNCH_GRACE past it.
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            log::debug!("None of {round} punches to {peer_addr} was met");
            return None;
        }
        // Cut at the deadline rather than slept out past it, so the window ends on a punch and
        // not on a gap of up to PUNCH_MAX_INTERVAL: the controller's window opened after ours,
        // on the PunchHoleSent hbbs relayed, so one as long as ours is still open through our tail.
        tokio::time::sleep(Duration::from_secs_f32(interval).min(left)).await;
        interval = (interval * PUNCH_BACKOFF).min(PUNCH_MAX_INTERVAL);
        let ms = until.saturating_duration_since(Instant::now()).as_millis() as u64 + PUNCH_GRACE;
        match punch(ms).await {
            // The controller's SYN crossed this punch, so the stream is the connection it
            // dialed, not a spare one: dropping it would reset that connection.
            Ok(stream) => return Some(stream),
            // Not logged one by one, but the count says which gateway it was: RST fails a
            // punch at once and fits a dozen into the window, a silent drop holds the one
            // punch for the whole of it. `connect_tcp_local` keeps no errno anyway.
            Err(_) => round += 1,
        }
    }
}

async fn serve_punched(
    server: ServerPtr,
    stream: Stream,
    peer_addr: SocketAddr,
    meta: ConnectionMeta,
) {
    log::info!("Punched tcp hole to {peer_addr}, connected on the punch itself");
    if let Err(err) =
        crate::server::create_tcp_connection(server, stream, peer_addr, true, meta).await
    {
        log::warn!("Failed to serve the connection punched to {peer_addr}: {err}");
    }
}

/// The accept half of `accept_connection`, kept here because only the accept may race the punch.
async fn accept_punched_connection(
    server: ServerPtr,
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    meta: ConnectionMeta,
) {
    use crate::server::create_tcp_connection;

    stream.set_nodelay(true).ok();
    match stream.local_addr() {
        Ok(stream_addr) => {
            let stream = Stream::from(stream, stream_addr);
            if let Err(err) = create_tcp_connection(server, stream, addr, true, meta).await {
                log::warn!("Failed to serve the connection from {addr}: {err}");
            }
        }
        Err(err) => log::warn!("Failed to read the address accepted from {addr}: {err}"),
    }
}

// When config is not yet synced from root, register_pk may have already been sent with a new generated pk.
// After config sync completes, the pk may change. This struct detects pk changes and triggers
// a re-registration by setting key_confirmed to false.
// NOTE:
// This only corrects PK registration for the current ID. If root uses a non-default mac-generated ID,
// this does not resolve the multi-ID issue by itself.
pub struct CheckIfResendPk {
    pk: Option<Vec<u8>>,
}
impl CheckIfResendPk {
    pub fn new() -> Self {
        Self {
            pk: Config::get_cached_pk(),
        }
    }
}
impl Drop for CheckIfResendPk {
    fn drop(&mut self) {
        if SENT_REGISTER_PK.load(Ordering::SeqCst) && Config::get_cached_pk() != self.pk {
            Config::set_key_confirmed(false);
            log::info!("Set key_confirmed to false due to pk changed, will resend register_pk");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{mpsc, socket_client, tokio, IceRoute, ICE_DEDUP_WINDOW, MAX_PENDING_REMOTE_ICE};
    use hbb_common::tcp::new_listener;
    use std::net::SocketAddr;

    // A SOCKS proxy makes `connect_tcp_local` dial the proxy and ignore the local address, so
    // nothing these two assert can hold. Read once, from the same global config production reads.
    fn proxied() -> bool {
        hbb_common::config::Config::get_socks().is_some()
    }

    /// Both held while their addresses are read, so the pair cannot be the same port - which
    /// `SO_REUSEPORT` would let bind twice rather than refuse, leaving the tests degenerate.
    async fn free_loopback_pair() -> (SocketAddr, SocketAddr) {
        let (a, b) = (
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(),
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(),
        );
        (a.local_addr().unwrap(), b.local_addr().unwrap())
    }

    fn queue(route: &mut IceRoute, candidate: &str) -> bool {
        route.queue(candidate.to_owned())
    }

    #[test]
    fn the_re_sent_copy_does_not_spend_a_queue_slot() {
        // Two slots, three sends: without the dedup the re-send takes the second and "relay",
        // the one that traverses NAT, is the one refused.
        let (tx, mut rx) = mpsc::channel::<String>(2);
        let mut route = IceRoute::new(tx);
        for _ in 0..2 {
            assert!(queue(&mut route, "host"));
        }
        assert!(queue(&mut route, "relay"));
        let mut queued = Vec::new();
        while let Ok(candidate) = rx.try_recv() {
            queued.push(candidate);
        }
        assert_eq!(queued, vec!["host".to_owned(), "relay".to_owned()]);
    }

    #[test]
    fn a_candidate_the_full_queue_refused_is_not_remembered() {
        let (tx, mut rx) = mpsc::channel::<String>(1);
        let mut route = IceRoute::new(tx);
        assert!(queue(&mut route, "host"));
        assert!(!queue(&mut route, "relay"));
        // The re-send is the only repair for a refused candidate; remembering it would swallow it.
        assert_eq!(rx.try_recv().ok(), Some("host".to_owned()));
        assert!(queue(&mut route, "relay"));
        assert_eq!(rx.try_recv().ok(), Some("relay".to_owned()));
    }

    #[test]
    fn a_re_send_is_skipped_while_the_original_is_still_queued() {
        let (tx, mut rx) = mpsc::channel::<String>(MAX_PENDING_REMOTE_ICE);
        let mut route = IceRoute::new(tx);
        for i in 0..MAX_PENDING_REMOTE_ICE {
            assert!(queue(&mut route, &format!("candidate-{}", i)));
        }
        assert!(queue(&mut route, "candidate-0"));
        let mut queued = 0;
        while rx.try_recv().is_ok() {
            queued += 1;
        }
        assert_eq!(queued, MAX_PENDING_REMOTE_ICE);
    }

    #[test]
    fn the_window_forgets_in_arrival_order() {
        let (tx, mut rx) = mpsc::channel::<String>(MAX_PENDING_REMOTE_ICE);
        let mut route = IceRoute::new(tx);
        for i in 0..=ICE_DEDUP_WINDOW {
            assert!(queue(&mut route, &format!("candidate-{}", i)));
            assert!(rx.try_recv().is_ok());
        }
        // The oldest digest made room for the newest, so its re-send is admitted again.
        assert!(queue(&mut route, "candidate-0"));
        assert!(rx.try_recv().is_ok());
        // A recent one is still skipped.
        let recent = format!("candidate-{}", ICE_DEDUP_WINDOW);
        assert!(queue(&mut route, &recent));
        assert!(rx.try_recv().is_err());
    }

    // The second way in that the repeat punch opens: a punch reaching a peer already in SYN_SENT
    // is answered by that socket rather than reset, and the two ends come up on one connection.
    // A punch that misses the crossing is reset outright here, loopback having no NAT to absorb
    // it and no round trip to hide behind - so a single punch lands only by luck, and repeating
    // is what makes it land at all. That is the premise of the repeat, asserted directly. A round
    // that misses costs one loopback RST, so rounds are cheap and there are many.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_punch_that_meets_the_peers_syn_connects_both_ends() {
        // The crossing needs both connects genuinely in flight at once. Loopback answers a SYN to
        // a port nobody is listening on with an instant RST, so on one CPU the first connect runs
        // to completion before the second is scheduled and no round can ever cross - a property of
        // the box, which this test cannot tell apart from a broken punch.
        if proxied() || std::thread::available_parallelism().map_or(true, |cpus| cpus.get() < 2) {
            return;
        }
        for _ in 0..256 {
            let (a, b) = free_loopback_pair().await;
            // Held for the whole crossing, because production always has one here and the design
            // rests on which of the two the kernel hands the connection to: the punch and the
            // peer's SYN share a four-tuple exactly, the listener only matches the address, and
            // the punch has to win that or every crossing would be swallowed as a plain accept.
            let listener = new_listener(a, true).await.unwrap();
            let to_b = tokio::spawn(socket_client::connect_tcp_local(b, Some(a), 3000));
            let to_a = tokio::spawn(socket_client::connect_tcp_local(a, Some(b), 3000));
            let (at_a, at_b) = tokio::join!(to_b, to_a);
            let (Ok(Ok(mut at_a)), Ok(Ok(mut at_b))) = (at_a, at_b) else {
                continue;
            };
            at_a.send_bytes(bytes::Bytes::from_static(b"punch"))
                .await
                .unwrap();
            let got = at_b.next_timeout(3000).await.unwrap().unwrap();
            assert_eq!(&got[..], b"punch", "both ends must share one connection");
            assert!(
                hbb_common::timeout(200, listener.accept()).await.is_err(),
                "the crossing must reach the punch, not be accepted as an inbound connection"
            );
            return;
        }
        panic!("no punch met the peer's SYN in 256 rounds on a machine that can cross them");
    }

    // The punch binds the address the listener already holds, so it has to go through the same
    // `connect_tcp_local` production uses - a punch built by hand here would still pass if
    // `new_socket` ever stopped setting the reuse flags, while every real punch failed to bind.
    // The peer's view of the source port is what proves the bind took: a fallback to an ephemeral
    // one would connect just as happily.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_punch_binds_the_address_the_listener_holds() {
        if proxied() {
            return;
        }
        // `free_loopback_pair` hands back ports it no longer holds, so another process can take
        // one in between; retry rather than fail for something the punch had no part in.
        for _ in 0..8 {
            let (local, peer_addr) = free_loopback_pair().await;
            let (Ok(listener), Ok(peer)) = (
                new_listener(local, true).await,
                new_listener(peer_addr, true).await,
            ) else {
                continue;
            };
            let punch = tokio::spawn(socket_client::connect_tcp_local(
                peer_addr,
                Some(local),
                1500,
            ));
            let (_peer_side, seen_as) = hbb_common::timeout(3000, peer.accept())
                .await
                .expect("the punch must reach the peer")
                .unwrap();
            assert_eq!(
                seen_as.port(),
                local.port(),
                "the punch must leave from the address the listener holds, not an ephemeral one"
            );
            // Held, not asserted and dropped: the coexistence below is only exercised while this
            // socket is still on the address, which is the state production spends its window in.
            let _punched = punch.await.unwrap().expect("the punch must connect");

            let dialed = tokio::spawn(tokio::net::TcpStream::connect(local));
            let accepted = hbb_common::timeout(3000, listener.accept()).await;
            assert!(
                matches!(accepted, Ok(Ok(_))),
                "the listener must still take connections while a punch shares its address: {accepted:?}"
            );
            assert!(dialed.await.unwrap().is_ok());
            return;
        }
        panic!("could not hold two free loopback addresses in 8 tries");
    }

    // The schedule on its own, against a paused clock: the window is CONNECT_TIMEOUT long, and
    // what these pin is where inside it the punches fall, which no socket could show.
    #[tokio::test(start_paused = true)]
    async fn the_punches_end_on_one_at_the_deadline() {
        use super::{punch_until, PUNCH_GRACE, PUNCH_INTERVAL, PUNCH_MAX_INTERVAL};
        use hbb_common::{anyhow::anyhow, config::CONNECT_TIMEOUT};
        use std::time::Duration;
        use tokio::time::Instant;

        let peer: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let start = Instant::now();
        let until = start + Duration::from_millis(CONNECT_TIMEOUT);
        let mut punches = Vec::new();
        // A gateway that answers with RST: every punch fails the moment it is made.
        let met = punch_until::<(), _, _>(until, peer, |ms| {
            punches.push((Instant::now(), ms));
            async { Err(anyhow!("RST")) }
        })
        .await;
        assert!(met.is_none());
        assert_eq!(
            Instant::now(),
            until,
            "must return the moment the window closes, not a backoff later"
        );
        // Tokio rounds every sleep up to the next millisecond.
        let slack = Duration::from_millis(1);
        assert!(punches[0].0 - start <= Duration::from_secs_f32(PUNCH_INTERVAL) + slack);
        for pair in punches.windows(2) {
            assert!(
                pair[1].0 - pair[0].0 <= Duration::from_secs_f32(PUNCH_MAX_INTERVAL) + slack,
                "no gap in the window may exceed the backoff ceiling: {pair:?}"
            );
        }
        assert_eq!(
            *punches.last().unwrap(),
            (until, PUNCH_GRACE),
            "the window must end on a punch, given the whole grace"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_punch_in_flight_runs_the_grace_past_the_deadline_and_no_further() {
        use super::{punch_until, PUNCH_GRACE};
        use hbb_common::{anyhow::anyhow, config::CONNECT_TIMEOUT};
        use std::time::Duration;
        use tokio::time::Instant;

        let peer: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let until = Instant::now() + Duration::from_millis(CONNECT_TIMEOUT);
        let mut punches = 0;
        // A gateway that drops the SYN in silence: the punch sits in SYN_SENT for all it is given.
        let met = punch_until::<(), _, _>(until, peer, |ms| {
            punches += 1;
            async move {
                tokio::time::sleep(Duration::from_millis(ms)).await;
                Err(anyhow!("timed out"))
            }
        })
        .await;
        assert!(met.is_none());
        assert_eq!(
            punches, 1,
            "a punch held in SYN_SENT is the only one the window needs"
        );
        assert_eq!(
            Instant::now(),
            until + Duration::from_millis(PUNCH_GRACE),
            "must return when the grace runs out, not a backoff later"
        );
    }
}
