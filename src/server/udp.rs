/// udp server
///
/// eg. discovery
///
use hbb_common::{base_proto::PeerInfo, config::SERVER_UDP_PORT, ResultType};
use socket_cs::udp::{Server, UdpHandlers};

/// Simple copy from ../connections.rs#send_logon_response
/// Should be merged into one function.
fn get_peer_info() -> PeerInfo {
    let username = crate::platform::get_active_username();
    #[allow(unused_mut)]
    let mut sas_enabled = false;
    #[cfg(windows)]
    if crate::platform::is_root() {
        sas_enabled = true;
    }
    PeerInfo {
        hostname: whoami::hostname(),
        username,
        platform: whoami::platform().to_string(),
        version: crate::VERSION.to_owned(),
        sas_enabled,
        ..Default::default()
    }
}

pub mod discovery {
    use super::get_peer_info;
    use crate::ipc;
    use hbb_common::{
        config::{PeerConfig, PeerInfoSerde, SERVER_UDP_PORT},
        discovery_proto::{Discovery as DiscoveryProto, DiscoveryBack as DiscoveryBackProto},
        log, protobuf, tokio, ResultType,
    };
    use socket_cs::{discovery::*, udp::UdpHandlers};

    async fn get_discovery_back_info() -> DiscoveryBackProto {
        let peer = get_peer_info();
        DiscoveryBackProto {
            id: ipc::get_id_async().await,
            peer: protobuf::MessageField::from_option(Some(peer)),
            ..Default::default()
        }
    }

    /// process sicovery bakc(response)
    /// 1. update current peers.
    /// 2. notify index window to udpate recent sessions.
    fn process_discovery_back(info: DiscoveryBackProto) {
        let mut config = PeerConfig::load(info.id.as_str());

        let peer = info.peer.as_ref().unwrap();
        let serde = PeerInfoSerde {
            username: peer.username.clone(),
            hostname: peer.hostname.clone(),
            platform: peer.platform.clone(),
        };
        config.info = serde;
        config.store(info.id.as_str());

        #[tokio::main(flavor = "current_thread")]
        async fn notify_index_window() -> ResultType<()> {
            let ms_timeout = 300;
            let mut c = ipc::connect(ms_timeout, "_index").await?;
            c.send(&ipc::Data::SessionsUpdated).await?;
            Ok(())
        }
        std::thread::spawn(move || {
            if let Err(e) = notify_index_window() {
                log::error!("Failed to notify index window, {}", e);
            }
        });
    }

    /// launch lan discover when user click "discover" button.
    pub fn launch_lan_discover() {
        std::thread::spawn(move || {
            if let Err(e) = lan_discover() {
                log::error!("Failed to lauch lan discover, {}", e);
            }
        });
    }

    #[tokio::main(flavor = "current_thread")]
    pub async fn lan_discover() -> ResultType<()> {
        let peer = get_peer_info();
        let client = DiscoveryClient::create(DiscoveryProto {
            id: ipc::get_id_async().await,
            peer: protobuf::MessageField::from_option(Some(peer)),
            port: SERVER_UDP_PORT as i32,
            ..Default::default()
        })
        .await?;

        client.lan_discover(SERVER_UDP_PORT).await
    }

    pub(super) async fn handle_discovery(handlers: UdpHandlers) -> UdpHandlers {
        let info = get_discovery_back_info().await;
        handlers
            // handle discover request
            .handle(
                CMD_DISCOVERY.as_bytes().to_vec(),
                Box::new(HandlerDiscovery::new(
                    // Some(|| Config::get_option("enable-be-discovered") == "Y".to_owned()),
                    Some(|| true),
                    info,
                )),
            )
            // handle discover back(response)
            .handle(
                CMD_DISCOVERY_BACK.as_bytes().to_vec(),
                Box::new(HandlerDiscoveryBack::new(process_discovery_back)),
            )
    }
}

pub(super) async fn start_udp_server() -> ResultType<Server> {
    let handlers = discovery::handle_discovery(UdpHandlers::new()).await;

    let mut server = Server::create(SERVER_UDP_PORT)?;
    server.start(handlers).await?;
    Ok(server)
}
