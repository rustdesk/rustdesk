pub use async_trait::async_trait;
#[cfg(not(any(target_os = "android")))]
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Host, StreamConfig,
};
use hbb_common::{
    allow_err,
    anyhow::{anyhow, Context},
    bail,
    config::{Config, PeerConfig, PeerInfoSerde, CONNECT_TIMEOUT, RELAY_PORT, RENDEZVOUS_TIMEOUT},
    log,
    message_proto::*,
    protobuf::Message as _,
    rendezvous_proto::*,
    sodiumoxide::crypto::{box_, secretbox, sign},
    tcp::FramedStream,
    timeout,
    tokio::time::Duration,
    AddrMangle, ResultType, Stream,
};
use magnum_opus::{Channels::*, Decoder as AudioDecoder};
use scrap::{Decoder, Image, VideoCodecId};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    net::SocketAddr,
    ops::Deref,
    sync::{Arc, Mutex, RwLock},
};
use uuid::Uuid;

pub const SEC30: Duration = Duration::from_secs(30);

pub struct Client;

#[cfg(not(any(target_os = "android")))]
lazy_static::lazy_static! {
static ref AUDIO_HOST: Host = cpal::default_host();
}

cfg_if::cfg_if! {
    if #[cfg(target_os = "android")] {

use libc::{c_float, c_int, c_void};
use std::cell::RefCell;
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
    pub async fn start(peer: &str, conn_type: ConnType) -> ResultType<(Stream, bool)> {
        // to-do: remember the port for each peer, so that we can retry easier
        let any_addr = Config::get_any_listen_addr();
        let rendezvous_server = crate::get_rendezvous_server(1_000).await;
        log::info!("rendezvous server: {}", rendezvous_server);
        let mut socket = FramedStream::new(rendezvous_server, any_addr, RENDEZVOUS_TIMEOUT)
            .await
            .with_context(|| "Failed to connect to rendezvous server")?;
        let my_addr = socket.get_ref().local_addr()?;
        let mut pk = Vec::new();
        let mut relay_server = "".to_owned();

        let start = std::time::Instant::now();
        let mut peer_addr = any_addr;
        let mut peer_nat_type = NatType::UNKNOWN_NAT;
        let my_nat_type = crate::get_nat_type(100).await;
        let mut is_local = false;
        for i in 1..=3 {
            log::info!("#{} punch attempt with {}, id: {}", i, my_addr, peer);
            let mut msg_out = RendezvousMessage::new();
            use hbb_common::protobuf::ProtobufEnum;
            let nat_type = NatType::from_i32(my_nat_type).unwrap_or(NatType::UNKNOWN_NAT);
            msg_out.set_punch_hole_request(PunchHoleRequest {
                id: peer.to_owned(),
                nat_type: nat_type.into(),
                conn_type: conn_type.into(),
                ..Default::default()
            });
            socket.send(&msg_out).await?;
            if let Some(Ok(bytes)) = socket.next_timeout(i * 6000).await {
                if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                    match msg_in.union {
                        Some(rendezvous_message::Union::punch_hole_response(ph)) => {
                            if ph.socket_addr.is_empty() {
                                match ph.failure.enum_value_or_default() {
                                    punch_hole_response::Failure::ID_NOT_EXIST => {
                                        bail!("ID does not exist");
                                    }
                                    punch_hole_response::Failure::OFFLINE => {
                                        bail!("Remote desktop is offline");
                                    }
                                    punch_hole_response::Failure::LICENCE_MISMATCH => {
                                        bail!("Key mismatch");
                                    }
                                    _ => {
                                        if !ph.other_failure.is_empty() {
                                            bail!(ph.other_failure);
                                        }
                                    }
                                }
                            } else {
                                peer_nat_type = ph.get_nat_type();
                                is_local = ph.get_is_local();
                                pk = ph.pk;
                                relay_server = ph.relay_server;
                                peer_addr = AddrMangle::decode(&ph.socket_addr);
                                log::info!("Hole Punched {} = {}", peer, peer_addr);
                                break;
                            }
                        }
                        Some(rendezvous_message::Union::relay_response(rr)) => {
                            log::info!(
                                "relay requested from peer, time used: {:?}, relay_server: {}",
                                start.elapsed(),
                                rr.relay_server
                            );
                            pk = rr.get_pk().into();
                            let mut conn =
                                Self::create_relay(peer, rr.uuid, rr.relay_server, conn_type)
                                    .await?;
                            Self::secure_connection(peer, pk, &mut conn).await?;
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
            pk,
            &relay_server,
            rendezvous_server,
            time_used,
            peer_nat_type,
            my_nat_type,
            is_local,
            conn_type,
        )
        .await
    }

    async fn connect(
        local_addr: SocketAddr,
        peer: SocketAddr,
        peer_id: &str,
        pk: Vec<u8>,
        relay_server: &str,
        rendezvous_server: SocketAddr,
        punch_time_used: u64,
        peer_nat_type: NatType,
        my_nat_type: i32,
        is_local: bool,
        conn_type: ConnType,
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
        let mut conn = FramedStream::new(peer, local_addr, connect_timeout).await;
        let direct = !conn.is_err();
        if conn.is_err() {
            if !relay_server.is_empty() {
                conn = Self::request_relay(
                    peer_id,
                    relay_server.to_owned(),
                    rendezvous_server,
                    pk.len() == sign::PUBLICKEYBYTES,
                    conn_type,
                )
                .await;
                if conn.is_err() {
                    bail!(
                        "Failed to connect via relay server: {}",
                        conn.err().unwrap()
                    );
                }
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
        Self::secure_connection(peer_id, pk, &mut conn).await?;
        Ok((conn, direct))
    }

    async fn secure_connection(peer_id: &str, pk: Vec<u8>, conn: &mut Stream) -> ResultType<()> {
        if pk.len() != sign::PUBLICKEYBYTES {
            // send an empty message out in case server is setting up secure and waiting for first message
            conn.send(&Message::new()).await?;
            return Ok(());
        }
        let mut pk_ = [0u8; sign::PUBLICKEYBYTES];
        pk_[..].copy_from_slice(&pk);
        let pk = sign::PublicKey(pk_);
        match timeout(CONNECT_TIMEOUT, conn.next()).await? {
            Some(res) => {
                let bytes = res?;
                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                    if let Some(message::Union::signed_id(si)) = msg_in.union {
                        let their_pk_b = if si.pk.len() == box_::PUBLICKEYBYTES {
                            let mut pk_ = [0u8; box_::PUBLICKEYBYTES];
                            pk_[..].copy_from_slice(&si.pk);
                            box_::PublicKey(pk_)
                        } else {
                            bail!("Handshake failed: invalid public box key length from peer");
                        };
                        if let Ok(id) = sign::verify(&si.id, &pk) {
                            if id == peer_id.as_bytes() {
                                let (our_pk_b, out_sk_b) = box_::gen_keypair();
                                let key = secretbox::gen_key();
                                let nonce = box_::Nonce([0u8; box_::NONCEBYTES]);
                                let sealed_key = box_::seal(&key.0, &nonce, &their_pk_b, &out_sk_b);
                                let mut msg_out = Message::new();
                                msg_out.set_public_key(PublicKey {
                                    asymmetric_value: our_pk_b.0.into(),
                                    symmetric_value: sealed_key,
                                    ..Default::default()
                                });
                                timeout(CONNECT_TIMEOUT, conn.send(&msg_out)).await??;
                                conn.set_key(key);
                            } else {
                                bail!("Handshake failed: sign failure");
                            }
                        } else {
                            // fall back to non-secure connection in case pk mismatch
                            log::info!("pk mismatch, fall back to non-secure");
                            let mut msg_out = Message::new();
                            msg_out.set_public_key(PublicKey::new());
                            timeout(CONNECT_TIMEOUT, conn.send(&msg_out)).await??;
                        }
                    } else {
                        bail!("Handshake failed: invalid message type");
                    }
                } else {
                    bail!("Handshake failed: invalid message format");
                }
            }
            None => {
                bail!("Reset by the peer");
            }
        }
        Ok(())
    }

    async fn request_relay(
        peer: &str,
        relay_server: String,
        rendezvous_server: SocketAddr,
        secure: bool,
        conn_type: ConnType,
    ) -> ResultType<Stream> {
        let any_addr = Config::get_any_listen_addr();
        let mut succeed = false;
        let mut uuid = "".to_owned();
        for i in 1..=3 {
            // use different socket due to current hbbs implement requiring different nat address for each attempt
            let mut socket = FramedStream::new(rendezvous_server, any_addr, RENDEZVOUS_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;
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
                uuid: uuid.clone(),
                relay_server: relay_server.clone(),
                secure,
                ..Default::default()
            });
            socket.send(&msg_out).await?;
            if let Some(Ok(bytes)) = socket.next_timeout(CONNECT_TIMEOUT).await {
                if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                    if let Some(rendezvous_message::Union::relay_response(rs)) = msg_in.union {
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
        Self::create_relay(peer, uuid, relay_server, conn_type).await
    }

    async fn create_relay(
        peer: &str,
        uuid: String,
        relay_server: String,
        conn_type: ConnType,
    ) -> ResultType<Stream> {
        let mut conn = FramedStream::new(
            crate::check_port(relay_server, RELAY_PORT),
            Config::get_any_listen_addr(),
            CONNECT_TIMEOUT,
        )
        .await
        .with_context(|| "Failed to connect to relay server")?;
        let mut msg_out = RendezvousMessage::new();
        msg_out.set_request_relay(RequestRelay {
            id: peer.to_owned(),
            uuid,
            conn_type: conn_type.into(),
            ..Default::default()
        });
        conn.send(&msg_out).await?;
        Ok(conn)
    }
}

#[derive(Default)]
pub struct AudioHandler {
    audio_decoder: Option<(AudioDecoder, Vec<f32>)>,
    #[cfg(any(target_os = "android"))]
    oboe: RefCell<OboePlayer>,
    #[cfg(not(any(target_os = "android")))]
    audio_buffer: Arc<Mutex<std::collections::vec_deque::VecDeque<f32>>>,
    sample_rate: (u32, u32),
    #[cfg(not(any(target_os = "android")))]
    audio_stream: Option<Box<dyn StreamTrait>>,
    channels: u16,
}

impl AudioHandler {
    #[cfg(any(target_os = "android"))]
    fn start_audio(&mut self, format0: AudioFormat) -> ResultType<()> {
        self.sample_rate = (format0.sample_rate, format0.sample_rate);
        Ok(())
    }

    #[cfg(not(any(target_os = "android")))]
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

    pub fn handle_frame(&mut self, frame: AudioFrame, play: bool) {
        if !play {
            return;
        }
        #[cfg(not(any(target_os = "android")))]
        if self.audio_stream.is_none() {
            return;
        }
        let sample_rate0 = self.sample_rate.0;
        let sample_rate = self.sample_rate.1;
        let channels = self.channels;
        cfg_if::cfg_if! {
        if #[cfg(not(target_os = "android"))] {
        let audio_buffer = self.audio_buffer.clone();
        // avoiding memory overflow if audio_buffer consumer side has problem
        if audio_buffer.lock().unwrap().len() as u32 > sample_rate * 120 {
            *audio_buffer.lock().unwrap() = Default::default();
        }
        } else {
        if self.oboe.borrow().is_null() {
            self.oboe = RefCell::new(OboePlayer::new(
                channels as _,
                sample_rate0 as _,
            ));
        }
        let mut oboe = self.oboe.borrow_mut();
        }
        }
        self.audio_decoder.as_mut().map(|(d, buffer)| {
            if let Ok(n) = d.decode_float(&frame.data, buffer, false) {
                let n = n * (channels as usize);
                #[cfg(not(any(target_os = "android")))]
                {
                    if sample_rate != sample_rate0 {
                        let buffer = crate::resample_channels(
                            &buffer[0..n],
                            sample_rate0,
                            sample_rate,
                            channels,
                        );
                        audio_buffer.lock().unwrap().extend(buffer);
                    } else {
                        audio_buffer.lock().unwrap().extend(buffer.iter().cloned());
                    }
                }
                #[cfg(any(target_os = "android"))]
                {
                    oboe.push(&buffer[0..n]);
                }
            }
        });
    }

    #[cfg(not(any(target_os = "android")))]
    fn build_output_stream<T: cpal::Sample>(
        &mut self,
        config: &StreamConfig,
        device: &Device,
    ) -> ResultType<()> {
        let err_fn = move |err| {
            log::error!("an error occurred on stream: {}", err);
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

pub struct VideoHandler {
    decoder: Decoder,
    pub rgb: Vec<u8>,
}

impl VideoHandler {
    pub fn new() -> Self {
        VideoHandler {
            decoder: Decoder::new(VideoCodecId::VP9, 1).unwrap(),
            rgb: Default::default(),
        }
    }

    pub fn handle_vp9s(&mut self, vp9s: &VP9s) -> ResultType<bool> {
        let mut last_frame = Image::new();
        for vp9 in vp9s.frames.iter() {
            for frame in self.decoder.decode(&vp9.data)? {
                drop(last_frame);
                last_frame = frame;
            }
        }
        for frame in self.decoder.flush()? {
            drop(last_frame);
            last_frame = frame;
        }
        if last_frame.is_null() {
            Ok(false)
        } else {
            last_frame.rgb(1, true, &mut self.rgb);
            Ok(true)
        }
    }

    pub fn reset(&mut self) {
        self.decoder = Decoder::new(VideoCodecId::VP9, 1).unwrap();
    }
}

#[derive(Default)]
pub struct LoginConfigHandler {
    id: String,
    is_file_transfer: bool,
    is_port_forward: bool,
    hash: Hash,
    password: Vec<u8>, // remember password for reconnect
    pub remember: bool,
    config: PeerConfig,
    pub port_forward: (String, i32),
    pub support_press: bool,
    pub support_refresh: bool,
}

impl Deref for LoginConfigHandler {
    type Target = PeerConfig;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

#[inline]
pub fn load_config(id: &str) -> PeerConfig {
    PeerConfig::load(id)
}

impl LoginConfigHandler {
    pub fn initialize(&mut self, id: String, is_file_transfer: bool, is_port_forward: bool) {
        self.id = id;
        self.is_file_transfer = is_file_transfer;
        self.is_port_forward = is_port_forward;
        let config = self.load_config();
        self.remember = !config.password.is_empty();
        self.config = config;
    }

    fn load_config(&self) -> PeerConfig {
        load_config(&self.id)
    }

    pub fn save_config(&mut self, config: PeerConfig) {
        config.store(&self.id);
        self.config = config;
    }

    pub fn save_view_style(&mut self, value: String) {
        let mut config = self.load_config();
        config.view_style = value;
        self.save_config(config);
    }

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
            config.privacy_mode = !config.privacy_mode;
            option.privacy_mode = (if config.privacy_mode {
                BoolOption::Yes
            } else {
                BoolOption::No
            })
            .into();
        } else if name == "block-input" {
            option.block_input = BoolOption::Yes.into();
        } else if name == "unblock-input" {
            option.block_input = BoolOption::No.into();
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
        self.save_config(config);
        let mut misc = Misc::new();
        misc.set_option(option);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        Some(msg_out)
    }

    fn get_option_message(&self, ignore_default: bool) -> Option<OptionMessage> {
        if self.is_port_forward || self.is_file_transfer {
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
            let mut it = config.custom_image_quality.iter();
            let bitrate = it.next();
            let quantizer = it.next();
            if let Some(bitrate) = bitrate {
                if let Some(quantizer) = quantizer {
                    msg.custom_image_quality = bitrate << 8 | quantizer;
                    n += 1;
                }
            }
        }
        if self.get_toggle_option("show-remote-cursor") {
            msg.show_remote_cursor = BoolOption::Yes.into();
            n += 1;
        }
        if self.get_toggle_option("lock-after-session-end") {
            msg.lock_after_session_end = BoolOption::Yes.into();
            n += 1;
        }
        if self.get_toggle_option("privacy_mode") {
            msg.privacy_mode = BoolOption::Yes.into();
            n += 1;
        }
        if n > 0 {
            Some(msg)
        } else {
            None
        }
    }

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

    pub fn get_toggle_option(&self, name: &str) -> bool {
        if name == "show-remote-cursor" {
            self.config.show_remote_cursor
        } else if name == "lock-after-session-end" {
            self.config.lock_after_session_end
        } else if name == "privacy-mode" {
            self.config.privacy_mode
        } else if name == "disable-audio" {
            self.config.disable_audio
        } else if name == "disable-clipboard" {
            self.config.disable_clipboard
        } else {
            !self.get_option(name).is_empty()
        }
    }

    pub fn refresh() -> Message {
        let mut misc = Misc::new();
        misc.set_refresh_video(true);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        msg_out
    }

    pub fn save_custom_image_quality(&mut self, bitrate: i32, quantizer: i32) -> Message {
        let mut misc = Misc::new();
        misc.set_option(OptionMessage {
            custom_image_quality: bitrate << 8 | quantizer,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        let mut config = self.load_config();
        config.image_quality = "custom".to_owned();
        config.custom_image_quality = vec![bitrate, quantizer];
        self.save_config(config);
        msg_out
    }

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

    pub fn get_option(&self, k: &str) -> String {
        if let Some(v) = self.config.options.get(k) {
            v.clone()
        } else {
            "".to_owned()
        }
    }

    pub fn handle_login_error(&mut self, err: &str, interface: &impl Interface) -> bool {
        if err == "Wrong Password" {
            self.password = Default::default();
            interface.msgbox("re-input-password", err, "Do you want to enter again?");
            true
        } else {
            interface.msgbox("error", "Login Error", err);
            false
        }
    }

    pub fn get_username(&self, pi: &PeerInfo) -> String {
        return if pi.username.is_empty() {
            self.info.username.clone()
        } else {
            pi.username.clone()
        };
    }

    pub fn handle_peer_info(&mut self, username: String, pi: PeerInfo) {
        if !pi.version.is_empty() {
            self.support_press = true;
            self.support_refresh = true;
        }
        let serde = PeerInfoSerde {
            username,
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
        // no matter if change, for update file time
        self.save_config(config);
    }

    fn create_login_msg(&self, password: Vec<u8>) -> Message {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let my_id = crate::common::MOBILE_INFO1.lock().unwrap().clone();
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let my_id = Config::get_id();
        let mut lr = LoginRequest {
            username: self.id.clone(),
            password,
            my_id,
            my_name: crate::username(),
            option: self.get_option_message(true).into(),
            ..Default::default()
        };
        if self.is_file_transfer {
            lr.set_file_transfer(FileTransfer {
                dir: self.get_option("remote_dir"),
                show_hidden: !self.get_option("remote_show_hidden").is_empty(),
                ..Default::default()
            });
        } else if self.is_port_forward {
            lr.set_port_forward(PortForward {
                host: self.port_forward.0.clone(),
                port: self.port_forward.1,
                ..Default::default()
            });
        }
        let mut msg_out = Message::new();
        msg_out.set_login_request(lr);
        msg_out
    }
}

pub async fn handle_test_delay(t: TestDelay, peer: &mut Stream) {
    if !t.from_client {
        let mut msg_out = Message::new();
        msg_out.set_test_delay(t);
        allow_err!(peer.send(&msg_out).await);
    }
}

pub async fn handle_hash(
    lc: Arc<RwLock<LoginConfigHandler>>,
    hash: Hash,
    interface: &impl Interface,
    peer: &mut Stream,
) {
    let mut password = lc.read().unwrap().password.clone();
    if password.is_empty() {
        password = lc.read().unwrap().config.password.clone();
    }
    if password.is_empty() {
        // login without password, the remote side can click accept
        send_login(lc.clone(), Vec::new(), peer).await;
        interface.msgbox("input-password", "Password Required", "");
    } else {
        let mut hasher = Sha256::new();
        hasher.update(&password);
        hasher.update(&hash.challenge);
        send_login(lc.clone(), hasher.finalize()[..].into(), peer).await;
    }
    lc.write().unwrap().hash = hash;
}

async fn send_login(lc: Arc<RwLock<LoginConfigHandler>>, password: Vec<u8>, peer: &mut Stream) {
    let msg_out = lc.read().unwrap().create_login_msg(password);
    allow_err!(peer.send(&msg_out).await);
}

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

#[async_trait]
pub trait Interface: Send + Clone + 'static + Sized {
    fn msgbox(&self, msgtype: &str, title: &str, text: &str);
    fn handle_login_error(&mut self, err: &str) -> bool;
    fn handle_peer_info(&mut self, pi: PeerInfo);
    async fn handle_hash(&mut self, hash: Hash, peer: &mut Stream);
    async fn handle_login_from_ui(&mut self, password: String, remember: bool, peer: &mut Stream);
    async fn handle_test_delay(&mut self, t: TestDelay, peer: &mut Stream);
}

#[derive(Clone)]
pub enum Data {
    Close,
    Login((String, bool)),
    Message(Message),
    SendFiles((i32, String, String, bool, bool)),
    RemoveDirAll((i32, String, bool)),
    ConfirmDeleteFiles((i32, i32)),
    SetNoConfirm(i32),
    RemoveDir((i32, String)),
    RemoveFile((i32, String, i32, bool)),
    CreateDir((i32, String, bool)),
    CancelJob(i32),
    RemovePortForward(i32),
    AddPortForward((i32, String, i32)),
    NewRDP,
}

#[derive(Clone)]
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

#[inline]
pub fn check_if_retry(msgtype: &str, title: &str, text: &str) -> bool {
    msgtype == "error"
        && title == "Connection Error"
        && !text.to_lowercase().contains("offline")
        && !text.to_lowercase().contains("exist")
        && !text.to_lowercase().contains("handshake")
        && !text.to_lowercase().contains("failed")
        && !text.to_lowercase().contains("resolve")
        && !text.to_lowercase().contains("mismatch")
        && !text.to_lowercase().contains("manually")
}
