pub mod compress;
pub mod platform;
pub mod protos;
pub use bytes;
use config::Config;
pub use futures;
pub use protobuf;
pub use protos::message as message_proto;
pub use protos::rendezvous as rendezvous_proto;
use std::{
    fs::File,
    io::{self, BufRead},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    path::Path,
    time::{self, SystemTime, UNIX_EPOCH},
};
pub use tokio;
pub use tokio_util;
pub mod socket_client;
pub mod tcp;
pub mod udp;
pub use env_logger;
pub use log;
pub mod bytes_codec;
#[cfg(feature = "quic")]
pub mod quic;
pub use anyhow::{self, bail};
pub use futures_util;
pub mod config;
pub mod fs;
pub use lazy_static;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use mac_address;
pub use rand;
pub use regex;
pub use sodiumoxide;
pub use tokio_socks;
pub use tokio_socks::IntoTargetAddr;
pub use tokio_socks::TargetAddr;
pub mod password_security;
pub use chrono;
pub use directories_next;

#[cfg(feature = "quic")]
pub type Stream = quic::Connection;
#[cfg(not(feature = "quic"))]
pub type Stream = tcp::FramedStream;

#[inline]
pub async fn sleep(sec: f32) {
    tokio::time::sleep(time::Duration::from_secs_f32(sec)).await;
}

#[macro_export]
macro_rules! allow_err {
    ($e:expr) => {
        if let Err(err) = $e {
            log::debug!(
                "{:?}, {}:{}:{}:{}",
                err,
                module_path!(),
                file!(),
                line!(),
                column!()
            );
        } else {
        }
    };
}

#[inline]
pub fn timeout<T: std::future::Future>(ms: u64, future: T) -> tokio::time::Timeout<T> {
    tokio::time::timeout(std::time::Duration::from_millis(ms), future)
}

pub type ResultType<F, E = anyhow::Error> = anyhow::Result<F, E>;

/// Certain router and firewalls scan the packet and if they
/// find an IP address belonging to their pool that they use to do the NAT mapping/translation, so here we mangle the ip address

pub struct AddrMangle;

impl AddrMangle {
    pub fn encode(version: u8, addr: SocketAddr) -> Vec<u8> {
        match version {
            0 => Self::encode_version_0(addr),
            1 => Self::encode_version_1(addr),
            _ => unimplemented!("AddrMangle version unsupported"),
        }
    }

    pub fn decode(bytes: &[u8]) -> SocketAddr {
        if bytes.len() < 16 {
            Self::decode_default_v4(bytes)
        } else if bytes.len() == 16 {
            // unreachable for now
            let version = bytes[15];
            match version {
                1 => Self::decode_default_v4(&bytes[..15]),
                _ => unimplemented!("AddrMangle version unsupported"),
            }
        } else {
            let mut padded = [0u8; 16];
            let version = bytes[bytes.len() - 1];
            padded.copy_from_slice(&bytes[..16]);
            let v1 = u128::from_le_bytes(padded);
            padded = [0u8; 16];
            padded[..(bytes.len() - 17)].copy_from_slice(&bytes[16..(bytes.len() - 1)]);
            let v2 = u128::from_le_bytes(padded);

            match version {
                1 => Self::decode_v6_1(v1, v2),
                _ => unimplemented!("AddrMangle version unsupported"),
            }
        }
    }

    pub fn encode_version_0(addr: SocketAddr) -> Vec<u8> {
        match addr {
            SocketAddr::V4(addr_v4) => Self::encode_default_v4(addr_v4),
            _ => {
                panic!("Only support ipv4");
            }
        }
    }

