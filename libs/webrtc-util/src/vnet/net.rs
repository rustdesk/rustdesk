#[cfg(test)]
mod net_test;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use async_trait::async_trait;
use ipnet::IpNet;
use portable_atomic::AtomicU64;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

use super::conn_map::*;
use super::interface::*;
use crate::error::*;
use crate::vnet::chunk::Chunk;
use crate::vnet::conn::{ConnObserver, UdpConn};
use crate::vnet::router::*;
use crate::{conn, ifaces, Conn};

pub(crate) const LO0_STR: &str = "lo0";
pub(crate) const UDP_STR: &str = "udp";

lazy_static! {
    pub static ref MAC_ADDR_COUNTER: AtomicU64 = AtomicU64::new(0xBEEFED910200);
}

pub(crate) type HardwareAddr = Vec<u8>;

pub(crate) fn new_mac_address() -> HardwareAddr {
    let b = MAC_ADDR_COUNTER
        .fetch_add(1, Ordering::SeqCst)
        .to_be_bytes();
    b[2..].to_vec()
}

#[derive(Default)]
pub(crate) struct VNetInternal {
    pub(crate) interfaces: Vec<Interface>,         // read-only
    pub(crate) router: Option<Arc<Mutex<Router>>>, // read-only
    pub(crate) udp_conns: UdpConnMap,              // read-only
}

impl VNetInternal {
    fn get_interface(&self, ifc_name: &str) -> Option<&Interface> {
        self.interfaces.iter().find(|ifc| ifc.name == ifc_name)
    }
}

#[async_trait]
impl ConnObserver for VNetInternal {
    async fn write(&self, c: Box<dyn Chunk + Send + Sync>) -> Result<()> {
        if c.network() == UDP_STR && c.get_destination_ip().is_loopback() {
            if let Some(conn) = self.udp_conns.find(&c.destination_addr()).await {
                let read_ch_tx = conn.get_inbound_ch();
                let ch_tx = read_ch_tx.lock().await;
                if let Some(tx) = &*ch_tx {
                    let _ = tx.send(c).await;
                }
            }
            return Ok(());
        }

        if let Some(r) = &self.router {
            let p = r.lock().await;
            p.push(c).await;
            Ok(())
        } else {
            Err(Error::ErrNoRouterLinked)
        }
    }

    async fn on_closed(&self, addr: SocketAddr) {
        let _ = self.udp_conns.delete(&addr).await;
    }

    // This method determines the srcIP based on the dstIP when locIP
    // is any IP address ("0.0.0.0" or "::"). If locIP is a non-any addr,
    // this method simply returns locIP.
    // caller must hold the mutex
    fn determine_source_ip(&self, loc_ip: IpAddr, dst_ip: IpAddr) -> Option<IpAddr> {
        if !loc_ip.is_unspecified() {
            return Some(loc_ip);
        }

        if dst_ip.is_loopback() {
            let src_ip = IpAddr::from_str("127.0.0.1").ok();
            return src_ip;
        }

        if let Some(ifc) = self.get_interface("eth0") {
            for ipnet in ifc.addrs() {
                if (ipnet.addr().is_ipv4() && loc_ip.is_ipv4())
                    || (ipnet.addr().is_ipv6() && loc_ip.is_ipv6())
                {
                    return Some(ipnet.addr());
                }
            }
        }

        None
    }
}

#[derive(Default)]
pub struct VNet {
    pub(crate) interfaces: Vec<Interface>, // read-only
    pub(crate) static_ips: Vec<IpAddr>,    // read-only
    pub(crate) vi: Arc<Mutex<VNetInternal>>,
}

#[async_trait]
impl Nic for VNet {
    async fn get_interface(&self, ifc_name: &str) -> Option<Interface> {
        for ifc in &self.interfaces {
            if ifc.name == ifc_name {
                return Some(ifc.clone());
            }
        }
        None
    }

    async fn add_addrs_to_interface(&mut self, ifc_name: &str, addrs: &[IpNet]) -> Result<()> {
        {
            let mut vi = self.vi.lock().await;
            for ifc in &mut vi.interfaces {
                if ifc.name == ifc_name {
                    for addr in addrs {
                        ifc.add_addr(*addr);
                    }
                    break;
                }
            }
        }

        for ifc in &mut self.interfaces {
            if ifc.name == ifc_name {
                for addr in addrs {
                    ifc.add_addr(*addr);
                }
                return Ok(());
            }
        }

        Err(Error::ErrNotFound)
    }

