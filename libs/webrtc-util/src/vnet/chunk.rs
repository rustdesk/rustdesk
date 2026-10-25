#[cfg(test)]
mod chunk_test;

use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::ops::{BitAnd, BitOr};
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use portable_atomic::AtomicU64;

use super::net::*;
use crate::error::Result;

lazy_static! {
    static ref TAG_CTR: AtomicU64 = AtomicU64::new(0);
}

/// Encodes a u64 value to a lowercase base 36 string.
pub fn base36(value: impl Into<u64>) -> String {
    let mut digits: Vec<u8> = vec![];

    let mut value = value.into();
    while value > 0 {
        let digit = (value % 36) as usize;
        value /= 36;

        digits.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[digit]);
    }

    digits.reverse();
    format!("{:0>8}", String::from_utf8(digits).unwrap())
}

// Generate a base36-encoded unique tag
// See: https://play.golang.org/p/0ZaAID1q-HN
fn assign_chunk_tag() -> String {
    let n = TAG_CTR.fetch_add(1, Ordering::SeqCst);
    base36(n)
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub(crate) struct TcpFlag(pub(crate) u8);

pub(crate) const TCP_FLAG_ZERO: TcpFlag = TcpFlag(0x00);
pub(crate) const TCP_FLAG_FIN: TcpFlag = TcpFlag(0x01);
pub(crate) const TCP_FLAG_SYN: TcpFlag = TcpFlag(0x02);
pub(crate) const TCP_FLAG_RST: TcpFlag = TcpFlag(0x04);
pub(crate) const TCP_FLAG_PSH: TcpFlag = TcpFlag(0x08);
pub(crate) const TCP_FLAG_ACK: TcpFlag = TcpFlag(0x10);

impl BitOr for TcpFlag {
    type Output = Self;

    // rhs is the "right-hand side" of the expression `a | b`
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitAnd for TcpFlag {
    type Output = Self;

    // rhs is the "right-hand side" of the expression `a & b`
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl fmt::Display for TcpFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut sa = vec![];
        if *self & TCP_FLAG_FIN != TCP_FLAG_ZERO {
            sa.push("FIN");
        }
        if *self & TCP_FLAG_SYN != TCP_FLAG_ZERO {
            sa.push("SYN");
        }
        if *self & TCP_FLAG_RST != TCP_FLAG_ZERO {
            sa.push("RST");
        }
        if *self & TCP_FLAG_PSH != TCP_FLAG_ZERO {
            sa.push("PSH");
        }
        if *self & TCP_FLAG_ACK != TCP_FLAG_ZERO {
            sa.push("ACK");
        }

        write!(f, "{}", sa.join("-"))
    }
}

// Chunk represents a packet passed around in the vnet
pub trait Chunk: fmt::Display + fmt::Debug {
    fn set_timestamp(&mut self) -> SystemTime; // used by router
    fn get_timestamp(&self) -> SystemTime; // used by router
    fn get_source_ip(&self) -> IpAddr; // used by routee
    fn get_destination_ip(&self) -> IpAddr; // used by router
    fn set_source_addr(&mut self, address: &str) -> Result<()>; // used by nat
    fn set_destination_addr(&mut self, address: &str) -> Result<()>; // used by nat

    fn source_addr(&self) -> SocketAddr;
    fn destination_addr(&self) -> SocketAddr;
    fn user_data(&self) -> Vec<u8>;
    fn tag(&self) -> String;
    fn network(&self) -> String; // returns "udp" or "tcp"
    fn clone_to(&self) -> Box<dyn Chunk + Send + Sync>;
}

#[derive(PartialEq, Debug)]
pub(crate) struct ChunkIp {
    pub(crate) timestamp: SystemTime,
    pub(crate) source_ip: IpAddr,
    pub(crate) destination_ip: IpAddr,
    pub(crate) tag: String,
}

impl ChunkIp {
    fn set_timestamp(&mut self) -> SystemTime {
        self.timestamp = SystemTime::now();
        self.timestamp
    }

    fn get_timestamp(&self) -> SystemTime {
        self.timestamp
    }

    fn get_destination_ip(&self) -> IpAddr {
        self.destination_ip
    }

    fn get_source_ip(&self) -> IpAddr {
        self.source_ip
    }

    fn tag(&self) -> String {
        self.tag.clone()
    }
}

#[derive(PartialEq, Debug)]
pub(crate) struct ChunkUdp {
    pub(crate) chunk_ip: ChunkIp,
    pub(crate) source_port: u16,
    pub(crate) destination_port: u16,
    pub(crate) user_data: Vec<u8>,
}

impl fmt::Display for ChunkUdp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} chunk {} {} => {}",
            self.network(),
            self.tag(),
            self.source_addr(),
            self.destination_addr(),
        )
    }
}

impl Chunk for ChunkUdp {
    fn set_timestamp(&mut self) -> SystemTime {
        self.chunk_ip.set_timestamp()
    }

    fn get_timestamp(&self) -> SystemTime {
        self.chunk_ip.get_timestamp()
    }

    fn get_destination_ip(&self) -> IpAddr {
        self.chunk_ip.get_destination_ip()
    }

