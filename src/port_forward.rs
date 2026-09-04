use std::sync::{Arc, RwLock};

use crate::client::*;
use crate::port_forward_mux::{Claim, Tunnel, CHANNEL_WINDOW};
use hbb_common::{
    allow_err, bail,
    config::READ_TIMEOUT,
    futures::{SinkExt, StreamExt},
    log,
    message_proto::*,
    protobuf::Message as _,
    rendezvous_proto::ConnType,
    tcp, timeout,
    tokio::{self, net::TcpStream, sync::mpsc},
    tokio_util::codec::{BytesCodec, Framed},
    ResultType, Stream,
};

fn run_rdp(port: u16, name: &str) {
    std::process::Command::new("cmdkey")
        .arg("/delete:localhost")
        .output()
        .ok();
    let username = std::env::var("rdp_username").unwrap_or_default();
    let password = std::env::var("rdp_password").unwrap_or_default();
    if !username.is_empty() || !password.is_empty() {
        let mut args = vec!["/generic:localhost".to_owned()];
        if !username.is_empty() {
            args.push(format!("/user:{}", username));
        }
        if !password.is_empty() {
            args.push(format!("/pass:{}", password));
        }
        std::process::Command::new("cmdkey")
            .args(&args)
            .output()
            .ok();
    }
    // Keep using /v instead of a generated .rdp file: mstsc then preserves the
    // user's Default.rdp settings and avoids unsigned-file warnings or policies.
    match std::process::Command::new("mstsc")
        .arg(format!("/v:localhost:{}", port))
        .spawn()
    {
        Ok(child) => {
            #[cfg(windows)]
            crate::platform::set_rdp_window_title(child, name.to_owned());
            #[cfg(not(windows))]
            let _ = (child, name);
        }
        Err(err) => log::warn!("Failed to launch mstsc: {}", err),
    }
}

// Show the peer identity with its hostname, using the ID when no alias exists.
fn rdp_display_name(lc: &Arc<RwLock<LoginConfigHandler>>, id: &str) -> String {
    let lc = lc.read().unwrap();
    let alias = lc
        .options
        .get("alias")
        .map(|s| s.trim())
        .unwrap_or_default();
    let hostname = lc.info.hostname.trim();
    let identity = if !alias.is_empty() { alias } else { id };
    if hostname.is_empty() || hostname == identity {
        identity.to_owned()
    } else {
        format!("{} ({})", identity, hostname)
    }
}

