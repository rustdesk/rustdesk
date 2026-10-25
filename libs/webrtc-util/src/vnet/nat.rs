#[cfg(test)]
mod nat_test;

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::ops::Add;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::SystemTime;

use portable_atomic::AtomicU16;
use tokio::sync::Mutex;
use tokio::time::Duration;

use crate::error::*;
use crate::vnet::chunk::Chunk;
use crate::vnet::net::UDP_STR;

const DEFAULT_NAT_MAPPING_LIFE_TIME: Duration = Duration::from_secs(30);

// EndpointDependencyType defines a type of behavioral dependency on the
// remote endpoint's IP address or port number. This is used for the two
// kinds of behaviors:
//  - Port Mapping behavior
//  - Filtering behavior
// See: https://tools.ietf.org/html/rfc4787
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum EndpointDependencyType {
    // EndpointIndependent means the behavior is independent of the endpoint's address or port
    #[default]
    EndpointIndependent,
    // EndpointAddrDependent means the behavior is dependent on the endpoint's address
    EndpointAddrDependent,
    // EndpointAddrPortDependent means the behavior is dependent on the endpoint's address and port
    EndpointAddrPortDependent,
}

// NATMode defines basic behavior of the NAT
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum NatMode {
    // NATModeNormal means the NAT behaves as a standard NAPT (RFC 2663).
    #[default]
    Normal,
    // NATModeNAT1To1 exhibits 1:1 DNAT where the external IP address is statically mapped to
    // a specific local IP address with port number is preserved always between them.
    // When this mode is selected, mapping_behavior, filtering_behavior, port_preservation and
    // mapping_life_time of NATType are ignored.
    Nat1To1,
}

// NATType has a set of parameters that define the behavior of NAT.
#[derive(Default, Debug, Copy, Clone)]
pub struct NatType {
    pub mode: NatMode,
    pub mapping_behavior: EndpointDependencyType,
    pub filtering_behavior: EndpointDependencyType,
    pub hair_pining: bool,       // Not implemented yet
    pub port_preservation: bool, // Not implemented yet
    pub mapping_life_time: Duration,
}

#[derive(Default, Debug, Clone)]
pub(crate) struct NatConfig {
    pub(crate) name: String,
    pub(crate) nat_type: NatType,
    pub(crate) mapped_ips: Vec<IpAddr>, // mapped IPv4
    pub(crate) local_ips: Vec<IpAddr>,  // local IPv4, required only when the mode is NATModeNAT1To1
}

#[derive(Debug, Clone)]
pub(crate) struct Mapping {
    proto: String,                        // "udp" or "tcp"
    local: String,                        // "<local-ip>:<local-port>"
    mapped: String,                       // "<mapped-ip>:<mapped-port>"
    bound: String,                        // key: "[<remote-ip>[:<remote-port>]]"
    filters: Arc<Mutex<HashSet<String>>>, // key: "[<remote-ip>[:<remote-port>]]"
    expires: Arc<Mutex<SystemTime>>,      // time to expire
}

impl Default for Mapping {
    fn default() -> Self {
        Mapping {
            proto: String::new(),                             // "udp" or "tcp"
            local: String::new(),                             // "<local-ip>:<local-port>"
            mapped: String::new(),                            // "<mapped-ip>:<mapped-port>"
            bound: String::new(), // key: "[<remote-ip>[:<remote-port>]]"
            filters: Arc::new(Mutex::new(HashSet::new())), // key: "[<remote-ip>[:<remote-port>]]"
            expires: Arc::new(Mutex::new(SystemTime::now())), // time to expire
        }
    }
}

#[derive(Default, Debug, Clone)]
pub(crate) struct NetworkAddressTranslator {
    pub(crate) name: String,
    pub(crate) nat_type: NatType,
    pub(crate) mapped_ips: Vec<IpAddr>, // mapped IPv4
    pub(crate) local_ips: Vec<IpAddr>,  // local IPv4, required only when the mode is NATModeNAT1To1
    pub(crate) outbound_map: Arc<Mutex<HashMap<String, Arc<Mapping>>>>, // key: "<proto>:<local-ip>:<local-port>[:remote-ip[:remote-port]]
    pub(crate) inbound_map: Arc<Mutex<HashMap<String, Arc<Mapping>>>>, // key: "<proto>:<mapped-ip>:<mapped-port>"
    pub(crate) udp_port_counter: Arc<AtomicU16>,
}

