use crate::ipc::Data;
use bytes::Bytes;
pub use connection::*;
use hbb_common::{
    allow_err,
    anyhow::{anyhow, Context},
    bail,
    config::{Config, Config2, CONNECT_TIMEOUT, RELAY_PORT},
    log,
    message_proto::*,
    protobuf::{Enum, Message as _},
    rendezvous_proto::*,
    socket_client,
    sodiumoxide::crypto::{box_, secretbox, sign},
    timeout, tokio, ResultType, Stream,
};
use service::{GenericService, Service, ServiceTmpl, Subscriber};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock, Weak},
    time::Duration,
};

pub mod audio_service;
cfg_if::cfg_if! {
if #[cfg(not(any(target_os = "android", target_os = "ios")))] {
mod clipboard_service;
#[cfg(target_os = "linux")]
pub(crate) mod wayland;
#[cfg(target_os = "linux")]
pub mod uinput;
#[cfg(target_os = "linux")]
pub mod dbus;
pub mod input_service;
} else {
mod clipboard_service {
pub const NAME: &'static str = "";
}
pub mod input_service {
pub const NAME_CURSOR: &'static str = "";
pub const NAME_POS: &'static str = "";
}
}
}

mod connection;
mod service;
mod video_qos;
pub mod video_service;

use hbb_common::tcp::new_listener;

pub type Childs = Arc<Mutex<Vec<std::process::Child>>>;
type ConnMap = HashMap<i32, ConnInner>;

lazy_static::lazy_static! {
    pub static ref CHILD_PROCESS: Childs = Default::default();
}

pub struct Server {
    connections: ConnMap,
    services: HashMap<&'static str, Box<dyn Service>>,
    id_count: i32,
}

pub type ServerPtr = Arc<RwLock<Server>>;
pub type ServerPtrWeak = Weak<RwLock<Server>>;

pub fn new() -> ServerPtr {
    let mut server = Server {
        connections: HashMap::new(),
        services: HashMap::new(),
        id_count: 0,
    };
    server.add_service(Box::new(audio_service::new()));
    server.add_service(Box::new(video_service::new()));
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        server.add_service(Box::new(clipboard_service::new()));
        server.add_service(Box::new(input_service::new_cursor()));
        server.add_service(Box::new(input_service::new_pos()));
    }
    Arc::new(RwLock::new(server))
}

async fn accept_connection_(server: ServerPtr, socket: Stream, secure: bool) -> ResultType<()> {
    let local_addr = socket.local_addr();
    drop(socket);
    // even we drop socket, below still may fail if not use reuse_addr,
    // there is TIME_WAIT before socket really released, so sometimes we
    // see “Only one usage of each socket address is normally permitted” on windows sometimes,
    let listener = new_listener(local_addr, true).await?;
    log::info!("Server listening on: {}", &listener.local_addr()?);
    if let Ok((stream, addr)) = timeout(CONNECT_TIMEOUT, listener.accept()).await? {
        stream.set_nodelay(true).ok();
        let stream_addr = stream.local_addr()?;
        create_tcp_connection(server, Stream::from(stream, stream_addr), addr, secure).await?;
    }
    Ok(())
}

async fn check_privacy_mode_on(stream: &mut Stream) -> ResultType<()> {
    if video_service::get_privacy_mode_conn_id() > 0 {
        let msg_out =
            crate::common::make_privacy_mode_msg(back_notification::PrivacyModeState::PrvOnByOther);
        timeout(CONNECT_TIMEOUT, stream.send(&msg_out)).await??;
    }
    Ok(())
}

