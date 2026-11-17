#[cfg(test)]
mod router_test;

use std::collections::HashMap;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::ops::{Add, Sub};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use std::time::SystemTime;

use async_trait::async_trait;
use ipnet::*;
use portable_atomic::AtomicU64;
use tokio::sync::{mpsc, Mutex};
use tokio::time::Duration;

use crate::error::*;
use crate::vnet::chunk::*;
use crate::vnet::chunk_queue::*;
use crate::vnet::interface::*;
use crate::vnet::nat::*;
use crate::vnet::net::*;
use crate::vnet::resolver::*;

const DEFAULT_ROUTER_QUEUE_SIZE: usize = 0; // unlimited

lazy_static! {
    pub static ref ROUTER_ID_CTR: AtomicU64 = AtomicU64::new(0);
}

// Generate a unique router name
fn assign_router_name() -> String {
    let n = ROUTER_ID_CTR.fetch_add(1, Ordering::SeqCst);
    format!("router{n}")
}

// RouterConfig ...
#[derive(Default)]
pub struct RouterConfig {
    // name of router. If not specified, a unique name will be assigned.
    pub name: String,
    // cidr notation, like "192.0.2.0/24"
    pub cidr: String,
    // static_ips is an array of static IP addresses to be assigned for this router.
    // If no static IP address is given, the router will automatically assign
    // an IP address.
    // This will be ignored if this router is the root.
    pub static_ips: Vec<String>,
    // static_ip is deprecated. Use static_ips.
    pub static_ip: String,
    // Internal queue size
    pub queue_size: usize,
    // Effective only when this router has a parent router
    pub nat_type: Option<NatType>,
    // Minimum Delay
    pub min_delay: Duration,
    // Max Jitter
    pub max_jitter: Duration,
}

// NIC is a network interface controller that interfaces Router
#[async_trait]
pub trait Nic {
    async fn get_interface(&self, ifc_name: &str) -> Option<Interface>;
    async fn add_addrs_to_interface(&mut self, ifc_name: &str, addrs: &[IpNet]) -> Result<()>;
    async fn on_inbound_chunk(&self, c: Box<dyn Chunk + Send + Sync>);
    async fn get_static_ips(&self) -> Vec<IpAddr>;
    async fn set_router(&self, r: Arc<Mutex<Router>>) -> Result<()>;
}

// ChunkFilter is a handler users can add to filter chunks.
// If the filter returns false, the packet will be dropped.
pub type ChunkFilterFn = Box<dyn (Fn(&(dyn Chunk + Send + Sync)) -> bool) + Send + Sync>;

#[derive(Default)]
pub struct RouterInternal {
    pub(crate) nat_type: Option<NatType>,           // read-only
    pub(crate) ipv4net: IpNet,                      // read-only
    pub(crate) parent: Option<Weak<Mutex<Router>>>, // read-only
    pub(crate) nat: NetworkAddressTranslator,       // read-only
    pub(crate) nics: HashMap<String, Weak<Mutex<dyn Nic + Send + Sync>>>, // read-only
    pub(crate) chunk_filters: Vec<ChunkFilterFn>,   // requires mutex [x]
    pub(crate) last_id: u8, // requires mutex [x], used to assign the last digit of IPv4 address
}

// Router ...
#[derive(Default)]
pub struct Router {
    name: String,                              // read-only
    ipv4net: IpNet,                            // read-only
    min_delay: Duration,                       // requires mutex [x]
    max_jitter: Duration,                      // requires mutex [x]
    queue: Arc<ChunkQueue>,                    // read-only
    interfaces: Vec<Interface>,                // read-only
    static_ips: Vec<IpAddr>,                   // read-only
    static_local_ips: HashMap<String, IpAddr>, // read-only,
    children: Vec<Arc<Mutex<Router>>>,         // read-only
    done: Option<mpsc::Sender<()>>,            // requires mutex [x]
    pub(crate) resolver: Arc<Mutex<Resolver>>, // read-only
    push_ch: Option<mpsc::Sender<()>>,         // writer requires mutex
    router_internal: Arc<Mutex<RouterInternal>>,
}

#[async_trait]
impl Nic for Router {
    async fn get_interface(&self, ifc_name: &str) -> Option<Interface> {
        for ifc in &self.interfaces {
            if ifc.name == ifc_name {
                return Some(ifc.clone());
            }
        }
        None
    }

