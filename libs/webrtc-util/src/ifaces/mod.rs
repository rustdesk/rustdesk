pub mod ffi;
pub use ffi::ifaces;

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum NextHop {
    Broadcast(::std::net::SocketAddr),
    Destination(::std::net::SocketAddr),
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Kind {
    Packet,
    Link,
    Ipv4,
    Ipv6,
    Unknow(i32),
}

#[derive(Debug, Clone)]
pub struct Interface {
    pub name: String,
    pub kind: Kind,
    pub addr: Option<::std::net::SocketAddr>,
    pub mask: Option<::std::net::SocketAddr>,
    pub hop: Option<NextHop>,
}
