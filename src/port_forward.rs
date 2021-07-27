use crate::client::*;
use hbb_common::{
    allow_err, bail,
    config::CONNECT_TIMEOUT,
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

fn run_rdp(port: u16) {
    std::process::Command::new("mstsc")
        .arg(format!("/v:localhost:{}", port))
        .spawn()
        .ok();
}

pub async fn listen(
    id: String,
    port: i32,
    interface: impl Interface,
    ui_receiver: mpsc::UnboundedReceiver<Data>,
) -> ResultType<()> {
    let listener = tcp::new_listener(format!("0.0.0.0:{}", port), true).await?;
    let addr = listener.local_addr()?;
    log::info!("listening on port {:?}", addr);
    let is_rdp = port == 0;
    if is_rdp {
        run_rdp(addr.port());
    }
    let mut ui_receiver = ui_receiver;
    loop {
        tokio::select! {
            Ok((forward, addr)) = listener.accept() => {
                log::info!("new connection from {:?}", addr);
                let id = id.clone();
                let mut forward = Framed::new(forward, BytesCodec::new());
                match connect_and_login(&id, &mut ui_receiver, interface.clone(), &mut forward, is_rdp).await {
                    Ok(Some(stream)) => {
                        let interface = interface.clone();
                        tokio::spawn(async move {
                            if let Err(err) = run_forward(forward, stream).await {
                               interface.msgbox("error", "Error", &err.to_string());
                            }
                            log::info!("connection from {:?} closed", addr);
                       });
                    }
                    Err(err) => {
                        interface.msgbox("error", "Error", &err.to_string());
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
                        run_rdp(addr.port());
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
    ui_receiver: &mut mpsc::UnboundedReceiver<Data>,
    interface: impl Interface,
    forward: &mut Framed<TcpStream, BytesCodec>,
    is_rdp: bool,
) -> ResultType<Option<Stream>> {
    let conn_type = if is_rdp {
        ConnType::RDP
    } else {
        ConnType::PORT_FORWARD
    };
    let (mut stream, _) = Client::start(id, conn_type).await?;
    let mut interface = interface;
    let mut buffer = Vec::new();
    loop {
        tokio::select! {
            res = timeout(CONNECT_TIMEOUT, stream.next()) => match res {
                Err(_) => {
                    bail!("Timeout");
                }
                Ok(Some(Ok(bytes))) => {
                    let msg_in = Message::parse_from_bytes(&bytes)?;
                    match msg_in.union {
                        Some(message::Union::hash(hash)) => {
                            interface.handle_hash(hash, &mut stream).await;
                        }
                        Some(message::Union::login_response(lr)) => match lr.union {
                            Some(login_response::Union::error(err)) => {
                                interface.handle_login_error(&err);
                                return Ok(None);
                            }
                            Some(login_response::Union::peer_info(pi)) => {
                                interface.handle_peer_info(pi);
                                break;
                            }
                            _ => {}
                        }
                        Some(message::Union::test_delay(t)) => {
                            interface.handle_test_delay(t, &mut stream).await;
                        }
                        _ => {}
                    }
                }
                _ => {
                    bail!("Reset by the peer");
                }
            },
            d = ui_receiver.recv() => {
                match d {
                    Some(Data::Login((password, remember))) => {
                        interface.handle_login_from_ui(password, remember, &mut stream).await;
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
                    allow_err!(forward.send(bytes.into()).await);
                } else {
                    break;
                }
            },
        }
    }
    Ok(())
}
