use crate::{
    config::{Config, NetworkType},
    tcp::FramedStream,
    udp::FramedSocket,
    ResultType,
};
use anyhow::Context;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use tokio_socks::{IntoTargetAddr, TargetAddr};

fn to_socket_addr<T: ToSocketAddrs>(host: T) -> ResultType<SocketAddr> {
    let mut addr_ipv4 = None;
    let mut addr_ipv6 = None;
    for addr in host.to_socket_addrs()? {
        if addr.is_ipv4() && addr_ipv4.is_none() {
            addr_ipv4 = Some(addr);
        }
        if addr.is_ipv6() && addr_ipv6.is_none() {
            addr_ipv6 = Some(addr);
        }
    }
    if let Some(addr) = addr_ipv4 {
        Ok(addr)
    } else {
        addr_ipv6.context("Failed to solve")
    }
}

pub fn get_target_addr(host: &str) -> ResultType<TargetAddr<'static>> {
    let addr = match Config::get_network_type() {
        NetworkType::Direct => to_socket_addr(&host)?.into_target_addr()?,
        NetworkType::ProxySocks => host.into_target_addr()?,
    }
    .to_owned();
    Ok(addr)
}

pub fn test_if_valid_server(host: &str) -> String {
    let mut host = host.to_owned();
    if !host.contains(":") {
        host = format!("{}:{}", host, 0);
    }

    match Config::get_network_type() {
        NetworkType::Direct => match to_socket_addr(&host) {
            Err(err) => err.to_string(),
            Ok(_) => "".to_owned(),
        },
        NetworkType::ProxySocks => match &host.into_target_addr() {
            Err(err) => err.to_string(),
            Ok(_) => "".to_owned(),
        },
    }
}

pub trait IntoTargetAddr2<'a> {
    /// Converts the value of self to a `TargetAddr`.
    fn into_target_addr2(&self) -> ResultType<TargetAddr<'a>>;
}

impl<'a> IntoTargetAddr2<'a> for SocketAddr {
    fn into_target_addr2(&self) -> ResultType<TargetAddr<'a>> {
        Ok(TargetAddr::Ip(*self))
    }
}

impl<'a> IntoTargetAddr2<'a> for TargetAddr<'a> {
    fn into_target_addr2(&self) -> ResultType<TargetAddr<'a>> {
        Ok(self.clone())
    }
}

impl<'a> IntoTargetAddr2<'a> for String {
    fn into_target_addr2(&self) -> ResultType<TargetAddr<'a>> {
        Ok(to_socket_addr(self)?.into_target_addr()?)
    }
}

impl<'a> IntoTargetAddr2<'a> for &str {
    fn into_target_addr2(&self) -> ResultType<TargetAddr<'a>> {
        Ok(to_socket_addr(self)?.into_target_addr()?)
    }
}

pub async fn connect_tcp<'t, T: IntoTargetAddr2<'t> + std::fmt::Debug>(
    target: T,
    ms_timeout: u64,
) -> ResultType<FramedStream> {
    let target_addr = target.into_target_addr2()?;
    let local = Config::get_any_listen_addr(is_ipv4(&target_addr));
    connect_tcp_local(target_addr, local, ms_timeout)
        .await
        .context(format!("Invalid target addr: {:?}", target))
}

pub async fn connect_tcp_local<'t, T: IntoTargetAddr<'t> + std::fmt::Debug>(
    target: T,
    local: SocketAddr,
    ms_timeout: u64,
) -> ResultType<FramedStream> {
    let target_addr = target.into_target_addr()?;
    if let Some(conf) = Config::get_socks() {
        return FramedStream::connect(
            conf.proxy.as_str(),
            target_addr,
            local,
            conf.username.as_str(),
            conf.password.as_str(),
            ms_timeout,
        )
        .await;
    }
    let mut addr = ToSocketAddrs::to_socket_addrs(&target_addr)?
        .next()
        .context(format!("Invalid target addr: {:?}", target_addr))?;
    if local.is_ipv6() && addr.is_ipv4() {
        addr = query_nip_io(&addr)?;
    }
    Ok(FramedStream::new(addr, local, ms_timeout).await?)
}

#[inline]
pub fn is_ipv4(target: &TargetAddr<'_>) -> bool {
    match target {
        TargetAddr::Ip(addr) => addr.is_ipv4(),
        _ => true,
    }
}

#[inline]
pub fn query_nip_io(addr: &SocketAddr) -> ResultType<SocketAddr> {
    to_socket_addr(format!("{}.nip.io:{}", addr.ip(), addr.port()))
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

pub async fn new_udp_for(target: &TargetAddr<'_>, ms_timeout: u64) -> ResultType<FramedSocket> {
    new_udp(Config::get_any_listen_addr(is_ipv4(target)), ms_timeout).await
}

async fn new_udp<T: ToSocketAddrs>(local: T, ms_timeout: u64) -> ResultType<FramedSocket> {
    match Config::get_socks() {
        None => Ok(FramedSocket::new(to_socket_addr(&local)?).await?),
        Some(conf) => {
            let socket = FramedSocket::new_proxy(
                conf.proxy.as_str(),
                to_socket_addr(local)?,
                conf.username.as_str(),
                conf.password.as_str(),
                ms_timeout,
            )
            .await?;
            Ok(socket)
        }
    }
}

pub async fn rebind_udp_for(target: &TargetAddr<'_>) -> ResultType<Option<FramedSocket>> {
    match Config::get_network_type() {
        NetworkType::Direct => Ok(Some(
            FramedSocket::new(Config::get_any_listen_addr(is_ipv4(target))).await?,
        )),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_to_socket_addr() {
        assert_eq!(
            to_socket_addr("127.0.0.1:8080").unwrap(),
            "127.0.0.1:8080".parse().unwrap()
        );
        assert!(to_socket_addr("[ff::]:0").unwrap().is_ipv6());
        assert!(to_socket_addr("xx").is_err());
    }

    #[test]
    fn test_nat64() {
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
        if to_socket_addr("rustdesk.com:80").unwrap().is_ipv6() {
            assert!(query_nip_io(&"1.1.1.1:80".parse().unwrap())
                .unwrap()
                .is_ipv6());
            return;
        }
        assert_eq!(
            query_nip_io(&"1.1.1.1:80".parse().unwrap()).unwrap(),
            "1.1.1.1:80".parse().unwrap()
        );
    }
}
