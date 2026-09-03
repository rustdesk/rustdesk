#[cfg(not(target_os = "ios"))]
use hbb_common::whoami;
use hbb_common::{
    allow_err,
    anyhow::{anyhow, bail},
    config::Config,
    config::{self, RENDEZVOUS_PORT},
    log,
    protobuf::Message as _,
    rendezvous_proto::*,
    tokio::{
        self,
        io::AsyncReadExt,
        sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    },
    ResultType,
};

use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs, UdpSocket},
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

type Message = RendezvousMessage;

pub struct DiscoveryResult {
    pub found_peers: bool,
    pub firewall_blocked: bool,
}

struct DiscoveryProbe {
    socket: UdpSocket,
    targets: Vec<SocketAddr>,
}

const DISCOVERY_SCAN_TIMEOUT: Duration = Duration::from_secs(15);
const DISCOVERY_RETRY_BASE: Duration = Duration::from_millis(500);
const DISCOVERY_RETRY_CAP: Duration = Duration::from_secs(8);
const TAILSCALE_STATUS_MAX_BYTES: usize = 1024 * 1024;
const TAILSCALE_STATUS_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[derive(serde::Deserialize)]
struct TailscaleStatus {
    #[serde(rename = "Peer", default)]
    peers: HashMap<String, TailscalePeer>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[derive(serde::Deserialize)]
struct TailscalePeer {
    #[serde(rename = "TailscaleIPs", default)]
    ips: Vec<IpAddr>,
    #[serde(rename = "Online", default)]
    online: bool,
}

#[cfg(not(target_os = "ios"))]
pub(super) fn start_listening() -> ResultType<()> {
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
                        if p.cmd == "ping"
                            && config::option2bool(
                                "enable-lan-discovery",
                                &Config::get_option("enable-lan-discovery"),
                            )
                        {
                            let id = Config::get_id();
                            if p.id == id {
                                continue;
                            }
                            if let Some(self_addr) = get_ipaddr_by_peer(&addr) {
                                let mut msg_out = Message::new();
                                let mut hostname = crate::whoami_hostname();
                                // The default hostname is "localhost" which is a bit confusing
                                if hostname == "localhost" {
                                    hostname = "unknown".to_owned();
                                }
                                let peer = PeerDiscovery {
                                    cmd: "pong".to_owned(),
                                    mac: get_mac(&self_addr),
                                    id,
                                    hostname,
                                    username: crate::platform::get_active_username(),
                                    platform: whoami::platform().to_string(),
                                    ..Default::default()
                                };
                                msg_out.set_peer_discovery(peer);
                                socket.send_to(&msg_out.write_to_bytes()?, addr).ok();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn discover(local_id: String) -> ResultType<DiscoveryResult> {
    let local_ips = Arc::new(get_local_ips());
    let (sockets, probes, query, scan_started, scan_deadline) =
        send_query(&local_id, &local_ips).await?;
    let response_ports = sockets
        .iter()
        .filter_map(|socket| socket.local_addr().ok().map(|addr| addr.port()))
        .collect::<Vec<_>>();
    spawn_query_retries(probes, query, scan_deadline);
    let rx = spawn_wait_responses(sockets, local_id.clone(), local_ips, scan_deadline);
    let found_peers = handle_received_peers(rx, &local_id).await?;
    #[cfg(target_os = "linux")]
    let firewall_blocked =
        crate::platform::linux::lan_discovery_firewall_blocked(scan_started, &response_ports).await;
    #[cfg(not(target_os = "linux"))]
    let firewall_blocked = false;

    log::info!("discover ping done");
    Ok(DiscoveryResult {
        found_peers,
        firewall_blocked,
    })
}

pub fn send_wol(id: String) {
    let interfaces = default_net::get_interfaces();
    for peer in &config::LanPeers::load().peers {
        if peer.id == id {
            for (_, mac) in peer.ip_mac.iter() {
                if let Ok(mac_addr) = mac.parse() {
                    for interface in &interfaces {
                        for ipv4 in &interface.ipv4 {
                            // remove below mask check to avoid unexpected bug
                            // if (u32::from(ipv4.addr) & u32::from(ipv4.netmask)) == (u32::from(peer_ip) & u32::from(ipv4.netmask))
                            log::info!("Send wol to {mac_addr} of {}", ipv4.addr);
                            allow_err!(wol::send_wol(mac_addr, None, Some(IpAddr::V4(ipv4.addr))));
                        }
                    }
                }
            }
            break;
        }
    }
}

#[inline]
fn get_broadcast_port() -> u16 {
    (RENDEZVOUS_PORT + 3) as _
}

fn get_mac(_ip: &IpAddr) -> String {
    #[cfg(not(target_os = "ios"))]
    if let Ok(mac) = get_mac_by_ip(_ip) {
        mac.to_string()
    } else {
        "".to_owned()
    }
    #[cfg(target_os = "ios")]
    "".to_owned()
}

#[cfg(not(target_os = "ios"))]
fn get_mac_by_ip(ip: &IpAddr) -> ResultType<String> {
    for interface in default_net::get_interfaces() {
        match ip {
            IpAddr::V4(local_ipv4) => {
                if interface.ipv4.iter().any(|x| x.addr == *local_ipv4) {
                    if let Some(mac_addr) = interface.mac_addr {
                        return Ok(mac_addr.address());
                    }
                }
            }
            IpAddr::V6(local_ipv6) => {
                if interface.ipv6.iter().any(|x| x.addr == *local_ipv6) {
                    if let Some(mac_addr) = interface.mac_addr {
                        return Ok(mac_addr.address());
                    }
                }
            }
        }
    }
    bail!("No interface found for ip: {:?}", ip);
}

// Mainly from https://github.com/shellrow/default-net/blob/cf7ca24e7e6e8e566ed32346c9cfddab3f47e2d6/src/interface/shared.rs#L4
fn get_ipaddr_by_peer<A: ToSocketAddrs>(peer: A) -> Option<IpAddr> {
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return None,
    };

    match socket.connect(peer) {
        Ok(()) => (),
        Err(_) => return None,
    };

    match socket.local_addr() {
        Ok(addr) => return Some(addr.ip()),
        Err(_) => return None,
    };
}

fn get_local_ips() -> HashSet<IpAddr> {
    #[cfg(not(target_os = "ios"))]
    return default_net::get_interfaces()
        .into_iter()
        .flat_map(|interface| {
            interface
                .ipv4
                .into_iter()
                .map(|network| IpAddr::V4(network.addr))
                .chain(
                    interface
                        .ipv6
                        .into_iter()
                        .map(|network| IpAddr::V6(network.addr)),
                )
        })
        .collect();

    #[cfg(target_os = "ios")]
    HashSet::new()
}

fn create_broadcast_sockets() -> Vec<UdpSocket> {
    let mut ipv4s = Vec::new();
    // TODO: maybe we should use a better way to get ipv4 addresses.
    // But currently, it's ok to use `[Ipv4Addr::UNSPECIFIED]` for discovery.
    // `default_net::get_interfaces()` causes undefined symbols error when `flutter build` on iOS simulator x86_64
    #[cfg(not(any(target_os = "ios")))]
    for interface in default_net::get_interfaces() {
        for ipv4 in &interface.ipv4 {
            ipv4s.push(ipv4.addr.clone());
        }
    }
    ipv4s.push(Ipv4Addr::UNSPECIFIED); // for robustness
    let mut sockets = Vec::new();
    for v4_addr in ipv4s {
        // removing v4_addr.is_private() check, https://github.com/rustdesk/rustdesk/issues/4663
        if let Ok(s) = UdpSocket::bind(SocketAddr::from((v4_addr, 0))) {
            if s.set_broadcast(true).is_ok() {
                sockets.push(s);
            }
        }
    }
    sockets
}

async fn send_query(
    local_id: &str,
    local_ips: &HashSet<IpAddr>,
) -> ResultType<(
    Vec<UdpSocket>,
    Vec<DiscoveryProbe>,
    Vec<u8>,
    SystemTime,
    Instant,
)> {
    let mut sockets = create_broadcast_sockets();
    if sockets.is_empty() {
        bail!("Found no bindable ipv4 addresses");
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let targets = tailscale_targets(local_ips).await;

    let mut msg_out = Message::new();
    // We may not be able to get the mac address on mobile platforms.
    // So we need to use the id to avoid discovering ourselves.
    let peer = PeerDiscovery {
        cmd: "ping".to_owned(),
        id: local_id.to_owned(),
        ..Default::default()
    };
    msg_out.set_peer_discovery(peer);
    let out = msg_out.write_to_bytes()?;
    let scan_started = SystemTime::now();
    let scan_deadline = Instant::now() + DISCOVERY_SCAN_TIMEOUT;
    let maddr = SocketAddr::from(([255, 255, 255, 255], get_broadcast_port()));
    let mut probes = Vec::new();
    for socket in &sockets {
        allow_err!(socket.send_to(&out, maddr));
        probes.push(DiscoveryProbe {
            socket: socket.try_clone()?,
            targets: vec![maddr],
        });
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if !targets.is_empty() {
            let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)))?;
            for target in &targets {
                allow_err!(socket.send_to(&out, target));
            }
            probes.push(DiscoveryProbe {
                socket: socket.try_clone()?,
                targets,
            });
            sockets.push(socket);
        }
    }
    log::info!("discover ping sent");
    Ok((sockets, probes, out, scan_started, scan_deadline))
}

fn retry_ceiling(attempt: u32) -> Duration {
    DISCOVERY_RETRY_BASE
        .saturating_mul(2_u32.saturating_pow(attempt))
        .min(DISCOVERY_RETRY_CAP)
}

fn full_jitter(ceiling: Duration) -> Duration {
    use hbb_common::rand::Rng as _;

    Duration::from_millis(hbb_common::rand::thread_rng().gen_range(0..=ceiling.as_millis() as u64))
}

fn retry_time(now: Instant, deadline: Instant, jitter: Duration) -> Option<Instant> {
    now.checked_add(jitter).filter(|retry| *retry < deadline)
}

fn spawn_query_retries(probes: Vec<DiscoveryProbe>, query: Vec<u8>, deadline: Instant) {
    std::thread::spawn(move || {
        let mut attempt = 0;
        loop {
            let now = Instant::now();
            let Some(retry) = retry_time(now, deadline, full_jitter(retry_ceiling(attempt))) else {
                break;
            };
            std::thread::sleep(retry.saturating_duration_since(now));
            if Instant::now() >= deadline {
                break;
            }
            for probe in &probes {
                for target in &probe.targets {
                    allow_err!(probe.socket.send_to(&query, target));
                }
            }
            attempt = attempt.saturating_add(1);
        }
    });
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn parse_tailscale_targets(
    data: &[u8],
    local_ips: &HashSet<IpAddr>,
) -> ResultType<Vec<SocketAddr>> {
    if data.len() > TAILSCALE_STATUS_MAX_BYTES {
        bail!("Tailscale status output exceeds limit");
    }
    let status: TailscaleStatus = serde_json::from_slice(data)?;
    let mut targets = HashSet::new();
    for peer in status.peers.into_values().filter(|peer| peer.online) {
        for ip in peer.ips {
            if ip.is_ipv4() && !local_ips.contains(&ip) {
                targets.insert(SocketAddr::new(ip, get_broadcast_port()));
            }
        }
    }
    Ok(targets.into_iter().collect())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn read_tailscale_status(command: &str) -> ResultType<Vec<u8>> {
    let mut child = tokio::process::Command::new(command)
        .args(["status", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let Some(stdout) = child.stdout.take() else {
        bail!("Failed to read Tailscale status output");
    };
    let read = async {
        let mut data = Vec::new();
        stdout
            .take((TAILSCALE_STATUS_MAX_BYTES + 1) as u64)
            .read_to_end(&mut data)
            .await?;
        if data.len() > TAILSCALE_STATUS_MAX_BYTES {
            child.kill().await.ok();
            bail!("Tailscale status output exceeds limit");
        }
        let status = child.wait().await?;
        if !status.success() {
            bail!("Tailscale status command failed");
        }
        Ok(data)
    };
    match tokio::time::timeout(TAILSCALE_STATUS_TIMEOUT, read).await {
        Ok(result) => result,
        Err(_) => {
            child.kill().await.ok();
            Err(anyhow!("Tailscale status command timed out"))
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn tailscale_targets(local_ips: &HashSet<IpAddr>) -> Vec<SocketAddr> {
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut commands = vec!["tailscale"];
    #[cfg(target_os = "macos")]
    commands.push("/Applications/Tailscale.app/Contents/MacOS/Tailscale");

    for command in commands {
        match read_tailscale_status(command).await {
            Ok(data) => match parse_tailscale_targets(&data, local_ips) {
                Ok(targets) => return targets,
                Err(err) => {
                    log::debug!("Failed to parse Tailscale status: {err}");
                    return Vec::new();
                }
            },
            Err(err) => log::debug!("Failed to query Tailscale status with {command}: {err}"),
        }
    }
    Vec::new()
}

fn is_usable_mac(mac: &str) -> bool {
    !mac.is_empty() && mac != "00:00:00:00:00:00"
}

fn is_self_response(
    local_id: &str,
    local_ips: &HashSet<IpAddr>,
    source_ip: IpAddr,
    local_mac: &str,
    peer: &PeerDiscovery,
) -> bool {
    (!local_id.is_empty() && peer.id == local_id)
        || local_ips.contains(&source_ip)
        || (is_usable_mac(local_mac)
            && is_usable_mac(&peer.mac)
            && local_mac.eq_ignore_ascii_case(&peer.mac))
}

fn wait_response(
    socket: UdpSocket,
    tx: UnboundedSender<config::DiscoveryPeer>,
    local_id: String,
    local_ips: Arc<HashSet<IpAddr>>,
    deadline: Instant,
) -> ResultType<()> {
    let local_addr = socket.local_addr();
    let try_get_ip_by_peer = match local_addr.as_ref() {
        Err(..) => true,
        Ok(addr) => addr.ip().is_unspecified(),
    };
    let mut mac: Option<String> = None;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        socket.set_read_timeout(Some(remaining.min(Duration::from_millis(10))))?;
        let mut buf = [0; 2048];
        if let Ok((len, addr)) = socket.recv_from(&mut buf) {
            if let Ok(msg_in) = Message::parse_from_bytes(&buf[0..len]) {
                match msg_in.union {
                    Some(rendezvous_message::Union::PeerDiscovery(p)) => {
                        if p.cmd == "pong" {
                            if !crate::common::is_valid_untrusted_peer_id(&p.id) {
                                log::warn!(
                                    "Ignoring LAN discovery response from {} with invalid peer id",
                                    addr
                                );
                                continue;
                            }

                            let local_mac = if try_get_ip_by_peer {
                                if let Some(self_addr) = get_ipaddr_by_peer(&addr) {
                                    get_mac(&self_addr)
                                } else {
                                    "".to_owned()
                                }
                            } else {
                                match mac.as_ref() {
                                    Some(m) => m.clone(),
                                    None => {
                                        let m = if let Ok(local_addr) = local_addr {
                                            get_mac(&local_addr.ip())
                                        } else {
                                            "".to_owned()
                                        };
                                        mac = Some(m.clone());
                                        m
                                    }
                                }
                            };

                            if !is_self_response(&local_id, &local_ips, addr.ip(), &local_mac, &p) {
                                allow_err!(tx.send(config::DiscoveryPeer {
                                    id: p.id.clone(),
                                    ip_mac: HashMap::from([
                                        (addr.ip().to_string(), p.mac.clone(),)
                                    ]),
                                    username: p.username.clone(),
                                    hostname: p.hostname.clone(),
                                    platform: p.platform.clone(),
                                    online: true,
                                }));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn spawn_wait_responses(
    sockets: Vec<UdpSocket>,
    local_id: String,
    local_ips: Arc<HashSet<IpAddr>>,
    deadline: Instant,
) -> UnboundedReceiver<config::DiscoveryPeer> {
    let (tx, rx) = unbounded_channel::<_>();
    for socket in sockets {
        let tx_clone = tx.clone();
        let local_id = local_id.clone();
        let local_ips = local_ips.clone();
        std::thread::spawn(move || {
            allow_err!(wait_response(
                socket, tx_clone, local_id, local_ips, deadline,
            ));
        });
    }
    rx
}

async fn handle_received_peers(
    mut rx: UnboundedReceiver<config::DiscoveryPeer>,
    local_id: &str,
) -> ResultType<bool> {
    let mut peers = config::LanPeers::load().peers;
    if !local_id.is_empty() {
        peers.retain(|peer| peer.id != local_id);
    }
    peers.iter_mut().for_each(|peer| {
        peer.online = false;
    });

    let mut response_set = HashSet::new();
    let mut last_write_time: Option<Instant> = None;
    loop {
        tokio::select! {
            data = rx.recv() => match data {
                Some(mut peer) => {
                    let in_response_set = !response_set.insert(peer.id.clone());
                    if let Some(pos) = peers.iter().position(|x| x.is_same_peer(&peer) ) {
                        let peer1 = peers.remove(pos);
                        if in_response_set {
                            peer.ip_mac.extend(peer1.ip_mac);
                            peer.online = true;
                        }
                    }
                    peers.insert(0, peer);
                    if last_write_time.map(|t| t.elapsed().as_millis() > 300).unwrap_or(true)  {
                        config::LanPeers::store(&peers);
                        #[cfg(feature = "flutter")]
                        crate::flutter_ffi::main_load_lan_peers();
                        last_write_time = Some(Instant::now());
                    }
                }
                None => {
                    break
                }
            }
        }
    }

    config::LanPeers::store(&peers);
    #[cfg(feature = "flutter")]
    crate::flutter_ffi::main_load_lan_peers();
    Ok(!response_set.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(id: &str, mac: &str) -> PeerDiscovery {
        PeerDiscovery {
            id: id.to_owned(),
            mac: mac.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn self_response_uses_id_ip_and_only_usable_macs() {
        let local_ip: IpAddr = "100.64.0.1".parse().unwrap();
        let remote_ip: IpAddr = "100.64.0.2".parse().unwrap();
        let local_ips = HashSet::from([local_ip]);

        assert!(is_self_response(
            "123",
            &local_ips,
            remote_ip,
            "",
            &peer("123", "")
        ));
        assert!(is_self_response(
            "123",
            &local_ips,
            local_ip,
            "",
            &peer("456", "")
        ));
        assert!(is_self_response(
            "123",
            &local_ips,
            remote_ip,
            "aa:bb:cc:dd:ee:ff",
            &peer("456", "AA:BB:CC:DD:EE:FF")
        ));
        assert!(!is_self_response(
            "123",
            &local_ips,
            remote_ip,
            "",
            &peer("456", "")
        ));
        assert!(!is_self_response(
            "123",
            &local_ips,
            remote_ip,
            "00:00:00:00:00:00",
            &peer("456", "00:00:00:00:00:00")
        ));
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn parses_online_remote_tailscale_ipv4_targets() {
        let local_ip: IpAddr = "100.64.0.1".parse().unwrap();
        let data = br#"{
            "Unknown": true,
            "Peer": {
                "a": {"Online": true, "TailscaleIPs": ["100.64.0.1", "100.64.0.2", "fd7a:115c:a1e0::1"]},
                "b": {"Online": false, "TailscaleIPs": ["100.64.0.3"]},
                "c": {"Online": true, "TailscaleIPs": ["100.64.0.2"]}
            }
        }"#;
        let targets = parse_tailscale_targets(data, &HashSet::from([local_ip])).unwrap();

        assert_eq!(targets, vec!["100.64.0.2:21119".parse().unwrap()]);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn rejects_invalid_or_oversized_tailscale_status() {
        assert!(parse_tailscale_targets(b"not json", &HashSet::new()).is_err());
        assert!(parse_tailscale_targets(
            &vec![b' '; TAILSCALE_STATUS_MAX_BYTES + 1],
            &HashSet::new()
        )
        .is_err());
    }

    #[test]
    fn retry_backoff_doubles_and_caps() {
        assert_eq!(retry_ceiling(0), Duration::from_millis(500));
        assert_eq!(retry_ceiling(1), Duration::from_secs(1));
        assert_eq!(retry_ceiling(2), Duration::from_secs(2));
        assert_eq!(retry_ceiling(3), Duration::from_secs(4));
        assert_eq!(retry_ceiling(4), Duration::from_secs(8));
        assert_eq!(retry_ceiling(10), Duration::from_secs(8));
    }

    #[test]
    fn retry_time_uses_jitter_and_stays_before_deadline() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(15);
        let ceiling = Duration::from_secs(4);

        for _ in 0..100 {
            assert!(full_jitter(ceiling) <= ceiling);
        }

        assert_eq!(
            retry_time(now, deadline, Duration::from_millis(375)),
            Some(now + Duration::from_millis(375))
        );
        assert_eq!(retry_time(now, deadline, Duration::from_secs(15)), None);
        assert_eq!(retry_time(now, deadline, Duration::from_secs(16)), None);
    }
}
