use std::time::Duration;

use base::message_proto::{message, Hash, Message, PublicKey, SignedId};
use hbb_common::{
    anyhow::{anyhow, Context},
    bail,
    protobuf::Message as ProtoMessage,
    sodiumoxide::crypto::{box_, secretbox, sign},
    tcp::{self, KxTranscript},
    Stream,
};

use crate::ResultType;

pub struct OpenedId {
    pub id: String,
    pub pk: [u8; 32],
    pub kx_version: u32,
}

pub struct SessionKeys {
    pub encrypted: bool,
}

/// Open an `IdPk` blob signed by `signer`.
pub fn open_signed_id(signed: &[u8], signer: &sign::PublicKey) -> ResultType<OpenedId> {
    let raw = sign::verify(signed, signer).map_err(|_| anyhow!("signature mismatch"))?;
    let parsed = hbb_common::rendezvous_proto::IdPk::parse_from_bytes(&raw)?;
    if parsed.pk.len() != sign::PUBLICKEYBYTES {
        bail!("signed public key length {}", parsed.pk.len());
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&parsed.pk);
    Ok(OpenedId {
        id: parsed.id,
        pk,
        kx_version: parsed.kx_version,
    })
}

pub fn rs_public_key(b64: &str) -> ResultType<sign::PublicKey> {
    use hbb_common::sodiumoxide::base64;
    let bytes = base64::decode(b64, base64::Variant::Original)
        .map_err(|_| anyhow!("rendezvous public key is not base64"))?;
    if bytes.len() != sign::PUBLICKEYBYTES {
        bail!("rendezvous public key length {}", bytes.len());
    }
    let mut pk = [0u8; sign::PUBLICKEYBYTES];
    pk.copy_from_slice(&bytes);
    Ok(sign::PublicKey(pk))
}

/// Seal a fresh secretbox key to `their_pk`, the same way the desktop client does.
pub fn create_symmetric_key_msg(their_pk: [u8; 32]) -> (Vec<u8>, Vec<u8>, secretbox::Key) {
    let their_pk = box_::PublicKey(their_pk);
    let (our_pk, our_sk) = box_::gen_keypair();
    let key = secretbox::gen_key();
    let nonce = box_::Nonce([0u8; box_::NONCEBYTES]);
    let sealed = box_::seal(&key.0, &nonce, &their_pk, &our_sk);
    (our_pk.0.to_vec(), sealed, key)
}

/// Read the host until its password challenge.
///
/// A direct-IP host sends `Hash` in the clear. A punched or relayed host sends
/// `SignedId` first and waits for `PublicKey` before the challenge.
pub async fn read_until_hash(
    stream: &mut Stream,
    peer_sign_pk: Option<&sign::PublicKey>,
    expected_id: &str,
) -> ResultType<(Hash, SessionKeys)> {
    let msg = recv_message(stream, 10_000).await?;
    match msg.union {
        Some(message::Union::Hash(hash)) => Ok((hash, SessionKeys { encrypted: false })),
        Some(message::Union::SignedId(signed)) => {
            finish_signed_id(stream, signed, peer_sign_pk, expected_id).await
        }
        _ => bail!("expected Hash or SignedId, got another message"),
    }
}

async fn finish_signed_id(
    stream: &mut Stream,
    signed: SignedId,
    peer_sign_pk: Option<&sign::PublicKey>,
    expected_id: &str,
) -> ResultType<(Hash, SessionKeys)> {
    let verified = peer_sign_pk.and_then(|pk| open_signed_id(&signed.id, pk).ok());
    let accept = match &verified {
        Some(opened) => expected_id.is_empty() || opened.id == expected_id,
        None => false,
    };
    if !accept {
        hbb_common::log::info!("signed identity not verified, continuing without a stream key");
        let mut msg = Message::new();
        msg.set_public_key(PublicKey::new());
        stream.send(&msg).await?;
        let hash = recv_hash(stream).await?;
        return Ok((hash, SessionKeys { encrypted: false }));
    }
    let opened = verified.context("verified identity")?;
    let (asymmetric, symmetric, key) = create_symmetric_key_msg(opened.pk);
    let picked = tcp::kx_version_for(opened.kx_version);
    let mut msg = Message::new();
    msg.set_public_key(PublicKey {
        asymmetric_value: asymmetric.clone().into(),
        symmetric_value: symmetric.into(),
        kx_version: picked,
        ..Default::default()
    });
    stream.send(&msg).await?;
    stream.set_negotiated_key(
        key,
        true,
        &KxTranscript {
            initiator_pk: &asymmetric,
            responder_pk: &opened.pk,
            advertised: opened.kx_version,
            picked,
        },
    )?;
    let hash = recv_hash(stream).await?;
    Ok((hash, SessionKeys { encrypted: true }))
}

