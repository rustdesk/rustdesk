//! SOS 信令注册与连接接入模块
//!
//! 与 RustDesk 信令服务器通信，负责：
//! 1. 设备注册（RegisterPeer 心跳）
//! 2. 公钥注册（RegisterPk）
//! 3. 监听 PunchHole（直连）和 RequestRelay（中继）消息
//! 4. 建立数据连接并派发到 sos_connection::handle()

use crate::sos_config::SosConfig;
use hbb_common::protobuf::Message as ProtobufMessage;
use hbb_common::rendezvous_proto::*;
use hbb_common::socket_client::{check_port, connect_tcp, new_udp_for};
use hbb_common::IntoTargetAddr;
use hbb_common::ResultType;
use std::time::Instant;

/// 打洞时等待控制端连入的超时（毫秒）。
///
/// 控制端自己的 TCP 打洞超时通常是「打洞耗时 × 若干倍」，实际在 1-6 秒量级；
/// 取 10 秒与内网直连（`send_local_addr_fla`）保持一致，超时后再回退中继。
const PUNCH_ACCEPT_TIMEOUT: u64 = 10_000;

/// 决定本次连接要用的中继候选列表。
///
/// 优先级见 [`crate::sos_relay::resolve_relay_candidates`]：
/// 本地 `--relay` > 服务器下发 > 信号服务器端口 +1。
fn resolve_relay_candidates(config: &SosConfig, provided: &str) -> Vec<String> {
    let mut list = crate::sos_relay::resolve_relay_candidates(&config.relay_server, provided);
    if list.is_empty() {
        // 兜底：RustDesk 默认把 hbbr 放在 hbbs 同一台主机的下一个端口
        list.push(hbb_common::socket_client::increase_port(
            &config.rendezvous_server,
            1,
        ));
    }
    list
}

