use sha2::{Digest, Sha256};

/// Challenge response the controlled side checks in `validate_password_plain`.
///
/// `h1 = SHA256(password || salt)`, `response = SHA256(h1 || challenge)`.
pub fn password_response(password: &str, salt: &str, challenge: &str) -> Vec<u8> {
    let mut h1 = Sha256::new();
    h1.update(password.as_bytes());
    h1.update(salt.as_bytes());
    let h1 = h1.finalize();
    let mut h2 = Sha256::new();
    h2.update(&h1[..]);
    h2.update(challenge.as_bytes());
    h2.finalize().to_vec()
}

/// Login `username` for a direct TCP dial. The host accepts the session only
/// when this string is an IP (optional port) or a `host:port` domain.
pub fn direct_username(host: &str, port: u16) -> String {
    let host = host.trim();
    if hbb_common::is_ip_str(host) && host_has_port(host) {
        return host.to_owned();
    }
    if let Some(ip) = resolve_ip(host) {
        return format!("{ip}:{port}");
    }
    format!("{host}:{port}")
}

fn host_has_port(host: &str) -> bool {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.contains("]:");
    }
    host.matches(':').count() == 1
}

fn resolve_ip(host: &str) -> Option<std::net::IpAddr> {
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return Some(ip);
    }
    let portless = host.split(':').next().unwrap_or(host);
    std::net::ToSocketAddrs::to_socket_addrs(&(portless, 0))
        .ok()
        .and_then(|mut it| it.next())
        .map(|addr| addr.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_response_matches_the_host_formula() {
        let got = password_response("pw", "salt", "b");
        assert_eq!(
            got,
            hex_decode("803235e4d1ed7843e834f5e27655ea12b7ab5c194226dd10e6aebcc9d3dc5ec1")
        );
    }

    #[test]
    fn direct_username_keeps_an_ipv4_with_port() {
        assert_eq!(direct_username("127.0.0.1:21118", 21118), "127.0.0.1:21118");
        assert_eq!(direct_username("127.0.0.1", 21118), "127.0.0.1:21118");
    }

    fn hex_decode(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
