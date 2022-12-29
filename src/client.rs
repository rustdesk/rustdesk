pub use async_trait::async_trait;
#[cfg(not(any(target_os = "android", target_os = "linux")))]
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Host, StreamConfig,
};
#[cfg(not(features = "cli"))]
use magnum_opus::{Channels::*, Decoder as AudioDecoder};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    net::SocketAddr,
    ops::{Deref, Not},
    sync::{atomic::AtomicBool, mpsc, Arc, Mutex, RwLock},
};
use uuid::Uuid;

pub use file_trait::FileManager;
use hbb_common::{
    allow_err,
    anyhow::{anyhow, Context},
    bail,
    config::{
        Config, PeerConfig, PeerInfoSerde, CONNECT_TIMEOUT, READ_TIMEOUT, RELAY_PORT,
        RENDEZVOUS_TIMEOUT,
    },
    log,
    message_proto::{option_message::BoolOption, *},
    protobuf::Message as _,
    rand,
    rendezvous_proto::*,
    socket_client,
    sodiumoxide::crypto::{box_, secretbox, sign},
    timeout,
    tokio::time::Duration,
    AddrMangle, ResultType, Stream,
};
pub use helper::LatencyController;
pub use helper::*;
#[cfg(not(features = "cli"))]
use scrap::{
    codec::{Decoder, DecoderCfg},
    record::{Recorder, RecorderContext},
    VpxDecoderConfig, VpxVideoCodecId,
};

pub use super::lang::*;

pub mod file_trait;
pub mod helper;
pub mod io_loop;
use crate::video_service::{SCRAP_X11_REQUIRED, SCRAP_X11_REF_URL};
pub static SERVER_KEYBOARD_ENABLED: AtomicBool = AtomicBool::new(true);
pub static SERVER_FILE_TRANSFER_ENABLED: AtomicBool = AtomicBool::new(true);
pub static SERVER_CLIPBOARD_ENABLED: AtomicBool = AtomicBool::new(true);
pub const MILLI1: Duration = Duration::from_millis(1);
pub const SEC30: Duration = Duration::from_secs(30);

/// Client of the remote desktop.
pub struct Client;

