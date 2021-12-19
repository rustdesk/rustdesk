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

mod discovery {
    use super::get_peer_info;
    use crate::ipc;
    use hbb_common::{
        base_proto::PeerInfo,
        config::{PeerConfig, PeerInfoSerde},
        discovery_proto::{Discovery as DiscoveryProto, DiscoveryBack as DiscoveryBackProto},
        log, protobuf,
        tokio::runtime::Runtime,
        ResultType,
    };
    use socket_cs::{discovery::*, udp::UdpHandlers};

    fn get_discovery_back_info() -> DiscoveryBackProto {
        let peer = get_peer_info();
        DiscoveryBackProto {
            id: ipc::get_id(),
            peer: protobuf::MessageField::from_option(Some(peer)),
            ..Default::default()
        }
    }

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

        let rt = match Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                log::error!("Failed to notify index window, {}", e);
                return;
            }
        };

        async fn notify_index_window() -> ResultType<()> {
            let ms_timeout = 1000;
            let mut c = ipc::connect(ms_timeout, "_index").await?;
            c.send(&ipc::Data::SessionsUpdated).await?;
            Ok(())
        }
        rt.block_on(async move {
            if let Err(e) = notify_index_window().await {
                log::error!("Failed to notify index window, {}", e);
            }
        });
    }

    // pub(crate) fn lan_discover();

    pub(super) fn handle_discovery(handlers: UdpHandlers) -> UdpHandlers {
        let info = get_discovery_back_info();
        handlers
            .handle(
                CMD_DISCOVERY.as_bytes().to_vec(),
                Box::new(HandlerDiscovery::new(Some(|| true), info)),
            )
            .handle(
                CMD_DISCOVERY_BACK.as_bytes().to_vec(),
                Box::new(HandlerDiscoveryBack::new(process_discovery_back)),
            )
    }
}

pub(super) async fn start_udp_server() -> ResultType<Server> {
    let handlers = discovery::handle_discovery(UdpHandlers::new());

    let mut server = Server::create(SERVER_UDP_PORT)?;
    server.start(handlers).await?;
    Ok(server)
}
