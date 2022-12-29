use crate::{
    config::{Config, NetworkType},
    tcp::FramedStream,
    udp::FramedSocket,
    ResultType,
};
use anyhow::Context;
use std::net::SocketAddr;
use tokio::net::ToSocketAddrs;
use tokio_socks::{IntoTargetAddr, TargetAddr};

pub fn test_if_valid_server(host: &str) -> String {
    let mut host = host.to_owned();
    if !host.contains(":") {
        host = format!("{}:{}", host, 0);
    }

    use std::net::ToSocketAddrs;
    match Config::get_network_type() {
        NetworkType::Direct => match host.to_socket_addrs() {
            Err(err) => err.to_string(),
            Ok(_) => "".to_owned(),
        },
        NetworkType::ProxySocks => match &host.into_target_addr() {
            Err(err) => err.to_string(),
            Ok(_) => "".to_owned(),
        },
    }
}

pub trait IsResolvedSocketAddr {
    fn resolve(&self) -> Option<&SocketAddr>;
}

impl IsResolvedSocketAddr for SocketAddr {
    fn resolve(&self) -> Option<&SocketAddr> {
        Some(&self)
    }
}

impl IsResolvedSocketAddr for String {
    fn resolve(&self) -> Option<&SocketAddr> {
        None
    }
}

impl IsResolvedSocketAddr for &str {
    fn resolve(&self) -> Option<&SocketAddr> {
        None
    }
}

#[inline]
pub async fn connect_tcp<
    't,
    T: IntoTargetAddr<'t> + ToSocketAddrs + IsResolvedSocketAddr + std::fmt::Display,
>(
    target: T,
    ms_timeout: u64,
) -> ResultType<FramedStream> {
    connect_tcp_local(target, None, ms_timeout).await
}

pub async fn connect_tcp_local<
    't,
    T: IntoTargetAddr<'t> + ToSocketAddrs + IsResolvedSocketAddr + std::fmt::Display,
>(
    target: T,
    local: Option<SocketAddr>,
    ms_timeout: u64,
) -> ResultType<FramedStream> {
    if let Some(conf) = Config::get_socks() {
        return FramedStream::connect(
            conf.proxy.as_str(),
            target,
            local,
            conf.username.as_str(),
            conf.password.as_str(),
            ms_timeout,
        )
        .await;
    }
    if let Some(target) = target.resolve() {
        if let Some(local) = local {
            if local.is_ipv6() && target.is_ipv4() {
                let target = query_nip_io(&target).await?;
                return Ok(FramedStream::new(target, Some(local), ms_timeout).await?);
            }
        }
    }
    Ok(FramedStream::new(target, local, ms_timeout).await?)
}

#[inline]
pub fn is_ipv4(target: &TargetAddr<'_>) -> bool {
    match target {
        TargetAddr::Ip(addr) => addr.is_ipv4(),
        _ => true,
    }
}

#[inline]
pub async fn query_nip_io(addr: &SocketAddr) -> ResultType<SocketAddr> {
    tokio::net::lookup_host(format!("{}.nip.io:{}", addr.ip(), addr.port()))
        .await?
        .filter(|x| x.is_ipv6())
        .next()
        .context("Failed to get ipv6 from nip.io")
}

#[inline]
pub fn ipv4_to_ipv6(addr: String, ipv4: bool) -> String {
    if !ipv4 && crate::is_ipv4_str(&addr) {
        if let Some(ip) = addr.split(":").next() {
            return addr.replace(ip, &format!("{}.nip.io", ip));
        }
    }
    addr
}

async fn test_is_ipv4(target: &str) -> bool {
    if let Ok(Ok(s)) = super::timeout(1000, tokio::net::TcpStream::connect(target)).await {
        return s.local_addr().map(|x| x.is_ipv4()).unwrap_or(true);
    }
    true
}

#[inline]
pub async fn new_udp_for(target: &str, ms_timeout: u64) -> ResultType<FramedSocket> {
    new_udp(
        Config::get_any_listen_addr(test_is_ipv4(target).await),
        ms_timeout,
    )
    .await
}

async fn new_udp<T: ToSocketAddrs>(local: T, ms_timeout: u64) -> ResultType<FramedSocket> {
    match Config::get_socks() {
        None => Ok(FramedSocket::new(local).await?),
        Some(conf) => {
            let socket = FramedSocket::new_proxy(
                conf.proxy.as_str(),
                local,
                conf.username.as_str(),
                conf.password.as_str(),
                ms_timeout,
            )
            .await?;
            Ok(socket)
        }
    }
}

pub async fn rebind_udp_for(target: &str) -> ResultType<Option<FramedSocket>> {
    match Config::get_network_type() {
        NetworkType::Direct => Ok(Some(
            FramedSocket::new(Config::get_any_listen_addr(test_is_ipv4(target).await)).await?,
        )),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use std::net::ToSocketAddrs;

    use super::*;

    #[test]
    fn test_nat64() {
        test_nat64_async();
    }

    #[tokio::main(flavor = "current_thread")]
    async fn test_nat64_async() {
        assert_eq!(ipv4_to_ipv6("1.1.1.1".to_owned(), true), "1.1.1.1");
        assert_eq!(ipv4_to_ipv6("1.1.1.1".to_owned(), false), "1.1.1.1.nip.io");
        assert_eq!(
            ipv4_to_ipv6("1.1.1.1:8080".to_owned(), false),
            "1.1.1.1.nip.io:8080"
        );
        assert_eq!(
            ipv4_to_ipv6("rustdesk.com".to_owned(), false),
            "rustdesk.com"
        );
        if ("rustdesk.com:80")
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap()
            .is_ipv6()
        {
            assert!(query_nip_io(&"1.1.1.1:80".parse().unwrap())
                .await
                .unwrap()
                .is_ipv6());
            return;
        }
        assert!(query_nip_io(&"1.1.1.1:80".parse().unwrap()).await.is_err());
    }
}