impl NetworkAddressTranslator {
    pub(crate) fn new(config: NatConfig) -> Result<Self> {
        let mut nat_type = config.nat_type;

        if nat_type.mode == NatMode::Nat1To1 {
            // 1:1 NAT behavior
            nat_type.mapping_behavior = EndpointDependencyType::EndpointIndependent;
            nat_type.filtering_behavior = EndpointDependencyType::EndpointIndependent;
            nat_type.port_preservation = true;
            nat_type.mapping_life_time = Duration::from_secs(0);

            if config.mapped_ips.is_empty() {
                return Err(Error::ErrNatRequiresMapping);
            }
            if config.mapped_ips.len() != config.local_ips.len() {
                return Err(Error::ErrMismatchLengthIp);
            }
        } else {
            // Normal (NAPT) behavior
            nat_type.mode = NatMode::Normal;
            if nat_type.mapping_life_time == Duration::from_secs(0) {
                nat_type.mapping_life_time = DEFAULT_NAT_MAPPING_LIFE_TIME;
            }
        }

        Ok(NetworkAddressTranslator {
            name: config.name,
            nat_type,
            mapped_ips: config.mapped_ips,
            local_ips: config.local_ips,
            outbound_map: Arc::new(Mutex::new(HashMap::new())),
            inbound_map: Arc::new(Mutex::new(HashMap::new())),
            udp_port_counter: Arc::new(AtomicU16::new(0)),
        })
    }

    pub(crate) fn get_paired_mapped_ip(&self, loc_ip: &IpAddr) -> Option<&IpAddr> {
        for (i, ip) in self.local_ips.iter().enumerate() {
            if ip == loc_ip {
                return self.mapped_ips.get(i);
            }
        }
        None
    }

    pub(crate) fn get_paired_local_ip(&self, mapped_ip: &IpAddr) -> Option<&IpAddr> {
        for (i, ip) in self.mapped_ips.iter().enumerate() {
            if ip == mapped_ip {
                return self.local_ips.get(i);
            }
        }
        None
    }

