use std::net::SocketAddr;

use ipnet::*;

use crate::error::*;

#[derive(Debug, Clone, Default)]
pub struct Interface {
    pub(crate) name: String,
    pub(crate) addrs: Vec<IpNet>,
}

impl Interface {
    pub fn new(name: String, addrs: Vec<IpNet>) -> Self {
        Interface { name, addrs }
    }

    pub fn add_addr(&mut self, addr: IpNet) {
        self.addrs.push(addr);
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn addrs(&self) -> &[IpNet] {
        &self.addrs
    }

    pub fn convert(addr: SocketAddr, mask: Option<SocketAddr>) -> Result<IpNet> {
        if let Some(mask) = mask {
            Ok(IpNet::with_netmask(addr.ip(), mask.ip()).map_err(|_| Error::ErrInvalidMask)?)
        } else {
            Ok(IpNet::new(addr.ip(), if addr.is_ipv4() { 32 } else { 128 })
                .expect("ipv4 should always work with prefix 32 and ipv6 with prefix 128"))
        }
    }
}