/// 运行信令注册与连接接入循环
///
/// 连接到信令服务器 → 注册设备 ID → 公钥注册 → 持续监听 PunchHole/RequestRelay
/// `password_refresh_tx`：连接断开后通知主进程刷新临时密码
pub async fn run(
    mut config: SosConfig,
    _tray_tx: tokio::sync::mpsc::UnboundedSender<crate::sos_tray::TrayCommand>,
    password_refresh_tx: tokio::sync::mpsc::UnboundedSender<()>,
) -> ResultType<()> {
    log::info!(
        "[RENDEZVOUS] run() entered id={} server={} fixed_pwd={}",
        config.id,
        config.rendezvous_server,
        !config.password.is_empty()
    );
    let host = check_port(
        &config.rendezvous_server,
        crate::sos_constants::RENDEZVOUS_PORT,
    );
    log::info!("Connecting to rendezvous server: {}", host);

    let (mut socket, target_addr) =
        new_udp_for(&host, crate::sos_constants::CONNECT_TIMEOUT).await?;
    let mut peer_addr: std::net::SocketAddr = target_addr.into_target_addr()?.into();
    log::info!("UDP connection to rendezvous server established");

    // 首次注册
    send_register_peer(&mut socket, &peer_addr, &config.id, 0).await?;

    // 主动发送 RegisterPk（不等待服务器请求）
    // SOS 每次启动生成新的 Ed25519 密钥对，必须主动注册到服务器
    let (_sk, pk) = crate::sos_config::RegistryConfig::get_key_pair();
    if !pk.is_empty() {
        if let Err(e) = send_register_pk(&mut socket, &peer_addr, &config, &pk).await {
            log::warn!("Proactive RegisterPk failed: {}", e);
        } else {
            log::info!("Proactive RegisterPk sent");
        }
    }

    // NAT 类型测试（异步，不阻塞主循环）
    let rz_host = host.clone();
    tokio::spawn(async move {
        test_nat_type(&rz_host).await;
    });

    let mut last_reg = Instant::now();
    let reg_interval = std::time::Duration::from_millis(crate::sos_constants::REG_INTERVAL);
    let listen_timeout = std::time::Duration::from_secs(5);
    let serial = std::sync::atomic::AtomicU32::new(1);

    let mut iteration = 0u64;
    loop {
        iteration += 1;
        log::info!("[RENDEZVOUS] iteration #{} start", iteration);
        // 定时重新注册（心跳）
        if last_reg.elapsed() >= reg_interval {
            log::info!("[RENDEZVOUS] heartbeat due, sending RegisterPeer");
            let s = serial.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Err(e) = send_register_peer(&mut socket, &peer_addr, &config.id, s).await {
                log::warn!("[RENDEZVOUS] RegisterPeer heartbeat failed: {}", e);
                // 不退出，下次循环重试
            } else {
                last_reg = Instant::now();
            }
        }

        // 监听信令消息
        log::info!(
            "[RENDEZVOUS] waiting for UDP message (timeout={}ms)",
            listen_timeout.as_millis()
        );
        match hbb_common::timeout(listen_timeout.as_millis() as u64, socket.next()).await {
            Ok(Some(Ok((bytes, _from)))) => {
                log::info!("[RENDEZVOUS] received {} bytes via UDP", bytes.len());
                log::info!(
                    "[RENDEZVOUS] raw bytes (first 32): {}",
                    hex::encode(&bytes[..bytes.len().min(32)])
                );
                match RendezvousMessage::parse_from_bytes(&bytes) {
                    Ok(rm) => {
                        if let Err(e) = handle_rendezvous_message(
                            rm,
                            &mut config,
                            &mut socket,
                            &peer_addr,
                            &password_refresh_tx,
                        )
                        .await
                        {
                            log::warn!("[RENDEZVOUS] handle message error: {}", e);
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "[RENDEZVOUS] failed to parse message ({} bytes): {}",
                            bytes.len(),
                            e
                        );
                    }
                }
            }
            Ok(Some(Err(e))) => {
                log::warn!("[RENDEZVOUS] UDP recv error: {}", e);
            }
            Ok(None) => {
                // 连接关闭
                log::warn!("[RENDEZVOUS] connection closed, reconnecting...");
                match new_udp_for(&host, crate::sos_constants::CONNECT_TIMEOUT).await {
                    Ok((new_sock, new_addr)) => {
                        socket = new_sock;
                        peer_addr = new_addr
                            .into_target_addr()
                            .map(|a| a.into())
                            .unwrap_or_else(|_| {
                                log::warn!(
                                    "[RENDEZVOUS] failed to resolve target addr, using previous"
                                );
                                peer_addr
                            });
                        let s = serial.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if let Err(e) =
                            send_register_peer(&mut socket, &peer_addr, &config.id, s).await
                        {
                            log::warn!("[RENDEZVOUS] re-register after reconnection failed: {}", e);
                        }
                        last_reg = Instant::now();
                    }
                    Err(e) => {
                        log::error!("[RENDEZVOUS] reconnect failed: {}", e);
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
            Err(_) => {
                // 超时，正常循环
                log::info!(
                    "[RENDEZVOUS] UDP recv timeout (no data in {}ms)",
                    listen_timeout.as_millis()
                );
            }
        }
        log::info!("[RENDEZVOUS] iteration #{} complete", iteration);
    }
}

/// 发送 RegisterPeer 消息
async fn send_register_peer(
    socket: &mut hbb_common::udp::FramedSocket,
    peer_addr: &std::net::SocketAddr,
    device_id: &str,
    serial: u32,
) -> ResultType<()> {
    let mut msg = RendezvousMessage::new();
    msg.set_register_peer(RegisterPeer {
        id: device_id.to_owned(),
        serial: serial as i32,
        ..Default::default()
    });
    socket.send(&msg, *peer_addr).await?;
    log::info!("RegisterPeer sent for id={} serial={}", device_id, serial);
    Ok(())
}

/// 处理信令消息
async fn handle_rendezvous_message(
    rm: RendezvousMessage,
    config: &mut SosConfig,
    socket: &mut hbb_common::udp::FramedSocket,
    peer_addr: &std::net::SocketAddr,
    password_refresh_tx: &tokio::sync::mpsc::UnboundedSender<()>,
) -> ResultType<()> {
    // 记录收到的消息类型名称
    let msg_type = match rm.union {
        Some(rendezvous_message::Union::PunchHole(_)) => "PunchHole",
        Some(rendezvous_message::Union::RequestRelay(_)) => "RequestRelay",
        Some(rendezvous_message::Union::RegisterPeerResponse(_)) => "RegisterPeerResponse",
        Some(rendezvous_message::Union::RegisterPkResponse(_)) => "RegisterPkResponse",
        Some(rendezvous_message::Union::FetchLocalAddr(_)) => "FetchLocalAddr",
        _ => "Other",
    };
    log::info!("[RENDEZVOUS] message type={}", msg_type);
    match rm.union {
        Some(rendezvous_message::Union::PunchHole(ph)) => {
            log::info!("收到 PunchHole（直连请求），处理传入连接...");
            let config = (*config).clone();
            let refresh_tx = password_refresh_tx.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_punch_hole(ph, config, refresh_tx).await {
                    log::error!("PunchHole connection failed: {}", e);
                }
            });
        }
        Some(rendezvous_message::Union::RequestRelay(rr)) => {
            log::info!("收到 RequestRelay（中继请求），建立中继连接...");
            let config = (*config).clone();
            let refresh_tx = password_refresh_tx.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_request_relay(rr, config, refresh_tx).await {
                    log::error!("Relay connection failed: {}", e);
                }
            });
        }
        Some(rendezvous_message::Union::RegisterPeerResponse(rpr)) => {
            if rpr.request_pk {
                log::info!("信令服务器请求 RegisterPk（公钥注册）");
                let (_sk, pk) = crate::sos_config::RegistryConfig::get_key_pair();
                if !pk.is_empty() {
                    if let Err(e) = send_register_pk(socket, peer_addr, config, &pk).await {
                        log::warn!("Send RegisterPk failed: {}", e);
                    }
                } else {
                    log::warn!("密钥对未初始化，跳过 RegisterPk");
                }
            }
        }
        Some(rendezvous_message::Union::RegisterPkResponse(rpr)) => {
            log::info!("RegisterPk response: {:?}", rpr.result);
            match rpr.result.enum_value_or_default() {
                register_pk_response::Result::OK => {
                    crate::sos_config::RegistryConfig::set_key_confirmed(true);
                    log::info!("公钥注册成功");
                }
                register_pk_response::Result::UUID_MISMATCH => {
                    log::warn!("UUID_MISMATCH，设备身份冲突，更换 ID 后重新注册");
                    // 1. 标记 key_confirmed = false
                    crate::sos_config::RegistryConfig::set_key_confirmed(false);
                    // 2. 生成新 ID 并写入注册表，同时更新内存 config
                    config.id = crate::sos_config::RegistryConfig::update_id();
                    // 3. 用新 ID 重新注册公钥
                    let (_sk, pk) = crate::sos_config::RegistryConfig::get_key_pair();
                    if !pk.is_empty() {
                        if let Err(e) = send_register_pk(socket, peer_addr, config, &pk).await {
                            log::warn!("Re-register after UUID_MISMATCH failed: {}", e);
                        } else {
                            log::info!("Re-register after UUID_MISMATCH succeeded");
                        }
                    }
                }
                other => {
                    log::warn!("公钥注册失败: {:?}", other);
                }
            }
        }
        Some(rendezvous_message::Union::FetchLocalAddr(fla)) => {
            log::info!("FetchLocalAddr received, connecting back via TCP...");
            let config = (*config).clone();
            let refresh_tx = password_refresh_tx.clone();
            tokio::spawn(async move {
                if let Err(e) = send_local_addr_fla(fla, config, refresh_tx).await {
                    log::error!("LocalAddr/TCP connection failed: {}", e);
                }
            });
        }
        _ => {
            log::info!("收到未处理的消息类型");
        }
    }
    Ok(())
}