    async fn add_addrs_to_interface(&mut self, ifc_name: &str, addrs: &[IpNet]) -> Result<()> {
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

    async fn on_inbound_chunk(&self, c: Box<dyn Chunk + Send + Sync>) {
        let from_parent: Box<dyn Chunk + Send + Sync> = {
            let router_internal = self.router_internal.lock().await;
            match router_internal.nat.translate_inbound(&*c).await {
                Ok(from) => {
                    if let Some(from) = from {
                        from
                    } else {
                        return;
                    }
                }
                Err(err) => {
                    log::warn!("[{}] {}", self.name, err);
                    return;
                }
            }
        };

        self.push(from_parent).await;
    }

    async fn get_static_ips(&self) -> Vec<IpAddr> {
        self.static_ips.clone()
    }

    // caller must hold the mutex
    async fn set_router(&self, parent: Arc<Mutex<Router>>) -> Result<()> {
        {
            let mut router_internal = self.router_internal.lock().await;
            router_internal.parent = Some(Arc::downgrade(&parent));
        }

        let parent_resolver = {
            let p = parent.lock().await;
            Arc::clone(&p.resolver)
        };
        {
            let mut resolver = self.resolver.lock().await;
            resolver.set_parent(Arc::downgrade(&parent_resolver));
        }

        let mut mapped_ips = vec![];
        let mut local_ips = vec![];

        // when this method is called, one or more IP address has already been assigned by
        // the parent router.
        if let Some(ifc) = self.get_interface("eth0").await {
            for ifc_addr in ifc.addrs() {
                let ip = ifc_addr.addr();
                mapped_ips.push(ip);

                if let Some(loc_ip) = self.static_local_ips.get(&ip.to_string()) {
                    local_ips.push(*loc_ip);
                }
            }
        } else {
            return Err(Error::ErrNoIpaddrEth0);
        }

        // Set up NAT here
        {
            let mut router_internal = self.router_internal.lock().await;
            if router_internal.nat_type.is_none() {
                router_internal.nat_type = Some(NatType {
                    mapping_behavior: EndpointDependencyType::EndpointIndependent,
                    filtering_behavior: EndpointDependencyType::EndpointAddrPortDependent,
                    hair_pining: false,
                    port_preservation: false,
                    mapping_life_time: Duration::from_secs(30),
                    ..Default::default()
                });
            }

            router_internal.nat = NetworkAddressTranslator::new(NatConfig {
                name: self.name.clone(),
                nat_type: router_internal.nat_type.unwrap(),
                mapped_ips,
                local_ips,
            })?;
        }

        Ok(())
    }
}

impl Router {
    pub fn new(config: RouterConfig) -> Result<Self> {
        let ipv4net: IpNet = config.cidr.parse()?;

        let queue_size = if config.queue_size > 0 {
            config.queue_size
        } else {
            DEFAULT_ROUTER_QUEUE_SIZE
        };

        // set up network interface, lo0
        let mut lo0 = Interface::new(LO0_STR.to_owned(), vec![]);
        if let Ok(ipnet) = Interface::convert(
            SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 0),
            Some(SocketAddr::new(Ipv4Addr::new(255, 0, 0, 0).into(), 0)),
        ) {
            lo0.add_addr(ipnet);
        }

        // set up network interface, eth0
        let eth0 = Interface::new("eth0".to_owned(), vec![]);

        // local host name resolver
        let resolver = Arc::new(Mutex::new(Resolver::new()));

        let name = if config.name.is_empty() {
            assign_router_name()
        } else {
            config.name.clone()
        };

        let mut static_ips = vec![];
        let mut static_local_ips = HashMap::new();
        for ip_str in &config.static_ips {
            let ip_pair: Vec<&str> = ip_str.split('/').collect();
            if let Ok(ip) = IpAddr::from_str(ip_pair[0]) {
                if ip_pair.len() > 1 {
                    let loc_ip = IpAddr::from_str(ip_pair[1])?;
                    if !ipv4net.contains(&loc_ip) {
                        return Err(Error::ErrLocalIpBeyondStaticIpsSubset);
                    }
                    static_local_ips.insert(ip.to_string(), loc_ip);
                }
                static_ips.push(ip);
            }
        }
        if !config.static_ip.is_empty() {
            log::warn!("static_ip is deprecated. Use static_ips instead");
            if let Ok(ip) = IpAddr::from_str(&config.static_ip) {
                static_ips.push(ip);
            }
        }

        let n_static_local = static_local_ips.len();
        if n_static_local > 0 && n_static_local != static_ips.len() {
            return Err(Error::ErrLocalIpNoStaticsIpsAssociated);
        }

        let router_internal = RouterInternal {
            nat_type: config.nat_type,
            ipv4net,
            nics: HashMap::new(),
            ..Default::default()
        };

        Ok(Router {
            name,
            ipv4net,
            interfaces: vec![lo0, eth0],
            static_ips,
            static_local_ips,
            resolver,
            router_internal: Arc::new(Mutex::new(router_internal)),
            queue: Arc::new(ChunkQueue::new(queue_size)),
            min_delay: config.min_delay,
            max_jitter: config.max_jitter,
            ..Default::default()
        })
    }