pub async fn create_tcp_connection(
    server: ServerPtr,
    stream: Stream,
    addr: SocketAddr,
    secure: bool,
) -> ResultType<()> {
    let mut stream = stream;
    check_privacy_mode_on(&mut stream).await?;

    let id = {
        let mut w = server.write().unwrap();
        w.id_count += 1;
        w.id_count
    };
    let (sk, pk) = Config::get_key_pair();
    if secure && pk.len() == sign::PUBLICKEYBYTES && sk.len() == sign::SECRETKEYBYTES {
        let mut sk_ = [0u8; sign::SECRETKEYBYTES];
        sk_[..].copy_from_slice(&sk);
        let sk = sign::SecretKey(sk_);
        let mut msg_out = Message::new();
        let (our_pk_b, our_sk_b) = box_::gen_keypair();
        msg_out.set_signed_id(SignedId {
            id: sign::sign(
                &IdPk {
                    id: Config::get_id(),
                    pk: Bytes::from(our_pk_b.0.to_vec()),
                    ..Default::default()
                }
                .write_to_bytes()
                .unwrap_or_default(),
                &sk,
            )
            .into(),
            ..Default::default()
        });
        timeout(CONNECT_TIMEOUT, stream.send(&msg_out)).await??;
        match timeout(CONNECT_TIMEOUT, stream.next()).await? {
            Some(res) => {
                let bytes = res?;
                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                    if let Some(message::Union::PublicKey(pk)) = msg_in.union {
                        if pk.asymmetric_value.len() == box_::PUBLICKEYBYTES {
                            let nonce = box_::Nonce([0u8; box_::NONCEBYTES]);
                            let mut pk_ = [0u8; box_::PUBLICKEYBYTES];
                            pk_[..].copy_from_slice(&pk.asymmetric_value);
                            let their_pk_b = box_::PublicKey(pk_);
                            let symmetric_key =
                                box_::open(&pk.symmetric_value, &nonce, &their_pk_b, &our_sk_b)
                                    .map_err(|_| {
                                        anyhow!("Handshake failed: box decryption failure")
                                    })?;
                            if symmetric_key.len() != secretbox::KEYBYTES {
                                bail!("Handshake failed: invalid secret key length from peer");
                            }
                            let mut key = [0u8; secretbox::KEYBYTES];
                            key[..].copy_from_slice(&symmetric_key);
                            stream.set_key(secretbox::Key(key));
                        } else if pk.asymmetric_value.is_empty() {
                            Config::set_key_confirmed(false);
                            log::info!("Force to update pk");
                        } else {
                            bail!("Handshake failed: invalid public sign key length from peer");
                        }
                    } else {
                        log::error!("Handshake failed: invalid message type");
                    }
                } else {
                    bail!("Handshake failed: invalid message format");
                }
            }
            None => {
                bail!("Failed to receive public key");
            }
        }
    }

    Connection::start(addr, stream, id, Arc::downgrade(&server)).await;
    Ok(())
}

pub async fn accept_connection(
    server: ServerPtr,
    socket: Stream,
    peer_addr: SocketAddr,
    secure: bool,
) {
    if let Err(err) = accept_connection_(server, socket, secure).await {
        log::error!("Failed to accept connection from {}: {}", peer_addr, err);
    }
}

pub async fn create_relay_connection(
    server: ServerPtr,
    relay_server: String,
    uuid: String,
    peer_addr: SocketAddr,
    secure: bool,
) {
    if let Err(err) =
        create_relay_connection_(server, relay_server, uuid.clone(), peer_addr, secure).await
    {
        log::error!(
            "Failed to create relay connection for {} with uuid {}: {}",
            peer_addr,
            uuid,
            err
        );
    }
}

async fn create_relay_connection_(
    server: ServerPtr,
    relay_server: String,
    uuid: String,
    peer_addr: SocketAddr,
    secure: bool,
) -> ResultType<()> {
    let mut stream = socket_client::connect_tcp(
        crate::check_port(relay_server, RELAY_PORT),
        Config::get_any_listen_addr(),
        CONNECT_TIMEOUT,
    )
    .await?;
    let mut msg_out = RendezvousMessage::new();
    let mut licence_key = Config::get_option("key");
    if licence_key.is_empty() {
        licence_key = crate::platform::get_license_key();
    }
    msg_out.set_request_relay(RequestRelay {
        licence_key,
        uuid,
        ..Default::default()
    });
    stream.send(&msg_out).await?;
    create_tcp_connection(server, stream, peer_addr, secure).await?;
    Ok(())
}

impl Server {
    pub fn add_connection(&mut self, conn: ConnInner, noperms: &Vec<&'static str>) {
        for s in self.services.values() {
            if !noperms.contains(&s.name()) {
                s.on_subscribe(conn.clone());
            }
        }
        self.connections.insert(conn.id(), conn);
    }

    pub fn remove_connection(&mut self, conn: &ConnInner) {
        for s in self.services.values() {
            s.on_unsubscribe(conn.id());
        }
        self.connections.remove(&conn.id());
    }

    pub fn close_connections(&mut self) {
        let conn_inners: Vec<_> = self.connections.values_mut().collect();
        for c in conn_inners {
            let mut misc = Misc::new();
            misc.set_stop_service(true);
            let mut msg = Message::new();
            msg.set_misc(misc);
            c.send(Arc::new(msg));
        }
    }

    fn add_service(&mut self, service: Box<dyn Service>) {
        let name = service.name();
        self.services.insert(name, service);
    }

