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
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
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
pub mod keyboard;

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

pub struct AddrMangle();

impl AddrMangle {
    pub fn encode(addr: SocketAddr) -> Vec<u8> {
        match addr {
            SocketAddr::V4(addr_v4) => {
                let tm = (SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_micros() as u32) as u128;
                let ip = u32::from_le_bytes(addr_v4.ip().octets()) as u128;
                let port = addr.port() as u128;
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
            _ => {
                panic!("Only support ipv4");
            }
        }
    }

    pub fn decode(bytes: &[u8]) -> SocketAddr {
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
    use std::io::prelude::*;
    let mut file = File::create("./src/version.rs").unwrap();
    for line in read_lines("Cargo.toml").unwrap() {
        if let Ok(line) = line {
            let ab: Vec<&str> = line.split("=").map(|x| x.trim()).collect();
            if ab.len() == 2 && ab[0] == "version" {
                file.write_all(format!("pub const VERSION: &str = {};\n", ab[1]).as_bytes())
                    .ok();
                break;
            }
        }
    }
    // generate build date
    let build_date = format!("{}", chrono::Local::now().format("%Y-%m-%d %H:%M"));
    file.write_all(format!("pub const BUILD_DATE: &str = \"{}\";", build_date).as_bytes())
        .ok();
    file.sync_all().ok();
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mangle() {
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 16, 32), 21116));
        assert_eq!(addr, AddrMangle::decode(&AddrMangle::encode(addr)));
    }
}
