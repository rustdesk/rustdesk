use std::net::IpAddr;
use std::str::FromStr;

use async_trait::async_trait;

use super::*;
use crate::vnet::chunk::*;
use crate::vnet::conn::*;

#[derive(Default)]
struct DummyObserver;

#[async_trait]
impl ConnObserver for DummyObserver {
    async fn write(&self, _c: Box<dyn Chunk + Send + Sync>) -> Result<()> {
        Ok(())
    }

    async fn on_closed(&self, _addr: SocketAddr) {}

    fn determine_source_ip(&self, loc_ip: IpAddr, _dst_ip: IpAddr) -> Option<IpAddr> {
        Some(loc_ip)
    }
}

#[tokio::test]
async fn test_udp_conn_map_insert_remove() -> Result<()> {
    let conn_map = UdpConnMap::new();

    let obs: Arc<Mutex<dyn ConnObserver + Send + Sync>> = Arc::new(Mutex::new(DummyObserver));

    let conn_in = Arc::new(UdpConn::new(
        SocketAddr::from_str("127.0.0.1:1234")?,
        None,
        obs,
    ));

    conn_map.insert(Arc::clone(&conn_in)).await?;

    let conn_out = conn_map.find(&conn_in.local_addr()?).await;
    assert!(conn_out.is_some(), "should succeed");
    if let Some(conn_out) = conn_out {
        assert_eq!(
            conn_in.local_addr()?,
            conn_out.local_addr()?,
            "should match"
        );
        let port_map = conn_map.port_map.lock().await;
        assert_eq!(port_map.len(), 1, "should match");
    }

    conn_map.delete(&conn_in.local_addr()?).await?;
    {
        let port_map = conn_map.port_map.lock().await;
        assert_eq!(port_map.len(), 0, "should match");
    }

    let result = conn_map.delete(&conn_in.local_addr()?).await;
    assert!(result.is_err(), "should fail");

    Ok(())
}

#[tokio::test]
async fn test_udp_conn_map_insert_0_remove() -> Result<()> {
    let conn_map = UdpConnMap::new();

    let obs: Arc<Mutex<dyn ConnObserver + Send + Sync>> = Arc::new(Mutex::new(DummyObserver));

    let conn_in = Arc::new(UdpConn::new(
        SocketAddr::from_str("0.0.0.0:1234")?,
        None,
        obs,
    ));

    conn_map.insert(Arc::clone(&conn_in)).await?;

    let conn_out = conn_map.find(&conn_in.local_addr()?).await;
    assert!(conn_out.is_some(), "should succeed");
    if let Some(conn_out) = conn_out {
        assert_eq!(
            conn_in.local_addr()?,
            conn_out.local_addr()?,
            "should match"
        );
        let port_map = conn_map.port_map.lock().await;
        assert_eq!(port_map.len(), 1, "should match");
    }

    conn_map.delete(&conn_in.local_addr()?).await?;
    {
        let port_map = conn_map.port_map.lock().await;
        assert_eq!(port_map.len(), 0, "should match");
    }

    let result = conn_map.delete(&conn_in.local_addr()?).await;
    assert!(result.is_err(), "should fail");

    Ok(())
}

#[tokio::test]
async fn test_udp_conn_map_find_0() -> Result<()> {
    let conn_map = UdpConnMap::new();

    let obs: Arc<Mutex<dyn ConnObserver + Send + Sync>> = Arc::new(Mutex::new(DummyObserver));

    let conn_in = Arc::new(UdpConn::new(
        SocketAddr::from_str("0.0.0.0:1234")?,
        None,
        obs,
    ));

    conn_map.insert(Arc::clone(&conn_in)).await?;

    let addr = SocketAddr::from_str("192.168.0.1:1234")?;
    let conn_out = conn_map.find(&addr).await;
    assert!(conn_out.is_some(), "should succeed");
    if let Some(conn_out) = conn_out {
        let addr_in = conn_in.local_addr()?;
        let addr_out = conn_out.local_addr()?;
        assert_eq!(addr_in, addr_out, "should match");
        let port_map = conn_map.port_map.lock().await;
        assert_eq!(port_map.len(), 1, "should match");
    }

    Ok(())
}

#[tokio::test]
async fn test_udp_conn_map_insert_many_ips_with_same_port() -> Result<()> {
    let conn_map = UdpConnMap::new();

    let obs: Arc<Mutex<dyn ConnObserver + Send + Sync>> = Arc::new(Mutex::new(DummyObserver));

    let conn_in1 = Arc::new(UdpConn::new(
        SocketAddr::from_str("10.1.2.1:5678")?,
        None,
        Arc::clone(&obs),
    ));

    let conn_in2 = Arc::new(UdpConn::new(
        SocketAddr::from_str("10.1.2.2:5678")?,
        None,
        Arc::clone(&obs),
    ));

    conn_map.insert(Arc::clone(&conn_in1)).await?;
    conn_map.insert(Arc::clone(&conn_in2)).await?;

    let addr1 = SocketAddr::from_str("10.1.2.1:5678")?;
    let conn_out1 = conn_map.find(&addr1).await;
    assert!(conn_out1.is_some(), "should succeed");
    if let Some(conn_out1) = conn_out1 {
        let addr_in = conn_in1.local_addr()?;
        let addr_out = conn_out1.local_addr()?;
        assert_eq!(addr_in, addr_out, "should match");
        let port_map = conn_map.port_map.lock().await;
        assert_eq!(port_map.len(), 1, "should match");
    }

    let addr2 = SocketAddr::from_str("10.1.2.2:5678")?;
    let conn_out2 = conn_map.find(&addr2).await;
    assert!(conn_out2.is_some(), "should succeed");
    if let Some(conn_out2) = conn_out2 {
        let addr_in = conn_in2.local_addr()?;
        let addr_out = conn_out2.local_addr()?;
        assert_eq!(addr_in, addr_out, "should match");
        let port_map = conn_map.port_map.lock().await;
        assert_eq!(port_map.len(), 1, "should match");
    }

    Ok(())
}