    // caller must hold the mutex
    pub(crate) fn get_interfaces(&self) -> &[Interface] {
        &self.interfaces
    }

    // Start ...
    pub fn start(&mut self) -> Pin<Box<dyn Future<Output = Result<()>>>> {
        if self.done.is_some() {
            return Box::pin(async move { Err(Error::ErrRouterAlreadyStarted) });
        }

        let (done_tx, mut done_rx) = mpsc::channel(1);
        let (push_ch_tx, mut push_ch_rx) = mpsc::channel(1);
        self.done = Some(done_tx);
        self.push_ch = Some(push_ch_tx);

        let router_internal = Arc::clone(&self.router_internal);
        let queue = Arc::clone(&self.queue);
        let max_jitter = self.max_jitter;
        let min_delay = self.min_delay;
        let name = self.name.clone();
        let ipv4net = self.ipv4net;

        tokio::spawn(async move {
            while let Ok(d) = Router::process_chunks(
                &name,
                ipv4net,
                max_jitter,
                min_delay,
                &queue,
                &router_internal,
            )
            .await
            {
                if d == Duration::from_secs(0) {
                    tokio::select! {
                     _ = push_ch_rx.recv() =>{},
                     _ = done_rx.recv() => break,
                    }
                } else {
                    let t = tokio::time::sleep(d);
                    tokio::pin!(t);

                    tokio::select! {
                    _ = t.as_mut() => {},
                    _ = done_rx.recv() => break,
                    }
                }
            }
        });

        let children = self.children.clone();
        Box::pin(async move { Router::start_children(children).await })
    }

    // Stop ...
    pub fn stop(&mut self) -> Pin<Box<dyn Future<Output = Result<()>>>> {
        if self.done.is_none() {
            return Box::pin(async move { Err(Error::ErrRouterAlreadyStopped) });
        }
        self.push_ch.take();
        self.done.take();

        let children = self.children.clone();
        Box::pin(async move { Router::stop_children(children).await })
    }

    async fn start_children(children: Vec<Arc<Mutex<Router>>>) -> Result<()> {
        for child in children {
            let mut c = child.lock().await;
            c.start().await?;
        }

        Ok(())
    }

    async fn stop_children(children: Vec<Arc<Mutex<Router>>>) -> Result<()> {
        for child in children {
            let mut c = child.lock().await;
            c.stop().await?;
        }

        Ok(())
    }

    // AddRouter adds a chile Router.
    // after parent.add_router(child), also call child.set_router(parent) to set child's parent router
    pub async fn add_router(&mut self, child: Arc<Mutex<Router>>) -> Result<()> {
        // Router is a NIC. Add it as a NIC so that packets are routed to this child
        // router.
        let nic = Arc::clone(&child) as Arc<Mutex<dyn Nic + Send + Sync>>;
        self.children.push(child);
        self.add_net(nic).await
    }

    // AddNet ...
    // after router.add_net(nic), also call nic.set_router(router) to set nic's router
    pub async fn add_net(&mut self, nic: Arc<Mutex<dyn Nic + Send + Sync>>) -> Result<()> {
        let mut router_internal = self.router_internal.lock().await;
        router_internal.add_nic(nic).await
    }

    // AddHost adds a mapping of hostname and an IP address to the local resolver.
    pub async fn add_host(&mut self, host_name: String, ip_addr: String) -> Result<()> {
        let mut resolver = self.resolver.lock().await;
        resolver.add_host(host_name, ip_addr)
    }

    // AddChunkFilter adds a filter for chunks traversing this router.
    // You may add more than one filter. The filters are called in the order of this method call.
    // If a chunk is dropped by a filter, subsequent filter will not receive the chunk.
    pub async fn add_chunk_filter(&self, filter: ChunkFilterFn) {
        let mut router_internal = self.router_internal.lock().await;
        router_internal.chunk_filters.push(filter);
    }

    pub(crate) async fn push(&self, mut c: Box<dyn Chunk + Send + Sync>) {
        log::debug!("[{}] route {}", self.name, c);
        if self.done.is_some() {
            c.set_timestamp();

            if self.queue.push(c).await {
                if let Some(push_ch) = &self.push_ch {
                    let _ = push_ch.try_send(());
                }
            } else {
                log::warn!("[{}] queue was full. dropped a chunk", self.name);
            }
        } else {
            log::warn!("router is done");
        }
    }

