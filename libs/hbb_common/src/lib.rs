pub mod compress;
#[path = "./protos/message.rs"]
pub mod message_proto;
#[path = "./protos/rendezvous.rs"]
pub mod rendezvous_proto;
pub use bytes;
pub use futures;
pub use protobuf;
use socket2::{Domain, Socket, Type};
use std::{
    fs::File,
    io::{self, BufRead},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs},
    path::Path,
    time::{self, SystemTime, UNIX_EPOCH},
};
pub use tokio;
pub use tokio_util;
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
pub use sodiumoxide;

#[cfg(feature = "quic")]
pub type Stream = quic::Connection;
#[cfg(not(feature = "quic"))]
pub type Stream = tcp::FramedStream;

#[inline]
pub async fn sleep(sec: f32) {
    tokio::time::delay_for(time::Duration::from_secs_f32(sec)).await;
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

fn new_socket(addr: SocketAddr, tcp: bool, reuse: bool) -> Result<Socket, std::io::Error> {
    let stype = {
        if tcp {
            Type::stream()
        } else {
            Type::dgram()
        }
    };
    let socket = match addr {
        SocketAddr::V4(..) => Socket::new(Domain::ipv4(), stype, None),
        SocketAddr::V6(..) => Socket::new(Domain::ipv6(), stype, None),
    }?;
    if reuse {
        // windows has no reuse_port, but it's reuse_address
        // almost equals to unix's reuse_port + reuse_address,
        // though may introduce nondeterministic bahavior
        #[cfg(unix)]
        socket.set_reuse_port(true)?;
        socket.set_reuse_address(true)?;
    }
    socket.bind(&addr.into())?;
    Ok(socket)
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
                let ip = u32::from_ne_bytes(addr_v4.ip().octets()) as u128;
                let port = addr.port() as u128;
                let v = ((ip + tm) << 49) | (tm << 17) | (port + (tm & 0xFFFF));
                let bytes = v.to_ne_bytes();
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
        let number = u128::from_ne_bytes(padded);
        let tm = (number >> 17) & (u32::max_value() as u128);
        let ip = (((number >> 49) - tm) as u32).to_ne_bytes();
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

pub fn to_socket_addr(host: &str) -> ResultType<SocketAddr> {
    let addrs: Vec<SocketAddr> = host.to_socket_addrs()?.collect();
    if addrs.is_empty() {
        bail!("Failed to solve {}", host);
    }
    Ok(addrs[0])
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mangle() {
        let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 16, 32), 21116));
        assert_eq!(addr, AddrMangle::decode(&AddrMangle::encode(addr)));
    }
}