/// 发送 RegisterPk（公钥注册）消息
async fn send_register_pk(
    socket: &mut hbb_common::udp::FramedSocket,
    peer_addr: &std::net::SocketAddr,
    config: &SosConfig,
    pk: &[u8],
) -> ResultType<()> {
    // 与 RustDesk 主库一致：发送设备 UUID 用于服务器端设备身份识别
    let uuid = hbb_common::get_uuid();
    let mut msg = RendezvousMessage::new();
    msg.set_register_pk(RegisterPk {
        id: config.id.clone(),
        uuid: uuid.into(),
        pk: pk.to_vec().into(),
        ..Default::default()
    });
    socket.send(&msg, *peer_addr).await?;
    log::info!("RegisterPk sent");
    Ok(())
}

/// 回复 LocalAddr + 建立数据连接
///
/// 与 RustDesk 主库一致：建立一条 TCP 连接到信令服务器，
/// 通过 AddrMangle 编码本端 socket 地址后以 protobuf 响应，
/// 然后丢弃此 TCP 连接，在同一端口监听对端直连。
///
/// 信令服务器收到 LocalAddr 后会将本端的 LAN 地址转发给对端，
/// 对端尝试直连本端。本端 accept 对端的 TCP 连接后作为数据通道。
async fn send_local_addr_fla(
    fla: FetchLocalAddr,
    config: SosConfig,
    password_refresh_tx: tokio::sync::mpsc::UnboundedSender<()>,
) -> ResultType<()> {
    log::info!(
        "[CONN] >>> send_local_addr_fla called: relay_server='{}'",
        fla.relay_server
    );
    use hbb_common::socket_client::connect_tcp;

    // 连接到信令服务器
    let rendezvous_host = check_port(
        &config.rendezvous_server,
        crate::sos_constants::RENDEZVOUS_PORT,
    );
    let mut tcp_stream =
        connect_tcp(rendezvous_host, crate::sos_constants::CONNECT_TIMEOUT).await?;

    // 获取本端 TCP socket 地址，用 AddrMangle 编码
    let local_sock_addr = tcp_stream.local_addr();
    let local_encoded = hbb_common::AddrMangle::encode(local_sock_addr);

    // 从 FetchLocalAddr 解码对端地址，再重新编码（与 RustDesk 主库一致）
    let peer_addr = hbb_common::AddrMangle::decode(&fla.socket_addr);
    let socket_addr_encoded = hbb_common::AddrMangle::encode(peer_addr);

    // 构造 LocalAddr 响应（使用 RendezvousMessage）
    let mut msg = hbb_common::rendezvous_proto::RendezvousMessage::new();
    msg.set_local_addr(hbb_common::rendezvous_proto::LocalAddr {
        id: config.id.clone(),
        socket_addr: socket_addr_encoded.into(),
        local_addr: local_encoded.into(),
        relay_server: fla.relay_server.clone(),
        version: crate::sos_constants::VERSION.to_string(),
        socket_addr_v6: Default::default(),
        special_fields: hbb_common::protobuf::SpecialFields::new(),
    });

    // 通过 TCP 发送响应（序列化为 bytes 后用 send_raw）
    let bytes = msg.write_to_bytes()?;
    tcp_stream.send_raw(bytes).await?;
    log::info!("LocalAddr sent via TCP, local_addr={}", local_sock_addr);

    // ---- 与 RustDesk accept_connection 一致 ----
    // 信令服务器已处理 LocalAddr 并关闭连接。本端在同一端口监听，
    // 对端（远程客户端）会主动连接本端进行直连。
    drop(tcp_stream);

    let listener = hbb_common::tcp::new_listener(local_sock_addr, true).await?;
    log::info!(
        "Listening on {} for incoming intranet connection...",
        local_sock_addr
    );

    // 等待对端连接（超时 10 秒）
    let (stream, _accept_addr) = hbb_common::timeout(10_000, listener.accept())
        .await
        .map_err(|_| anyhow::anyhow!("Timeout waiting for incoming connection"))?
        .map_err(|e| anyhow::anyhow!("Accept error: {}", e))?;
    stream.set_nodelay(true).ok();
    let stream_local_addr = stream.local_addr()?;
    log::info!("Accepted intranet connection from {}", stream_local_addr);

    crate::sos_connection::handle(
        hbb_common::Stream::from(stream, stream_local_addr),
        "0.0.0.0:0".parse().unwrap(),
        config.clone(),
        Some(&password_refresh_tx),
    )
    .await
}

