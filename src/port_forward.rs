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

#[cfg(windows)]
type RdpLoopbackLease = crate::platform::RdpLoopbackAddress;

#[cfg(not(windows))]
#[derive(Clone)]
struct RdpLoopbackLease;

fn run_rdp(
    port: u16,
    host: &str,
    lc: &Arc<RwLock<LoginConfigHandler>>,
    id: &str,
    loopback: Option<RdpLoopbackLease>,
) {
    #[cfg(windows)]
    let (username, password) = {
        let lc = lc.read().unwrap();
        (lc.get_option("rdp_username"), lc.get_option("rdp_password"))
    };
    #[cfg(windows)]
    let credential_target = format!("TERMSRV/{}", host);
    #[cfg(windows)]
    let (rdp_credential, prompt_for_credentials) =
        match crate::platform::prepare_temporary_rdp_credential(
            &credential_target,
            &username,
            &password,
        ) {
            Ok(credential) => (credential, false),
            Err(err) => {
                log::warn!(
                    "Failed to prepare RDP credential for target '{}': {}",
                    credential_target,
                    err
                );
                (None, true)
            }
        };
    // Keep using /v instead of a generated .rdp file: mstsc then preserves the
    // user's Default.rdp settings and avoids unsigned-file warnings or policies.
    let mut command = std::process::Command::new("mstsc");
    command.arg(format!("/v:{}:{}", host, port));
    #[cfg(windows)]
    if prompt_for_credentials {
        command.arg("/prompt");
    }
    match command.spawn() {
        Ok(child) => {
            #[cfg(windows)]
            {
                let lc = lc.clone();
                let id = id.to_owned();
                crate::platform::set_rdp_window_title(
                    child,
                    move || rdp_display_name(&lc, &id),
                    host.to_owned(),
                    rdp_credential,
                    loopback,
                );
            }
            #[cfg(not(windows))]
            let _ = (child, lc, id, loopback);
        }
        Err(err) => log::warn!("Failed to launch mstsc: {}", err),
    }
}

#[cfg(windows)]
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
    let is_rdp = port == 0;
    #[cfg(windows)]
    let rdp_loopback = if is_rdp {
        let has_username = {
            let lc = lc.read().unwrap();
            !lc.get_option("rdp_username").is_empty()
        };
        if has_username {
            Some(crate::platform::reserve_rdp_loopback_address()?)
        } else {
            None
        }
    } else {
        None
    };
    #[cfg(not(windows))]
    let rdp_loopback: Option<RdpLoopbackLease> = None;
    #[cfg(windows)]
    let listener_host = rdp_loopback
        .as_ref()
        .map(|address| address.bind_host())
        .unwrap_or("127.0.0.1");
    #[cfg(not(windows))]
    let listener_host = "127.0.0.1";
    let listener = tcp::new_listener(format!("{}:{}", listener_host, port), true).await?;
    let addr = listener.local_addr()?;
    log::info!("listening on port {:?}", addr);
    #[cfg(windows)]
    let rdp_host = rdp_loopback
        .as_ref()
        .map(|address| address.mstsc_host())
        .unwrap_or("localhost");
    #[cfg(not(windows))]
    let rdp_host = "localhost";
    if is_rdp {
        run_rdp(addr.port(), rdp_host, &lc, &id, rdp_loopback.clone());
    }
    let mut ui_receiver = ui_receiver;
    loop {
        tokio::select! {
            Ok((forward, addr)) = listener.accept() => {
                log::info!("new connection from {:?}", addr);
                lc.write().unwrap().port_forward = (remote_host.clone(), remote_port);
                let id = id.clone();
                let password = password.clone();
                let mut forward = Framed::new(forward, BytesCodec::new());
                let mut close_port_forward = false;
                match connect_and_login(&id, &password, &mut ui_receiver, interface.clone(), &mut forward, key, token, is_rdp, &mut close_port_forward).await {
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
                        run_rdp(addr.port(), rdp_host, &lc, &id, rdp_loopback.clone());
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
                            if !interface.handle_hash(password, hash, &mut stream).await {
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
                    Some(Data::Login((os_username, os_password, password, remember))) => {
                        interface.handle_login_from_ui(os_username, os_password, password, remember, &mut stream).await;
                    }
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