    async fn process_chunks(
        name: &str,
        ipv4net: IpNet,
        max_jitter: Duration,
        min_delay: Duration,
        queue: &Arc<ChunkQueue>,
        router_internal: &Arc<Mutex<RouterInternal>>,
    ) -> Result<Duration> {
        // Introduce jitter by delaying the processing of chunks.
        let mj = max_jitter.as_nanos() as u64;
        if mj > 0 {
            let jitter = Duration::from_nanos(rand::random::<u64>() % mj);
            tokio::time::sleep(jitter).await;
        }

        //      cut_off
        //         v min delay
        //         |<--->|
        //  +------------:--
        //  |OOOOOOXXXXX :   --> time
        //  +------------:--
        //  |<--->|     now
        //    due

        let entered_at = SystemTime::now();
        let cut_off = entered_at.sub(min_delay);

        // the next sleep duration
        let mut d;

        loop {
            d = Duration::from_secs(0);

            if let Some(c) = queue.peek().await {
                // check timestamp to find if the chunk is due
                if c.get_timestamp().duration_since(cut_off).is_ok() {
                    // There is one or more chunk in the queue but none of them are due.
                    // Calculate the next sleep duration here.
                    let next_expire = c.get_timestamp().add(min_delay);
                    if let Ok(diff) = next_expire.duration_since(entered_at) {
                        d = diff;
                        break;
                    }
                }
            } else {
                break; // no more chunk in the queue
            }

            if let Some(c) = queue.pop().await {
                let ri = router_internal.lock().await;
                let mut blocked = false;
                for filter in &ri.chunk_filters {
                    if !filter(&*c) {
                        blocked = true;
                        break;
                    }
                }
                if blocked {
                    continue; // discard
                }

                let dst_ip = c.get_destination_ip();

                // check if the destination is in our subnet
                if ipv4net.contains(&dst_ip) {
                    // search for the destination NIC
                    if let Some(nic) = ri.nics.get(&dst_ip.to_string()).and_then(|p| p.upgrade()) {
                        // found the NIC, forward the chunk to the NIC.
                        // call to NIC must unlock mutex
                        let ni = nic.lock().await;
                        ni.on_inbound_chunk(c).await;
                    } else {
                        // NIC not found. drop it.
                        log::debug!("[{}] {} unreachable", name, c);
                    }
                } else {
                    // the destination is outside of this subnet
                    // is this WAN?
                    if let Some(parent) = &ri.parent.clone().and_then(|p| p.upgrade()) {
                        // Pass it to the parent via NAT
                        if let Some(to_parent) = ri.nat.translate_outbound(&*c).await? {
                            // call to parent router mutex unlock mutex
                            let p = parent.lock().await;
                            p.push(to_parent).await;
                        }
                    } else {
                        // this WAN. No route for this chunk
                        log::debug!("[{}] no route found for {}", name, c);
                    }
                }
            } else {
                break; // no more chunk in the queue
            }
        }

        Ok(d)
    }
}

impl RouterInternal {
    // caller must hold the mutex
    pub(crate) async fn add_nic(&mut self, nic: Arc<Mutex<dyn Nic + Send + Sync>>) -> Result<()> {
        let mut ips = {
            let ni = nic.lock().await;
            ni.get_static_ips().await
        };

        if ips.is_empty() {
            // assign an IP address
            let ip = self.assign_ip_address()?;
            log::debug!("assign_ip_address: {}", ip);
            ips.push(ip);
        }

        let mut ipnets = vec![];
        for ip in &ips {
            if !self.ipv4net.contains(ip) {
                return Err(Error::ErrStaticIpIsBeyondSubnet);
            }
            self.nics.insert(ip.to_string(), Arc::downgrade(&nic));
            ipnets.push(IpNet::from_str(&format!(
                "{}/{}",
                ip,
                self.ipv4net.prefix_len()
            ))?);
        }

        {
            let mut ni = nic.lock().await;
            let _ = ni.add_addrs_to_interface("eth0", &ipnets).await;
        }

        Ok(())
    }

    // caller should hold the mutex
    fn assign_ip_address(&mut self) -> Result<IpAddr> {
        // See: https://stackoverflow.com/questions/14915188/ip-address-ending-with-zero

        if self.last_id == 0xfe {
            return Err(Error::ErrAddressSpaceExhausted);
        }

        self.last_id += 1;
        match self.ipv4net.addr() {
            IpAddr::V4(ipv4) => {
                let mut ip = ipv4.octets();
                ip[3] = self.last_id;
                Ok(IpAddr::V4(Ipv4Addr::from(ip)))
            }
            IpAddr::V6(ipv6) => {
                let mut ip = ipv6.octets();
                ip[15] += self.last_id;
                Ok(IpAddr::V6(Ipv6Addr::from(ip)))
            }
        }
    }
}
