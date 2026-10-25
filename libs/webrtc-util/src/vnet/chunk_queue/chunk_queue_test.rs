use std::net::SocketAddr;
use std::str::FromStr;

use super::*;
use crate::error::Result;

const DEMO_IP: &str = "1.2.3.4";

#[tokio::test]
async fn test_chunk_queue() -> Result<()> {
    let c: Box<dyn Chunk> = Box::new(ChunkUdp::new(
        SocketAddr::from_str("192.188.0.2:1234")?,
        SocketAddr::from_str(&(DEMO_IP.to_owned() + ":5678"))?,
    ));

    let q = ChunkQueue::new(0);

    let d = q.peek().await;
    assert!(d.is_none(), "should return none");

    let ok = q.push(c.clone_to()).await;
    assert!(ok, "should succeed");

    let d = q.pop().await;
    assert!(d.is_some(), "should succeed");
    if let Some(d) = d {
        assert_eq!(c.to_string(), d.to_string(), "should be the same");
    }

    let d = q.pop().await;
    assert!(d.is_none(), "should fail");

    let q = ChunkQueue::new(1);
    let ok = q.push(c.clone_to()).await;
    assert!(ok, "should succeed");

    let ok = q.push(c.clone_to()).await;
    assert!(!ok, "should fail");

    let d = q.peek().await;
    assert!(d.is_some(), "should succeed");
    if let Some(d) = d {
        assert_eq!(c.to_string(), d.to_string(), "should be the same");
    }

    Ok(())
}