async fn recv_hash(stream: &mut Stream) -> ResultType<Hash> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let left = deadline
            .saturating_duration_since(std::time::Instant::now())
            .as_millis() as u64;
        if left == 0 {
            bail!("timed out waiting for the password challenge");
        }
        let msg = recv_message(stream, left).await?;
        if let Some(message::Union::Hash(hash)) = msg.union {
            return Ok(hash);
        }
    }
}

pub async fn recv_message(stream: &mut Stream, timeout_ms: u64) -> ResultType<Message> {
    let next = stream
        .next_timeout(timeout_ms)
        .await
        .context("timed out waiting for a RustDesk message")?
        .map_err(|err| anyhow!(err))?;
    Ok(Message::parse_from_bytes(&next)?)
}

/// Rendezvous / relay key exchange. The server speaks first.
///
/// Returns whether the socket is now encrypted. A server that sends something
/// other than `KeyExchange` is left unencrypted, matching the desktop client.
pub async fn secure_rendezvous(stream: &mut Stream, key_b64: &str) -> ResultType<bool> {
    if key_b64.is_empty() {
        return Ok(false);
    }
    let rs_pk = rs_public_key(key_b64)?;
    let bytes = stream
        .next_timeout(8_000)
        .await
        .context("rendezvous key exchange timed out")?
        .map_err(|err| anyhow!(err))?;
    let msg = hbb_common::rendezvous_proto::RendezvousMessage::parse_from_bytes(&bytes)?;
    let Some(hbb_common::rendezvous_proto::rendezvous_message::Union::KeyExchange(ex)) = msg.union
    else {
        hbb_common::log::warn!("rendezvous server did not open with a key exchange");
        return Ok(false);
    };
    if ex.keys.len() != 1 {
        bail!("rendezvous key exchange did not carry one public key");
    }
    let their = sign::verify(&ex.keys[0], &rs_pk)
        .map_err(|_| anyhow!("rendezvous key exchange signature mismatch"))?;
    if their.len() != box_::PUBLICKEYBYTES {
        bail!("rendezvous box key length {}", their.len());
    }
    let mut their_pk = [0u8; box_::PUBLICKEYBYTES];
    their_pk.copy_from_slice(&their);
    if their_pk[31] & 0x80 != 0 || !ex.signed_params.is_empty() {
        let signed = sign::verify(&ex.signed_params, &rs_pk)
            .map_err(|_| anyhow!("signed key exchange parameters were rejected"))?;
        let params = signed
            .strip_prefix(tcp::KX_PARAMS_DOMAIN)
            .context("key exchange parameters were not domain-separated")?;
        let params = hbb_common::rendezvous_proto::KxParams::parse_from_bytes(params)?;
        if params.pk.as_ref() != their_pk.as_ref() || params.version != ex.version {
            bail!("key exchange parameters do not match the signed public key");
        }
    }
    let (asymmetric, symmetric, key) = create_symmetric_key_msg(their_pk);
    let picked = tcp::kx_version_for(ex.version);
    let mut out = hbb_common::rendezvous_proto::RendezvousMessage::new();
    out.set_key_exchange(hbb_common::rendezvous_proto::KeyExchange {
        keys: vec![asymmetric.clone().into(), symmetric.into()],
        version: picked,
        ..Default::default()
    });
    stream.send(&out).await?;
    stream.set_negotiated_key(
        key,
        true,
        &KxTranscript {
            initiator_pk: &asymmetric,
            responder_pk: &their_pk,
            advertised: ex.version,
            picked,
        },
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::password_response;
    use base::message_proto::{LoginRequest, Message};
    use hbb_common::{
        protobuf::Message as ProtoMessage,
        rendezvous_proto::IdPk,
        sodiumoxide::crypto::{box_, sign},
        tcp::{self, Encrypt, FramedStream, KxTranscript},
        tokio::{self, net::TcpListener},
        Stream,
    };

    fn init_sodium() {
        let _ = hbb_common::sodiumoxide::init();
    }

    async fn pair() -> (Stream, Stream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = hbb_common::tokio::net::TcpStream::connect(addr).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        let client_addr = client.local_addr().unwrap();
        let server_addr = server.local_addr().unwrap();
        (
            Stream::Tcp(FramedStream::from(client, client_addr)),
            Stream::Tcp(FramedStream::from(server, server_addr)),
        )
    }

    #[tokio::test]
    async fn plaintext_hash_then_login_roundtrip() {
        init_sodium();
        let (mut client, mut host) = pair().await;
        let host_task = tokio::spawn(async move {
            let mut hash_msg = Message::new();
            hash_msg.set_hash(Hash {
                salt: "salt".into(),
                challenge: "b".into(),
                ..Default::default()
            });
            host.send(&hash_msg).await.unwrap();
            let login = recv_message(&mut host, 3_000).await.unwrap();
            let message::Union::LoginRequest(lr) = login.union.unwrap() else {
                panic!("login");
            };
            assert_eq!(lr.password, password_response("pw", "salt", "b"));
            assert_eq!(lr.username, "127.0.0.1:21118");
        });
        let (hash, keys) = read_until_hash(&mut client, None, "").await.unwrap();
        assert!(!keys.encrypted);
        let mut lr = LoginRequest::new();
        lr.username = "127.0.0.1:21118".into();
        lr.password = password_response("pw", &hash.salt, &hash.challenge).into();
        let mut msg = Message::new();
        msg.set_login_request(lr);
        client.send(&msg).await.unwrap();
        host_task.await.unwrap();
    }

    #[tokio::test]
    async fn nacl_handshake_encrypts_the_password_challenge() {
        init_sodium();
        let (sign_pk, sign_sk) = sign::gen_keypair();
        let (box_pk, box_sk) = box_::gen_keypair();
        let (mut client, mut host) = pair().await;

        let host_task = tokio::spawn(async move {
            let id_pk = IdPk {
                id: "peer".into(),
                pk: box_pk.0.to_vec().into(),
                kx_version: tcp::KX_VERSION_LATEST,
                ..Default::default()
            };
            let mut signed = Message::new();
            signed.set_signed_id(SignedId {
                id: sign::sign(&id_pk.write_to_bytes().unwrap(), &sign_sk).into(),
                ..Default::default()
            });
            host.send(&signed).await.unwrap();
            let reply = recv_message(&mut host, 3_000).await.unwrap();
            let message::Union::PublicKey(pk) = reply.union.unwrap() else {
                panic!("public key");
            };
            let key = Encrypt::decode(&pk.symmetric_value, &pk.asymmetric_value, &box_sk).unwrap();
            host.set_negotiated_key(
                key,
                false,
                &KxTranscript {
                    initiator_pk: &pk.asymmetric_value,
                    responder_pk: &box_pk.0,
                    advertised: tcp::KX_VERSION_LATEST,
                    picked: pk.kx_version,
                },
            )
            .unwrap();
            let mut hash_msg = Message::new();
            hash_msg.set_hash(Hash {
                salt: "salt".into(),
                challenge: "b".into(),
                ..Default::default()
            });
            host.send(&hash_msg).await.unwrap();
            let login = recv_message(&mut host, 3_000).await.unwrap();
            let message::Union::LoginRequest(lr) = login.union.unwrap() else {
                panic!("login");
            };
            assert_eq!(lr.password, password_response("secret", "salt", "b"));
            assert!(host.is_secured());
        });

        let (hash, keys) = read_until_hash(&mut client, Some(&sign_pk), "peer")
            .await
            .unwrap();
        assert!(keys.encrypted);
        assert!(client.is_secured());
        assert_eq!(hash.challenge, "b");
        let mut lr = LoginRequest::new();
        lr.password = password_response("secret", &hash.salt, &hash.challenge).into();
        let mut msg = Message::new();
        msg.set_login_request(lr);
        client.send(&msg).await.unwrap();
        host_task.await.unwrap();
    }
}
