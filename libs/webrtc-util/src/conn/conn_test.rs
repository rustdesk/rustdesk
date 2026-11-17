use super::*;

#[tokio::test]
async fn test_conn_lookup_host() -> Result<()> {
    let stun_serv_addr = "stun1.l.google.com:19302";

    if let Ok(ipv4_addr) = lookup_host(true, stun_serv_addr).await {
        assert!(
            ipv4_addr.is_ipv4(),
            "expected ipv4 but got ipv6: {ipv4_addr}"
        );
    }

    if let Ok(ipv6_addr) = lookup_host(false, stun_serv_addr).await {
        assert!(
            ipv6_addr.is_ipv6(),
            "expected ipv6 but got ipv4: {ipv6_addr}"
        );
    }

    Ok(())
}