pub async fn listen(
    id: String,
    password: String,
    port: i32,
    interface: impl Interface,
    ui_receiver: mpsc::UnboundedReceiver<Data>,
    key: &str,
    token: &str,
    lc: Arc<RwLock<LoginConfigHandler>>,
    remote_host: String,
    remote_port: i32,
) -> ResultType<()> {
    let listener = tcp::new_listener(format!("127.0.0.1:{}", port), true).await?;
    // One tunnel per mapping: every accept here goes to the one target the
    // peer authenticated, and dropping it at the end closes that tunnel.
    let tunnel = Tunnel::new();
    let addr = listener.local_addr()?;
    log::info!("listening on port {:?}", addr);
    let is_rdp = port == 0;
    if is_rdp {
        run_rdp(addr.port(), &rdp_display_name(&lc, &id));
    }
    let mut ui_receiver = ui_receiver;
    loop {
        tokio::select! {
            // `addr` above the loop is the listener's own address; `run_rdp` needs
            // that port. The accepted peer's address gets its own name so it can
            // never shadow it.
            Ok((forward, peer_addr)) = listener.accept() => {
                log::debug!("new connection from {:?}", peer_addr);
                match tunnel.claim() {
                    Claim::Muxed(handle) => {
                        if let Err(e) = handle.open(&remote_host, remote_port, forward, Vec::new()) {
                            log::debug!("cannot open channel for {:?}: {}", peer_addr, e);
                        }
                    }
                    // The claiming accept negotiates: it asks for the tunnel, and
                    // the peer's answer fixes this listener's mode until it closes.
                    Claim::Claimed => {
                        {
                            let mut lc = lc.write().unwrap();
                            lc.port_forward = (remote_host.clone(), remote_port);
                            lc.port_forward_mux = crate::common::get_port_forward_mux_enabled();
                        }
                        let mut forward = Framed::new(forward, BytesCodec::new());
                        let mut close_port_forward = false;
                        match connect_and_login(&id, &password, &mut ui_receiver, interface.clone(), &mut forward, key, token, is_rdp, &mut close_port_forward).await {
                            Ok(Some(outcome)) if outcome.mux => {
                                let handle = tunnel.set_muxed(outcome.stream, interface.clone());
                                if !outcome.local_eof {
                                    let (socket, prebuf) = take_socket(forward, outcome.prebuf);
                                    if let Err(e) = handle.open(&remote_host, remote_port, socket, prebuf) {
                                        log::debug!("cannot open channel for {:?}: {}", peer_addr, e);
                                    }
                                }
                            }
                            Ok(Some(outcome)) => {
                                tunnel.set_legacy();
                                if outcome.local_eof {
                                    log::debug!("legacy peer and local {:?} already gone", peer_addr);
                                } else {
                                    run_legacy(outcome, forward, peer_addr, interface.clone());
                                }
                            }
                            _ if close_port_forward => {
                                tunnel.set_failed();
                                break;
                            }
                            Err(err) => {
                                tunnel.set_failed();
                                interface.on_establish_connection_error(err.to_string());
                            }
                            _ => tunnel.set_failed(),
                        }
                    }
                    // A `Legacy` listener stays legacy until it closes: every accept
                    // logs in on its own, asks for no tunnel, and takes the raw pipe
                    // whatever the peer reports. Re-adding the mapping, or reopening
                    // the window, is how a user picks up an upgraded peer; nothing
                    // switches modes underneath live connections.
                    Claim::Legacy => {
                        {
                            let mut lc = lc.write().unwrap();
                            lc.port_forward = (remote_host.clone(), remote_port);
                            lc.port_forward_mux = false;
                        }
                        let mut forward = Framed::new(forward, BytesCodec::new());
                        let mut close_port_forward = false;
                        match connect_and_login(&id, &password, &mut ui_receiver, interface.clone(), &mut forward, key, token, is_rdp, &mut close_port_forward).await {
                            Ok(Some(outcome)) if outcome.local_eof => {
                                log::debug!("legacy peer and local {:?} already gone", peer_addr);
                            }
                            Ok(Some(outcome)) => run_legacy(outcome, forward, peer_addr, interface.clone()),
                            _ if close_port_forward => break,
                            Err(err) => interface.on_establish_connection_error(err.to_string()),
                            _ => {}
                        }
                    }
                }
            }
            d = ui_receiver.recv() => if on_ui_command(d, addr.port(), &lc, &id) {
                break;
            },
        }
    }
    Ok(())
}

/// Commands the window sends its listener. `true` means stop listening: the
/// window is closing, or its sender is gone.
fn on_ui_command(d: Option<Data>, port: u16, lc: &Arc<RwLock<LoginConfigHandler>>, id: &str) -> bool {
    match d {
        Some(Data::Close) | None => true,
        Some(Data::NewRDP) => {
            run_rdp(port, &rdp_display_name(lc, id));
            false
        }
        _ => false,
    }
}

/// Today's raw pipe, for peers without multiplexing.
fn run_legacy(
    outcome: LoginOutcome,
    forward: Framed<TcpStream, BytesCodec>,
    addr: std::net::SocketAddr,
    interface: impl Interface,
) {
    let mut stream = outcome.stream;
    let prebuf = outcome.prebuf;
    tokio::spawn(async move {
        stream.set_raw();
        if !prebuf.is_empty() {
            allow_err!(stream.send_bytes(prebuf.into()).await);
        }
        if let Err(err) = run_forward(forward, stream).await {
            interface.msgbox("error", "Error", &err.to_string(), "");
        }
        log::info!("connection from {:?} closed", addr);
    });
}

pub(crate) struct LoginOutcome {
    stream: Stream,
    mux: bool,
    prebuf: Vec<u8>,
    local_eof: bool,
}

fn peer_supports_mux(pi: &PeerInfo) -> bool {
    pi.features.as_ref().map(|f| f.port_forward_mux).unwrap_or(false)
}

/// `into_inner()` would drop bytes the codec pulled but never yielded.
fn take_socket(forward: Framed<TcpStream, BytesCodec>, mut prebuf: Vec<u8>) -> (TcpStream, Vec<u8>) {
    let parts = forward.into_parts();
    prebuf.extend_from_slice(&parts.read_buf);
    (parts.io, prebuf)
}