    async fn set_router(&self, r: Arc<Mutex<Router>>) -> Result<()> {
        let mut vi = self.vi.lock().await;
        vi.router = Some(r);

        Ok(())
    }

    async fn on_inbound_chunk(&self, c: Box<dyn Chunk + Send + Sync>) {
        if c.network() == UDP_STR {
            let vi = self.vi.lock().await;
            if let Some(conn) = vi.udp_conns.find(&c.destination_addr()).await {
                let read_ch_tx = conn.get_inbound_ch();
                let ch_tx = read_ch_tx.lock().await;
                if let Some(tx) = &*ch_tx {
                    let _ = tx.send(c).await;
                }
            }
        }
    }

    async fn get_static_ips(&self) -> Vec<IpAddr> {
        self.static_ips.clone()
    }
}

impl VNet {
    pub(crate) fn get_interfaces(&self) -> &[Interface] {
        &self.interfaces
    }

    // caller must hold the mutex
    pub(crate) fn get_all_ipaddrs(&self, ipv6: bool) -> Vec<IpAddr> {
        let mut ips = vec![];

        for ifc in &self.interfaces {
            for ipnet in ifc.addrs() {
                if (ipv6 && ipnet.addr().is_ipv6()) || (!ipv6 && ipnet.addr().is_ipv4()) {
                    ips.push(ipnet.addr());
                }
            }
        }

        ips
    }