/// 测试 NAT 类型（异步，应仅在启动时调用一次）
///
/// 判断依据：从**同一个本地端口**向信令服务器的两个端口发起 TCP 连接，
/// 比较服务器看到的源端口是否一致。一致 = ASYMMETRIC（端口保持），
/// 不一致 = SYMMETRIC（每次映射换端口）。
///
/// 与上游 `src/common.rs::test_nat_type_` 逐项对齐，三处关键差异都曾导致误判：
///
/// 1. **第二个探测端口是主端口 - 1，不是 + 1。**
///    hbbs 在 `21115`（= 21116 - 1）上处理 `TestNatRequest`；`21117` 是 hbbr
///    中继端口，根本不应答这个请求，所以拿到的必然是 "stream closed"。
/// 2. **两次探测必须复用同一个本地端口。** 每次新建连接的本地端口天然不同，
///    比较两个不同的源端口毫无意义，结论必然是 SYMMETRIC。
/// 3. **探测失败时保持 UNKNOWN，不能写 SYMMETRIC。** 探测失败可能只是网络抖动，
///    而 [`handle_punch_hole`] 会因为「自己是 SYMMETRIC」直接放弃打洞改走中继。
///    之前这两个 bug 叠加，使 SOS 从启动起就永久放弃了 P2P。
async fn test_nat_type(rendezvous_host: &str) {
    use hbb_common::socket_client::increase_port;

    // 与上游一致：主端口、主端口 - 1（hbbs 的 NAT 检测端口）
    let servers = [
        rendezvous_host.to_string(),
        increase_port(rendezvous_host, -1),
    ];

    let mut ports = [0i32; 2];
    // 复用第一个探测的本地端口做第二次探测 —— 这是整个判断成立的前提
    let mut local_addr: Option<std::net::SocketAddr> = None;
    for (i, server) in servers.iter().enumerate() {
        match nat_probe(server.clone(), local_addr).await {
            Ok((port, la)) => {
                ports[i] = port;
                if local_addr.is_none() {
                    local_addr = Some(la);
                }
            }
            Err(e) => log::warn!("TestNat attempt[{}] failed: {}", i, e),
        }
    }

    if ports[0] > 0 && ports[1] > 0 {
        let nat_type = if ports[0] == ports[1] {
            NatType::ASYMMETRIC
        } else {
            NatType::SYMMETRIC
        };
        crate::sos_config::set_nat_type(nat_type as i32);
        log::info!(
            "NAT type test result: {:?} (ports={}:{})",
            nat_type,
            ports[0],
            ports[1]
        );
    } else {
        // 绝不在这里写 SYMMETRIC：探测失败 ≠ 对称 NAT
        log::warn!(
            "NAT type test inconclusive (ports={}:{}), keeping UNKNOWN",
            ports[0],
            ports[1]
        );
    }
}

