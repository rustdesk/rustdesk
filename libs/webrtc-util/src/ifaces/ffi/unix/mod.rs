use crate::ifaces::{Interface, Kind, NextHop};

use nix::sys::socket::{AddressFamily, SockaddrLike, SockaddrStorage};
use std::io::Error;
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};

fn ss_to_netsa(ss: &SockaddrStorage) -> Option<SocketAddr> {
    match ss.family() {
        Some(AddressFamily::Inet) => ss.as_sockaddr_in().map(|sin| {
            SocketAddr::V4(SocketAddrV4::new(
                std::net::Ipv4Addr::from(sin.ip()),
                sin.port(),
            ))
        }),
        Some(AddressFamily::Inet6) => ss.as_sockaddr_in6().map(|sin6| {
            SocketAddr::V6(SocketAddrV6::new(
                sin6.ip(),
                sin6.port(),
                sin6.flowinfo(),
                sin6.scope_id(),
            ))
        }),
        _ => None,
    }
}

/// Query the local system for all interface addresses.
pub fn ifaces() -> Result<Vec<Interface>, Error> {
    let mut ret = Vec::new();
    for ifa in nix::ifaddrs::getifaddrs()? {
        if let Some(kind) = ifa
            .address
            .as_ref()
            .and_then(SockaddrStorage::family)
            .and_then(|af| match af {
                AddressFamily::Inet => Some(Kind::Ipv4),
                AddressFamily::Inet6 => Some(Kind::Ipv6),
                #[cfg(any(
                    target_os = "android",
                    target_os = "linux",
                    target_os = "illumos",
                    target_os = "fuchsia",
                    target_os = "solaris"
                ))]
                AddressFamily::Packet => Some(Kind::Packet),
                #[cfg(any(
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "ios",
                    target_os = "macos",
                    target_os = "illumos",
                    target_os = "netbsd",
                    target_os = "openbsd"
                ))]
                AddressFamily::Link => Some(Kind::Link),
                _ => None,
            })
        {
            let name = ifa.interface_name;
            let dst = ifa.destination.as_ref().and_then(ss_to_netsa);
            let broadcast = ifa.broadcast.as_ref().and_then(ss_to_netsa);
            let hop = dst
                .map(NextHop::Destination)
                .or(broadcast.map(NextHop::Broadcast));
            let addr = ifa.address.as_ref().and_then(ss_to_netsa);
            let mask = ifa.netmask.as_ref().and_then(ss_to_netsa);

            ret.push(Interface {
                name,
                kind,
                addr,
                mask,
                hop,
            });
        }
    }

    Ok(ret)
}
