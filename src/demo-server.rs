use hbb_common::{
    bytes::BytesMut,
    protobuf::Message as _,
    rendezvous_proto::*,
    tcp::{new_listener, FramedStream},
    tokio,
    udp::FramedSocket,
};

#[tokio::main(basic_scheduler)]
async fn main() {
    let mut socket = FramedSocket::new("0.0.0.0:21116").await.unwrap();
    let mut listener = new_listener("0.0.0.0:21116", false).await.unwrap();
    let mut rlistener = new_listener("0.0.0.0:21117", false).await.unwrap();
    let mut id_map = std::collections::HashMap::new();
    let relay_server = std::env::var("IP").unwrap();
    let mut saved_stream = None;
    loop {
        tokio::select! {
            Some(Ok((bytes, addr))) = socket.next() => {
                handle_udp(&mut socket, bytes, addr, &mut id_map).await;
            }
            Ok((stream, addr)) = listener.accept() => {
                let mut stream = FramedStream::from(stream);
                if let Some(Ok(bytes)) = stream.next_timeout(3000).await {
                    if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                        match msg_in.union {
                            Some(rendezvous_message::Union::punch_hole_request(ph)) => {
                                println!("punch_hole_request {:?}", addr);
                                if let Some(addr) = id_map.get(&ph.id) {
                                    let mut msg_out = RendezvousMessage::new();
                                    msg_out.set_request_relay(RequestRelay {
                                        relay_server: relay_server.clone(),
                                        ..Default::default()
                                    });
                                    socket.send(&msg_out, addr.clone()).await.ok();
                                    saved_stream = Some(stream);
                                }
                            }
                            Some(rendezvous_message::Union::relay_response(_)) => {
                                println!("relay_response {:?}", addr);
                                let mut msg_out = RendezvousMessage::new();
                                msg_out.set_relay_response(RelayResponse {
                                    relay_server: relay_server.clone(),
                                    ..Default::default()
                                });
                                if let Some(mut stream) = saved_stream.take() {
                                    stream.send(&msg_out).await.ok();
                                    if let Ok((stream_a, _)) = rlistener.accept().await {
                                        let mut stream_a = FramedStream::from(stream_a);
                                        stream_a.next_timeout(3_000).await;
                                        if let Ok((stream_b, _)) = rlistener.accept().await {
                                            let mut stream_b = FramedStream::from(stream_b);
                                            stream_b.next_timeout(3_000).await;
                                            relay(stream_a, stream_b, &mut socket, &mut id_map).await;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

async fn relay(
    stream: FramedStream,
    peer: FramedStream,
    socket: &mut FramedSocket,
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
) {
    let mut peer = peer;
    let mut stream = stream;
    peer.set_raw();
    stream.set_raw();
    loop {
        tokio::select! {
            Some(Ok((bytes, addr))) = socket.next() => {
                handle_udp(socket, bytes, addr, id_map).await;
            }
            res = peer.next() => {
                if let Some(Ok(bytes)) = res {
                    stream.send_bytes(bytes.into()).await.ok();
                } else {
                    break;
                }
            },
            res = stream.next() => {
                if let Some(Ok(bytes)) = res {
                    peer.send_bytes(bytes.into()).await.ok();
                } else {
                    break;
                }
            },
        }
    }
}

async fn handle_udp(
    socket: &mut FramedSocket,
    bytes: BytesMut,
    addr: std::net::SocketAddr,
    id_map: &mut std::collections::HashMap<String, std::net::SocketAddr>,
) {
    if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
        match msg_in.union {
            Some(rendezvous_message::Union::register_peer(rp)) => {
                println!("register_peer {:?}", addr);
                id_map.insert(rp.id, addr);
                let mut msg_out = RendezvousMessage::new();
                msg_out.set_register_peer_response(RegisterPeerResponse::new());
                socket.send(&msg_out, addr).await.ok();
            }
            Some(rendezvous_message::Union::register_pk(_)) => {
                println!("register_pk {:?}", addr);
                let mut msg_out = RendezvousMessage::new();
                msg_out.set_register_pk_response(RegisterPkResponse {
                    result: register_pk_response::Result::OK.into(),
                    ..Default::default()
                });
                socket.send(&msg_out, addr).await.ok();
            }
            _ => {}
        }
    }
}