    // caller must hold the mutex
    pub(crate) fn has_ipaddr(&self, ip: IpAddr) -> bool {
        for ifc in &self.interfaces {
            for ipnet in ifc.addrs() {
                let loc_ip = ipnet.addr();

                match ip.to_string().as_str() {
                    "0.0.0.0" => {
                        if loc_ip.is_ipv4() {
                            return true;
                        }
                    }
                    "::" => {
                        if loc_ip.is_ipv6() {
                            return true;
                        }
                    }
                    _ => {
                        if loc_ip == ip {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    // caller must hold the mutex
    pub(crate) async fn allocate_local_addr(&self, ip: IpAddr, port: u16) -> Result<()> {
        // gather local IP addresses to bind
        let mut ips = vec![];
        if ip.is_unspecified() {
            ips = self.get_all_ipaddrs(ip.is_ipv6());
        } else if self.has_ipaddr(ip) {
            ips.push(ip);
        }

        if ips.is_empty() {
            return Err(Error::ErrBindFailed);
        }

        // check if all these transport addresses are not in use
        for ip2 in ips {
            let addr = SocketAddr::new(ip2, port);
            let vi = self.vi.lock().await;
            if vi.udp_conns.find(&addr).await.is_some() {
                return Err(Error::ErrAddressAlreadyInUse);
            }
        }

        Ok(())
    }

    // caller must hold the mutex
    pub(crate) async fn assign_port(&self, ip: IpAddr, start: u16, end: u16) -> Result<u16> {
        // choose randomly from the range between start and end (inclusive)
        if end < start {
            return Err(Error::ErrEndPortLessThanStart);
        }

        let space = end + 1 - start;
        let offset = rand::random::<u16>() % space;
        for i in 0..space {
            let port = ((offset + i) % space) + start;
            let result = self.allocate_local_addr(ip, port).await;
            if result.is_ok() {
                return Ok(port);
            }
        }

        Err(Error::ErrPortSpaceExhausted)
    }

    pub(crate) async fn resolve_addr(&self, use_ipv4: bool, address: &str) -> Result<SocketAddr> {
        let v: Vec<&str> = address.splitn(2, ':').collect();
        if v.len() != 2 {
            return Err(Error::ErrAddrNotUdpAddr);
        }
        let (host, port) = (v[0], v[1]);

        // Check if host is a domain name
        let ip: IpAddr = match host.parse() {
            Ok(ip) => ip,
            Err(_) => {
                let host = host.to_lowercase();
                if host == "localhost" {
                    if use_ipv4 {
                        Ipv4Addr::new(127, 0, 0, 1).into()
                    } else {
                        Ipv6Addr::from_str("::1")?.into()
                    }
                } else {
                    // host is a domain name. resolve IP address by the name
                    let vi = self.vi.lock().await;
                    if let Some(router) = &vi.router {
                        let r = router.lock().await;
                        let resolver = r.resolver.lock().await;
                        if let Some(ip) = resolver.lookup(host).await {
                            ip
                        } else {
                            return Err(Error::ErrNotFound);
                        }
                    } else {
                        return Err(Error::ErrNoRouterLinked);
                    }
                }
            }
        };

        let port: u16 = port.parse()?;

        let remote_addr = SocketAddr::new(ip, port);
        if (use_ipv4 && remote_addr.is_ipv4()) || (!use_ipv4 && remote_addr.is_ipv6()) {
            Ok(remote_addr)
        } else {
            Err(Error::Other(format!(
                "No available {} IP address found!",
                if use_ipv4 { "ipv4" } else { "ipv6" },
            )))
        }
    }

    // caller must hold the mutex
    pub(crate) async fn bind(
        &self,
        mut local_addr: SocketAddr,
    ) -> Result<Arc<dyn Conn + Send + Sync>> {
        // validate address. do we have that address?
        if !self.has_ipaddr(local_addr.ip()) {
            return Err(Error::ErrCantAssignRequestedAddr);
        }

        if local_addr.port() == 0 {
            // choose randomly from the range between 5000 and 5999
            local_addr.set_port(self.assign_port(local_addr.ip(), 5000, 5999).await?);
        } else {
            let vi = self.vi.lock().await;
            if vi.udp_conns.find(&local_addr).await.is_some() {
                return Err(Error::ErrAddressAlreadyInUse);
            }
        }

        let v = Arc::clone(&self.vi) as Arc<Mutex<dyn ConnObserver + Send + Sync>>;
        let conn = Arc::new(UdpConn::new(local_addr, None, v));

        {
            let vi = self.vi.lock().await;
            vi.udp_conns.insert(Arc::clone(&conn)).await?;
        }

        Ok(conn)
    }

    pub(crate) async fn dail(
        &self,
        use_ipv4: bool,
        remote_addr: &str,
    ) -> Result<Arc<dyn Conn + Send + Sync>> {
        let rem_addr = self.resolve_addr(use_ipv4, remote_addr).await?;

        // Determine source address
        let src_ip = {
            let vi = self.vi.lock().await;
            let any_ip = if use_ipv4 {
                Ipv4Addr::new(0, 0, 0, 0).into()
            } else {
                Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).into()
            };
            if let Some(src_ip) = vi.determine_source_ip(any_ip, rem_addr.ip()) {
                src_ip
            } else {
                any_ip
            }
        };

        let loc_addr = SocketAddr::new(src_ip, 0);

        let conn = self.bind(loc_addr).await?;
        conn.connect(rem_addr).await?;

        Ok(conn)
    }
}

// NetConfig is a bag of configuration parameters passed to NewNet().
#[derive(Debug, Default)]
pub struct NetConfig {
    // static_ips is an array of static IP addresses to be assigned for this Net.
    // If no static IP address is given, the router will automatically assign
    // an IP address.
    pub static_ips: Vec<String>,

    // static_ip is deprecated. Use static_ips.
    pub static_ip: String,
}

// Net represents a local network stack equivalent to a set of layers from NIC
// up to the transport (UDP / TCP) layer.
pub enum Net {
    VNet(Arc<Mutex<VNet>>),
    Ifs(Vec<Interface>),
}

impl Net {
    // NewNet creates an instance of Net.
    // If config is nil, the virtual network is disabled. (uses corresponding
    // net.Xxxx() operations.
    // By design, it always have lo0 and eth0 interfaces.
    // The lo0 has the address 127.0.0.1 assigned by default.
    // IP address for eth0 will be assigned when this Net is added to a router.
    pub fn new(config: Option<NetConfig>) -> Self {
        if let Some(config) = config {
            let mut lo0 = Interface::new(LO0_STR.to_owned(), vec![]);
            if let Ok(ipnet) = Interface::convert(
                SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 0),
                Some(SocketAddr::new(Ipv4Addr::new(255, 0, 0, 0).into(), 0)),
            ) {
                lo0.add_addr(ipnet);
            }

            let eth0 = Interface::new("eth0".to_owned(), vec![]);

            let mut static_ips = vec![];
            for ip_str in &config.static_ips {
                if let Ok(ip) = IpAddr::from_str(ip_str) {
                    static_ips.push(ip);
                }
            }
            if !config.static_ip.is_empty() {
                if let Ok(ip) = IpAddr::from_str(&config.static_ip) {
                    static_ips.push(ip);
                }
            }

            let vnet = VNet {
                interfaces: vec![lo0.clone(), eth0.clone()],
                static_ips,
                vi: Arc::new(Mutex::new(VNetInternal {
                    interfaces: vec![lo0, eth0],
                    router: None,
                    udp_conns: UdpConnMap::new(),
                })),
            };

            Net::VNet(Arc::new(Mutex::new(vnet)))
        } else {
            let interfaces = ifaces::ifaces().unwrap_or_default();

            let mut m: HashMap<String, Vec<IpNet>> = HashMap::new();
            for iface in interfaces {
                if let Some(addrs) = m.get_mut(&iface.name) {
                    if let Some(addr) = iface.addr {
                        if let Ok(inet) = Interface::convert(addr, iface.mask) {
                            addrs.push(inet);
                        }
                    }
                } else if let Some(addr) = iface.addr {
                    if let Ok(inet) = Interface::convert(addr, iface.mask) {
                        m.insert(iface.name, vec![inet]);
                    }
                }
            }

            let mut ifs = vec![];
            for (name, addrs) in m.into_iter() {
                ifs.push(Interface::new(name, addrs));
            }

            Net::Ifs(ifs)
        }
    }

    // Interfaces returns a list of the system's network interfaces.
    pub async fn get_interfaces(&self) -> Vec<Interface> {
        match self {
            Net::VNet(vnet) => {
                let net = vnet.lock().await;
                net.get_interfaces().to_vec()
            }
            Net::Ifs(ifs) => ifs.clone(),
        }
    }

    // InterfaceByName returns the interface specified by name.
    pub async fn get_interface(&self, ifc_name: &str) -> Option<Interface> {
        match self {
            Net::VNet(vnet) => {
                let net = vnet.lock().await;
                net.get_interface(ifc_name).await
            }
            Net::Ifs(ifs) => {
                for ifc in ifs {
                    if ifc.name == ifc_name {
                        return Some(ifc.clone());
                    }
                }
                None
            }
        }
    }

    // IsVirtual tests if the virtual network is enabled.
    pub fn is_virtual(&self) -> bool {
        match self {
            Net::VNet(_) => true,
            Net::Ifs(_) => false,
        }
    }

    pub async fn resolve_addr(&self, use_ipv4: bool, address: &str) -> Result<SocketAddr> {
        match self {
            Net::VNet(vnet) => {
                let net = vnet.lock().await;
                net.resolve_addr(use_ipv4, address).await
            }
            Net::Ifs(_) => Ok(conn::lookup_host(use_ipv4, address).await?),
        }
    }

    pub async fn bind(&self, addr: SocketAddr) -> Result<Arc<dyn Conn + Send + Sync>> {
        match self {
            Net::VNet(vnet) => {
                let net = vnet.lock().await;
                net.bind(addr).await
            }
            Net::Ifs(_) => Ok(Arc::new(UdpSocket::bind(addr).await?)),
        }
    }

    pub async fn dail(
        &self,
        use_ipv4: bool,
        remote_addr: &str,
    ) -> Result<Arc<dyn Conn + Send + Sync>> {
        match self {
            Net::VNet(vnet) => {
                let net = vnet.lock().await;
                net.dail(use_ipv4, remote_addr).await
            }
            Net::Ifs(_) => {
                let any_ip = if use_ipv4 {
                    Ipv4Addr::new(0, 0, 0, 0).into()
                } else {
                    Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).into()
                };
                let local_addr = SocketAddr::new(any_ip, 0);

                let conn = UdpSocket::bind(local_addr).await?;
                conn.connect(remote_addr).await?;

                Ok(Arc::new(conn))
            }
        }
    }

    pub fn get_nic(&self) -> Result<Arc<Mutex<dyn Nic + Send + Sync>>> {
        match self {
            Net::VNet(vnet) => Ok(Arc::clone(vnet) as Arc<Mutex<dyn Nic + Send + Sync>>),
            Net::Ifs(_) => Err(Error::ErrVnetDisabled),
        }
    }
}