#[cfg(not(any(target_os = "android", target_os = "linux")))]
lazy_static::lazy_static! {
    static ref AUDIO_HOST: Host = cpal::default_host();
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
lazy_static::lazy_static! {
    static ref ENIGO: Arc<Mutex<enigo::Enigo>> = Arc::new(Mutex::new(enigo::Enigo::new()));
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_key_state(key: enigo::Key) -> bool {
    use enigo::KeyboardControllable;
    #[cfg(target_os = "macos")]
    if key == enigo::Key::NumLock {
        return true;
    }
    ENIGO.lock().unwrap().get_key_state(key)
}

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {

use libc::{c_float, c_int, c_void};
type Oboe = *mut c_void;
extern "C" {
    fn create_oboe_player(channels: c_int, sample_rate: c_int) -> Oboe;
    fn push_oboe_data(oboe: Oboe, d: *const c_float, n: c_int);
    fn destroy_oboe_player(oboe: Oboe);
}

struct OboePlayer {
    raw: Oboe,
}

impl Default for OboePlayer {
    fn default() -> Self {
        Self {
            raw: std::ptr::null_mut(),
        }
    }
}

impl OboePlayer {
    fn new(channels: i32, sample_rate: i32) -> Self {
        unsafe {
            Self {
                raw: create_oboe_player(channels, sample_rate),
            }
        }
    }

    fn is_null(&self) -> bool {
        self.raw.is_null()
    }

    fn push(&mut self, d: &[f32]) {
        if self.raw.is_null() {
            return;
        }
        unsafe {
            push_oboe_data(self.raw, d.as_ptr(), d.len() as _);
        }
    }
}

impl Drop for OboePlayer {
    fn drop(&mut self) {
        unsafe {
            if !self.raw.is_null() {
                destroy_oboe_player(self.raw);
            }
        }
    }
}

}
}

impl Client {
    /// Start a new connection.
    pub async fn start(
        peer: &str,
        key: &str,
        token: &str,
        conn_type: ConnType,
        interface: impl Interface,
    ) -> ResultType<(Stream, bool)> {
        match Self::_start(peer, key, token, conn_type, interface).await {
            Err(err) => {
                let err_str = err.to_string();
                if err_str.starts_with("Failed") {
                    bail!(err_str + ": Please try later");
                } else {
                    return Err(err);
                }
            }
            Ok(x) => Ok(x),
        }
    }

    /// Start a new connection.
    async fn _start(
        peer: &str,
        key: &str,
        token: &str,
        conn_type: ConnType,
        interface: impl Interface,
    ) -> ResultType<(Stream, bool)> {
        // to-do: remember the port for each peer, so that we can retry easier
        if hbb_common::is_ipv4_str(peer) {
            return Ok((
                socket_client::connect_tcp(
                    crate::check_port(peer, RELAY_PORT + 1),
                    RENDEZVOUS_TIMEOUT,
                )
                .await?,
                true,
            ));
        }
        let (mut rendezvous_server, servers, contained) = crate::get_rendezvous_server(1_000).await;
        let mut socket = socket_client::connect_tcp(&*rendezvous_server, RENDEZVOUS_TIMEOUT).await;
        debug_assert!(!servers.contains(&rendezvous_server));
        if socket.is_err() && !servers.is_empty() {
            log::info!("try the other servers: {:?}", servers);
            for server in servers {
                socket = socket_client::connect_tcp(&*server, RENDEZVOUS_TIMEOUT).await;
                if socket.is_ok() {
                    rendezvous_server = server;
                    break;
                }
            }
            crate::refresh_rendezvous_server();
        } else if !contained {
            crate::refresh_rendezvous_server();
        }
        log::info!("rendezvous server: {}", rendezvous_server);
        let mut socket = socket?;
        let my_addr = socket.local_addr();
        let mut signed_id_pk = Vec::new();
        let mut relay_server = "".to_owned();

        let start = std::time::Instant::now();
        let mut peer_addr = Config::get_any_listen_addr(true);
        let mut peer_nat_type = NatType::UNKNOWN_NAT;
        let my_nat_type = crate::get_nat_type(100).await;
        let mut is_local = false;
        for i in 1..=3 {
            log::info!("#{} punch attempt with {}, id: {}", i, my_addr, peer);
            let mut msg_out = RendezvousMessage::new();
            use hbb_common::protobuf::Enum;
            let nat_type = if interface.is_force_relay() {
                NatType::SYMMETRIC
            } else {
                NatType::from_i32(my_nat_type).unwrap_or(NatType::UNKNOWN_NAT)
            };
            msg_out.set_punch_hole_request(PunchHoleRequest {
                id: peer.to_owned(),
                token: token.to_owned(),
                nat_type: nat_type.into(),
                licence_key: key.to_owned(),
                conn_type: conn_type.into(),
                ..Default::default()
            });
            socket.send(&msg_out).await?;
            if let Some(Ok(bytes)) = socket.next_timeout(i * 6000).await {
                if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                    match msg_in.union {
                        Some(rendezvous_message::Union::PunchHoleResponse(ph)) => {
                            if ph.socket_addr.is_empty() {
                                if !ph.other_failure.is_empty() {
                                    bail!(ph.other_failure);
                                }
                                match ph.failure.enum_value_or_default() {
                                    punch_hole_response::Failure::ID_NOT_EXIST => {
                                        bail!("ID does not exist");
                                    }
                                    punch_hole_response::Failure::OFFLINE => {
                                        bail!("Remote desktop is offline");
                                    }
                                    punch_hole_response::Failure::LICENSE_MISMATCH => {
                                        bail!("Key mismatch");
                                    }
                                    punch_hole_response::Failure::LICENSE_OVERUSE => {
                                        bail!("Key overuse");
                                    }
                                }
                            } else {
                                peer_nat_type = ph.nat_type();
                                is_local = ph.is_local();
                                signed_id_pk = ph.pk.into();
                                relay_server = ph.relay_server;
                                peer_addr = AddrMangle::decode(&ph.socket_addr);
                                log::info!("Hole Punched {} = {}", peer, peer_addr);
                                break;
                            }
                        }
                        Some(rendezvous_message::Union::RelayResponse(rr)) => {
                            log::info!(
                                "relay requested from peer, time used: {:?}, relay_server: {}",
                                start.elapsed(),
                                rr.relay_server
                            );
                            signed_id_pk = rr.pk().into();
                            let mut conn = Self::create_relay(
                                peer,
                                rr.uuid,
                                rr.relay_server,
                                key,
                                conn_type,
                                my_addr.is_ipv4(),
                            )
                            .await?;
                            Self::secure_connection(
                                peer,
                                signed_id_pk,
                                key,
                                &mut conn,
                                false,
                                interface,
                            )
                            .await?;
                            return Ok((conn, false));
                        }
                        _ => {
                            log::error!("Unexpected protobuf msg received: {:?}", msg_in);
                        }
                    }
                } else {
                    log::error!("Non-protobuf message bytes received: {:?}", bytes);
                }
            }
        }
        drop(socket);
        if peer_addr.port() == 0 {
            bail!("Failed to connect via rendezvous server");
        }
        let time_used = start.elapsed().as_millis() as u64;
        log::info!(
            "{} ms used to punch hole, relay_server: {}, {}",
            time_used,
            relay_server,
            if is_local {
                "is_local: true".to_owned()
            } else {
                format!("nat_type: {:?}", peer_nat_type)
            }
        );
        Self::connect(
            my_addr,
            peer_addr,
            peer,
            signed_id_pk,
            &relay_server,
            &rendezvous_server,
            time_used,
            peer_nat_type,
            my_nat_type,
            is_local,
            key,
            token,
            conn_type,
            interface,
        )
        .await
    }

    /// Connect to the peer.
    async fn connect(
        local_addr: SocketAddr,
        peer: SocketAddr,
        peer_id: &str,
        signed_id_pk: Vec<u8>,
        relay_server: &str,
        rendezvous_server: &str,
        punch_time_used: u64,
        peer_nat_type: NatType,
        my_nat_type: i32,
        is_local: bool,
        key: &str,
        token: &str,
        conn_type: ConnType,
        interface: impl Interface,
    ) -> ResultType<(Stream, bool)> {
        let direct_failures = PeerConfig::load(peer_id).direct_failures;
        let mut connect_timeout = 0;
        const MIN: u64 = 1000;
        if is_local || peer_nat_type == NatType::SYMMETRIC {
            connect_timeout = MIN;
        } else {
            if relay_server.is_empty() {
                connect_timeout = CONNECT_TIMEOUT;
            } else {
                if peer_nat_type == NatType::ASYMMETRIC {
                    let mut my_nat_type = my_nat_type;
                    if my_nat_type == NatType::UNKNOWN_NAT as i32 {
                        my_nat_type = crate::get_nat_type(100).await;
                    }
                    if my_nat_type == NatType::ASYMMETRIC as i32 {
                        connect_timeout = CONNECT_TIMEOUT;
                        if direct_failures > 0 {
                            connect_timeout = punch_time_used * 6;
                        }
                    } else if my_nat_type == NatType::SYMMETRIC as i32 {
                        connect_timeout = MIN;
                    }
                }
                if connect_timeout == 0 {
                    let n = if direct_failures > 0 { 3 } else { 6 };
                    connect_timeout = punch_time_used * (n as u64);
                }
            }
            if connect_timeout < MIN {
                connect_timeout = MIN;
            }
        }
        log::info!("peer address: {}, timeout: {}", peer, connect_timeout);
        let start = std::time::Instant::now();
        // NOTICE: Socks5 is be used event in intranet. Which may be not a good way.
        let mut conn = socket_client::connect_tcp_local(peer, local_addr, connect_timeout).await;
        let mut direct = !conn.is_err();
        if interface.is_force_relay() || conn.is_err() {
            if !relay_server.is_empty() {
                conn = Self::request_relay(
                    peer_id,
                    relay_server.to_owned(),
                    rendezvous_server,
                    !signed_id_pk.is_empty(),
                    key,
                    token,
                    conn_type,
                )
                .await;
                if conn.is_err() {
                    bail!(
                        "Failed to connect via relay server: {}",
                        conn.err().unwrap()
                    );
                }
                direct = false;
            } else {
                bail!("Failed to make direct connection to remote desktop");
            }
        }
        if !relay_server.is_empty() && (direct_failures == 0) != direct {
            let mut config = PeerConfig::load(peer_id);
            config.direct_failures = if direct { 0 } else { 1 };
            log::info!("direct_failures updated to {}", config.direct_failures);
            config.store(peer_id);
        }
        let mut conn = conn?;
        log::info!("{:?} used to establish connection", start.elapsed());
        Self::secure_connection(peer_id, signed_id_pk, key, &mut conn, direct, interface).await?;
        Ok((conn, direct))
    }

    /// Establish secure connection with the server.
    async fn secure_connection(
        peer_id: &str,
        signed_id_pk: Vec<u8>,
        key: &str,
        conn: &mut Stream,
        direct: bool,
        interface: impl Interface,
    ) -> ResultType<()> {
        let rs_pk = get_rs_pk(if key.is_empty() {
            hbb_common::config::RS_PUB_KEY
        } else {
            key
        });
        let mut sign_pk = None;
        if !signed_id_pk.is_empty() && rs_pk.is_some() {
            if let Ok((id, pk)) = decode_id_pk(&signed_id_pk, &rs_pk.unwrap()) {
                if id == peer_id {
                    sign_pk = Some(sign::PublicKey(pk));
                }
            }
            if sign_pk.is_none() {
                log::error!("Handshake failed: invalid public key from rendezvous server");
            }
        }
        let sign_pk = match sign_pk {
            Some(v) => v,
            None => {
                // send an empty message out in case server is setting up secure and waiting for first message
                conn.send(&Message::new()).await?;
                return Ok(());
            }
        };
        match timeout(READ_TIMEOUT, conn.next()).await? {
            Some(res) => {
                let bytes = match res {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        interface.set_force_relay(direct, false);
                        bail!("{}", err);
                    }
                };
                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                    if let Some(message::Union::SignedId(si)) = msg_in.union {
                        if let Ok((id, their_pk_b)) = decode_id_pk(&si.id, &sign_pk) {
                            if id == peer_id {
                                let their_pk_b = box_::PublicKey(their_pk_b);
                                let (our_pk_b, out_sk_b) = box_::gen_keypair();
                                let key = secretbox::gen_key();
                                let nonce = box_::Nonce([0u8; box_::NONCEBYTES]);
                                let sealed_key = box_::seal(&key.0, &nonce, &their_pk_b, &out_sk_b);
                                let mut msg_out = Message::new();
                                msg_out.set_public_key(PublicKey {
                                    asymmetric_value: Vec::from(our_pk_b.0).into(),
                                    symmetric_value: sealed_key.into(),
                                    ..Default::default()
                                });
                                timeout(CONNECT_TIMEOUT, conn.send(&msg_out)).await??;
                                conn.set_key(key);
                            } else {
                                log::error!("Handshake failed: sign failure");
                                conn.send(&Message::new()).await?;
                            }
                        } else {
                            // fall back to non-secure connection in case pk mismatch
                            log::info!("pk mismatch, fall back to non-secure");
                            let mut msg_out = Message::new();
                            msg_out.set_public_key(PublicKey::new());
                            conn.send(&msg_out).await?;
                        }
                    } else {
                        log::error!("Handshake failed: invalid message type");
                        conn.send(&Message::new()).await?;
                    }
                } else {
                    log::error!("Handshake failed: invalid message format");
                    conn.send(&Message::new()).await?;
                }
            }
            None => {
                bail!("Reset by the peer");
            }
        }
        Ok(())
    }

    /// Request a relay connection to the server.
    async fn request_relay(
        peer: &str,
        relay_server: String,
        rendezvous_server: &str,
        secure: bool,
        key: &str,
        token: &str,
        conn_type: ConnType,
    ) -> ResultType<Stream> {
        let mut succeed = false;
        let mut uuid = "".to_owned();
        let mut ipv4 = true;
        for i in 1..=3 {
            // use different socket due to current hbbs implement requiring different nat address for each attempt
            let mut socket = socket_client::connect_tcp(rendezvous_server, RENDEZVOUS_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;

            ipv4 = socket.local_addr().is_ipv4();
            let mut msg_out = RendezvousMessage::new();
            uuid = Uuid::new_v4().to_string();
            log::info!(
                "#{} request relay attempt, id: {}, uuid: {}, relay_server: {}, secure: {}",
                i,
                peer,
                uuid,
                relay_server,
                secure,
            );
            msg_out.set_request_relay(RequestRelay {
                id: peer.to_owned(),
                token: token.to_owned(),
                uuid: uuid.clone(),
                relay_server: relay_server.clone(),
                secure,
                ..Default::default()
            });
            socket.send(&msg_out).await?;
            if let Some(Ok(bytes)) = socket.next_timeout(CONNECT_TIMEOUT).await {
                if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                    if let Some(rendezvous_message::Union::RelayResponse(rs)) = msg_in.union {
                        if !rs.refuse_reason.is_empty() {
                            bail!(rs.refuse_reason);
                        }
                        succeed = true;
                        break;
                    }
                }
            }
        }
        if !succeed {
            bail!("Timeout");
        }
        Self::create_relay(peer, uuid, relay_server, key, conn_type, ipv4).await
    }

    /// Create a relay connection to the server.
    async fn create_relay(
        peer: &str,
        uuid: String,
        relay_server: String,
        key: &str,
        conn_type: ConnType,
        ipv4: bool,
    ) -> ResultType<Stream> {
        let mut conn = socket_client::connect_tcp(
            socket_client::ipv4_to_ipv6(crate::check_port(relay_server, RELAY_PORT), ipv4),
            CONNECT_TIMEOUT,
        )
        .await
        .with_context(|| "Failed to connect to relay server")?;
        let mut msg_out = RendezvousMessage::new();
        msg_out.set_request_relay(RequestRelay {
            licence_key: key.to_owned(),
            id: peer.to_owned(),
            uuid,
            conn_type: conn_type.into(),
            ..Default::default()
        });
        conn.send(&msg_out).await?;
        Ok(conn)
    }
}