    // For compatibility, will be refactor with version later. See ipv6 implementation.
    fn encode_default_v4(addr_v4: SocketAddrV4) -> Vec<u8> {
        let tm = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u32) as u128;
        let ip = u32::from_le_bytes(addr_v4.ip().octets()) as u128;
        let port = addr_v4.port() as u128;
        let v = ((ip + tm) << 49) | (tm << 17) | (port + (tm & 0xFFFF));
        let bytes = v.to_le_bytes();
        let mut n_padding = 0;
        for i in bytes.iter().rev() {
            if i == &0u8 {
                n_padding += 1;
            } else {
                break;
            }
        }
        bytes[..(16 - n_padding)].to_vec()
    }

    // For compatibility, will be refactor with version later. See ipv6 implementation.
    fn decode_default_v4(bytes: &[u8]) -> SocketAddr {
        let mut padded = [0u8; 16];
        padded[..bytes.len()].copy_from_slice(&bytes);
        let number = u128::from_le_bytes(padded);
        let tm = (number >> 17) & (u32::max_value() as u128);
        let ip = (((number >> 49) - tm) as u32).to_le_bytes();
        let port = (number & 0xFFFFFF) - (tm & 0xFFFF);
        SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]),
            port as u16,
        ))
    }

    fn encode_version_1(addr: SocketAddr) -> Vec<u8> {
        let version = 1u8;
        let tm = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u32) as u128;
        match addr {
            SocketAddr::V4(addr_v4) => Self::encode_default_v4(addr_v4),
            SocketAddr::V6(addr_v6) => {
                let ip = u128::from(*addr_v6.ip());
                let port = addr.port() as u128;
                let low_78 = (-1i128 as u128) >> 50;
                let v1 = (((ip & low_78) + tm) << 49) | (tm << 17) | (port + (tm & 0xFFFF));
                let v2 = (ip >> 78) + tm;
                let mut bytes: Vec<u8> = v1
                    .to_le_bytes()
                    .iter()
                    .copied()
                    .chain(v2.to_le_bytes().iter().copied())
                    .collect();
                let mut n_padding = 0;
                for i in bytes.iter().rev() {
                    if i == &0u8 {
                        n_padding += 1;
                        if n_padding == 128 {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                assert!(n_padding > 0);
                bytes[(32 - n_padding)] = version;
                bytes[..(32 - n_padding + 1)].to_vec()
            }
        }
    }

    fn decode_v6_1(v1: u128, v2: u128) -> SocketAddr {
        let tm = (v1 >> 17) & (u32::max_value() as u128);
        let ip_low_78 = (v1 >> 49) - tm;
        let port = (v1 & 0xFFFFFF) - (tm & 0xFFFF);
        let ip_high_50 = v2 - tm;
        let ip = ip_high_50 << 78 | ip_low_78;
        SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(ip), port as u16, 0, 0))
    }
}

pub fn get_version_from_url(url: &str) -> String {
    let n = url.chars().count();
    let a = url
        .chars()
        .rev()
        .enumerate()
        .filter(|(_, x)| x == &'-')
        .next()
        .map(|(i, _)| i);
    if let Some(a) = a {
        let b = url
            .chars()
            .rev()
            .enumerate()
            .filter(|(_, x)| x == &'.')
            .next()
            .map(|(i, _)| i);
        if let Some(b) = b {
            if a > b {
                if url
                    .chars()
                    .skip(n - b)
                    .collect::<String>()
                    .parse::<i32>()
                    .is_ok()
                {
                    return url.chars().skip(n - a).collect();
                } else {
                    return url.chars().skip(n - a).take(a - b - 1).collect();
                }
            } else {
                return url.chars().skip(n - a).collect();
            }
        }
    }
    "".to_owned()
}

pub fn gen_version() {
    let mut file = File::create("./src/version.rs").unwrap();
    for line in read_lines("Cargo.toml").unwrap() {
        if let Ok(line) = line {
            let ab: Vec<&str> = line.split("=").map(|x| x.trim()).collect();
            if ab.len() == 2 && ab[0] == "version" {
                use std::io::prelude::*;
                file.write_all(format!("pub const VERSION: &str = {};", ab[1]).as_bytes())
                    .ok();
                file.sync_all().ok();
                break;
            }
        }
    }
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

pub fn is_valid_custom_id(id: &str) -> bool {
    regex::Regex::new(r"^[a-zA-Z]\w{5,15}$")
        .unwrap()
        .is_match(id)
}

pub fn get_version_number(v: &str) -> i64 {
    let mut n = 0;
    for x in v.split(".") {
        n = n * 1000 + x.parse::<i64>().unwrap_or(0);
    }
    n
}

pub fn get_modified_time(path: &std::path::Path) -> SystemTime {
    std::fs::metadata(&path)
        .map(|m| m.modified().unwrap_or(UNIX_EPOCH))
        .unwrap_or(UNIX_EPOCH)
}

pub fn get_created_time(path: &std::path::Path) -> SystemTime {
    std::fs::metadata(&path)
        .map(|m| m.created().unwrap_or(UNIX_EPOCH))
        .unwrap_or(UNIX_EPOCH)
}

pub fn get_exe_time() -> SystemTime {
    std::env::current_exe().map_or(UNIX_EPOCH, |path| {
        let m = get_modified_time(&path);
        let c = get_created_time(&path);
        if m > c {
            m
        } else {
            c
        }
    })
}

pub fn get_uuid() -> Vec<u8> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if let Ok(id) = machine_uid::get() {
        return id.into();
    }
    Config::get_key_pair().1
}

#[inline]
pub fn get_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0) as _
}

pub fn try_set_port(host: &str, port: u16) -> String {
    if host.parse::<SocketAddr>().is_ok() {
        return host.to_string();
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return SocketAddr::new(ip, port).to_string();
    }
    if !host.contains(":") {
        format!("{}:{}", host, port)
    } else {
        host.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mangle() {
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 16, 32), 21116));
        assert_eq!(addr, AddrMangle::decode(&AddrMangle::encode(0, addr)));

        let addr = SocketAddr::V6(SocketAddrV6::new(
            "ff:ff:ff:ff:ff:ff:ff:ff".parse::<Ipv6Addr>().unwrap(),
            65535,
            0,
            0,
        ));
        assert_eq!(addr, AddrMangle::decode(&AddrMangle::encode(1, addr)));
    }
}
