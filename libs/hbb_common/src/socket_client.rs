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

#[inline]
pub fn check_port<T: std::string::ToString>(host: T, port: i32) -> String {
    let host = host.to_string();
    if crate::is_ipv6_str(&host) {
        if host.starts_with('[') {
            return host;
        }
        return format!("[{host}]:{port}");
    }
    if !host.contains(':') {
        return format!("{host}:{port}");
    }
    host
}

#[inline]
pub fn increase_port<T: std::string::ToString>(host: T, offset: i32) -> String {
    let host = host.to_string();
    if crate::is_ipv6_str(&host) {
        if host.starts_with('[') {
            let tmp: Vec<&str> = host.split("]:").collect();
            if tmp.len() == 2 {
                let port: i32 = tmp[1].parse().unwrap_or(0);
                if port > 0 {
                    return format!("{}]:{}", tmp[0], port + offset);
                }
            }
        }
    } else if host.contains(':') {
        let tmp: Vec<&str> = host.split(':').collect();
        if tmp.len() == 2 {
            let port: i32 = tmp[1].parse().unwrap_or(0);
            if port > 0 {
                return format!("{}:{}", tmp[0], port + offset);
            }
        }
    }
    host
}

pub fn test_if_valid_server(host: &str) -> String {
    let host = check_port(host, 0);

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
        Some(self)
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
                let target = query_nip_io(target).await?;
                return FramedStream::new(target, Some(local), ms_timeout).await;
            }
        }
    }
    FramedStream::new(target, local, ms_timeout).await
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
        .find(|x| x.is_ipv6())
        .context("Failed to get ipv6 from nip.io")
}

#[inline]
pub fn ipv4_to_ipv6(addr: String, ipv4: bool) -> String {
    if !ipv4 && crate::is_ipv4_str(&addr) {
        if let Some(ip) = addr.split(':').next() {
            return addr.replace(ip, &format!("{ip}.nip.io"));
        }
    }
    addr
}

async fn test_target(target: &str) -> ResultType<SocketAddr> {
    if let Ok(Ok(s)) = super::timeout(1000, tokio::net::TcpStream::connect(target)).await {
        if let Ok(addr) = s.peer_addr() {
            return Ok(addr);
        }
    }
    tokio::net::lookup_host(target)
        .await?
        .next()
        .context(format!("Failed to look up host for {target}"))
}

#[inline]
pub async fn new_udp_for(
    target: &str,
    ms_timeout: u64,
) -> ResultType<(FramedSocket, TargetAddr<'static>)> {
    let (ipv4, target) = if NetworkType::Direct == Config::get_network_type() {
        let addr = test_target(target).await?;
        (addr.is_ipv4(), addr.into_target_addr()?)
    } else {
        (true, target.into_target_addr()?)
    };
    Ok((
        new_udp(Config::get_any_listen_addr(ipv4), ms_timeout).await?,
        target.to_owned(),
    ))
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

pub async fn rebind_udp_for(
    target: &str,
) -> ResultType<Option<(FramedSocket, TargetAddr<'static>)>> {
    if Config::get_network_type() != NetworkType::Direct {
        return Ok(None);
    }
    let addr = test_target(target).await?;
    let v4 = addr.is_ipv4();
    Ok(Some((
        FramedSocket::new(Config::get_any_listen_addr(v4)).await?,
        addr.into_target_addr()?.to_owned(),
    )))
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

    #[test]
    fn test_test_if_valid_server() {
        assert!(!test_if_valid_server("a").is_empty());
        // on Linux, "1" is resolved to "0.0.0.1"
        assert!(test_if_valid_server("1.1.1.1").is_empty());
        assert!(test_if_valid_server("1.1.1.1:1").is_empty());
    }

    #[test]
    fn test_check_port() {
        assert_eq!(check_port("[1:2]:12", 32), "[1:2]:12");
        assert_eq!(check_port("1:2", 32), "[1:2]:32");
        assert_eq!(check_port("z1:2", 32), "z1:2");
        assert_eq!(check_port("1.1.1.1", 32), "1.1.1.1:32");
        assert_eq!(check_port("1.1.1.1:32", 32), "1.1.1.1:32");
        assert_eq!(check_port("test.com:32", 0), "test.com:32");
        assert_eq!(increase_port("[1:2]:12", 1), "[1:2]:13");
        assert_eq!(increase_port("1.2.2.4:12", 1), "1.2.2.4:13");
        assert_eq!(increase_port("1.2.2.4", 1), "1.2.2.4");
        assert_eq!(increase_port("test.com", 1), "test.com");
        assert_eq!(increase_port("test.com:13", 4), "test.com:17");
        assert_eq!(increase_port("1:13", 4), "1:13");
        assert_eq!(increase_port("22:1:13", 4), "22:1:13");
        assert_eq!(increase_port("z1:2", 1), "z1:3");
    }
}
