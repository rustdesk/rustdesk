use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message as WsMessage, WebSocket};
use base::message_proto::{message, misc, video_frame, Message, PeerInfo};
use futures_util::{SinkExt, StreamExt};
use hbb_common::{bail, log, protobuf::Message as ProtoMessage, Stream};
use serde_json::{json, Value};

use crate::{
    control::{self, ClientMsg, CodecCaps},
    direct::{self, DirectTarget, LoginResult},
    rendezvous::{self, IdTarget},
    video::{self, EncodedPacket},
    ResultType,
};

pub async fn serve_socket(socket: WebSocket, gateway_token: Option<String>) {
    let (mut tx, mut rx) = socket.split();
    if tx
        .send(WsMessage::Text(json!({"type":"hello","service":"rustdesk-web-gateway"}).to_string()))
        .await
        .is_err()
    {
        return;
    }
    let connect = loop {
        match rx.next().await {
            Some(Ok(WsMessage::Text(text))) => match control::parse_client_msg(&text) {
                Ok(ClientMsg::Connect { .. }) => break text,
                Ok(_) => {
                    let _ = send_text(
                        &mut tx,
                        &json!({"type":"error","message":"send a connect message first"}),
                    )
                    .await;
                }
                Err(err) => {
                    let _ = send_text(
                        &mut tx,
                        &json!({"type":"error","message":format!("bad message: {err}")}),
                    )
                    .await;
                }
            },
            Some(Ok(WsMessage::Close(_))) | None => return,
            Some(Ok(_)) => {}
            Some(Err(_)) => return,
        }
    };
    let Ok(ClientMsg::Connect {
        mode,
        host,
        port,
        password,
        peer_id,
        id_server,
        relay_server,
        key,
        force_relay,
        token,
        codecs,
    }) = control::parse_client_msg(&connect)
    else {
        return;
    };
    if !control::token_matches(gateway_token.as_deref(), &token) {
        let _ = send_text(
            &mut tx,
            &json!({
                "type": "status",
                "state": "error",
                "detail": control::humanize_error("gateway access token"),
            }),
        )
        .await;
        return;
    }
    let codecs = codecs.selected();
    if !codecs.any() {
        let _ = send_text(
            &mut tx,
            &json!({
                "type": "status",
                "state": "error",
                "detail": "This browser cannot decode VP8, VP9, H.264, HEVC, or AV1.",
            }),
        )
        .await;
        return;
    }
    if let Err(err) = run(
        &mut tx,
        &mut rx,
        SessionStart {
            mode,
            host,
            port,
            password,
            peer_id,
            id_server,
            relay_server,
            key,
            force_relay,
            codecs,
        },
    )
    .await
    {
        log::info!("session ended: {err:#}");
        let detail = control::humanize_error(&format!("{err:#}"));
        let _ = send_text(
            &mut tx,
            &json!({"type":"status","state":"error","detail": detail}),
        )
        .await;
    }
}

struct SessionStart {
    mode: String,
    host: String,
    port: u16,
    password: String,
    peer_id: String,
    id_server: String,
    relay_server: String,
    key: String,
    force_relay: bool,
    codecs: CodecCaps,
}

type WsTx = futures_util::stream::SplitSink<WebSocket, WsMessage>;
type WsRx = futures_util::stream::SplitStream<WebSocket>;

