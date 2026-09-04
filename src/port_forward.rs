use std::sync::{Arc, RwLock};

use crate::client::*;
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
    let addr = listener.local_addr()?;
    log::info!("listening on port {:?}", addr);
    let is_rdp = port == 0;
    if is_rdp {
        run_rdp(addr.port(), &rdp_display_name(&lc, &id));
    }
    let mut ui_receiver = ui_receiver;
    loop {
        tokio::select! {
            Ok((forward, addr)) = listener.accept() => {
                log::info!("new connection from {:?}", addr);
                let id = id.clone();
                let password = password.clone();
                let mut forward = Framed::new(forward, BytesCodec::new());
                let mut close_port_forward = false;
                match connect_and_login(&id, &password, &mut ui_receiver, interface.clone(), &mut forward, key, token, is_rdp, &mut close_port_forward, &remote_host, remote_port).await {
                    Ok(Some(stream)) => {
                        let interface = interface.clone();
                        tokio::spawn(async move {
                            if let Err(err) = run_forward(forward, stream).await {
                                interface.msgbox("error", "Error", &err.to_string(), "");
                            }
                            log::info!("connection from {:?} closed", addr);
                       });
                    }
                    _ if close_port_forward => {
                        break;
                    }
                    Err(err) => {
                        interface.on_establish_connection_error(err.to_string());
                    }
                    _ => {}
                }
            }
            d = ui_receiver.recv() => {
                match d {
                    Some(Data::Close) => {
                        break;
                    }
                    Some(Data::NewRDP) => {
                        println!("receive run_rdp from ui_receiver");
                        run_rdp(addr.port(), &rdp_display_name(&lc, &id));
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
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
    remote_host: &str,
    remote_port: i32,
) -> ResultType<Option<Stream>> {
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
            res = forward.next() => {
                if let Some(Ok(bytes)) = res {
                    buffer.extend(bytes);
                } else {
                    return Ok(None);
                }
            },
        }
    }
    stream.set_raw();
    if !buffer.is_empty() {
        allow_err!(stream.send_bytes(buffer.into()).await);
    }
    Ok(Some(stream))
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
mod login_tests {
    use super::*;
    use async_trait::async_trait;
    use hbb_common::{
        tcp::FramedStream,
        tokio::time::{sleep, Duration},
    };
    use sha2::{Digest, Sha256};

    /// A window's interface over its shared handler. `handle_hash` can pause
    /// before building the login, where the real one looks passwords up.
    #[derive(Clone)]
    struct Ui {
        lc: Arc<RwLock<LoginConfigHandler>>,
        pause: Duration,
    }

    #[async_trait]
    impl Interface for Ui {
        fn send(&self, _data: Data) {}
        fn msgbox(&self, _msgtype: &str, _title: &str, _text: &str, _link: &str) {}
        fn handle_login_error(&self, _err: &str) -> bool {
            false
        }
        fn handle_peer_info(&self, _pi: PeerInfo) {}
        fn set_multiple_windows_session(&self, _sessions: Vec<WindowsSession>) {}
        async fn handle_hash(&self, pass: &str, hash: Hash, peer: &mut Stream) -> bool {
            sleep(self.pause).await;
            crate::client::handle_hash(self.lc.clone(), pass, hash, self, peer).await
        }
        async fn handle_login_from_ui(
            &self,
            os_username: String,
            os_password: String,
            password: String,
            remember: bool,
            peer: &mut Stream,
        ) {
            crate::client::handle_login_from_ui(
                self.lc.clone(),
                os_username,
                os_password,
                password,
                remember,
                peer,
            )
            .await
        }
        async fn handle_test_delay(&self, _t: TestDelay, _peer: &mut Stream) {}
        fn get_lch(&self) -> Arc<RwLock<LoginConfigHandler>> {
            self.lc.clone()
        }
    }

    fn window() -> Ui {
        let mut lc = LoginConfigHandler::default();
        lc.conn_type = ConnType::PORT_FORWARD;
        Ui {
            lc: Arc::new(RwLock::new(lc)),
            pause: Duration::ZERO,
        }
    }

    /// (our end, the peer's end) of one connection.
    async fn loopback() -> (Stream, Stream) {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let client = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (server, _) = l.accept().await.unwrap();
        (
            Stream::Tcp(FramedStream::from(client, addr)),
            Stream::Tcp(FramedStream::from(server, addr)),
        )
    }

    async fn login_at(peer: &mut Stream) -> LoginRequest {
        let bytes = peer.next().await.unwrap().unwrap();
        Message::parse_from_bytes(&bytes)
            .unwrap()
            .login_request()
            .clone()
    }

    fn target(lr: &LoginRequest) -> (String, i32) {
        (lr.port_forward().host.clone(), lr.port_forward().port)
    }

    fn hash(challenge: &str) -> Hash {
        Hash {
            salt: "salt".to_owned(),
            challenge: challenge.to_owned(),
            ..Default::default()
        }
    }

    /// What the peer expects for password `pw` under `hash(challenge)`.
    fn digest(challenge: &str) -> Vec<u8> {
        let mut h = Sha256::new();
        h.update("pw");
        h.update("salt");
        let salted = h.finalize();
        let mut h2 = Sha256::new();
        h2.update(&salted[..]);
        h2.update(challenge);
        h2.finalize()[..].to_vec()
    }

    #[test]
    fn mappings_logging_in_at_once_each_carry_their_own_target() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut ui = window();
            ui.pause = Duration::from_millis(50);
            let (mut a, mut a_peer) = loopback().await;
            let (mut b, mut b_peer) = loopback().await;
            tokio::join!(
                login_with_hash(&ui, "pw", hash("a"), "a", 1, &mut a),
                login_with_hash(&ui, "pw", hash("b"), "b", 2, &mut b),
            );
            assert_eq!(target(&login_at(&mut a_peer).await), ("a".to_owned(), 1));
            assert_eq!(target(&login_at(&mut b_peer).await), ("b".to_owned(), 2));
        });
    }

    #[test]
    fn a_mapping_answers_the_prompt_with_its_own_challenge() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let ui = window();
            let (mut a, mut a_peer) = loopback().await;
            let (mut b, mut b_peer) = loopback().await;
            // A's hash arrived last, so it is the one the handler holds.
            assert!(login_with_hash(&ui, "pw", hash("a"), "a", 1, &mut a).await);
            login_at(&mut a_peer).await;
            let typed = (String::new(), String::new(), "pw".to_owned(), false);
            login_from_ui(&ui, &hash("b"), typed, "b", 2, &mut b).await;
            let lr = login_at(&mut b_peer).await;
            assert_eq!(lr.password, digest("b"));
            assert_eq!(target(&lr), ("b".to_owned(), 2));
        });
    }

    #[test]
    fn a_password_typed_before_this_connections_hash_answers_it_when_it_comes() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let ui = window();
            let (mut b, mut b_peer) = loopback().await;
            // The prompt's password reached B before its hash, and no other
            // mapping has stored it in the handler yet.
            let typed = (String::new(), String::new(), "pw".to_owned(), false);
            assert!(hash_arrived(&ui, "", hash("b"), Some(typed), "b", 2, &mut b).await);
            let lr = login_at(&mut b_peer).await;
            assert_eq!(lr.password, digest("b"));
            assert_eq!(target(&lr), ("b".to_owned(), 2));
        });
    }
}