/// 单次 NAT 探测：返回 (服务器看到的源端口, 本次连接的本地地址)
async fn nat_probe(
    server: String,
    local: Option<std::net::SocketAddr>,
) -> ResultType<(i32, std::net::SocketAddr)> {
    use hbb_common::rendezvous_proto::{rendezvous_message, TestNatRequest};
    use hbb_common::socket_client::connect_tcp_local;

    // `local` 为 Some 时复用该本地端口（NAT 类型判断的必要条件）
    let mut stream =
        connect_tcp_local(server, local, crate::sos_constants::CONNECT_TIMEOUT).await?;
    let local_addr = stream.local_addr();

    let mut msg = RendezvousMessage::new();
    msg.set_test_nat_request(TestNatRequest {
        serial: 1,
        ..Default::default()
    });
    stream.send(&msg).await?;

    let bytes = match hbb_common::timeout(5_000, stream.next()).await {
        Ok(Some(Ok(b))) => b,
        Ok(Some(Err(e))) => Err(anyhow::anyhow!("recv: {}", e))?,
        Ok(None) => Err(anyhow::anyhow!("stream closed"))?,
        Err(_) => Err(anyhow::anyhow!("timeout"))?,
    };

    if let Ok(rm) = RendezvousMessage::parse_from_bytes(&bytes) {
        if let Some(rendezvous_message::Union::TestNatResponse(tnr)) = rm.union {
            log::info!("TestNat server saw port: {}", tnr.port);
            return Ok((tnr.port, local_addr));
        }
    }
    Err(anyhow::anyhow!("unexpected response to TestNatRequest"))
}

// ── PunchHole 去重 ──
//
// hbbs 可能重复下发同一个 PunchHole（例如客户端重发 PunchHoleRequest）。
// 不去重会为同一次请求开多个监听 socket，互相抢端口。
// 与上游 `LAST_MSG` 一致：同一对端 100ms 内的重复消息直接忽略。
static LAST_PUNCH: std::sync::OnceLock<std::sync::Mutex<Option<(std::net::SocketAddr, Instant)>>> =
    std::sync::OnceLock::new();

/// 若判定为重复的 PunchHole 则返回 `true`
fn is_duplicate_punch(addr: std::net::SocketAddr) -> bool {
    let m = LAST_PUNCH.get_or_init(|| std::sync::Mutex::new(None));
    let Ok(mut g) = m.lock() else { return false };
    if let Some((last_addr, at)) = *g {
        if last_addr == addr && at.elapsed().as_millis() < 100 {
            return true;
        }
    }
    *g = Some((addr, Instant::now()));
    false
}

/// 处理 PunchHole（打洞请求）
///
/// 与上游 `rendezvous_mediator.rs::handle_punch_hole` 的 TCP 分支对齐。
///
/// **方向很关键**：收到 `PunchHole` 的是受控端，它要**监听**，
/// 由控制端发起连接。之前 SOS 在这里反过来去 `connect` 对端，
/// 而对端此时并没有在监听，所以必然失败，只能回退中继。
///
/// 完整流程（与上游逐步对应）：
/// 1. 连一条 TCP 到信令服务器，取得本地端口 `L`（顺带让网关建立映射）；
/// 2. 用**同一个本地端口 `L`** 去连一次对端，只等 30ms 且忽略结果 ——
///    目的是让本地网关把对端标记为「可信」，从而放行它稍后的入站连接；
/// 3. 在这条信令连接上发 `PunchHoleSent`，告诉服务器「我准备好了」；
/// 4. 丢掉该连接，用 `SO_REUSEADDR` 重新在 `L` 上监听，等待控制端连进来。
async fn handle_punch_hole(
    ph: PunchHole,
    config: SosConfig,
    password_refresh_tx: tokio::sync::mpsc::UnboundedSender<()>,
) -> ResultType<()> {
    // `socket_addr` 是 AddrMangle 编码的**二进制**，不是 UTF-8 字符串。
    // 之前用 `String::from_utf8_lossy` 解出来是一堆乱码，必然连不上。
    let peer_addr = hbb_common::AddrMangle::decode(&ph.socket_addr);
    let peer_addr_v6 = hbb_common::AddrMangle::decode(&ph.socket_addr_v6);
    log::info!(
        "[CONN] >>> handle_punch_hole: peer={} peer_v6={} relay='{}' force_relay={}",
        peer_addr,
        peer_addr_v6,
        ph.relay_server,
        ph.force_relay
    );

    if is_duplicate_punch(peer_addr) {
        log::debug!("[CONN] duplicate PunchHole for {}, ignored", peer_addr);
        return Ok(());
    }

    // 中继判定：与上游一致，仅在「明确已知对称 NAT」或对端要求强制中继时放弃
    use hbb_common::protobuf::Enum;
    let remote_nat = ph.nat_type.enum_value().unwrap_or(NatType::UNKNOWN_NAT);
    let our_nat =
        NatType::from_i32(crate::sos_config::get_nat_type()).unwrap_or(NatType::UNKNOWN_NAT);
    log::info!(
        "[CONN] PunchHole NAT: remote={:?} us={:?}",
        remote_nat,
        our_nat
    );

    let must_relay = ph.force_relay
        || remote_nat == NatType::SYMMETRIC
        || our_nat == NatType::SYMMETRIC
        || peer_addr.port() == 0;
    if must_relay {
        log::info!(
            "[CONN] punch hole not applicable (force_relay={}, remote={:?}, us={:?}, port={}), using relay",
            ph.force_relay,
            remote_nat,
            our_nat,
            peer_addr.port()
        );
        return handle_relay_fallback(
            &ph.relay_server,
            &config,
            &password_refresh_tx,
            ph.socket_addr.clone(),
            None,
            "",
        )
        .await;
    }

    // 主路径：IPv4 TCP 打洞
    match punch_tcp_and_serve(peer_addr, &ph, &config, &password_refresh_tx).await {
        Ok(()) => return Ok(()),
        Err(e) => log::warn!("[CONN] IPv4 TCP punch failed ({}), trying IPv6", e),
    }

    // 次路径：IPv6（对端有 v6 地址时才尝试）
    if peer_addr_v6.port() > 0 {
        match punch_tcp_and_serve(peer_addr_v6, &ph, &config, &password_refresh_tx).await {
            Ok(()) => return Ok(()),
            Err(e) => log::warn!("[CONN] IPv6 TCP punch failed ({}), using relay", e),
        }
    }

    handle_relay_fallback(
        &ph.relay_server,
        &config,
        &password_refresh_tx,
        ph.socket_addr.clone(),
        None,
        "",
    )
    .await
}

