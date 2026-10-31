#[cfg(test)]
mod conn_map_test;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::error::*;
use crate::vnet::conn::UdpConn;
use crate::Conn;

type PortMap = Mutex<HashMap<u16, Vec<Arc<UdpConn>>>>;

#[derive(Default)]
pub(crate) struct UdpConnMap {
    port_map: PortMap,
}

impl UdpConnMap {
    pub(crate) fn new() -> Self {
        UdpConnMap {
            port_map: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn insert(&self, conn: Arc<UdpConn>) -> Result<()> {
        let addr = conn.local_addr()?;

        let mut port_map = self.port_map.lock().await;
        if let Some(conns) = port_map.get(&addr.port()) {
            if addr.ip().is_unspecified() {
                return Err(Error::ErrAddressAlreadyInUse);
            }

            for c in conns {
                let laddr = c.local_addr()?;
                if laddr.ip().is_unspecified() || laddr.ip() == addr.ip() {
                    return Err(Error::ErrAddressAlreadyInUse);
                }
            }
        }

        if let Some(conns) = port_map.get_mut(&addr.port()) {
            conns.push(conn);
        } else {
            port_map.insert(addr.port(), vec![conn]);
        }
        Ok(())
    }

    pub(crate) async fn find(&self, addr: &SocketAddr) -> Option<Arc<UdpConn>> {
        let port_map = self.port_map.lock().await;
        if let Some(conns) = port_map.get(&addr.port()) {
            if addr.ip().is_unspecified() {
                // pick the first one appears in the iteration
                if let Some(c) = conns.first() {
                    return Some(Arc::clone(c));
                } else {
                    return None;
                }
            }

            for c in conns {
                let laddr = {
                    match c.local_addr() {
                        Ok(laddr) => laddr,
                        Err(_) => return None,
                    }
                };
                if laddr.ip().is_unspecified() || laddr.ip() == addr.ip() {
                    return Some(Arc::clone(c));
                }
            }
        }

        None
    }

    pub(crate) async fn delete(&self, addr: &SocketAddr) -> Result<()> {
        let mut port_map = self.port_map.lock().await;
        let mut new_conns = vec![];
        if let Some(conns) = port_map.get(&addr.port()) {
            if !addr.ip().is_unspecified() {
                for c in conns {
                    let laddr = c.local_addr()?;
                    if laddr.ip().is_unspecified() {
                        // This can't happen!
                        return Err(Error::ErrCannotRemoveUnspecifiedIp);
                    }

                    if laddr.ip() == addr.ip() {
                        continue;
                    }
                    new_conns.push(Arc::clone(c));
                }
            }
        } else {
            return Err(Error::ErrNoSuchUdpConn);
        }

        if new_conns.is_empty() {
            port_map.remove(&addr.port());
        } else {
            port_map.insert(addr.port(), new_conns);
        }

        Ok(())
    }

    pub(crate) async fn len(&self) -> usize {
        let port_map = self.port_map.lock().await;
        let mut n = 0;
        for conns in port_map.values() {
            n += conns.len();
        }
        n
    }
}