async fn connect_and_login(
    id: &str,
    password: &str,
    ui_receiver: &mut mpsc::UnboundedReceiver<Data>,
    interface: impl Interface,
    forward: &mut Framed<TcpStream, BytesCodec>,
    key: &str,
    token: &str,
    is_rdp: bool,
    close_port_forward: &mut bool,
) -> ResultType<Option<LoginOutcome>> {
    let conn_type = if is_rdp {
        ConnType::RDP
    } else {
        ConnType::PORT_FORWARD
    };
    let ((mut stream, direct, _pk, _kcp, _stream_type), (feedback, rendezvous_server)) =
        Client::start(id, key, token, conn_type, interface.clone()).await?;
    interface.update_direct(Some(direct));
    if !stream.is_secured() && !crate::common::is_direct_ip_access(id) {
        if !confirm_insecure_connection(&interface, ui_receiver).await {
            *close_port_forward = true;
            return Ok(None);
        }
    }
    let mut buffer = Vec::new();
    let mut local_eof = false;
    let mux;
    let mut received = false;
    let mut challenge = None;
    let mut pending_login = None;

    let _keep_it = hc_connection(feedback, rendezvous_server, token).await;

    loop {
        tokio::select! {
            res = timeout(READ_TIMEOUT, stream.next()) => match res {
                Err(_) => {
                    bail!("Timeout");
                }
                Ok(Some(Ok(bytes))) => {
                    if !received {
                        received = true;
                        interface.update_received(true);
                    }
                    let msg_in = Message::parse_from_bytes(&bytes)?;
                    match msg_in.union {
                        Some(message::Union::Hash(hash)) => {
                            challenge = Some(hash.clone());
                            if !hash_arrived(&interface, password, hash, pending_login.take(), remote_host, remote_port, &mut stream).await {
                                return Ok(None);
                            }
                        }
                        Some(message::Union::LoginResponse(lr)) => match lr.union {
                            Some(login_response::Union::Error(err)) => {
                                if !interface.handle_login_error(&err) {
                                    return Ok(None);
                                }
                            }
                            Some(login_response::Union::PeerInfo(pi)) => {
                                mux = peer_supports_mux(&pi);
                                interface.handle_peer_info(pi);
                                break;
                            }
                            _ => {}
                        }
                        Some(message::Union::TestDelay(t)) => {
                            interface.handle_test_delay(t, &mut stream).await;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(err))) => {
                    bail!("Connection closed: {}", err);
                }
                _ => {
                    bail!("Reset by the peer");
                }
            },
            d = ui_receiver.recv() => {
                match d {
                    Some(Data::Login(login)) => match &challenge {
                        Some(hash) => login_from_ui(&interface, hash, login, remote_host, remote_port, &mut stream).await,
                        None => pending_login = Some(login),
                    },
                    Some(Data::Message(msg)) => {
                        allow_err!(stream.send(&msg).await);
                    }
                    _ => {}
                }
            },
            // Stop pulling once the pre-read buffer is a window deep; the
            // rest waits in the kernel until the channel opens. A local EOF
            // no longer aborts the login: the tunnel may still be wanted.
            res = forward.next(), if !local_eof && buffer.len() < CHANNEL_WINDOW as usize => {
                if let Some(Ok(bytes)) = res {
                    buffer.extend(bytes);
                } else {
                    local_eof = true;
                }
            },
        }
    }
    Ok(Some(LoginOutcome {
        stream,
        mux,
        prebuf: buffer,
        local_eof,
    }))
}


/// A mapping's login is built from the window's shared handler:
/// `create_login_msg` reads `port_forward` and `handle_login_from_ui` reads
/// `hash`. Mappings log in concurrently, so each fills them and sends under
/// the window's turn lock, or one login carried another mapping's target or
/// answered another's challenge.
async fn login_with_hash(
    interface: &impl Interface,
    password: &str,
    hash: Hash,
    remote_host: &str,
    remote_port: i32,
    stream: &mut Stream,
) -> bool {
    let lc = interface.get_lch();
    let turn = lc.read().unwrap().port_forward_login_turn.clone();
    let _turn = turn.lock().await;
    lc.write().unwrap().port_forward = (remote_host.to_owned(), remote_port);
    interface.handle_hash(password, hash, stream).await
}

type UiLogin = (String, String, String, bool);

/// This connection's `Hash`. The window's password prompt is broadcast to
/// every mapping and can reach this one first, so a password typed while
/// the `Hash` was on its way is kept and answers it now, rather than being
/// dropped in the hope that the mapping which prompted has already stored
/// it in the shared handler.
async fn hash_arrived(
    interface: &impl Interface,
    password: &str,
    hash: Hash,
    pending_login: Option<UiLogin>,
    remote_host: &str,
    remote_port: i32,
    stream: &mut Stream,
) -> bool {
    match pending_login {
        Some(login) => {
            login_from_ui(interface, &hash, login, remote_host, remote_port, stream).await;
            true
        }
        None => login_with_hash(interface, password, hash, remote_host, remote_port, stream).await,
    }
}

/// The window's password prompt is broadcast to every mapping; this one
/// answers it with its own challenge.
async fn login_from_ui(
    interface: &impl Interface,
    hash: &Hash,
    login: UiLogin,
    remote_host: &str,
    remote_port: i32,
    stream: &mut Stream,
) {
    let lc = interface.get_lch();
    let turn = lc.read().unwrap().port_forward_login_turn.clone();
    let _turn = turn.lock().await;
    {
        let mut lc = lc.write().unwrap();
        lc.port_forward = (remote_host.to_owned(), remote_port);
        lc.set_hash(hash.clone());
    }
    let (os_username, os_password, password, remember) = login;
    interface
        .handle_login_from_ui(os_username, os_password, password, remember, stream)
        .await;
}

async fn run_forward(forward: Framed<TcpStream, BytesCodec>, stream: Stream) -> ResultType<()> {
    log::info!("new port forwarding connection started");
    let mut forward = forward;
    let mut stream = stream;
    loop {
        tokio::select! {
            res = forward.next() => {
                if let Some(Ok(bytes)) = res {
                    allow_err!(stream.send_bytes(bytes.into()).await);
                } else {
                    break;
                }
            },
            res = stream.next() => {
                if let Some(Ok(bytes)) = res {
                    allow_err!(forward.send(bytes).await);
                } else {
                    break;
                }
            },
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_supports_mux_reads_the_features_bit() {
        let mut pi = PeerInfo::new();
        assert!(!peer_supports_mux(&pi));
        pi.features = Some(Features { port_forward_mux: false, ..Default::default() }).into();
        assert!(!peer_supports_mux(&pi));
        pi.features = Some(Features { port_forward_mux: true, ..Default::default() }).into();
        assert!(peer_supports_mux(&pi));
    }

    #[test]
    fn port_forward_mux_defaults_to_on() {
        use hbb_common::config::{keys, option2bool};
        // option2bool's fallback branch is also "on unless N", so the value
        // assertions below would pass for a prefixless key too. The `enable-`
        // prefix is what actually guarantees the default, and renaming the key
        // to an `allow-` one would silently flip it — pin the prefix itself.
        assert!(keys::OPTION_ENABLE_PORT_FORWARD_MUX.starts_with("enable-"));
        assert!(option2bool(keys::OPTION_ENABLE_PORT_FORWARD_MUX, ""));
        assert!(option2bool(keys::OPTION_ENABLE_PORT_FORWARD_MUX, "Y"));
        assert!(!option2bool(keys::OPTION_ENABLE_PORT_FORWARD_MUX, "N"));
    }

    #[test]
    fn take_socket_hands_back_a_working_socket_and_the_prebuf() {
        use hbb_common::tokio::io::{AsyncReadExt, AsyncWriteExt};
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = l.local_addr().unwrap();
            let mut client = TcpStream::connect(addr).await.unwrap();
            let (server, _) = l.accept().await.unwrap();
            let mut framed = Framed::new(server, BytesCodec::new());
            client.write_all(b"abc").await.unwrap();
            // Read through the codec, as connect_and_login does during login.
            let pulled = framed.next().await.unwrap().unwrap();
            assert_eq!(&pulled[..], b"abc");
            let (mut sock, prebuf) = take_socket(framed, pulled.to_vec());
            assert_eq!(prebuf, b"abc".to_vec());
            // Bytes written after the handoff arrive on the bare socket.
            client.write_all(b"def").await.unwrap();
            let mut buf = [0u8; 3];
            sock.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"def");
        });
    }
}