/// TCP 打洞并在打出的端口上监听，成功后把连接交给 `sos_connection::handle`
///
/// 见 [`handle_punch_hole`] 的流程说明。`peer_addr` 是控制端的公网地址。
async fn punch_tcp_and_serve(
    peer_addr: std::net::SocketAddr,
    ph: &PunchHole,
    config: &SosConfig,
    password_refresh_tx: &tokio::sync::mpsc::UnboundedSender<()>,
) -> ResultType<()> {
    use hbb_common::protobuf::Enum;
    use hbb_common::rendezvous_proto::PunchHoleSent;

    let rendezvous_host = check_port(
        &config.rendezvous_server,
        crate::sos_constants::RENDEZVOUS_PORT,
    );

    // 1. 连信令服务器，占住一个本地端口 L
    let mut socket = connect_tcp(rendezvous_host, crate::sos_constants::CONNECT_TIMEOUT).await?;
    let raw_local = socket.local_addr();
    // 代理环境下可能拿到 ::1 这类地址，改写成纯 ip:port 形式（与上游一致）
    let local_addr: std::net::SocketAddr =
        format!("{}:{}", raw_local.ip(), raw_local.port()).parse()?;
    log::info!("[CONN] punch hole local port {}", local_addr);

    // 2. 用同一个本地端口去捅对端，只等 30ms 且忽略结果（与上游一致）。
    //    这一步是打洞的**实质**：让本地网关为 (L → peer) 建立映射并放行入站。
    if let Err(e) =
        hbb_common::socket_client::connect_tcp_local(peer_addr, Some(local_addr), 30).await
    {
        log::debug!("[CONN] punch probe to {} finished: {}", peer_addr, e);
    }

    // 3. 通知信令服务器「已就绪」，它据此让控制端发起连接
    let nat_type =
        NatType::from_i32(crate::sos_config::get_nat_type()).unwrap_or(NatType::UNKNOWN_NAT);
    let mut msg = RendezvousMessage::new();
    msg.set_punch_hole_sent(PunchHoleSent {
        socket_addr: ph.socket_addr.clone(),
        id: config.id.clone(),
        relay_server: ph.relay_server.clone(),
        nat_type: nat_type.into(),
        version: crate::sos_constants::VERSION.to_string(),
        socket_addr_v6: ph.socket_addr_v6.clone(),
        ..Default::default()
    });
    socket.send_raw(msg.write_to_bytes()?).await?;
    log::info!("[CONN] PunchHoleSent sent for {}", peer_addr);

    // 4. 释放端口后立即用 SO_REUSEADDR 重新绑定，等待控制端连入
    drop(socket);
    let listener = hbb_common::tcp::new_listener(local_addr, true).await?;
    log::info!("[CONN] listening on {} for punched connection", local_addr);

    let (stream, from) = hbb_common::timeout(PUNCH_ACCEPT_TIMEOUT, listener.accept())
        .await
        .map_err(|_| anyhow::anyhow!("timeout waiting for punched connection"))?
        .map_err(|e| anyhow::anyhow!("accept: {}", e))?;
    stream.set_nodelay(true).ok();
    let stream_addr = stream.local_addr()?;
    log::info!(
        "[CONN] punched connection accepted from {} (local {})",
        from,
        stream_addr
    );

    crate::sos_connection::handle(
        hbb_common::Stream::from(stream, stream_addr),
        "0.0.0.0:0".parse().unwrap(),
        config.clone(),
        Some(password_refresh_tx),
    )
    .await
}