    pub fn subscribe(&mut self, name: &str, conn: ConnInner, sub: bool) {
        if let Some(s) = self.services.get(&name) {
            if s.is_subed(conn.id()) == sub {
                return;
            }
            if sub {
                s.on_subscribe(conn.clone());
            } else {
                s.on_unsubscribe(conn.id());
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        for s in self.services.values() {
            s.join();
        }
        #[cfg(target_os = "linux")]
        wayland::clear();
    }
}

pub fn check_zombie() {
    std::thread::spawn(|| loop {
        let mut lock = CHILD_PROCESS.lock().unwrap();
        let mut i = 0;
        while i != lock.len() {
            let c = &mut (*lock)[i];
            if let Ok(Some(_)) = c.try_wait() {
                lock.remove(i);
            } else {
                i += 1;
            }
        }
        drop(lock);
        std::thread::sleep(Duration::from_millis(100));
    });
}

/// Start the host server that allows the remote peer to control the current machine.
///
/// # Arguments
///
/// * `is_server` - Whether the current client is definitely the server.
/// If true, the server will be started.
/// Otherwise, client will check if there's already a server and start one if not.
#[cfg(any(target_os = "android", target_os = "ios"))]
#[tokio::main]
pub async fn start_server(is_server: bool) {
    crate::RendezvousMediator::start_all().await;
}

/// Start the host server that allows the remote peer to control the current machine.
///
/// # Arguments
///
/// * `is_server` - Whether the current client is definitely the server.
/// If true, the server will be started.
/// Otherwise, client will check if there's already a server and start one if not.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::main]
pub async fn start_server(is_server: bool) {
    #[cfg(target_os = "linux")]
    {
        log::info!("DISPLAY={:?}", std::env::var("DISPLAY"));
        log::info!("XAUTHORITY={:?}", std::env::var("XAUTHORITY"));
    }
    #[cfg(feature = "hwcodec")]
    {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            scrap::hwcodec::check_config_process(false);
        })
    }

    if is_server {
        std::thread::spawn(move || {
            if let Err(err) = crate::ipc::start("") {
                log::error!("Failed to start ipc: {}", err);
                std::process::exit(-1);
            }
        });
        #[cfg(windows)]
        crate::platform::windows::bootstrap();
        input_service::fix_key_down_timeout_loop();
        #[cfg(target_os = "macos")]
        tokio::spawn(async { sync_and_watch_config_dir().await });
        crate::RendezvousMediator::start_all().await;
    } else {
        match crate::ipc::connect(1000, "").await {
            Ok(mut conn) => {
                if conn.send(&Data::SyncConfig(None)).await.is_ok() {
                    if let Ok(Some(data)) = conn.next_timeout(1000).await {
                        match data {
                            Data::SyncConfig(Some((config, config2))) => {
                                if Config::set(config) {
                                    log::info!("config synced");
                                }
                                if Config2::set(config2) {
                                    log::info!("config2 synced");
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(err) => {
                log::info!("server not started (will try to start): {}", err);
                std::thread::spawn(|| start_server(true));
            }
        }
    }
}

#[cfg(target_os = "macos")]
async fn sync_and_watch_config_dir() {
    if crate::platform::is_root() {
        return;
    }

    let mut cfg0 = (Config::get(), Config2::get());
    let mut synced = false;
    let tries =
        if std::env::args().len() == 2 && std::env::args().nth(1) == Some("--server".to_owned()) {
            30
        } else {
            3
        };
    log::debug!("#tries of ipc service connection: {}", tries);
    use hbb_common::sleep;
    for i in 1..=tries {
        sleep(i as f32 * 0.3).await;
        match crate::ipc::connect(1000, "_service").await {
            Ok(mut conn) => {
                if !synced {
                    if conn.send(&Data::SyncConfig(None)).await.is_ok() {
                        if let Ok(Some(data)) = conn.next_timeout(1000).await {
                            match data {
                                Data::SyncConfig(Some((config, config2))) => {
                                    let _chk = crate::ipc::CheckIfRestart::new();
                                    if cfg0.0 != config {
                                        cfg0.0 = config.clone();
                                        Config::set(config);
                                        log::info!("sync config from root");
                                    }
                                    if cfg0.1 != config2 {
                                        cfg0.1 = config2.clone();
                                        Config2::set(config2);
                                        log::info!("sync config2 from root");
                                    }
                                    synced = true;
                                }
                                _ => {}
                            };
                        };
                    }
                }

                loop {
                    sleep(0.3).await;
                    let cfg = (Config::get(), Config2::get());
                    if cfg != cfg0 {
                        log::info!("config updated, sync to root");
                        match conn.send(&Data::SyncConfig(Some(cfg.clone()))).await {
                            Err(e) => {
                                log::error!("sync config to root failed: {}", e);
                                break;
                            }
                            _ => {
                                cfg0 = cfg;
                                conn.next_timeout(1000).await.ok();
                            }
                        }
                    }
                }
            }
            Err(_) => {
                log::info!("#{} try: failed to connect to ipc_service", i);
            }
        }
    }
    log::warn!("skipped config sync");
}