async fn run(tx: &mut WsTx, rx: &mut WsRx, start: SessionStart) -> ResultType<()> {
    send_text(
        tx,
        &json!({"type":"status","state":"connecting","detail":"dialing the host"}),
    )
    .await?;
    let (mut stream, logged) = open_host(&start).await?;
    let _ = stream.send(&control::refresh_message()).await;
    let mut size = display_size(&logged.peer);
    send_text(tx, &peer_json(&logged)).await?;
    send_text(
        tx,
        &json!({
            "type": "status",
            "state": "live",
            "detail": if logged.encrypted { "encrypted" } else { "direct" },
            "username": logged.username,
        }),
    )
    .await?;

    let mut frames: u64 = 0;
    let mut bytes_out: u64 = 0;
    let mut logged_at = Instant::now();
    let mut raw_warned = false;
    loop {
        tokio::select! {
            incoming = rx.next() => {
                match incoming {
                    Some(Ok(WsMessage::Text(text))) => {
                        if !handle_browser(&mut stream, tx, &text).await? {
                            break;
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(err)) => bail!("websocket: {err}"),
                }
            }
            incoming = stream.next() => {
                let Some(incoming) = incoming else {
                    bail!("host closed the connection");
                };
                let bytes = incoming.map_err(|err| hbb_common::anyhow::anyhow!(err))?;
                let msg = Message::parse_from_bytes(&bytes)?;
                if let Some(delay) = echo_test_delay(&mut stream, &msg).await? {
                    send_text(tx, &json!({"type":"rtt","ms": delay})).await?;
                }
                if let Some(note) = describe_control(&msg, &mut size) {
                    send_text(tx, &note).await?;
                }
                if !raw_warned && is_raw_video(&msg) {
                    raw_warned = true;
                    send_text(tx, &json!({
                        "type": "error",
                        "message": "the host sent uncompressed video; this client only decodes vp8, vp9, h264, and av1"
                    })).await?;
                }
                let packets = take_frames(&msg, &mut size);
                let had_video = !packets.is_empty();
                for packet in packets {
                    let encoded = video::encode(&packet);
                    bytes_out += encoded.len() as u64;
                    frames += 1;
                    tx.send(WsMessage::Binary(encoded)).await?;
                }
                if had_video {
                    stream.send(&control::video_received_message()).await?;
                }
                if logged_at.elapsed().as_secs() >= 5 && frames > 0 {
                    log::info!("forwarded {frames} video frames");
                    logged_at = Instant::now();
                }
            }
        }
    }
    let _ = send_text(tx, &json!({"type":"status","state":"closed","detail":"disconnected"})).await;
    log::info!("session closed after {frames} frames, {bytes_out} bytes");
    Ok(())
}

async fn open_host(start: &SessionStart) -> ResultType<(Stream, LoginResult)> {
    if start.mode == "id" {
        let (mut stream, prep) = rendezvous::connect_id(&IdTarget {
            peer_id: start.peer_id.clone(),
            password: start.password.clone(),
            id_server: start.id_server.clone(),
            relay_server: start.relay_server.clone(),
            key: start.key.clone(),
            force_relay: start.force_relay,
        })
        .await?;
        let logged = direct::login(
            &mut stream,
            &prep.username,
            &prep.password,
            &prep.hash,
            prep.keys,
            &start.codecs,
        )
        .await?;
        Ok((stream, logged))
    } else {
        direct::connect_and_login(&DirectTarget {
            host: start.host.clone(),
            port: start.port,
            password: start.password.clone(),
            codecs: start.codecs.clone(),
        })
        .await
    }
}

async fn handle_browser(stream: &mut Stream, tx: &mut WsTx, text: &str) -> ResultType<bool> {
    let msg = match control::parse_client_msg(text) {
        Ok(msg) => msg,
        Err(err) => {
            send_text(tx, &json!({"type":"error","message":format!("bad message: {err}")})).await?;
            return Ok(true);
        }
    };
    match msg {
        ClientMsg::Disconnect => return Ok(false),
        ClientMsg::Connect { .. } => {
            send_text(
                tx,
                &json!({"type":"error","message":"already connected"}),
            )
            .await?;
        }
        other => {
            if let Some(host_msg) = control::to_host_message(&other) {
                stream.send(&host_msg).await?;
            }
        }
    }
    Ok(true)
}

async fn echo_test_delay(stream: &mut Stream, msg: &Message) -> ResultType<Option<u32>> {
    let Some(message::Union::TestDelay(delay)) = &msg.union else {
        return Ok(None);
    };
    if delay.from_client {
        return Ok(None);
    }
    let mut echo = Message::new();
    echo.set_test_delay(delay.clone());
    stream.send(&echo).await?;
    Ok(Some(delay.last_delay).filter(|ms| *ms > 0))
}

fn display_size(peer: &PeerInfo) -> (u32, u32) {
    let idx = peer.current_display.max(0) as usize;
    peer.displays
        .get(idx)
        .or_else(|| peer.displays.first())
        .map(|d| (d.width.max(0) as u32, d.height.max(0) as u32))
        .unwrap_or((0, 0))
}

fn peer_json(logged: &LoginResult) -> Value {
    let peer = &logged.peer;
    let displays: Vec<_> = peer
        .displays
        .iter()
        .enumerate()
        .map(|(index, d)| {
            json!({
                "index": index,
                "name": d.name,
                "x": d.x,
                "y": d.y,
                "width": d.width,
                "height": d.height,
                "online": d.online,
            })
        })
        .collect();
    let encoding = peer.encoding.as_ref().map(|enc| {
        json!({
            "vp8": enc.vp8,
            "h264": enc.h264,
            "h265": enc.h265,
            "av1": enc.av1,
        })
    });
    json!({
        "type": "peer",
        "hostname": peer.hostname,
        "platform": peer.platform,
        "username": peer.username,
        "version": peer.version,
        "currentDisplay": peer.current_display,
        "displays": displays,
        "encoding": encoding,
        "encrypted": logged.encrypted,
    })
}

fn describe_control(msg: &Message, size: &mut (u32, u32)) -> Option<Value> {
    match &msg.union {
        Some(message::Union::Misc(m)) => match &m.union {
            Some(misc::Union::SwitchDisplay(s)) => {
                if s.width > 0 && s.height > 0 {
                    *size = (s.width as u32, s.height as u32);
                }
                Some(json!({
                    "type": "display",
                    "display": s.display,
                    "width": s.width,
                    "height": s.height,
                    "name": "",
                }))
            }
            Some(misc::Union::CloseReason(reason)) => Some(json!({
                "type": "status",
                "state": "error",
                "detail": reason,
            })),
            _ => None,
        },
        Some(message::Union::Clipboard(cb)) => {
            if cb.compress {
                return None;
            }
            let text = String::from_utf8_lossy(&cb.content);
            if text.len() > 256 * 1024 {
                return None;
            }
            Some(json!({"type":"clipboard","text":text}))
        }
        Some(message::Union::CursorPosition(pos)) => {
            Some(json!({"type":"cursor","x":pos.x,"y":pos.y}))
        }
        _ => None,
    }
}

fn is_raw_video(msg: &Message) -> bool {
    match &msg.union {
        Some(message::Union::VideoFrame(vf)) => matches!(
            &vf.union,
            Some(video_frame::Union::Rgb(_)) | Some(video_frame::Union::Yuv(_))
        ),
        _ => false,
    }
}

fn take_frames<'a>(msg: &'a Message, size: &mut (u32, u32)) -> Vec<EncodedPacket<'a>> {
    let Some(message::Union::VideoFrame(vf)) = &msg.union else {
        return Vec::new();
    };
    let (codec, frames) = match &vf.union {
        Some(video_frame::Union::Vp8s(frames)) => (video::CODEC_VP8, frames),
        Some(video_frame::Union::Vp9s(frames)) => (video::CODEC_VP9, frames),
        Some(video_frame::Union::H264s(frames)) => (video::CODEC_H264, frames),
        Some(video_frame::Union::H265s(frames)) => (video::CODEC_H265, frames),
        Some(video_frame::Union::Av1s(frames)) => (video::CODEC_AV1, frames),
        _ => return Vec::new(),
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut out = Vec::with_capacity(frames.frames.len());
    for frame in &frames.frames {
        if codec == video::CODEC_VP8 {
            if let Some((w, h)) = video::vp8_keyframe_size(&frame.data) {
                *size = (w, h);
            }
        }
        out.push(EncodedPacket {
            codec,
            key: frame.key,
            display: vf.display.max(0) as u16,
            width: size.0,
            height: size.1,
            pts: frame.pts,
            gateway_unix_ms: now,
            data: &frame.data,
        });
    }
    out
}

async fn send_text(tx: &mut WsTx, value: &Value) -> ResultType<()> {
    tx.send(WsMessage::Text(value.to_string())).await?;
    Ok(())
}