/// Audio handler for the [`Client`].
#[cfg(not(features = "cli"))]
#[derive(Default)]
pub struct AudioHandler {
    audio_decoder: Option<(AudioDecoder, Vec<f32>)>,
    #[cfg(target_os = "android")]
    oboe: Option<OboePlayer>,
    #[cfg(target_os = "linux")]
    simple: Option<psimple::Simple>,
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    audio_buffer: Arc<std::sync::Mutex<std::collections::vec_deque::VecDeque<f32>>>,
    sample_rate: (u32, u32),
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    audio_stream: Option<Box<dyn StreamTrait>>,
    channels: u16,
    latency_controller: Arc<Mutex<LatencyController>>,
}

#[cfg(not(features = "cli"))]
impl AudioHandler {
    /// Create a new audio handler.
    pub fn new(latency_controller: Arc<Mutex<LatencyController>>) -> Self {
        AudioHandler {
            latency_controller,
            ..Default::default()
        }
    }

    /// Start the audio playback.
    #[cfg(target_os = "linux")]
    fn start_audio(&mut self, format0: AudioFormat) -> ResultType<()> {
        use psimple::Simple;
        use pulse::sample::{Format, Spec};
        use pulse::stream::Direction;

        let spec = Spec {
            format: Format::F32le,
            channels: format0.channels as _,
            rate: format0.sample_rate as _,
        };
        if !spec.is_valid() {
            bail!("Invalid audio format");
        }

        self.simple = Some(Simple::new(
            None,                   // Use the default server
            &crate::get_app_name(), // Our application’s name
            Direction::Playback,    // We want a playback stream
            None,                   // Use the default device
            "playback",             // Description of our stream
            &spec,                  // Our sample format
            None,                   // Use default channel map
            None,                   // Use default buffering attributes
        )?);
        self.sample_rate = (format0.sample_rate, format0.sample_rate);
        Ok(())
    }

    /// Start the audio playback.
    #[cfg(target_os = "android")]
    fn start_audio(&mut self, format0: AudioFormat) -> ResultType<()> {
        self.oboe = Some(OboePlayer::new(
            format0.channels as _,
            format0.sample_rate as _,
        ));
        self.sample_rate = (format0.sample_rate, format0.sample_rate);
        Ok(())
    }

    /// Start the audio playback.
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    fn start_audio(&mut self, format0: AudioFormat) -> ResultType<()> {
        let device = AUDIO_HOST
            .default_output_device()
            .with_context(|| "Failed to get default output device")?;
        log::info!(
            "Using default output device: \"{}\"",
            device.name().unwrap_or("".to_owned())
        );
        let config = device.default_output_config().map_err(|e| anyhow!(e))?;
        let sample_format = config.sample_format();
        log::info!("Default output format: {:?}", config);
        log::info!("Remote input format: {:?}", format0);
        let mut config: StreamConfig = config.into();
        config.channels = format0.channels as _;
        match sample_format {
            cpal::SampleFormat::F32 => self.build_output_stream::<f32>(&config, &device)?,
            cpal::SampleFormat::I16 => self.build_output_stream::<i16>(&config, &device)?,
            cpal::SampleFormat::U16 => self.build_output_stream::<u16>(&config, &device)?,
        }
        self.sample_rate = (format0.sample_rate, config.sample_rate.0);
        Ok(())
    }

    /// Handle audio format and create an audio decoder.
    pub fn handle_format(&mut self, f: AudioFormat) {
        match AudioDecoder::new(f.sample_rate, if f.channels > 1 { Stereo } else { Mono }) {
            Ok(d) => {
                let buffer = vec![0.; f.sample_rate as usize * f.channels as usize];
                self.audio_decoder = Some((d, buffer));
                self.channels = f.channels as _;
                allow_err!(self.start_audio(f));
            }
            Err(err) => {
                log::error!("Failed to create audio decoder: {}", err);
            }
        }
    }

    /// Handle audio frame and play it.
    pub fn handle_frame(&mut self, frame: AudioFrame) {
        if frame.timestamp != 0 {
            if self
                .latency_controller
                .lock()
                .unwrap()
                .check_audio(frame.timestamp)
                .not()
            {
                return;
            }
        }

        #[cfg(not(any(target_os = "android", target_os = "linux")))]
        if self.audio_stream.is_none() {
            return;
        }
        #[cfg(target_os = "linux")]
        if self.simple.is_none() {
            return;
        }
        #[cfg(target_os = "android")]
        if self.oboe.is_none() {
            return;
        }
        self.audio_decoder.as_mut().map(|(d, buffer)| {
            if let Ok(n) = d.decode_float(&frame.data, buffer, false) {
                let channels = self.channels;
                let n = n * (channels as usize);
                #[cfg(not(any(target_os = "android", target_os = "linux")))]
                {
                    let sample_rate0 = self.sample_rate.0;
                    let sample_rate = self.sample_rate.1;
                    let audio_buffer = self.audio_buffer.clone();
                    // avoiding memory overflow if audio_buffer consumer side has problem
                    if audio_buffer.lock().unwrap().len() as u32 > sample_rate * 120 {
                        *audio_buffer.lock().unwrap() = Default::default();
                    }
                    if sample_rate != sample_rate0 {
                        let buffer = crate::resample_channels(
                            &buffer[0..n],
                            sample_rate0,
                            sample_rate,
                            channels,
                        );
                        audio_buffer.lock().unwrap().extend(buffer);
                    } else {
                        audio_buffer
                            .lock()
                            .unwrap()
                            .extend(buffer[0..n].iter().cloned());
                    }
                }
                #[cfg(target_os = "android")]
                {
                    self.oboe.as_mut().map(|x| x.push(&buffer[0..n]));
                }
                #[cfg(target_os = "linux")]
                {
                    let data_u8 =
                        unsafe { std::slice::from_raw_parts::<u8>(buffer.as_ptr() as _, n * 4) };
                    self.simple.as_mut().map(|x| x.write(data_u8));
                }
            }
        });
    }

    /// Build audio output stream for current device.
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    fn build_output_stream<T: cpal::Sample>(
        &mut self,
        config: &StreamConfig,
        device: &Device,
    ) -> ResultType<()> {
        let err_fn = move |err| {
            // too many errors, will improve later
            log::trace!("an error occurred on stream: {}", err);
        };
        let audio_buffer = self.audio_buffer.clone();
        let stream = device.build_output_stream(
            config,
            move |data: &mut [T], _: &_| {
                let mut lock = audio_buffer.lock().unwrap();
                let mut n = data.len();
                if lock.len() < n {
                    n = lock.len();
                }
                let mut input = lock.drain(0..n);
                for sample in data.iter_mut() {
                    *sample = match input.next() {
                        Some(x) => T::from(&x),
                        _ => T::from(&0.),
                    };
                }
            },
            err_fn,
        )?;
        stream.play()?;
        self.audio_stream = Some(Box::new(stream));
        Ok(())
    }
}

/// Video handler for the [`Client`].
#[cfg(not(features = "cli"))]
pub struct VideoHandler {
    decoder: Decoder,
    latency_controller: Arc<Mutex<LatencyController>>,
    pub rgb: Vec<u8>,
    recorder: Arc<Mutex<Option<Recorder>>>,
    record: bool,
}

