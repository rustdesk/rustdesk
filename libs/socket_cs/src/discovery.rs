use super::udp::UdpRequest;
use async_trait::async_trait;
use hbb_common::{
    base_proto::PeerInfo, discovery_proto::Discovery as DiscoveryProto, log,
    protobuf::Message, tokio::net::UdpSocket, ResultType,
};
use std::net::SocketAddr;

pub const CMD_DISCOVERY: &str = "discovery";
pub const CMD_DISCOVERY_BACK: &str = "discovery_back";

// TODO: make sure if UdpFramed is needed, or UdpSocket just works fine.
pub struct DiscoveryClient {
    socket: UdpSocket,
    send_data: Vec<u8>,
}

fn make_send_data(cmd: &str, msg: &impl Message) -> ResultType<Vec<u8>> {
    let info_bytes = msg.write_to_bytes()?;
    let mut send_data = cmd.as_bytes().to_vec();
    send_data.push(crate::CMD_TOKEN);
    send_data.extend(info_bytes);
    Ok(send_data)
}

impl DiscoveryClient {
    pub async fn create(info: DiscoveryProto) -> ResultType<Self> {
        let addr = "0.0.0.0:0";
        let socket = UdpSocket::bind(addr).await?;
        log::trace!("succeeded to bind {} for discovery client", addr);

        socket.set_broadcast(true)?;
        log::info!("Broadcast mode set to ON");

        let send_data = make_send_data(CMD_DISCOVERY, &info)?;
        Ok(Self {
            socket,
            send_data,
        })
    }

    pub async fn lan_discover(&self, peer_port: u16) -> ResultType<()> {
        let addr = SocketAddr::from(([255, 255, 255, 255], peer_port));
        self.socket.send_to(&self.send_data, addr).await?;
        Ok(())
    }
}

pub struct HandlerDiscovery {
    send_data: Vec<u8>,
}

impl HandlerDiscovery {
    pub fn new(self_info: PeerInfo) -> Self {
        let send_data = make_send_data(CMD_DISCOVERY_BACK, &self_info).unwrap();
        Self { send_data }
    }
}

#[async_trait]
impl crate::Handler<UdpRequest> for HandlerDiscovery {
    async fn call(&self, request: UdpRequest) -> ResultType<()> {
        log::trace!("received discover query from {}", request.addr);

        let discovery = DiscoveryProto::parse_from_bytes(&request.data)?;
        let peer = discovery.peer.as_ref().take().unwrap();
        log::debug!(
            "received discovery query from {} {}",
            peer.username,
            peer.hostname
        );

        let addr = "0.0.0.0:0";
        let socket = match UdpSocket::bind(addr).await {
            Ok(s) => s,
            Err(e) => {
                log::error!("cannot bind socket? {}", e);
                return Ok(());
            }
        };

        let mut peer_addr = request.addr;
        peer_addr.set_port(discovery.port as u16);

        // let peer_addr = SocketAddr::from(([255, 255, 255, 255], discovery.port as u16));
        // socket.set_broadcast(true).unwrap();
        log::debug!("send self peer info to {}", peer_addr);

        let send_len = self.send_data.len();
        let mut cur_len = 0usize;
        while cur_len < send_len {
            let len = socket.send_to(&self.send_data[cur_len..], peer_addr).await?;
            cur_len += len;
        }
        log::trace!("send self peer info to {} done", peer_addr);

        Ok(())
    }
}

pub struct HandlerDiscoveryBack {
    proc: fn(peer_info: PeerInfo),
}

impl HandlerDiscoveryBack {
    pub fn new(proc: fn(peer_info: PeerInfo)) -> Self {
        Self { proc }
    }
}

#[async_trait]
impl crate::Handler<UdpRequest> for HandlerDiscoveryBack {
    async fn call(&self, request: UdpRequest) -> ResultType<()> {
        log::trace!("recved discover back from {}", request.addr);

        let peer = PeerInfo::parse_from_bytes(&request.data)?;
        (self.proc)(peer);
        Ok(())
    }
}
