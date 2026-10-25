use super::*;
use crate::error::Result;

#[test]
fn test_tcp_frag_string() {
    let f = TCP_FLAG_FIN;
    assert_eq!(f.to_string(), "FIN", "should match");
    let f = TCP_FLAG_SYN;
    assert_eq!(f.to_string(), "SYN", "should match");
    let f = TCP_FLAG_RST;
    assert_eq!(f.to_string(), "RST", "should match");
    let f = TCP_FLAG_PSH;
    assert_eq!(f.to_string(), "PSH", "should match");
    let f = TCP_FLAG_ACK;
    assert_eq!(f.to_string(), "ACK", "should match");
    let f = TCP_FLAG_SYN | TCP_FLAG_ACK;
    assert_eq!(f.to_string(), "SYN-ACK", "should match");
}

const DEMO_IP: &str = "1.2.3.4";

#[test]
fn test_chunk_udp() -> Result<()> {
    let src = SocketAddr::from_str("192.168.0.2:1234")?;
    let dst = SocketAddr::from_str(&(DEMO_IP.to_owned() + ":5678"))?;

    let mut c = ChunkUdp::new(src, dst);
    let s = c.to_string();
    log::debug!("chunk: {}", s);
    assert_eq!(c.network(), UDP_STR, "should match");
    assert!(s.contains(&src.to_string()), "should include address");
    assert!(s.contains(&dst.to_string()), "should include address");
    assert_eq!(c.get_source_ip(), src.ip(), "ip should match");
    assert_eq!(c.get_destination_ip(), dst.ip(), "ip should match");

    // Test timestamp
    let ts = c.set_timestamp();
    assert_eq!(ts, c.get_timestamp(), "timestamp should match");

    c.user_data = "Hello".as_bytes().to_vec();

    let cloned = c.clone_to();

    // Test setSourceAddr
    c.set_source_addr("2.3.4.5:4000")?;
    assert_eq!(c.source_addr().to_string(), "2.3.4.5:4000");

    // Test Tag()
    assert!(!c.tag().is_empty(), "should not be empty");

    // Verify cloned chunk was not affected by the changes to original chunk
    c.user_data[0] = b'!'; // oroginal: "Hello" -> "Hell!"
    assert_eq!(cloned.user_data(), "Hello".as_bytes(), "should match");
    assert_eq!(cloned.source_addr().to_string(), "192.168.0.2:1234");
    assert_eq!(cloned.get_source_ip(), src.ip(), "ip should match");
    assert_eq!(cloned.get_destination_ip(), dst.ip(), "ip should match");

    Ok(())
}