/// 处理 RequestRelay（中继请求）
///
/// 对端要求通过中继服务器建立连接。
/// 关键：必须复用客户端的 uuid，否则中继服务器无法配对双方连接。
async fn handle_request_relay(
    rr: RequestRelay,
    config: SosConfig,
    password_refresh_tx: tokio::sync::mpsc::UnboundedSender<()>,
) -> ResultType<()> {
    log::info!(
        "[CONN] >>> handle_request_relay called: relay='{}' uuid.len={} id='{}'",
        rr.relay_server,
        rr.uuid.len(),
        rr.id
    );
    let relay_host = &rr.relay_server;
    if relay_host.is_empty() {
        return Err(anyhow::anyhow!("Relay server address is empty"));
    }
    // 使用客户端的 uuid（客户端已用它连接中继服务器，必须一致）
    let uuid = if rr.uuid.is_empty() {
        None
    } else {
        Some(rr.uuid.clone())
    };
    let peer_id = if rr.id.is_empty() {
        String::new()
    } else {
        rr.id.clone()
    };
    handle_relay_fallback(
        relay_host,
        &config,
        &password_refresh_tx,
        rr.socket_addr.clone(),
        uuid,
        &peer_id,
    )
    .await
}

/// 通过中继服务器建立连接
///
/// 与 RustDesk 主库一致：
/// 1. TCP 连接到信令服务器，发送 RelayResponse（含 uuid、对端地址、中继地址）
/// 2. 信令服务器收到后关闭连接
/// 3. TCP 连接中继服务器，发送 RequestRelay（含 uuid）
/// 4. 中继服务器桥接本端与对端，随后进行密钥交换
///
/// `peer_socket_addr` 是对端（远程客户端）的地址，来自 PunchHole 或 RequestRelay
/// `uuid` 为客户端生成的共享 uuid（来自 RequestRelay），若为 None 则随机生成（PunchHole 回退场景）
/// `peer_id` 为对端设备 ID（来自 RequestRelay），PunchHole 场景下为空字符串
async fn handle_relay_fallback(
    relay_host: &str,
    config: &SosConfig,
    password_refresh_tx: &tokio::sync::mpsc::UnboundedSender<()>,
    peer_socket_addr: hbb_common::bytes::Bytes,
    uuid: Option<String>,
    peer_id: &str,
) -> ResultType<()> {
    log::info!(
        "[CONN] >>> handle_relay_fallback: relay='{}' uuid={:?} peer_id='{}'",
        relay_host,
        uuid,
        peer_id
    );

    // 候选中继：--relay > 服务器下发 > 信令服务器+1。
    //
    // 服务器下发的 `relay_host` **恒为单个主机名**：hbbs 已经在
    // `relay_servers: Vec<String>` 里替我们选好一台（OSS 轮询，Pro 按地理最近），
    // 协议字段本身不承载列表。所以多候选只可能来自我们自己的 `--relay`。
    let candidates = resolve_relay_candidates(config, relay_host);
    if candidates.is_empty() {
        return Err(anyhow::anyhow!("no relay server available"));
    }
    log::info!("[CONN] relay candidates: {:?}", candidates);

    // 1. 先连上中继，拿到**实际可用**的那一台，再告诉信令服务器。
    //
    // 顺序很关键。`RelayResponse.relay_server` 不是给我们自己看的：
    // hbbs 收到后会原样**转发给对端**
    // （`rendezvous_server.rs`：`send_to_tcp_sync(msg_out, addr_b)`），
    // 对端据此决定连哪台中继。所以这个字段必须是真正建立连接的那一台，
    // 否则对端连到别的机器、uuid 永远配不上，会话就卡死。
    //
    // 只有一台候选时（hbbs 下发的正常情况下恒为 1 台），先连后报的额外开销
    // 就是一次 TCP 握手，可以忽略；多候选时的额外等待由
    // [`connect_any_relay`] 的预算封顶。
    let (mut relay_stream, chosen_relay) = connect_any_relay(&candidates).await?;
    log::info!(
        "[CONN] Relay TCP connection established via {}",
        chosen_relay
    );

    // 2. 向信令服务器发送 RelayResponse（含 uuid、对端地址、**实际选中的**中继）
    let rendezvous_host = check_port(
        &config.rendezvous_server,
        crate::sos_constants::RENDEZVOUS_PORT,
    );

    // uuid：优先使用客户端提供的（RequestRelay 场景），否则随机生成（PunchHole 回退场景）
    let uuid_str = uuid.unwrap_or_else(|| {
        use hbb_common::rand::Rng;
        hbb_common::rand::thread_rng()
            .sample_iter(&hbb_common::rand::distributions::Alphanumeric)
            .take(16)
            .map(char::from)
            .collect()
    });

    {
        log::info!(
            "[CONN] Step1: connecting to rendezvous server for RelayResponse: {}",
            rendezvous_host
        );
        let mut stream =
            connect_tcp(rendezvous_host, crate::sos_constants::CONNECT_TIMEOUT).await?;

        // 构造 RelayResponse（与 RustDesk create_relay 一致）
        let mut rr = hbb_common::rendezvous_proto::RelayResponse::new();
        rr.uuid = uuid_str.clone();
        rr.socket_addr = peer_socket_addr;
        rr.relay_server = chosen_relay.clone();
        rr.set_id(config.id.clone());
        rr.version = crate::sos_constants::VERSION.to_string();
        let mut msg = hbb_common::rendezvous_proto::RendezvousMessage::new();
        msg.set_relay_response(rr);

        let bytes = msg.write_to_bytes()?;
        stream.send_raw(bytes).await?;
        log::info!(
            "[CONN] RelayResponse sent to rendezvous server (uuid={}, relay={})",
            uuid_str,
            chosen_relay
        );
    } // 信令服务器关闭连接，stream 自动断开

    // 3. 在中继连接上发 RequestRelay（与 RustDesk create_relay_connection 一致）
    //
    // licence_key：优先使用配置中的 server_pub_key，空则回退 RS_PUB_KEY
    let licence_key = if !config.server_pub_key.is_empty() {
        config.server_pub_key.clone()
    } else {
        hbb_common::config::RS_PUB_KEY.to_string()
    };
    let mut msg = hbb_common::rendezvous_proto::RendezvousMessage::new();
    let mut rr = hbb_common::rendezvous_proto::RequestRelay::new();
    rr.uuid = uuid_str.clone();
    rr.licence_key = licence_key;
    rr.id = peer_id.to_string();
    rr.conn_type = hbb_common::rendezvous_proto::ConnType::DEFAULT_CONN.into();
    msg.set_request_relay(rr);

    let bytes = msg.write_to_bytes()?;
    relay_stream.send_raw(bytes).await?;
    log::info!(
        "[CONN] RequestRelay sent to relay server (uuid={}, peer_id={}), waiting for bridge...",
        uuid_str,
        peer_id
    );

    log::info!("[CONN] Step3: calling sos_connection::handle() via relay stream");
    let ret = crate::sos_connection::handle(
        relay_stream,
        "0.0.0.0:0".parse().unwrap(),
        config.clone(),
        Some(&password_refresh_tx),
    )
    .await;
    log::info!(
        "[CONN] sos_connection::handle() returned: {:?}",
        ret.as_ref().map(|_| "OK").unwrap_or("ERR")
    );
    ret
}

