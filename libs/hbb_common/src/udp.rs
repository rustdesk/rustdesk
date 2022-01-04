use crate::{bail, ResultType};
use anyhow::anyhow;
use bytes::{Bytes, BytesMut};
use futures::{SinkExt, StreamExt};
use protobuf::Message;
use socket2::{Domain, Socket, Type};
use std::net::SocketAddr;
use tokio::net::{ToSocketAddrs, UdpSocket};
use tokio_socks::{udp::Socks5UdpFramed, IntoTargetAddr, TargetAddr, ToProxyAddrs};
use tokio_util::{codec::BytesCodec, udp::UdpFramed};

pub enum FramedSocket {
    Direct(UdpFramed<BytesCodec>),
    ProxySocks(Socks5UdpFramed),
}

fn new_socket(addr: SocketAddr, reuse: bool) -> Result<Socket, std::io::Error> {
    let socket = match addr {
        SocketAddr::V4(..) => Socket::new(Domain::ipv4(), Type::dgram(), None),
        SocketAddr::V6(..) => Socket::new(Domain::ipv6(), Type::dgram(), None),
    }?;
    if reuse {
        // windows has no reuse_port, but it's reuse_address
        // almost equals to unix's reuse_port + reuse_address,
        // though may introduce nondeterministic behavior
        #[cfg(unix)]
        socket.set_reuse_port(true)?;
        socket.set_reuse_address(true)?;
    }
    socket.bind(&addr.into())?;
    Ok(socket)
}

impl FramedSocket {
    pub async fn new<T: ToSocketAddrs>(addr: T) -> ResultType<Self> {
        let socket = UdpSocket::bind(addr).await?;
        Ok(Self::Direct(UdpFramed::new(socket, BytesCodec::new())))
    }

    #[allow(clippy::never_loop)]
    pub async fn new_reuse<T: std::net::ToSocketAddrs>(addr: T) -> ResultType<Self> {
        for addr in addr.to_socket_addrs()? {
            let socket = new_socket(addr, true)?.into_udp_socket();
            return Ok(Self::Direct(UdpFramed::new(
                UdpSocket::from_std(socket)?,
                BytesCodec::new(),
            )));
        }
        bail!("could not resolve to any address");
    }

    pub async fn new_proxy<'a, 't, P: ToProxyAddrs, T1: IntoTargetAddr<'t>, T2: ToSocketAddrs>(
        proxy: P,
        target: T1,
        local: T2,
        username: &'a str,
        password: &'a str,
        ms_timeout: u64,
    ) -> ResultType<(Self, SocketAddr)> {
        let framed = if username.trim().is_empty() {
            super::timeout(
                ms_timeout,
                Socks5UdpFramed::connect(proxy, target, Some(local)),
            )
            .await??
        } else {
            super::timeout(
                ms_timeout,
                Socks5UdpFramed::connect_with_password(
                    proxy,
                    target,
                    Some(local),
                    username,
                    password,
                ),
            )
            .await??
        };
        let addr = if let TargetAddr::Ip(c) = framed.target_addr() {
            c
        } else {
            unreachable!()
        };
        log::trace!(
            "Socks5 udp connected, local addr: {}, target addr: {}",
            framed.local_addr().unwrap(),
            &addr
        );
        Ok((Self::ProxySocks(framed), addr))
    }

    #[inline]
    pub async fn send(&mut self, msg: &impl Message, addr: SocketAddr) -> ResultType<()> {
        let send_data = (Bytes::from(msg.write_to_bytes().unwrap()), addr);
        let _ = match self {
            Self::Direct(f) => f.send(send_data).await?,
            Self::ProxySocks(f) => f.send(send_data).await?,
        };
        Ok(())
    }

    #[inline]
    pub async fn send_raw(&mut self, msg: &'static [u8], addr: SocketAddr) -> ResultType<()> {
        let _ = match self {
            Self::Direct(f) => f.send((Bytes::from(msg), addr)).await?,
            Self::ProxySocks(f) => f.send((Bytes::from(msg), addr)).await?,
        };
        Ok(())
    }

    #[inline]
    pub async fn next(&mut self) -> Option<ResultType<(BytesMut, SocketAddr)>> {
        match self {
            Self::Direct(f) => match f.next().await {
                Some(Ok((data, addr))) => Some(Ok((data, addr))),
                Some(Err(e)) => Some(Err(anyhow!(e))),
                None => None,
            },
            Self::ProxySocks(f) => match f.next().await {
                Some(Ok((data, addr))) => Some(Ok((data.data, addr))),
                Some(Err(e)) => Some(Err(anyhow!(e))),
                None => None,
            },
        }
    }

    #[inline]
    pub async fn next_timeout(&mut self, ms: u64) -> Option<ResultType<(BytesMut, SocketAddr)>> {
        if let Ok(res) =
            tokio::time::timeout(std::time::Duration::from_millis(ms), self.next()).await
        {
            res
        } else {
            None
        }
    }
}