    pub(crate) async fn translate_outbound(
        &self,
        from: &(dyn Chunk + Send + Sync),
    ) -> Result<Option<Box<dyn Chunk + Send + Sync>>> {
        let mut to = from.clone_to();

        if from.network() == UDP_STR {
            if self.nat_type.mode == NatMode::Nat1To1 {
                // 1:1 NAT behavior
                let src_addr = from.source_addr();
                if let Some(src_ip) = self.get_paired_mapped_ip(&src_addr.ip()) {
                    to.set_source_addr(&format!("{}:{}", src_ip, src_addr.port()))?;
                } else {
                    log::debug!(
                        "[{}] drop outbound chunk {} with not route",
                        self.name,
                        from
                    );
                    return Ok(None); // silently discard
                }
            } else {
                // Normal (NAPT) behavior
                let bound = match self.nat_type.mapping_behavior {
                    EndpointDependencyType::EndpointIndependent => "".to_owned(),
                    EndpointDependencyType::EndpointAddrDependent => {
                        from.get_destination_ip().to_string()
                    }
                    EndpointDependencyType::EndpointAddrPortDependent => {
                        from.destination_addr().to_string()
                    }
                };

                let filter_key = match self.nat_type.filtering_behavior {
                    EndpointDependencyType::EndpointIndependent => "".to_owned(),
                    EndpointDependencyType::EndpointAddrDependent => {
                        from.get_destination_ip().to_string()
                    }
                    EndpointDependencyType::EndpointAddrPortDependent => {
                        from.destination_addr().to_string()
                    }
                };

                let o_key = format!("udp:{}:{}", from.source_addr(), bound);
                let name = self.name.clone();

                let m_mapped = if let Some(m) = self.find_outbound_mapping(&o_key).await {
                    let mut filters = m.filters.lock().await;
                    if !filters.contains(&filter_key) {
                        log::debug!(
                            "[{}] permit access from {} to {}",
                            name,
                            filter_key,
                            m.mapped
                        );
                        filters.insert(filter_key);
                    }
                    m.mapped.clone()
                } else {
                    // Create a new Mapping
                    let udp_port_counter = self.udp_port_counter.load(Ordering::SeqCst);
                    let mapped_port = 0xC000 + udp_port_counter;
                    if udp_port_counter == 0xFFFF - 0xC000 {
                        self.udp_port_counter.store(0, Ordering::SeqCst);
                    } else {
                        self.udp_port_counter.fetch_add(1, Ordering::SeqCst);
                    }

                    let m = if let Some(mapped_ips_first) = self.mapped_ips.first() {
                        Mapping {
                            proto: "udp".to_owned(),
                            local: from.source_addr().to_string(),
                            bound,
                            mapped: format!("{mapped_ips_first}:{mapped_port}"),
                            filters: Arc::new(Mutex::new(HashSet::new())),
                            expires: Arc::new(Mutex::new(
                                SystemTime::now().add(self.nat_type.mapping_life_time),
                            )),
                        }
                    } else {
                        return Err(Error::ErrNatRequiresMapping);
                    };

                    {
                        let mut outbound_map = self.outbound_map.lock().await;
                        outbound_map.insert(o_key.clone(), Arc::new(m.clone()));
                    }

                    let i_key = format!("udp:{}", m.mapped);

                    log::debug!(
                        "[{}] created a new NAT binding oKey={} i_key={}",
                        self.name,
                        o_key,
                        i_key
                    );
                    log::debug!(
                        "[{}] permit access from {} to {}",
                        self.name,
                        filter_key,
                        m.mapped
                    );

                    {
                        let mut filters = m.filters.lock().await;
                        filters.insert(filter_key);
                    }

                    let m_mapped = m.mapped.clone();
                    {
                        let mut inbound_map = self.inbound_map.lock().await;
                        inbound_map.insert(i_key, Arc::new(m));
                    }
                    m_mapped
                };

                to.set_source_addr(&m_mapped)?;
            }

            log::debug!(
                "[{}] translate outbound chunk from {} to {}",
                self.name,
                from,
                to
            );

            return Ok(Some(to));
        }

        Err(Error::ErrNonUdpTranslationNotSupported)
    }

    pub(crate) async fn translate_inbound(
        &self,
        from: &(dyn Chunk + Send + Sync),
    ) -> Result<Option<Box<dyn Chunk + Send + Sync>>> {
        let mut to = from.clone_to();

        if from.network() == UDP_STR {
            if self.nat_type.mode == NatMode::Nat1To1 {
                // 1:1 NAT behavior
                let dst_addr = from.destination_addr();
                if let Some(dst_ip) = self.get_paired_local_ip(&dst_addr.ip()) {
                    let dst_port = from.destination_addr().port();
                    to.set_destination_addr(&format!("{dst_ip}:{dst_port}"))?;
                } else {
                    return Err(Error::Other(format!(
                        "drop {from} as {:?}",
                        Error::ErrNoAssociatedLocalAddress
                    )));
                }
            } else {
                // Normal (NAPT) behavior
                let filter_key = match self.nat_type.filtering_behavior {
                    EndpointDependencyType::EndpointIndependent => "".to_owned(),
                    EndpointDependencyType::EndpointAddrDependent => {
                        from.get_source_ip().to_string()
                    }
                    EndpointDependencyType::EndpointAddrPortDependent => {
                        from.source_addr().to_string()
                    }
                };

                let i_key = format!("udp:{}", from.destination_addr());
                if let Some(m) = self.find_inbound_mapping(&i_key).await {
                    {
                        let filters = m.filters.lock().await;
                        if !filters.contains(&filter_key) {
                            return Err(Error::Other(format!(
                                "drop {} as the remote {} {:?}",
                                from,
                                filter_key,
                                Error::ErrHasNoPermission
                            )));
                        }
                    }

                    // See RFC 4847 Section 4.3.  Mapping Refresh
                    // a) Inbound refresh may be useful for applications with no outgoing
                    //   UDP traffic.  However, allowing inbound refresh may allow an
                    //   external attacker or misbehaving application to keep a Mapping
                    //   alive indefinitely.  This may be a security risk.  Also, if the
                    //   process is repeated with different ports, over time, it could
                    //   use up all the ports on the NAT.

                    to.set_destination_addr(&m.local)?;
                } else {
                    return Err(Error::Other(format!(
                        "drop {} as {:?}",
                        from,
                        Error::ErrNoNatBindingFound
                    )));
                }
            }

            log::debug!(
                "[{}] translate inbound chunk from {} to {}",
                self.name,
                from,
                to
            );

            return Ok(Some(to));
        }

        Err(Error::ErrNonUdpTranslationNotSupported)
    }

