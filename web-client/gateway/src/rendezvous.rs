use hbb_common::{
    anyhow::{anyhow, Context},
    bail,
    config::{RELAY_PORT, RENDEZVOUS_PORT},
    protobuf::Message as ProtoMessage,
    rendezvous_proto::{
        punch_hole_response, ConnType, NatType, PunchHoleRequest, PunchHoleResponse, RelayResponse,
        RendezvousMessage, RequestRelay,
    },
    socket_client::check_port,
    sodiumoxide::crypto::sign,
    tcp::FramedStream,
    AddrMangle, Stream,
};

use crate::{
    handshake::{self, SessionKeys},
    ResultType,
};

pub struct IdTarget {
    pub peer_id: String,
    pub password: String,
    pub id_server: String,
    pub relay_server: String,
    pub key: String,
    pub force_relay: bool,
}

/// Punch or relay to `peer_id`, then stop once the password challenge is in hand.
/// The caller sends `LoginRequest` on the returned stream.
pub async fn connect_id(target: &IdTarget) -> ResultType<(Stream, LoginPrep)> {
    if target.peer_id.trim().is_empty() {
        bail!("peer id is empty");
    }
    if target.id_server.trim().is_empty() {
        bail!("id server is empty");
    }
    let server = check_port(target.id_server.trim(), RENDEZVOUS_PORT);
    let framed = FramedStream::new(&server, None, 10_000)
        .await
        .with_context(|| format!("failed to connect to id server {server}"))?;
    let mut id_socket = Stream::Tcp(framed);
    if !target.key.is_empty() {
        let _ = handshake::secure_rendezvous(&mut id_socket, target.key.trim()).await?;
    }

    let mut req = RendezvousMessage::new();
    req.set_punch_hole_request(PunchHoleRequest {
        id: target.peer_id.trim().to_owned(),
        licence_key: target.key.trim().to_owned(),
        nat_type: if target.force_relay {
            NatType::SYMMETRIC
        } else {
            NatType::UNKNOWN_NAT
        }
        .into(),
        conn_type: ConnType::DEFAULT_CONN.into(),
        force_relay: target.force_relay,
        version: "1.5.3".into(),
        ..Default::default()
    });
    id_socket.send(&req).await?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let left = deadline
            .saturating_duration_since(std::time::Instant::now())
            .as_millis() as u64;
        if left == 0 {
            bail!("timed out waiting for the id server");
        }
        let bytes = id_socket
            .next_timeout(left)
            .await
            .context("id server timed out")?
            .map_err(|err| anyhow!(err))?;
        let msg = RendezvousMessage::parse_from_bytes(&bytes)?;
        match msg.union {
            Some(hbb_common::rendezvous_proto::rendezvous_message::Union::PunchHoleResponse(ph)) => {
                return finish_punch(target, ph).await;
            }
            Some(hbb_common::rendezvous_proto::rendezvous_message::Union::RelayResponse(rr)) => {
                return finish_relay(target, rr).await;
            }
            Some(hbb_common::rendezvous_proto::rendezvous_message::Union::PunchHoleSent(_)) => {}
            _ => {}
        }
    }
}

async fn finish_punch(target: &IdTarget, ph: PunchHoleResponse) -> ResultType<(Stream, LoginPrep)> {
    if ph.socket_addr.is_empty() {
        let reason = if !ph.other_failure.is_empty() {
            ph.other_failure
        } else {
            match ph.failure.enum_value() {
                Ok(punch_hole_response::Failure::ID_NOT_EXIST) => "ID does not exist".into(),
                Ok(punch_hole_response::Failure::OFFLINE) => "remote desktop is offline".into(),
                Ok(punch_hole_response::Failure::LICENSE_MISMATCH) => "key mismatch".into(),
                Ok(punch_hole_response::Failure::LICENSE_OVERUSE) => "key overuse".into(),
                _ => "punch hole failed".into(),
            }
        };
        bail!(reason);
    }
    let addr = AddrMangle::decode(&ph.socket_addr);
    let framed = FramedStream::new(addr, None, 10_000)
        .await
        .with_context(|| format!("failed to connect to punched address {addr}"))?;
    let mut stream = Stream::Tcp(framed);
    let sign_pk = peer_sign_key(target, &ph.pk)?;
    let (hash, keys) =
        handshake::read_until_hash(&mut stream, sign_pk.as_ref(), target.peer_id.trim()).await?;
    Ok((
        stream,
        LoginPrep {
            username: target.peer_id.trim().to_owned(),
            password: target.password.clone(),
            hash,
            keys,
        },
    ))
}

async fn finish_relay(target: &IdTarget, rr: RelayResponse) -> ResultType<(Stream, LoginPrep)> {
    if !rr.refuse_reason.is_empty() {
        bail!(rr.refuse_reason);
    }
    if rr.uuid.is_empty()
        || rr.uuid.len() > 128
        || !rr
            .uuid
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        bail!("invalid relay uuid");
    }
    let relay_host = if target.relay_server.trim().is_empty() {
        rr.relay_server.clone()
    } else {
        target.relay_server.trim().to_owned()
    };
    if relay_host.is_empty() {
        bail!("relay server is empty");
    }
    let relay = check_port(relay_host, RELAY_PORT);
    let framed = FramedStream::new(&relay, None, 10_000)
        .await
        .with_context(|| format!("failed to connect to relay {relay}"))?;
    let mut stream = Stream::Tcp(framed);
    let signed = rr.pk().to_vec();
    let mut req = RendezvousMessage::new();
    req.set_request_relay(RequestRelay {
        id: target.peer_id.trim().to_owned(),
        uuid: rr.uuid,
        licence_key: target.key.trim().to_owned(),
        conn_type: ConnType::DEFAULT_CONN.into(),
        ..Default::default()
    });
    stream.send(&req).await?;
    let sign_pk = peer_sign_key(target, &signed)?;
    let (hash, keys) =
        handshake::read_until_hash(&mut stream, sign_pk.as_ref(), target.peer_id.trim()).await?;
    Ok((
        stream,
        LoginPrep {
            username: target.peer_id.trim().to_owned(),
            password: target.password.clone(),
            hash,
            keys,
        },
    ))
}

fn peer_sign_key(target: &IdTarget, signed: &[u8]) -> ResultType<Option<sign::PublicKey>> {
    if target.key.trim().is_empty() || signed.is_empty() {
        return Ok(None);
    }
    let rs = handshake::rs_public_key(target.key.trim())?;
    let opened = handshake::open_signed_id(signed, &rs)?;
    if opened.id != target.peer_id.trim() {
        bail!("identity server signed a different id ({})", opened.id);
    }
    Ok(Some(sign::PublicKey(opened.pk)))
}

pub struct LoginPrep {
    pub username: String,
    pub password: String,
    pub hash: base::message_proto::Hash,
    pub keys: SessionKeys,
}
