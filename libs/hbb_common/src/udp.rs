use crate::{bail, ResultType};
use anyhow::anyhow;
use bytes::{Bytes, BytesMut};
use futures::{SinkExt, StreamExt};
use futures_core::Stream;
use futures_sink::Sink;
use pin_project::pin_project;
use protobuf::Message;
use socket2::{Domain, Socket, Type};
use std::{
    net::SocketAddr,
    ops::{Deref, DerefMut},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::net::{ToSocketAddrs, UdpSocket};
use tokio_socks::{
    udp::{Socks5UdpFramed, Socks5UdpMessage},
    IntoTargetAddr, TargetAddr, ToProxyAddrs,
};
use tokio_util::{codec::BytesCodec, udp::UdpFramed};

pub struct FramedSocket<F>(F);

#[pin_project]
pub struct UdpFramedWrapper<F>(#[pin] F);

pub trait BytesMutGetter<'a> {
    fn get_bytes_mut(&'a self) -> &'a BytesMut;
}

impl<F> Deref for FramedSocket<F> {
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<F> DerefMut for FramedSocket<F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<F> Deref for UdpFramedWrapper<F> {
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<F> DerefMut for UdpFramedWrapper<F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
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
        // though may introduce nondeterministic behavior
        #[cfg(unix)]
        socket.set_reuse_port(true)?;
        socket.set_reuse_address(true)?;
    }
    socket.bind(&addr.into())?;
    Ok(socket)
}

impl FramedSocket<UdpFramedWrapper<UdpFramed<BytesCodec>>> {
    pub async fn new<T: ToSocketAddrs>(addr: T) -> ResultType<Self> {
        let socket = UdpSocket::bind(addr).await?;
        Ok(Self(UdpFramedWrapper(UdpFramed::new(
            socket,
            BytesCodec::new(),
        ))))
    }

    #[allow(clippy::never_loop)]
    pub async fn new_reuse<T: std::net::ToSocketAddrs>(addr: T) -> ResultType<Self> {
        for addr in addr.to_socket_addrs()? {
            let socket = new_socket(addr, true)?.into_udp_socket();
            return Ok(Self(UdpFramedWrapper(UdpFramed::new(
                UdpSocket::from_std(socket)?,
                BytesCodec::new(),
            ))));
        }
        bail!("could not resolve to any address");
    }
}

impl FramedSocket<UdpFramedWrapper<Socks5UdpFramed>> {
    pub async fn connect<'a, 't, P: ToProxyAddrs, T1: IntoTargetAddr<'t>, T2: ToSocketAddrs>(
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
        Ok((Self(UdpFramedWrapper(framed)), addr))
    }
}

// TODO: simplify this constraint
impl<F> FramedSocket<F>
where
    F: Unpin + Stream + Sink<(Bytes, SocketAddr)>,
    <F as Sink<(Bytes, SocketAddr)>>::Error: Sync + Send + std::error::Error + 'static,
{
    pub async fn new_with(self) -> ResultType<Self> {
        Ok(self)
    }

    #[inline]
    pub async fn send(&mut self, msg: &impl Message, addr: SocketAddr) -> ResultType<()> {
        self.0
            .send((Bytes::from(msg.write_to_bytes().unwrap()), addr))
            .await?;
        Ok(())
    }

    #[inline]
    pub async fn send_raw(&mut self, msg: &'static [u8], addr: SocketAddr) -> ResultType<()> {
        self.0.send((Bytes::from(msg), addr)).await?;
        Ok(())
    }

    #[inline]
    pub async fn next(&mut self) -> Option<<F as Stream>::Item> {
        self.0.next().await
    }

    #[inline]
    pub async fn next_timeout(&mut self, ms: u64) -> Option<<F as Stream>::Item> {
        if let Ok(res) =
            tokio::time::timeout(std::time::Duration::from_millis(ms), self.0.next()).await
        {
            res
        } else {
            None
        }
    }
}

impl<'a> BytesMutGetter<'a> for BytesMut {
    fn get_bytes_mut(&'a self) -> &'a BytesMut {
        self
    }
}

impl<'a> BytesMutGetter<'a> for Socks5UdpMessage {
    fn get_bytes_mut(&'a self) -> &'a BytesMut {
        &self.data
    }
}

impl<F, M, E> Stream for UdpFramedWrapper<F>
where
    F: Stream<Item = std::result::Result<(M, SocketAddr), E>>,
    for<'b> M: BytesMutGetter<'b> + std::fmt::Debug,
    E: std::error::Error + Into<anyhow::Error>,
{
    type Item = ResultType<(BytesMut, SocketAddr)>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().0.poll_next(cx) {
            Poll::Ready(Some(Ok((msg, addr)))) => {
                Poll::Ready(Some(Ok((msg.get_bytes_mut().clone(), addr))))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(anyhow!(e)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<F> Sink<(Bytes, SocketAddr)> for UdpFramedWrapper<F>
where
    F: Sink<(Bytes, SocketAddr)>,
{
    type Error = <F as Sink<(Bytes, SocketAddr)>>::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().0.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: (Bytes, SocketAddr)) -> Result<(), Self::Error> {
        self.project().0.start_send(item)
    }

    #[allow(unused_mut)]
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().0.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().0.poll_close(cx)
    }
}