#[cfg(not(features = "cli"))]
impl VideoHandler {
    /// Create a new video handler.
    pub fn new(latency_controller: Arc<Mutex<LatencyController>>) -> Self {
        VideoHandler {
            decoder: Decoder::new(DecoderCfg {
                vpx: VpxDecoderConfig {
                    codec: VpxVideoCodecId::VP9,
                    num_threads: (num_cpus::get() / 2) as _,
                },
            }),
            latency_controller,
            rgb: Default::default(),
            recorder: Default::default(),
            record: false,
        }
    }

    /// Handle a new video frame.
    pub fn handle_frame(&mut self, vf: VideoFrame) -> ResultType<bool> {
        if vf.timestamp != 0 {
            // Update the lantency controller with the latest timestamp.
            self.latency_controller
                .lock()
                .unwrap()
                .update_video(vf.timestamp);
        }
        match &vf.union {
            Some(frame) => {
                let res = self.decoder.handle_video_frame(frame, &mut self.rgb);
                if self.record {
                    self.recorder
                        .lock()
                        .unwrap()
                        .as_mut()
                        .map(|r| r.write_frame(frame));
                }
                res
            }
            _ => Ok(false),
        }
    }

    /// Reset the decoder.
    pub fn reset(&mut self) {
        self.decoder = Decoder::new(DecoderCfg {
            vpx: VpxDecoderConfig {
                codec: VpxVideoCodecId::VP9,
                num_threads: 1,
            },
        });
    }

    /// Start or stop screen record.
    pub fn record_screen(&mut self, start: bool, w: i32, h: i32, id: String) {
        self.record = false;
        if start {
            self.recorder = Recorder::new(RecorderContext {
                server: false,
                id,
                default_dir: crate::ui_interface::default_video_save_directory(),
                filename: "".to_owned(),
                width: w as _,
                height: h as _,
                codec_id: scrap::record::RecordCodecID::VP9,
                tx: None,
            })
            .map_or(Default::default(), |r| Arc::new(Mutex::new(Some(r))));
        } else {
            self.recorder = Default::default();
        }
        self.record = start;
    }
}

/// Login config handler for [`Client`].
#[derive(Default)]
pub struct LoginConfigHandler {
    id: String,
    pub conn_type: ConnType,
    hash: Hash,
    password: Vec<u8>, // remember password for reconnect
    pub remember: bool,
    config: PeerConfig,
    pub port_forward: (String, i32),
    pub version: i64,
    pub conn_id: i32,
    features: Option<Features>,
    session_id: u64,
    pub supported_encoding: Option<(bool, bool)>,
    pub restarting_remote_device: bool,
    pub force_relay: bool,
    pub direct: Option<bool>,
    pub received: bool,
}

impl Deref for LoginConfigHandler {
    type Target = PeerConfig;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

/// Load [`PeerConfig`] from id.
///
/// # Arguments
///
/// * `id` - id of peer
#[inline]
pub fn load_config(id: &str) -> PeerConfig {
    PeerConfig::load(id)
}

impl LoginConfigHandler {
    /// Initialize the login config handler.
    ///
    /// # Arguments
    ///
    /// * `id` - id of peer
    /// * `conn_type` - Connection type enum.
    pub fn initialize(&mut self, id: String, conn_type: ConnType) {
        self.id = id;
        self.conn_type = conn_type;
        let config = self.load_config();
        self.remember = !config.password.is_empty();
        self.config = config;
        self.session_id = rand::random();
        self.supported_encoding = None;
        self.restarting_remote_device = false;
        self.force_relay = !self.get_option("force-always-relay").is_empty();
        self.direct = None;
        self.received = false;
    }

    /// Check if the client should auto login.
    /// Return password if the client should auto login, otherwise return empty string.
    pub fn should_auto_login(&self) -> String {
        let l = self.lock_after_session_end;
        let a = !self.get_option("auto-login").is_empty();
        let p = self.get_option("os-password");
        if !p.is_empty() && l && a {
            p
        } else {
            "".to_owned()
        }
    }

    /// Load [`PeerConfig`].
    fn load_config(&self) -> PeerConfig {
        load_config(&self.id)
    }

    /// Save a [`PeerConfig`] into the handler.
    ///
    /// # Arguments
    ///
    /// * `config` - [`PeerConfig`] to save.
    pub fn save_config(&mut self, config: PeerConfig) {
        config.store(&self.id);
        self.config = config;
    }

    /// Set an option for handler's [`PeerConfig`].
    ///
    /// # Arguments
    ///
    /// * `k` - key of option
    /// * `v` - value of option
    pub fn set_option(&mut self, k: String, v: String) {
        let mut config = self.load_config();
        config.options.insert(k, v);
        self.save_config(config);
    }

    //to-do: too many dup code below.

    /// Save view style to the current config.
    ///
    /// # Arguments
    ///
    /// * `value` - The view style to be saved.
    pub fn save_view_style(&mut self, value: String) {
        let mut config = self.load_config();
        config.view_style = value;
        self.save_config(config);
    }

    /// Save keyboard mode to the current config.
    ///
    /// # Arguments
    ///
    /// * `value` - The view style to be saved.
    pub fn save_keyboard_mode(&mut self, value: String) {
        let mut config = self.load_config();
        config.keyboard_mode = value;
        self.save_config(config);
    }

    /// Save scroll style to the current config.
    ///
    /// # Arguments
    ///
    /// * `value` - The view style to be saved.
    pub fn save_scroll_style(&mut self, value: String) {
        let mut config = self.load_config();
        config.scroll_style = value;
        self.save_config(config);
    }

    /// Set a ui config of flutter for handler's [`PeerConfig`].
    ///
    /// # Arguments
    ///
    /// * `k` - key of option
    /// * `v` - value of option
    pub fn save_ui_flutter(&mut self, k: String, v: String) {
        let mut config = self.load_config();
        config.ui_flutter.insert(k, v);
        self.save_config(config);
    }

    /// Get a ui config of flutter for handler's [`PeerConfig`].
    /// Return String if the option is found, otherwise return "".
    ///
    /// # Arguments
    ///
    /// * `k` - key of option
    pub fn get_ui_flutter(&self, k: &str) -> String {
        if let Some(v) = self.config.ui_flutter.get(k) {
            v.clone()
        } else {
            "".to_owned()
        }
    }

    /// Toggle an option in the handler.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the option to toggle.
    pub fn toggle_option(&mut self, name: String) -> Option<Message> {
        let mut option = OptionMessage::default();
        let mut config = self.load_config();
        if name == "show-remote-cursor" {
            config.show_remote_cursor = !config.show_remote_cursor;
            option.show_remote_cursor = (if config.show_remote_cursor {
                BoolOption::Yes
            } else {
                BoolOption::No
            })
            .into();
        } else if name == "disable-audio" {
            config.disable_audio = !config.disable_audio;
            option.disable_audio = (if config.disable_audio {
                BoolOption::Yes
            } else {
                BoolOption::No
            })
            .into();
        } else if name == "disable-clipboard" {
            config.disable_clipboard = !config.disable_clipboard;
            option.disable_clipboard = (if config.disable_clipboard {
                BoolOption::Yes
            } else {
                BoolOption::No
            })
            .into();
        } else if name == "lock-after-session-end" {
            config.lock_after_session_end = !config.lock_after_session_end;
            option.lock_after_session_end = (if config.lock_after_session_end {
                BoolOption::Yes
            } else {
                BoolOption::No
            })
            .into();
        } else if name == "privacy-mode" {
            // try toggle privacy mode
            option.privacy_mode = (if config.privacy_mode {
                BoolOption::No
            } else {
                BoolOption::Yes
            })
            .into();
        } else if name == "enable-file-transfer" {
            config.enable_file_transfer = !config.enable_file_transfer;
            option.enable_file_transfer = (if config.enable_file_transfer {
                BoolOption::Yes
            } else {
                BoolOption::No
            })
            .into();
        } else if name == "block-input" {
            option.block_input = BoolOption::Yes.into();
        } else if name == "unblock-input" {
            option.block_input = BoolOption::No.into();
        } else if name == "show-quality-monitor" {
            config.show_quality_monitor = !config.show_quality_monitor;
        } else {
            let v = self.options.get(&name).is_some();
            if v {
                self.config.options.remove(&name);
            } else {
                self.config.options.insert(name, "Y".to_owned());
            }
            self.config.store(&self.id);
            return None;
        }
        if !name.contains("block-input") {
            self.save_config(config);
        }
        let mut misc = Misc::new();
        misc.set_option(option);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        Some(msg_out)
    }

