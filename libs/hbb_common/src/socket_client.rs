use crate::{
    config::{Config, NetworkType, RENDEZVOUS_TIMEOUT},
    tcp::FramedStream,
    udp::FramedSocket,
    ResultType,
};
use anyhow::{bail, Context};
use std::net::SocketAddr;
use tokio::net::ToSocketAddrs;
use tokio_socks::IntoTargetAddr;

// fn get_socks5_conf() -> Option<Socks5Server> {
//     // Config::set_socks(Some(Socks5Server {
//     //     proxy: "139.186.136.143:1080".to_owned(),
//     //     ..Default::default()
//     // }));
//     Config::get_socks()
// }

pub async fn connect_tcp<'t, T: IntoTargetAddr<'t>>(
    target: T,
    local: SocketAddr,
    ms_timeout: u64,
) -> ResultType<FramedStream> {
    let target_addr = target.into_target_addr()?;

    if let Some(conf) = Config::get_socks() {
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

fn native_to_socket_addr(host: &str) -> ResultType<SocketAddr> {
    use std::net::ToSocketAddrs;
    let addrs: Vec<SocketAddr> = host.to_socket_addrs()?.collect();
    if addrs.is_empty() {
        bail!("Failed to solve {}", host);
    }
    Ok(addrs[0])
}

pub async fn to_socket_addr(host: &str) -> ResultType<SocketAddr> {
    Ok(
        new_udp(host, Config::get_any_listen_addr(), RENDEZVOUS_TIMEOUT)
            .await?
            .1,
    )
}

pub async fn new_udp<'t, T1: IntoTargetAddr<'t> + std::fmt::Display, T2: ToSocketAddrs>(
    target: T1,
    local: T2,
    ms_timeout: u64,
) -> ResultType<(FramedSocket, SocketAddr)> {
    match Config::get_socks() {
        None => Ok((
            FramedSocket::new(local).await?,
            native_to_socket_addr(&target.to_string())?,
        )),
        Some(conf) => {
            let (socket, addr) = FramedSocket::new_proxy(
                conf.proxy.as_str(),
                target,
                local,
                conf.username.as_str(),
                conf.password.as_str(),
                ms_timeout,
            )
            .await?;
            Ok((socket, addr))
        }
    }
}

pub async fn rebind<T: ToSocketAddrs>(local: T) -> ResultType<Option<FramedSocket>> {
    match Config::get_network_type() {
        NetworkType::Direct => Ok(Some(FramedSocket::new(local).await?)),
        _ => Ok(None),
    }
}