    // caller must hold the mutex
    pub(crate) async fn find_outbound_mapping(&self, o_key: &str) -> Option<Arc<Mapping>> {
        let mapping_life_time = self.nat_type.mapping_life_time;
        let mut expired = false;
        let (in_key, out_key) = {
            let outbound_map = self.outbound_map.lock().await;
            if let Some(m) = outbound_map.get(o_key) {
                let now = SystemTime::now();

                {
                    let mut expires = m.expires.lock().await;
                    // check if this Mapping is expired
                    if now.duration_since(*expires).is_ok() {
                        expired = true;
                    } else {
                        *expires = now.add(mapping_life_time);
                    }
                }
                (
                    NetworkAddressTranslator::get_inbound_map_key(m),
                    NetworkAddressTranslator::get_outbound_map_key(m),
                )
            } else {
                (String::new(), String::new())
            }
        };

        if expired {
            {
                let mut inbound_map = self.inbound_map.lock().await;
                inbound_map.remove(&in_key);
            }
            {
                let mut outbound_map = self.outbound_map.lock().await;
                outbound_map.remove(&out_key);
            }
        }

        let outbound_map = self.outbound_map.lock().await;
        outbound_map.get(o_key).cloned()
    }

    // caller must hold the mutex
    pub(crate) async fn find_inbound_mapping(&self, i_key: &str) -> Option<Arc<Mapping>> {
        let mut expired = false;
        let (in_key, out_key) = {
            let inbound_map = self.inbound_map.lock().await;
            if let Some(m) = inbound_map.get(i_key) {
                let now = SystemTime::now();

                {
                    let expires = m.expires.lock().await;
                    // check if this Mapping is expired
                    if now.duration_since(*expires).is_ok() {
                        expired = true;
                    }
                }
                (
                    NetworkAddressTranslator::get_inbound_map_key(m),
                    NetworkAddressTranslator::get_outbound_map_key(m),
                )
            } else {
                (String::new(), String::new())
            }
        };

        if expired {
            {
                let mut inbound_map = self.inbound_map.lock().await;
                inbound_map.remove(&in_key);
            }
            {
                let mut outbound_map = self.outbound_map.lock().await;
                outbound_map.remove(&out_key);
            }
        }

        let inbound_map = self.inbound_map.lock().await;
        inbound_map.get(i_key).cloned()
    }

    // caller must hold the mutex
    fn get_outbound_map_key(m: &Mapping) -> String {
        format!("{}:{}:{}", m.proto, m.local, m.bound)
    }

    fn get_inbound_map_key(m: &Mapping) -> String {
        format!("{}:{}", m.proto, m.mapped)
    }

    async fn inbound_map_len(&self) -> usize {
        let inbound_map = self.inbound_map.lock().await;
        inbound_map.len()
    }

    async fn outbound_map_len(&self) -> usize {
        let outbound_map = self.outbound_map.lock().await;
        outbound_map.len()
    }
}