    /// Get [`PeerConfig`] of the current [`LoginConfigHandler`].
    ///
    /// # Arguments
    pub fn get_config(&mut self) -> &mut PeerConfig {
        &mut self.config
    }

    /// Get [`OptionMessage`] of the current [`LoginConfigHandler`].
    /// Return `None` if there's no option, for example, when the session is only for file transfer.
    ///
    /// # Arguments
    ///
    /// * `ignore_default` - If `true`, ignore the default value of the option.
    fn get_option_message(&self, ignore_default: bool) -> Option<OptionMessage> {
        if self.conn_type.eq(&ConnType::FILE_TRANSFER) || self.conn_type.eq(&ConnType::PORT_FORWARD)
        {
            return None;
        }
        let mut n = 0;
        let mut msg = OptionMessage::new();
        let q = self.image_quality.clone();
        if let Some(q) = self.get_image_quality_enum(&q, ignore_default) {
            msg.image_quality = q.into();
            n += 1;
        } else if q == "custom" {
            let config = PeerConfig::load(&self.id);
            let quality = if config.custom_image_quality.is_empty() {
                50
            } else {
                config.custom_image_quality[0]
            };
            msg.custom_image_quality = quality << 8;
            n += 1;
        }
        if let Some(custom_fps) = self.options.get("custom-fps") {
            msg.custom_fps = custom_fps.parse().unwrap_or(30);
        }
        if self.get_toggle_option("show-remote-cursor") {
            msg.show_remote_cursor = BoolOption::Yes.into();
            n += 1;
        }
        if self.get_toggle_option("lock-after-session-end") {
            msg.lock_after_session_end = BoolOption::Yes.into();
            n += 1;
        }
        if self.get_toggle_option("disable-audio") {
            msg.disable_audio = BoolOption::Yes.into();
            n += 1;
        }
        if self.get_toggle_option("enable-file-transfer") {
            msg.enable_file_transfer = BoolOption::Yes.into();
            n += 1;
        }
        if self.get_toggle_option("disable-clipboard") {
            msg.disable_clipboard = BoolOption::Yes.into();
            n += 1;
        }
        let state = Decoder::video_codec_state(&self.id);
        msg.video_codec_state = hbb_common::protobuf::MessageField::some(state);
        n += 1;

        if n > 0 {
            Some(msg)
        } else {
            None
        }
    }

    pub fn get_option_message_after_login(&self) -> Option<OptionMessage> {
        if self.conn_type.eq(&ConnType::FILE_TRANSFER) || self.conn_type.eq(&ConnType::PORT_FORWARD)
        {
            return None;
        }
        let mut n = 0;
        let mut msg = OptionMessage::new();
        if self.get_toggle_option("privacy-mode") {
            msg.privacy_mode = BoolOption::Yes.into();
            n += 1;
        }
        if n > 0 {
            Some(msg)
        } else {
            None
        }
    }

    /// Parse the image quality option.
    /// Return [`ImageQuality`] if the option is valid, otherwise return `None`.
    ///
    /// # Arguments
    ///
    /// * `q` - The image quality option.
    /// * `ignore_default` - Ignore the default value.
    fn get_image_quality_enum(&self, q: &str, ignore_default: bool) -> Option<ImageQuality> {
        if q == "low" {
            Some(ImageQuality::Low)
        } else if q == "best" {
            Some(ImageQuality::Best)
        } else if q == "balanced" {
            if ignore_default {
                None
            } else {
                Some(ImageQuality::Balanced)
            }
        } else {
            None
        }
    }

    /// Get the status of a toggle option.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the toggle option.
    pub fn get_toggle_option(&self, name: &str) -> bool {
        if name == "show-remote-cursor" {
            self.config.show_remote_cursor
        } else if name == "lock-after-session-end" {
            self.config.lock_after_session_end
        } else if name == "privacy-mode" {
            self.config.privacy_mode
        } else if name == "enable-file-transfer" {
            self.config.enable_file_transfer
        } else if name == "disable-audio" {
            self.config.disable_audio
        } else if name == "disable-clipboard" {
            self.config.disable_clipboard
        } else if name == "show-quality-monitor" {
            self.config.show_quality_monitor
        } else {
            !self.get_option(name).is_empty()
        }
    }

    pub fn is_privacy_mode_supported(&self) -> bool {
        if let Some(features) = &self.features {
            features.privacy_mode
        } else {
            false
        }
    }

    /// Create a [`Message`] for refreshing video.
    pub fn refresh() -> Message {
        let mut misc = Misc::new();
        misc.set_refresh_video(true);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        msg_out
    }