/// 逐个尝试候选中继，返回首个连通的 `(流, 所用的中继地址)`。
///
/// 每一项都会补上默认中继端口（`RELAY_PORT`，除非地址里已带端口）。
/// 全部失败时返回错误，并把每项的失败原因汇总进消息里 ——
/// 只回一句「失败」的话，多地址场景下根本没法判断是哪一个挂了。
///
/// # 为什么必须有总时长预算
///
/// 这个函数的返回值要拿去填 `RelayResponse.relay_server`，而 hbbs 会把这条
/// 消息**转发给对端**（`rendezvous_server.rs` 里 `send_to_tcp_sync(msg_out, addr_b)`），
/// 对端据此决定连哪台中继。所以必须**先连通、再上报**，否则报出去的地址
/// 与实际使用的不是同一台，对端的 RequestRelay 就永远配不上对。
///
/// 但"先连通"意味着对端要多等这段时间。为了让最坏情况不比上游差，
/// 整个候选循环共用**一个** `CONNECT_TIMEOUT` 的预算：
/// 每次尝试取「剩余预算」与 `CONNECT_TIMEOUT` 的较小值。
/// 于是总耗时不劣于上游的单次尝试，而快速失败（连接被拒、DNS 解析失败）
/// 时又能继续试下一台。
async fn connect_any_relay(candidates: &[String]) -> ResultType<(hbb_common::Stream, String)> {
    let budget = std::time::Duration::from_millis(crate::sos_constants::CONNECT_TIMEOUT);
    let started = Instant::now();
    let mut errors: Vec<String> = Vec::new();

    for host in candidates {
        let remaining = budget.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            errors.push("budget exhausted before trying this candidate".to_string());
            break;
        }
        let host_port = hbb_common::socket_client::check_port(host, hbb_common::config::RELAY_PORT);
        log::info!("[CONN] Step2: connecting to relay server: {}", host_port);
        let attempt_ms = remaining.as_millis() as u64;
        match connect_tcp(host_port.clone(), attempt_ms).await {
            Ok(stream) => return Ok((stream, host.clone())),
            Err(e) => {
                log::warn!("[CONN] relay {} unreachable: {}", host_port, e);
                errors.push(format!("{}: {}", host_port, e));
            }
        }
    }
    Err(anyhow::anyhow!(
        "all {} relay candidate(s) unreachable: {}",
        candidates.len(),
        errors.join("; ")
    ))
}
