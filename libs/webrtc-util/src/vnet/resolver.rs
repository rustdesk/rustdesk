#[cfg(test)]
mod resolver_test;

use std::collections::HashMap;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Weak;

use tokio::sync::Mutex;

use crate::error::*;

#[derive(Default)]
pub(crate) struct Resolver {
    parent: Option<Weak<Mutex<Resolver>>>,
    hosts: HashMap<String, IpAddr>,
}

impl Resolver {
    pub(crate) fn new() -> Self {
        let mut r = Resolver {
            parent: None,
            hosts: HashMap::new(),
        };

        if let Err(err) = r.add_host("localhost".to_owned(), "127.0.0.1".to_owned()) {
            log::warn!("failed to add localhost to Resolver: {}", err);
        }
        r
    }

    pub(crate) fn set_parent(&mut self, p: Weak<Mutex<Resolver>>) {
        self.parent = Some(p);
    }

    pub(crate) fn add_host(&mut self, name: String, ip_addr: String) -> Result<()> {
        if name.is_empty() {
            return Err(Error::ErrHostnameEmpty);
        }
        let ip = IpAddr::from_str(&ip_addr)?;
        self.hosts.insert(name, ip);

        Ok(())
    }

    pub(crate) fn lookup(
        &self,
        host_name: String,
    ) -> Pin<Box<dyn Future<Output = Option<IpAddr>> + Send + 'static>> {
        if let Some(ip) = self.hosts.get(&host_name) {
            let ip2 = *ip;
            return Box::pin(async move { Some(ip2) });
        }

        // mutex must be unlocked before calling into parent Resolver
        if let Some(parent) = self.parent.clone().and_then(|p| p.upgrade()).clone() {
            Box::pin(async move {
                let p = parent.lock().await;
                p.lookup(host_name).await
            })
        } else {
            Box::pin(async move { None })
        }
    }
}