#[tokio::test]
async fn test_udp_conn_map_already_inuse_when_insert_0() -> Result<()> {
    let conn_map = UdpConnMap::new();

    let obs: Arc<Mutex<dyn ConnObserver + Send + Sync>> = Arc::new(Mutex::new(DummyObserver));

    let conn_in1 = Arc::new(UdpConn::new(
        SocketAddr::from_str("10.1.2.1:5678")?,
        None,
        Arc::clone(&obs),
    ));
    let conn_in2 = Arc::new(UdpConn::new(
        SocketAddr::from_str("0.0.0.0:5678")?,
        None,
        Arc::clone(&obs),
    ));

    conn_map.insert(Arc::clone(&conn_in1)).await?;
    let result = conn_map.insert(Arc::clone(&conn_in2)).await;
    assert!(result.is_err(), "should fail");

    Ok(())
}

#[tokio::test]
async fn test_udp_conn_map_already_inuse_when_insert_a_specified_ip() -> Result<()> {
    let conn_map = UdpConnMap::new();

    let obs: Arc<Mutex<dyn ConnObserver + Send + Sync>> = Arc::new(Mutex::new(DummyObserver));

    let conn_in1 = Arc::new(UdpConn::new(
        SocketAddr::from_str("0.0.0.0:5678")?,
        None,
        Arc::clone(&obs),
    ));
    let conn_in2 = Arc::new(UdpConn::new(
        SocketAddr::from_str("192.168.0.1:5678")?,
        None,
        Arc::clone(&obs),
    ));

    conn_map.insert(Arc::clone(&conn_in1)).await?;
    let result = conn_map.insert(Arc::clone(&conn_in2)).await;
    assert!(result.is_err(), "should fail");

    Ok(())
}

#[tokio::test]
async fn test_udp_conn_map_already_inuse_when_insert_same_specified_ip() -> Result<()> {
    let conn_map = UdpConnMap::new();

    let obs: Arc<Mutex<dyn ConnObserver + Send + Sync>> = Arc::new(Mutex::new(DummyObserver));

    let conn_in1 = Arc::new(UdpConn::new(
        SocketAddr::from_str("192.168.0.1:5678")?,
        None,
        Arc::clone(&obs),
    ));
    let conn_in2 = Arc::new(UdpConn::new(
        SocketAddr::from_str("192.168.0.1:5678")?,
        None,
        Arc::clone(&obs),
    ));

    conn_map.insert(Arc::clone(&conn_in1)).await?;
    let result = conn_map.insert(Arc::clone(&conn_in2)).await;
    assert!(result.is_err(), "should fail");

    Ok(())
}

#[tokio::test]
async fn test_udp_conn_map_find_failure_1() -> Result<()> {
    let conn_map = UdpConnMap::new();

    let obs: Arc<Mutex<dyn ConnObserver + Send + Sync>> = Arc::new(Mutex::new(DummyObserver));

    let conn_in = Arc::new(UdpConn::new(
        SocketAddr::from_str("192.168.0.1:5678")?,
        None,
        obs,
    ));

    conn_map.insert(Arc::clone(&conn_in)).await?;

    let addr = SocketAddr::from_str("192.168.0.2:5678")?;
    let result = conn_map.find(&addr).await;
    assert!(result.is_none(), "should be none");

    Ok(())
}

#[tokio::test]
async fn test_udp_conn_map_find_failure_2() -> Result<()> {
    let conn_map = UdpConnMap::new();

    let obs: Arc<Mutex<dyn ConnObserver + Send + Sync>> = Arc::new(Mutex::new(DummyObserver));

    let conn_in = Arc::new(UdpConn::new(
        SocketAddr::from_str("192.168.0.1:5678")?,
        None,
        obs,
    ));

    conn_map.insert(Arc::clone(&conn_in)).await?;

    let addr = SocketAddr::from_str("192.168.0.1:1234")?;
    let result = conn_map.find(&addr).await;
    assert!(result.is_none(), "should be none");

    Ok(())
}

#[tokio::test]
async fn test_udp_conn_map_insert_two_on_same_port_then_remove() -> Result<()> {
    let conn_map = UdpConnMap::new();

    let obs: Arc<Mutex<dyn ConnObserver + Send + Sync>> = Arc::new(Mutex::new(DummyObserver));

    let conn_in1 = Arc::new(UdpConn::new(
        SocketAddr::from_str("192.168.0.1:5678")?,
        None,
        Arc::clone(&obs),
    ));
    let conn_in2 = Arc::new(UdpConn::new(
        SocketAddr::from_str("192.168.0.2:5678")?,
        None,
        Arc::clone(&obs),
    ));

    conn_map.insert(Arc::clone(&conn_in1)).await?;
    conn_map.insert(Arc::clone(&conn_in2)).await?;

    conn_map.delete(&conn_in1.local_addr()?).await?;
    conn_map.delete(&conn_in2.local_addr()?).await?;

    Ok(())
}
