use super::*;
use std::sync::Arc;

const DEMO_IP: &str = "1.2.3.4";

#[tokio::test]
async fn test_resolver_standalone() -> Result<()> {
    let mut r = Resolver::new();

    // should have localhost by default
    let name = "localhost";
    let ip_addr = "127.0.0.1";
    let ip = IpAddr::from_str(ip_addr)?;

    if let Some(resolved) = r.lookup(name.to_owned()).await {
        assert_eq!(resolved, ip, "should match");
    } else {
        panic!("should Some, but got None");
    }

    let name = "abc.com";
    let ip_addr = DEMO_IP;
    let ip = IpAddr::from_str(ip_addr)?;
    log::debug!("adding {} {}", name, ip_addr);

    r.add_host(name.to_owned(), ip_addr.to_owned())?;

    if let Some(resolved) = r.lookup(name.to_owned()).await {
        assert_eq!(resolved, ip, "should match");
    } else {
        panic!("should Some, but got None");
    }

    Ok(())
}

#[tokio::test]
async fn test_resolver_cascaded() -> Result<()> {
    let mut r0 = Resolver::new();

    let name0 = "abc.com";
    let ip_addr0 = DEMO_IP;
    let ip0 = IpAddr::from_str(ip_addr0)?;
    r0.add_host(name0.to_owned(), ip_addr0.to_owned())?;

    let mut r1 = Resolver::new();

    let name1 = "myserver.local";
    let ip_addr1 = "10.1.2.5";
    let ip1 = IpAddr::from_str(ip_addr1)?;
    r1.add_host(name1.to_owned(), ip_addr1.to_owned())?;

    let resolver0 = Arc::new(Mutex::new(r0));
    r1.set_parent(Arc::downgrade(&resolver0));

    if let Some(resolved) = r1.lookup(name0.to_owned()).await {
        assert_eq!(resolved, ip0, "should match");
    } else {
        panic!("should Some, but got None");
    }

    if let Some(resolved) = r1.lookup(name1.to_owned()).await {
        assert_eq!(resolved, ip1, "should match");
    } else {
        panic!("should Some, but got None");
    }

    // should fail if the name does not exist
    let result = r1.lookup("bad.com".to_owned()).await;
    assert!(result.is_none(), "should fail");

    Ok(())
}
