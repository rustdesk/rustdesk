use crate::{bail, ResultType};
use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use protobuf::Message;
use socket2::{Domain, Socket, Type};
use std::{
    io::Error,
    net::SocketAddr,
    ops::{Deref, DerefMut},
};
use tokio::{net::ToSocketAddrs, net::UdpSocket};
use tokio_util::{codec::BytesCodec, udp::UdpFramed};

pub struct FramedSocket(UdpFramed<BytesCodec>);

impl Deref for FramedSocket {
    type Target = UdpFramed<BytesCodec>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn new_socket(addr: SocketAddr, reuse: bool) -> Result<Socket, std::io::Error> {
    let socket = match addr {
        SocketAddr::V4(..) => Socket::new(Domain::ipv4(), Type::dgram(), None),
        SocketAddr::V6(..) => Socket::new(Domain::ipv6(), Type::dgram(), None),
    }?;
    if reuse {
        // windows has no reuse_port, but it's reuse_address
        // almost equals to unix's reuse_port + reuse_address,
        // though may introduce nondeterministic bahavior
        #[cfg(unix)]
        socket.set_reuse_port(true)?;
        socket.set_reuse_address(true)?;
    }
    socket.bind(&addr.into())?;
    Ok(socket)
}

impl DerefMut for FramedSocket {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl FramedSocket {
    pub async fn new<T: ToSocketAddrs>(addr: T) -> ResultType<Self> {
        let socket = UdpSocket::bind(addr).await?;
        Ok(Self(UdpFramed::new(socket, BytesCodec::new())))
    }

    #[allow(clippy::never_loop)]
    pub async fn new_reuse<T: std::net::ToSocketAddrs>(addr: T) -> ResultType<Self> {
        for addr in addr.to_socket_addrs()? {
            return Ok(Self(UdpFramed::new(
                UdpSocket::from_std(new_socket(addr, true)?.into_udp_socket())?,
                BytesCodec::new(),
            )));
        }
        bail!("could not resolve to any address");
    }

    #[inline]
    pub async fn send(&mut self, msg: &impl Message, addr: SocketAddr) -> ResultType<()> {
        self.0
            .send((bytes::Bytes::from(msg.write_to_bytes().unwrap()), addr))
            .await?;
        Ok(())
    }

    #[inline]
    pub async fn send_raw(&mut self, msg: &'static [u8], addr: SocketAddr) -> ResultType<()> {
        self.0.send((bytes::Bytes::from(msg), addr)).await?;
        Ok(())
    }

    #[inline]
    pub async fn next(&mut self) -> Option<Result<(BytesMut, SocketAddr), Error>> {
        self.0.next().await
    }

    #[inline]
    pub async fn next_timeout(&mut self, ms: u64) -> Option<Result<(BytesMut, SocketAddr), Error>> {
        if let Ok(res) =
            tokio::time::timeout(std::time::Duration::from_millis(ms), self.0.next()).await
        {
            res
        } else {
            None
        }
    }
}