    fn get_source_ip(&self) -> IpAddr {
        self.chunk_ip.get_source_ip()
    }

    fn tag(&self) -> String {
        self.chunk_ip.tag()
    }

    fn source_addr(&self) -> SocketAddr {
        SocketAddr::new(self.chunk_ip.source_ip, self.source_port)
    }

    fn destination_addr(&self) -> SocketAddr {
        SocketAddr::new(self.chunk_ip.destination_ip, self.destination_port)
    }

    fn user_data(&self) -> Vec<u8> {
        self.user_data.clone()
    }

    fn clone_to(&self) -> Box<dyn Chunk + Send + Sync> {
        Box::new(ChunkUdp {
            chunk_ip: ChunkIp {
                timestamp: self.chunk_ip.timestamp,
                source_ip: self.chunk_ip.source_ip,
                destination_ip: self.chunk_ip.destination_ip,
                tag: self.chunk_ip.tag.clone(),
            },
            source_port: self.source_port,
            destination_port: self.destination_port,
            user_data: self.user_data.clone(),
        })
    }

    fn network(&self) -> String {
        UDP_STR.to_owned()
    }

    fn set_source_addr(&mut self, address: &str) -> Result<()> {
        let addr = SocketAddr::from_str(address)?;
        self.chunk_ip.source_ip = addr.ip();
        self.source_port = addr.port();
        Ok(())
    }

    fn set_destination_addr(&mut self, address: &str) -> Result<()> {
        let addr = SocketAddr::from_str(address)?;
        self.chunk_ip.destination_ip = addr.ip();
        self.destination_port = addr.port();
        Ok(())
    }
}

impl ChunkUdp {
    pub(crate) fn new(src_addr: SocketAddr, dst_addr: SocketAddr) -> Self {
        ChunkUdp {
            chunk_ip: ChunkIp {
                timestamp: SystemTime::now(),
                source_ip: src_addr.ip(),
                destination_ip: dst_addr.ip(),
                tag: assign_chunk_tag(),
            },
            source_port: src_addr.port(),
            destination_port: dst_addr.port(),
            user_data: vec![],
        }
    }
}

#[derive(PartialEq, Debug)]
pub(crate) struct ChunkTcp {
    chunk_ip: ChunkIp,
    source_port: u16,
    destination_port: u16,
    flags: TcpFlag, // control bits
    user_data: Vec<u8>, // only with PSH flag
                    // seq             :u32,  // always starts with 0
                    // ack             :u32,  // always starts with 0
}

impl fmt::Display for ChunkTcp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} chunk {} {} => {}",
            self.network(),
            self.flags,
            self.chunk_ip.tag,
            self.source_addr(),
            self.destination_addr(),
        )
    }
}

impl Chunk for ChunkTcp {
    fn set_timestamp(&mut self) -> SystemTime {
        self.chunk_ip.set_timestamp()
    }

    fn get_timestamp(&self) -> SystemTime {
        self.chunk_ip.get_timestamp()
    }

    fn get_destination_ip(&self) -> IpAddr {
        self.chunk_ip.get_destination_ip()
    }

    fn get_source_ip(&self) -> IpAddr {
        self.chunk_ip.get_source_ip()
    }

    fn tag(&self) -> String {
        self.chunk_ip.tag()
    }

    fn source_addr(&self) -> SocketAddr {
        SocketAddr::new(self.chunk_ip.source_ip, self.source_port)
    }

    fn destination_addr(&self) -> SocketAddr {
        SocketAddr::new(self.chunk_ip.destination_ip, self.destination_port)
    }

    fn user_data(&self) -> Vec<u8> {
        self.user_data.clone()
    }

    fn clone_to(&self) -> Box<dyn Chunk + Send + Sync> {
        Box::new(ChunkTcp {
            chunk_ip: ChunkIp {
                timestamp: self.chunk_ip.timestamp,
                source_ip: self.chunk_ip.source_ip,
                destination_ip: self.chunk_ip.destination_ip,
                tag: self.chunk_ip.tag.clone(),
            },
            source_port: self.source_port,
            destination_port: self.destination_port,
            flags: self.flags,
            user_data: self.user_data.clone(),
        })
    }

    fn network(&self) -> String {
        "tcp".to_owned()
    }

    fn set_source_addr(&mut self, address: &str) -> Result<()> {
        let addr = SocketAddr::from_str(address)?;
        self.chunk_ip.source_ip = addr.ip();
        self.source_port = addr.port();
        Ok(())
    }

    fn set_destination_addr(&mut self, address: &str) -> Result<()> {
        let addr = SocketAddr::from_str(address)?;
        self.chunk_ip.destination_ip = addr.ip();
        self.destination_port = addr.port();
        Ok(())
    }
}

impl ChunkTcp {
    pub(crate) fn new(src_addr: SocketAddr, dst_addr: SocketAddr, flags: TcpFlag) -> Self {
        ChunkTcp {
            chunk_ip: ChunkIp {
                timestamp: SystemTime::now(),
                source_ip: src_addr.ip(),
                destination_ip: dst_addr.ip(),
                tag: assign_chunk_tag(),
            },
            source_port: src_addr.port(),
            destination_port: dst_addr.port(),
            flags,
            user_data: vec![],
        }
    }
}
