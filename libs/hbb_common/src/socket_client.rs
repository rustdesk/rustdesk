use crate::{
    config::{Config, Socks5Server},
    tcp::FramedStream,
    udp::{FramedSocket, UdpFramedWrapper},
    ResultType,
};
use anyhow::{bail, Context};
use std::net::SocketAddr;
use tokio::net::ToSocketAddrs;
use tokio_socks::{udp::Socks5UdpFramed, IntoTargetAddr};
use tokio_util::{codec::BytesCodec, udp::UdpFramed};

pub fn get_socks5_conf() -> Option<Socks5Server> {
    // Config::set_socks(Some(Socks5Server {
    //     proxy: "139.186.136.143:1080".to_owned(),
    //     ..Default::default()
    // }));
    Config::get_socks()
}

pub async fn connect_tcp<'t, T: IntoTargetAddr<'t>>(
    target: T,
    local: SocketAddr,
    ms_timeout: u64,
) -> ResultType<FramedStream> {
    let target_addr = target.into_target_addr()?;

    if let Some(conf) = get_socks5_conf() {
        FramedStream::connect(
            conf.proxy.as_str(),
            target_addr,
            local,
            conf.username.as_str(),
            conf.password.as_str(),
            ms_timeout,
        )
        .await
    } else {
        let addrs: Vec<SocketAddr> =
            std::net::ToSocketAddrs::to_socket_addrs(&target_addr)?.collect();
        if addrs.is_empty() {
            bail!("Invalid target addr");
        };

        FramedStream::new(addrs[0], local, ms_timeout)
            .await
            .with_context(|| "Failed to connect to rendezvous server")
    }
}

// TODO: merge connect_udp and connect_udp_socks5
pub async fn connect_udp_socket<T1: ToSocketAddrs>(
    local: T1,
) -> ResultType<(
    FramedSocket<UdpFramedWrapper<UdpFramed<BytesCodec>>>,
    Option<SocketAddr>,
)> {
    Ok((FramedSocket::new(local).await?, None))
}

pub async fn connect_udp_socks5<'t, T1: IntoTargetAddr<'t>, T2: ToSocketAddrs>(
    target: T1,
    local: T2,
    socks5: &Option<Socks5Server>,
    ms_timeout: u64,
) -> ResultType<(
    FramedSocket<UdpFramedWrapper<Socks5UdpFramed>>,
    Option<SocketAddr>,
)> {
    match socks5 {
        Some(conf) => {
            let (socket, addr) = FramedSocket::connect(
                conf.proxy.as_str(),
                target,
                local,
                conf.username.as_str(),
                conf.password.as_str(),
                ms_timeout,
            )
            .await?;
            Ok((socket, Some(addr)))
        }
        None => {
            bail!("Nil socks5 server config")
        }
    }
}
