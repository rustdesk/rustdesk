use base::message_proto::{
    login_response, message, option_message::BoolOption, supported_decoding::PreferCodec, Chroma,
    ImageQuality, LoginRequest, Message, OptionMessage, PeerInfo, SupportedDecoding,
};
use hbb_common::{
    anyhow::Context,
    bail,
    config::RENDEZVOUS_PORT,
    protobuf::MessageField,
    socket_client::check_port,
    tcp::FramedStream,
    Stream,
};

use crate::{
    auth::{direct_username, password_response},
    control::CodecCaps,
    handshake::{self, SessionKeys},
    ResultType,
};

/// `21118`, the port a host listens on when direct IP access is enabled.
pub const DEFAULT_DIRECT_PORT: u16 = (RENDEZVOUS_PORT + 2) as u16;

pub struct DirectTarget {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub codecs: CodecCaps,
}

pub struct LoginResult {
    pub username: String,
    pub peer: PeerInfo,
    pub encrypted: bool,
}

pub async fn connect_and_login(target: &DirectTarget) -> ResultType<(Stream, LoginResult)> {
    let port = if target.port == 0 {
        DEFAULT_DIRECT_PORT
    } else {
        target.port
    };
    let dial = check_port(&target.host, port as i32);
    let username = direct_username(&target.host, port);
    let framed = FramedStream::new(&dial, None, 8_000)
        .await
        .with_context(|| format!("failed to connect to {dial}"))?;
    let mut stream = Stream::Tcp(framed);
    let (hash, keys) = handshake::read_until_hash(&mut stream, None, "").await?;
    let result = login(
        &mut stream,
        &username,
        &target.password,
        &hash,
        keys,
        &target.codecs,
    )
    .await?;
    Ok((stream, result))
}

pub async fn login(
    stream: &mut Stream,
    username: &str,
    password: &str,
    hash: &base::message_proto::Hash,
    keys: SessionKeys,
    codecs: &CodecCaps,
) -> ResultType<LoginResult> {
    let mut lr = LoginRequest::new();
    lr.username = username.to_owned();
    lr.password = password_response(password, &hash.salt, &hash.challenge).into();
    lr.my_id = "web-client".into();
    lr.my_name = "Web Client".into();
    lr.my_platform = "Web".into();
    lr.version = "1.5.3".into();
    lr.option = MessageField::some(login_options(codecs));
    let mut msg = Message::new();
    msg.set_login_request(lr);
    stream.send(&msg).await?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let left = deadline
            .saturating_duration_since(std::time::Instant::now())
            .as_millis() as u64;
        if left == 0 {
            bail!("timed out waiting for LoginResponse");
        }
        let incoming = handshake::recv_message(stream, left).await?;
        if let Some(result) = take_peer(username, keys.encrypted, incoming) {
            return result;
        }
    }
}

fn take_peer(username: &str, encrypted: bool, incoming: Message) -> Option<ResultType<LoginResult>> {
    let peer = match incoming.union {
        Some(message::Union::LoginResponse(lr)) => match lr.union {
            Some(login_response::Union::Error(err)) => {
                return Some(Err(hbb_common::anyhow::anyhow!(
                    "{}",
                    if err.is_empty() {
                        "the host rejected the login".to_owned()
                    } else {
                        err
                    }
                )));
            }
            Some(login_response::Union::PeerInfo(peer)) => peer,
            None | Some(_) => return None,
        },
        Some(message::Union::PeerInfo(peer)) => peer,
        _ => return None,
    };
    Some(Ok(LoginResult {
        username: username.to_owned(),
        peer,
        encrypted,
    }))
}

pub fn login_options(codecs: &CodecCaps) -> OptionMessage {
    let caps = codecs.selected();
    let mut decoding = SupportedDecoding::new();
    decoding.ability_vp8 = i32::from(caps.vp8);
    decoding.ability_vp9 = i32::from(caps.vp9);
    decoding.ability_av1 = i32::from(caps.av1);
    decoding.ability_h264 = i32::from(caps.h264);
    decoding.ability_h265 = i32::from(caps.h265);
    decoding.prefer = PreferCodec::Auto.into();
    decoding.prefer_chroma = Chroma::I420.into();
    OptionMessage {
        supported_decoding: MessageField::some(decoding),
        custom_fps: 30,
        image_quality: ImageQuality::Balanced.into(),
        disable_audio: BoolOption::Yes.into(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::config::RELAY_PORT;

    #[test]
    fn direct_port_matches_the_host_listener() {
        assert_eq!(DEFAULT_DIRECT_PORT as i32, RELAY_PORT + 1);
        assert_eq!(DEFAULT_DIRECT_PORT as i32, RENDEZVOUS_PORT + 2);
    }

    #[test]
    fn login_advertises_only_probed_codecs_and_i420() {
        let opt = login_options(&CodecCaps {
            vp8: true,
            h264: true,
            reported: true,
            ..CodecCaps::default()
        });
        let decoding = opt.supported_decoding.as_ref().unwrap();
        assert_eq!(decoding.ability_vp8, 1);
        assert_eq!(decoding.ability_h264, 1);
        assert_eq!(decoding.ability_vp9, 0);
        assert_eq!(decoding.ability_av1, 0);
        assert_eq!(decoding.ability_h265, 0);
        assert!(decoding.i444.is_none());
        assert_eq!(decoding.prefer_chroma.enum_value(), Ok(Chroma::I420));
    }
}