    /// Create a [`Message`] for saving custom image quality.
    ///
    /// # Arguments
    ///
    /// * `bitrate` - The given bitrate.
    /// * `quantizer` - The given quantizer.
    pub fn save_custom_image_quality(&mut self, image_quality: i32) -> Message {
        let mut misc = Misc::new();
        misc.set_option(OptionMessage {
            custom_image_quality: image_quality << 8,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        let mut config = self.load_config();
        config.image_quality = "custom".to_owned();
        config.custom_image_quality = vec![image_quality as _];
        self.save_config(config);
        msg_out
    }

    /// Save the given image quality to the config.
    /// Return a [`Message`] that contains image quality, or `None` if the image quality is not valid.
    /// # Arguments
    ///
    /// * `value` - The image quality.
    pub fn save_image_quality(&mut self, value: String) -> Option<Message> {
        let mut res = None;
        if let Some(q) = self.get_image_quality_enum(&value, false) {
            let mut misc = Misc::new();
            misc.set_option(OptionMessage {
                image_quality: q.into(),
                ..Default::default()
            });
            let mut msg_out = Message::new();
            msg_out.set_misc(misc);
            res = Some(msg_out);
        }
        let mut config = self.load_config();
        config.image_quality = value;
        self.save_config(config);
        res
    }

    /// Create a [`Message`] for saving custom fps.
    ///
    /// # Arguments
    ///
    /// * `fps` - The given fps.
    pub fn set_custom_fps(&mut self, fps: i32) -> Message {
        let mut misc = Misc::new();
        misc.set_option(OptionMessage {
            custom_fps: fps,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        let mut config = self.load_config();
        config
            .options
            .insert("custom-fps".to_owned(), fps.to_string());
        self.save_config(config);
        msg_out
    }

    pub fn get_option(&self, k: &str) -> String {
        if let Some(v) = self.config.options.get(k) {
            v.clone()
        } else {
            "".to_owned()
        }
    }

    /// Get user name.
    /// Return the name of the given peer. If the peer has no name, return the name in the config.
    ///
    /// # Arguments
    ///
    /// * `pi` - peer info.
    pub fn get_username(&self, pi: &PeerInfo) -> String {
        return if pi.username.is_empty() {
            self.info.username.clone()
        } else {
            pi.username.clone()
        };
    }

    /// Handle peer info.
    ///
    /// # Arguments
    ///
    /// * `username` - The name of the peer.
    /// * `pi` - The peer info.
    pub fn handle_peer_info(&mut self, pi: &PeerInfo) {
        if !pi.version.is_empty() {
            self.version = hbb_common::get_version_number(&pi.version);
        }
        self.features = pi.features.clone().into_option();
        let serde = PeerInfoSerde {
            username: pi.username.clone(),
            hostname: pi.hostname.clone(),
            platform: pi.platform.clone(),
        };
        let mut config = self.load_config();
        config.info = serde;
        let password = self.password.clone();
        let password0 = config.password.clone();
        let remember = self.remember;
        if remember {
            if !password.is_empty() && password != password0 {
                config.password = password;
                log::debug!("remember password of {}", self.id);
            }
        } else {
            if !password0.is_empty() {
                config.password = Default::default();
                log::debug!("remove password of {}", self.id);
            }
        }
        if config.keyboard_mode == "" {
            if hbb_common::get_version_number(&pi.version) < hbb_common::get_version_number("1.2.0")
            {
                config.keyboard_mode = "legacy".to_string();
            } else {
                config.keyboard_mode = "map".to_string();
            }
        }
        self.conn_id = pi.conn_id;
        // no matter if change, for update file time
        self.save_config(config);
        #[cfg(any(feature = "hwcodec", feature = "mediacodec"))]
        {
            self.supported_encoding = Some((pi.encoding.h264, pi.encoding.h265));
        }
    }

    pub fn get_remote_dir(&self) -> String {
        serde_json::from_str::<HashMap<String, String>>(&self.get_option("remote_dir"))
            .unwrap_or_default()
            .remove(&self.info.username)
            .unwrap_or_default()
    }

    pub fn get_all_remote_dir(&self, path: String) -> String {
        let d = self.get_option("remote_dir");
        let user = self.info.username.clone();
        let mut x = serde_json::from_str::<HashMap<String, String>>(&d).unwrap_or_default();
        if path.is_empty() {
            x.remove(&user);
        } else {
            x.insert(user, path);
        }
        serde_json::to_string::<HashMap<String, String>>(&x).unwrap_or_default()
    }

    /// Create a [`Message`] for login.
    fn create_login_msg(&self, password: Vec<u8>) -> Message {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let my_id = Config::get_id_or(crate::common::DEVICE_ID.lock().unwrap().clone());
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let my_id = Config::get_id();
        let mut lr = LoginRequest {
            username: self.id.clone(),
            password: password.into(),
            my_id,
            my_name: crate::username(),
            option: self.get_option_message(true).into(),
            session_id: self.session_id,
            version: crate::VERSION.to_string(),
            ..Default::default()
        };
        match self.conn_type {
            ConnType::FILE_TRANSFER => lr.set_file_transfer(FileTransfer {
                dir: self.get_remote_dir(),
                show_hidden: !self.get_option("remote_show_hidden").is_empty(),
                ..Default::default()
            }),
            ConnType::PORT_FORWARD => lr.set_port_forward(PortForward {
                host: self.port_forward.0.clone(),
                port: self.port_forward.1,
                ..Default::default()
            }),
            _ => {}
        }

        let mut msg_out = Message::new();
        msg_out.set_login_request(lr);
        msg_out
    }

    pub fn change_prefer_codec(&self) -> Message {
        let state = scrap::codec::Decoder::video_codec_state(&self.id);
        let mut misc = Misc::new();
        misc.set_option(OptionMessage {
            video_codec_state: hbb_common::protobuf::MessageField::some(state),
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        msg_out
    }

    pub fn restart_remote_device(&self) -> Message {
        let mut misc = Misc::new();
        misc.set_restart_remote_device(true);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        msg_out
    }

    pub fn set_force_relay(&mut self, direct: bool, received: bool) {
        self.force_relay = false;
        if direct && !received {
            let errno = errno::errno().0;
            log::info!("errno is {}", errno);
            // TODO: check mac and ios
            if cfg!(windows) && errno == 10054 || !cfg!(windows) && errno == 104 {
                self.force_relay = true;
                self.set_option("force-always-relay".to_owned(), "Y".to_owned());
            }
        }
    }
}

/// Media data.
pub enum MediaData {
    VideoFrame(VideoFrame),
    AudioFrame(AudioFrame),
    AudioFormat(AudioFormat),
    Reset,
    RecordScreen(bool, i32, i32, String),
}

pub type MediaSender = mpsc::Sender<MediaData>;

/// Start video and audio thread.
/// Return two [`MediaSender`], they should be given to the media producer.
///
/// # Arguments
///
/// * `video_callback` - The callback for video frame. Being called when a video frame is ready.
pub fn start_video_audio_threads<F>(video_callback: F) -> (MediaSender, MediaSender)
where
    F: 'static + FnMut(&[u8]) + Send,
{
    let (video_sender, video_receiver) = mpsc::channel::<MediaData>();
    let (audio_sender, audio_receiver) = mpsc::channel::<MediaData>();
    let mut video_callback = video_callback;

    let latency_controller = LatencyController::new();
    let latency_controller_cl = latency_controller.clone();

    #[cfg(not(features = "cli"))]
    std::thread::spawn(move || {
        let mut video_handler = VideoHandler::new(latency_controller);
        loop {
            if let Ok(data) = video_receiver.recv() {
                match data {
                    MediaData::VideoFrame(vf) => {
                        if let Ok(true) = video_handler.handle_frame(vf) {
                            video_callback(&video_handler.rgb);
                        }
                    }
                    MediaData::Reset => {
                        video_handler.reset();
                    }
                    MediaData::RecordScreen(start, w, h, id) => {
                        video_handler.record_screen(start, w, h, id)
                    }
                    _ => {}
                }
            } else {
                break;
            }
        }
        log::info!("Video decoder loop exits");
    });
    #[cfg(not(features = "cli"))]
    std::thread::spawn(move || {
        let mut audio_handler = AudioHandler::new(latency_controller_cl);
        loop {
            if let Ok(data) = audio_receiver.recv() {
                match data {
                    MediaData::AudioFrame(af) => {
                        audio_handler.handle_frame(af);
                    }
                    MediaData::AudioFormat(f) => {
                        audio_handler.handle_format(f);
                    }
                    _ => {}
                }
            } else {
                break;
            }
        }
        log::info!("Audio decoder loop exits");
    });
    return (video_sender, audio_sender);
}

/// Handle latency test.
///
/// # Arguments
///
/// * `t` - The latency test message.
/// * `peer` - The peer.
pub async fn handle_test_delay(t: TestDelay, peer: &mut Stream) {
    if !t.from_client {
        let mut msg_out = Message::new();
        msg_out.set_test_delay(t);
        allow_err!(peer.send(&msg_out).await);
    }
}

/// Whether is track pad scrolling.
#[inline]
#[cfg(all(target_os = "macos"))]
fn check_scroll_on_mac(mask: i32, x: i32, y: i32) -> bool {
    // flutter version we set mask type bit to 4 when track pad scrolling.
    if mask & 7 == 4 {
        return true;
    }
    if mask & 3 != 3 {
        return false;
    }
    let btn = mask >> 3;
    if y == -1 {
        btn != 0xff88 && btn != -0x780000
    } else if y == 1 {
        btn != 0x78 && btn != 0x780000
    } else if x != 0 {
        // No mouse support horizontal scrolling.
        true
    } else {
        false
    }
}

/// Send mouse data.
///
/// # Arguments
///
/// * `mask` - Mouse event.
///     * mask = buttons << 3 | type
///     * type, 1: down, 2: up, 3: wheel, 4: trackpad
///     * buttons, 1: left, 2: right, 4: middle
/// * `x` - X coordinate.
/// * `y` - Y coordinate.
/// * `alt` - Whether the alt key is pressed.
/// * `ctrl` - Whether the ctrl key is pressed.
/// * `shift` - Whether the shift key is pressed.
/// * `command` - Whether the command key is pressed.
/// * `interface` - The interface for sending data.
#[inline]
pub fn send_mouse(
    mask: i32,
    x: i32,
    y: i32,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
    interface: &impl Interface,
) {
    let mut msg_out = Message::new();
    let mut mouse_event = MouseEvent {
        mask,
        x,
        y,
        ..Default::default()
    };
    if alt {
        mouse_event.modifiers.push(ControlKey::Alt.into());
    }
    if shift {
        mouse_event.modifiers.push(ControlKey::Shift.into());
    }
    if ctrl {
        mouse_event.modifiers.push(ControlKey::Control.into());
    }
    if command {
        mouse_event.modifiers.push(ControlKey::Meta.into());
    }
    #[cfg(all(target_os = "macos"))]
    if check_scroll_on_mac(mask, x, y) {
        mouse_event.modifiers.push(ControlKey::Scroll.into());
    }
    msg_out.set_mouse_event(mouse_event);
    interface.send(Data::Message(msg_out));
}

/// Avtivate OS by sending mouse movement.
///
/// # Arguments
///
/// * `interface` - The interface for sending data.
fn activate_os(interface: &impl Interface) {
    send_mouse(0, 0, 0, false, false, false, false, interface);
    std::thread::sleep(Duration::from_millis(50));
    send_mouse(0, 3, 3, false, false, false, false, interface);
    std::thread::sleep(Duration::from_millis(50));
    send_mouse(1 | 1 << 3, 0, 0, false, false, false, false, interface);
    send_mouse(2 | 1 << 3, 0, 0, false, false, false, false, interface);
    /*
    let mut key_event = KeyEvent::new();
    // do not use Esc, which has problem with Linux
    key_event.set_control_key(ControlKey::RightArrow);
    key_event.press = true;
    let mut msg_out = Message::new();
    msg_out.set_key_event(key_event.clone());
    interface.send(Data::Message(msg_out.clone()));
    */
}

/// Input the OS's password.
///
/// # Arguments
///
/// * `p` - The password.
/// * `avtivate` - Whether to activate OS.
/// * `interface` - The interface for sending data.
pub fn input_os_password(p: String, activate: bool, interface: impl Interface) {
    std::thread::spawn(move || {
        _input_os_password(p, activate, interface);
    });
}

/// Input the OS's password.
///
/// # Arguments
///
/// * `p` - The password.
/// * `avtivate` - Whether to activate OS.
/// * `interface` - The interface for sending data.
fn _input_os_password(p: String, activate: bool, interface: impl Interface) {
    if activate {
        activate_os(&interface);
        std::thread::sleep(Duration::from_millis(1200));
    }
    let mut key_event = KeyEvent::new();
    key_event.press = true;
    let mut msg_out = Message::new();
    key_event.set_seq(p);
    msg_out.set_key_event(key_event.clone());
    interface.send(Data::Message(msg_out.clone()));
    key_event.set_control_key(ControlKey::Return);
    msg_out.set_key_event(key_event);
    interface.send(Data::Message(msg_out));
}

/// Handle login error.
/// Return true if the password is wrong, return false if there's an actual error.
pub fn handle_login_error(
    lc: Arc<RwLock<LoginConfigHandler>>,
    err: &str,
    interface: &impl Interface,
) -> bool {
    if err == "Wrong Password" {
        lc.write().unwrap().password = Default::default();
        interface.msgbox("re-input-password", err, "Do you want to enter again?", "");
        true
    } else if err == "No Password Access" {
        lc.write().unwrap().password = Default::default();
        interface.msgbox(
            "wait-remote-accept-nook",
            "Prompt",
            "Please wait for the remote side to accept your session request...",
            "",
        );
        true
    } else {
        if err.contains(SCRAP_X11_REQUIRED) {
            interface.msgbox("error", "Login Error", err, SCRAP_X11_REF_URL);
        } else {
            interface.msgbox("error", "Login Error", err, "");
        }
        false
    }
}

/// Handle hash message sent by peer.
/// Hash will be used for login.
///
/// # Arguments
///
/// * `lc` - Login config.
/// * `hash` - Hash sent by peer.
/// * `interface` - [`Interface`] for sending data.
/// * `peer` - [`Stream`] for communicating with peer.
pub async fn handle_hash(
    lc: Arc<RwLock<LoginConfigHandler>>,
    password_preset: &str,
    hash: Hash,
    interface: &impl Interface,
    peer: &mut Stream,
) {
    let mut password = lc.read().unwrap().password.clone();
    if password.is_empty() {
        if !password_preset.is_empty() {
            let mut hasher = Sha256::new();
            hasher.update(password_preset);
            hasher.update(&hash.salt);
            let res = hasher.finalize();
            password = res[..].into();
        }
    }
    if password.is_empty() {
        password = lc.read().unwrap().config.password.clone();
    }
    if password.is_empty() {
        // login without password, the remote side can click accept
        send_login(lc.clone(), Vec::new(), peer).await;
        interface.msgbox("input-password", "Password Required", "", "");
    } else {
        let mut hasher = Sha256::new();
        hasher.update(&password);
        hasher.update(&hash.challenge);
        send_login(lc.clone(), hasher.finalize()[..].into(), peer).await;
    }
    lc.write().unwrap().hash = hash;
}

/// Send login message to peer.
///
/// # Arguments
///
/// * `lc` - Login config.
/// * `password` - Password.
/// * `peer` - [`Stream`] for communicating with peer.
async fn send_login(lc: Arc<RwLock<LoginConfigHandler>>, password: Vec<u8>, peer: &mut Stream) {
    let msg_out = lc.read().unwrap().create_login_msg(password);
    allow_err!(peer.send(&msg_out).await);
}

/// Handle login request made from ui.
///
/// # Arguments
///
/// * `lc` - Login config.
/// * `password` - Password.
/// * `remember` - Whether to remember password.
/// * `peer` - [`Stream`] for communicating with peer.
pub async fn handle_login_from_ui(
    lc: Arc<RwLock<LoginConfigHandler>>,
    password: String,
    remember: bool,
    peer: &mut Stream,
) {
    let mut hasher = Sha256::new();
    hasher.update(password);
    hasher.update(&lc.read().unwrap().hash.salt);
    let res = hasher.finalize();
    lc.write().unwrap().remember = remember;
    lc.write().unwrap().password = res[..].into();
    let mut hasher2 = Sha256::new();
    hasher2.update(&res[..]);
    hasher2.update(&lc.read().unwrap().hash.challenge);
    send_login(lc.clone(), hasher2.finalize()[..].into(), peer).await;
}

/// Interface for client to send data and commands.
#[async_trait]
pub trait Interface: Send + Clone + 'static + Sized {
    /// Send message data to remote peer.
    fn send(&self, data: Data);
    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str);
    fn handle_login_error(&mut self, err: &str) -> bool;
    fn handle_peer_info(&mut self, pi: PeerInfo);
    fn on_error(&self, err: &str) {
        self.msgbox("error", "Error", err, "");
    }
    async fn handle_hash(&mut self, pass: &str, hash: Hash, peer: &mut Stream);
    async fn handle_login_from_ui(&mut self, password: String, remember: bool, peer: &mut Stream);
    async fn handle_test_delay(&mut self, t: TestDelay, peer: &mut Stream);

    fn get_login_config_handler(&self) -> Arc<RwLock<LoginConfigHandler>>;
    fn set_force_relay(&self, direct: bool, received: bool) {
        self.get_login_config_handler().write().unwrap().set_force_relay(direct, received);
    }
    fn is_force_relay(&self) -> bool {
        self.get_login_config_handler().read().unwrap().force_relay
    }
}

/// Data used by the client interface.
#[derive(Clone)]
pub enum Data {
    Close,
    Login((String, bool)),
    Message(Message),
    SendFiles((i32, String, String, i32, bool, bool)),
    RemoveDirAll((i32, String, bool, bool)),
    ConfirmDeleteFiles((i32, i32)),
    SetNoConfirm(i32),
    RemoveDir((i32, String)),
    RemoveFile((i32, String, i32, bool)),
    CreateDir((i32, String, bool)),
    CancelJob(i32),
    RemovePortForward(i32),
    AddPortForward((i32, String, i32)),
    ToggleClipboardFile,
    NewRDP,
    SetConfirmOverrideFile((i32, i32, bool, bool, bool)),
    AddJob((i32, String, String, i32, bool, bool)),
    ResumeJob((i32, bool)),
    RecordScreen(bool, i32, i32, String),
}

/// Keycode for key events.
#[derive(Clone, Debug)]
pub enum Key {
    ControlKey(ControlKey),
    Chr(u32),
    _Raw(u32),
}

lazy_static::lazy_static! {
    pub static ref KEY_MAP: HashMap<&'static str, Key> =
    [
        ("VK_A", Key::Chr('a' as _)),
        ("VK_B", Key::Chr('b' as _)),
        ("VK_C", Key::Chr('c' as _)),
        ("VK_D", Key::Chr('d' as _)),
        ("VK_E", Key::Chr('e' as _)),
        ("VK_F", Key::Chr('f' as _)),
        ("VK_G", Key::Chr('g' as _)),
        ("VK_H", Key::Chr('h' as _)),
        ("VK_I", Key::Chr('i' as _)),
        ("VK_J", Key::Chr('j' as _)),
        ("VK_K", Key::Chr('k' as _)),
        ("VK_L", Key::Chr('l' as _)),
        ("VK_M", Key::Chr('m' as _)),
        ("VK_N", Key::Chr('n' as _)),
        ("VK_O", Key::Chr('o' as _)),
        ("VK_P", Key::Chr('p' as _)),
        ("VK_Q", Key::Chr('q' as _)),
        ("VK_R", Key::Chr('r' as _)),
        ("VK_S", Key::Chr('s' as _)),
        ("VK_T", Key::Chr('t' as _)),
        ("VK_U", Key::Chr('u' as _)),
        ("VK_V", Key::Chr('v' as _)),
        ("VK_W", Key::Chr('w' as _)),
        ("VK_X", Key::Chr('x' as _)),
        ("VK_Y", Key::Chr('y' as _)),
        ("VK_Z", Key::Chr('z' as _)),
        ("VK_0", Key::Chr('0' as _)),
        ("VK_1", Key::Chr('1' as _)),
        ("VK_2", Key::Chr('2' as _)),
        ("VK_3", Key::Chr('3' as _)),
        ("VK_4", Key::Chr('4' as _)),
        ("VK_5", Key::Chr('5' as _)),
        ("VK_6", Key::Chr('6' as _)),
        ("VK_7", Key::Chr('7' as _)),
        ("VK_8", Key::Chr('8' as _)),
        ("VK_9", Key::Chr('9' as _)),
        ("VK_COMMA", Key::Chr(',' as _)),
        ("VK_SLASH", Key::Chr('/' as _)),
        ("VK_SEMICOLON", Key::Chr(';' as _)),
        ("VK_QUOTE", Key::Chr('\'' as _)),
        ("VK_LBRACKET", Key::Chr('[' as _)),
        ("VK_RBRACKET", Key::Chr(']' as _)),
        ("VK_BACKSLASH", Key::Chr('\\' as _)),
        ("VK_MINUS", Key::Chr('-' as _)),
        ("VK_PLUS", Key::Chr('=' as _)), // it is =, but sciter return VK_PLUS
        ("VK_DIVIDE", Key::ControlKey(ControlKey::Divide)), // numpad
        ("VK_MULTIPLY", Key::ControlKey(ControlKey::Multiply)), // numpad
        ("VK_SUBTRACT", Key::ControlKey(ControlKey::Subtract)), // numpad
        ("VK_ADD", Key::ControlKey(ControlKey::Add)), // numpad
        ("VK_DECIMAL", Key::ControlKey(ControlKey::Decimal)), // numpad
        ("VK_F1", Key::ControlKey(ControlKey::F1)),
        ("VK_F2", Key::ControlKey(ControlKey::F2)),
        ("VK_F3", Key::ControlKey(ControlKey::F3)),
        ("VK_F4", Key::ControlKey(ControlKey::F4)),
        ("VK_F5", Key::ControlKey(ControlKey::F5)),
        ("VK_F6", Key::ControlKey(ControlKey::F6)),
        ("VK_F7", Key::ControlKey(ControlKey::F7)),
        ("VK_F8", Key::ControlKey(ControlKey::F8)),
        ("VK_F9", Key::ControlKey(ControlKey::F9)),
        ("VK_F10", Key::ControlKey(ControlKey::F10)),
        ("VK_F11", Key::ControlKey(ControlKey::F11)),
        ("VK_F12", Key::ControlKey(ControlKey::F12)),
        ("VK_ENTER", Key::ControlKey(ControlKey::Return)),
        ("VK_CANCEL", Key::ControlKey(ControlKey::Cancel)),
        ("VK_BACK", Key::ControlKey(ControlKey::Backspace)),
        ("VK_TAB", Key::ControlKey(ControlKey::Tab)),
        ("VK_CLEAR", Key::ControlKey(ControlKey::Clear)),
        ("VK_RETURN", Key::ControlKey(ControlKey::Return)),
        ("VK_SHIFT", Key::ControlKey(ControlKey::Shift)),
        ("VK_CONTROL", Key::ControlKey(ControlKey::Control)),
        ("VK_MENU", Key::ControlKey(ControlKey::Alt)),
        ("VK_PAUSE", Key::ControlKey(ControlKey::Pause)),
        ("VK_CAPITAL", Key::ControlKey(ControlKey::CapsLock)),
        ("VK_KANA", Key::ControlKey(ControlKey::Kana)),
        ("VK_HANGUL", Key::ControlKey(ControlKey::Hangul)),
        ("VK_JUNJA", Key::ControlKey(ControlKey::Junja)),
        ("VK_FINAL", Key::ControlKey(ControlKey::Final)),
        ("VK_HANJA", Key::ControlKey(ControlKey::Hanja)),
        ("VK_KANJI", Key::ControlKey(ControlKey::Kanji)),
        ("VK_ESCAPE", Key::ControlKey(ControlKey::Escape)),
        ("VK_CONVERT", Key::ControlKey(ControlKey::Convert)),
        ("VK_SPACE", Key::ControlKey(ControlKey::Space)),
        ("VK_PRIOR", Key::ControlKey(ControlKey::PageUp)),
        ("VK_NEXT", Key::ControlKey(ControlKey::PageDown)),
        ("VK_END", Key::ControlKey(ControlKey::End)),
        ("VK_HOME", Key::ControlKey(ControlKey::Home)),
        ("VK_LEFT", Key::ControlKey(ControlKey::LeftArrow)),
        ("VK_UP", Key::ControlKey(ControlKey::UpArrow)),
        ("VK_RIGHT", Key::ControlKey(ControlKey::RightArrow)),
        ("VK_DOWN", Key::ControlKey(ControlKey::DownArrow)),
        ("VK_SELECT", Key::ControlKey(ControlKey::Select)),
        ("VK_PRINT", Key::ControlKey(ControlKey::Print)),
        ("VK_EXECUTE", Key::ControlKey(ControlKey::Execute)),
        ("VK_SNAPSHOT", Key::ControlKey(ControlKey::Snapshot)),
        ("VK_INSERT", Key::ControlKey(ControlKey::Insert)),
        ("VK_DELETE", Key::ControlKey(ControlKey::Delete)),
        ("VK_HELP", Key::ControlKey(ControlKey::Help)),
        ("VK_SLEEP", Key::ControlKey(ControlKey::Sleep)),
        ("VK_SEPARATOR", Key::ControlKey(ControlKey::Separator)),
        ("VK_NUMPAD0", Key::ControlKey(ControlKey::Numpad0)),
        ("VK_NUMPAD1", Key::ControlKey(ControlKey::Numpad1)),
        ("VK_NUMPAD2", Key::ControlKey(ControlKey::Numpad2)),
        ("VK_NUMPAD3", Key::ControlKey(ControlKey::Numpad3)),
        ("VK_NUMPAD4", Key::ControlKey(ControlKey::Numpad4)),
        ("VK_NUMPAD5", Key::ControlKey(ControlKey::Numpad5)),
        ("VK_NUMPAD6", Key::ControlKey(ControlKey::Numpad6)),
        ("VK_NUMPAD7", Key::ControlKey(ControlKey::Numpad7)),
        ("VK_NUMPAD8", Key::ControlKey(ControlKey::Numpad8)),
        ("VK_NUMPAD9", Key::ControlKey(ControlKey::Numpad9)),
        ("Apps", Key::ControlKey(ControlKey::Apps)),
        ("Meta", Key::ControlKey(ControlKey::Meta)),
        ("RAlt", Key::ControlKey(ControlKey::RAlt)),
        ("RWin", Key::ControlKey(ControlKey::RWin)),
        ("RControl", Key::ControlKey(ControlKey::RControl)),
        ("RShift", Key::ControlKey(ControlKey::RShift)),
        ("CTRL_ALT_DEL", Key::ControlKey(ControlKey::CtrlAltDel)),
        ("LOCK_SCREEN", Key::ControlKey(ControlKey::LockScreen)),
    ].iter().cloned().collect();
}

/// Check if the given message is an error and can be retried.
///
/// # Arguments
///
/// * `msgtype` - The message type.
/// * `title` - The title of the message.
/// * `text` - The text of the message.
#[inline]
pub fn check_if_retry(msgtype: &str, title: &str, text: &str, retry_for_relay: bool) -> bool {
    msgtype == "error"
        && title == "Connection Error"
        && ((text.contains("10054") || text.contains("104")) && retry_for_relay
            || (!text.to_lowercase().contains("offline")
                && !text.to_lowercase().contains("exist")
                && !text.to_lowercase().contains("handshake")
                && !text.to_lowercase().contains("failed")
                && !text.to_lowercase().contains("resolve")
                && !text.to_lowercase().contains("mismatch")
                && !text.to_lowercase().contains("manually")
                && !text.to_lowercase().contains("not allowed")
                && !text.to_lowercase().contains("reset by the peer")))
}

#[inline]
fn get_pk(pk: &[u8]) -> Option<[u8; 32]> {
    if pk.len() == 32 {
        let mut tmp = [0u8; 32];
        tmp[..].copy_from_slice(&pk);
        Some(tmp)
    } else {
        None
    }
}

#[inline]
fn get_rs_pk(str_base64: &str) -> Option<sign::PublicKey> {
    if let Ok(pk) = base64::decode(str_base64) {
        get_pk(&pk).map(|x| sign::PublicKey(x))
    } else {
        None
    }
}

fn decode_id_pk(signed: &[u8], key: &sign::PublicKey) -> ResultType<(String, [u8; 32])> {
    let res = IdPk::parse_from_bytes(
        &sign::verify(signed, key).map_err(|_| anyhow!("Signature mismatch"))?,
    )?;
    if let Some(pk) = get_pk(&res.pk) {
        Ok((res.id, pk))
    } else {
        bail!("Wrong public length");
    }
}
