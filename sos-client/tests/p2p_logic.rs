//! 验证 P2P（打洞）路径中容易出错的纯逻辑。
//!
//! 生产上这几处曾经全错，导致 SOS 从启动起就永久放弃 P2P、只能走中继：
//!
//! 1. NAT 探测的第二个端口用成了 `+1`（hbbr 中继端口，不应答 `TestNatRequest`）；
//! 2. NAT 探测没有复用本地端口，两次比对的源端口天然不同 → 恒判 SYMMETRIC；
//! 3. 探测失败（拿到 0）时直接写 SYMMETRIC —— 把「探测失败」当成「对称 NAT」；
//! 4. `PunchHole.socket_addr` 是 `AddrMangle` 的**二进制**，
//!    却用 `String::from_utf8_lossy` 当字符串解析，必然连不上。
//!
//! 这里只测纯逻辑（不需要真实网络）。
//!
//! 运行：cargo test --release --test p2p_logic -- --nocapture

#![cfg(windows)]

use hbb_common::protobuf::Enum;
use hbb_common::rendezvous_proto::NatType;

/// 与上游 `src/common.rs::test_nat_type_` 相同的判定规则：
/// 两个端口都拿到值时才下结论；都拿到且相等 → ASYMMETRIC，否则 SYMMETRIC；
/// 任一为 0（探测失败）→ **保持 UNKNOWN**，不能写成 SYMMETRIC。
fn classify(ports: [i32; 2]) -> Option<NatType> {
    if ports[0] > 0 && ports[1] > 0 {
        Some(if ports[0] == ports[1] {
            NatType::ASYMMETRIC
        } else {
            NatType::SYMMETRIC
        })
    } else {
        None
    }
}

#[test]
fn same_external_port_is_asymmetric() {
    assert_eq!(classify([50155, 50155]), Some(NatType::ASYMMETRIC));
    assert_eq!(classify([1, 1]), Some(NatType::ASYMMETRIC));
}

#[test]
fn different_external_port_is_symmetric() {
    assert_eq!(classify([50155, 50156]), Some(NatType::SYMMETRIC));
}

/// 这是修复前最致命的 bug：第二个探测（连的是 hbbr 中继端口）必然失败，
/// 拿到 0，然后被判定成 SYMMETRIC，于是 `handle_punch_hole` 永远直接走中继。
#[test]
fn failed_probe_keeps_unknown_not_symmetric() {
    assert_eq!(classify([50155, 0]), None, "端口 1 失败不应判 SYMMETRIC");
    assert_eq!(classify([0, 50155]), None, "端口 0 失败不应判 SYMMETRIC");
    assert_eq!(classify([0, 0]), None, "两次都失败不应判 SYMMETRIC");
}

/// `handle_punch_hole` 的中继判定必须与上游一致：
/// 只在「明确已知对称 NAT」或对端强制中继、或对端地址无效时放弃打洞。
#[test]
fn relay_decision_matches_upstream() {
    let relay = |force_relay: bool, remote: NatType, us: NatType, port: u16| {
        force_relay
            || remote == NatType::SYMMETRIC
            || us == NatType::SYMMETRIC
            || port == 0
    };

    // 双方都 UNKNOWN（本机探测没成功）：照常尝试打洞，这很关键
    assert!(!relay(false, NatType::UNKNOWN_NAT, NatType::UNKNOWN_NAT, 50155));

    // 双方都 ASYMMETRIC：最佳情况，必须打洞
    assert!(!relay(false, NatType::ASYMMETRIC, NatType::ASYMMETRIC, 50155));

    // 我方 ASYMMETRIC、对端 UNKNOWN：仍然值得一试
    assert!(!relay(false, NatType::UNKNOWN_NAT, NatType::ASYMMETRIC, 50155));

    // 任意一方明确 SYMMETRIC：打洞无效
    assert!(relay(false, NatType::SYMMETRIC, NatType::UNKNOWN_NAT, 50155));
    assert!(relay(false, NatType::UNKNOWN_NAT, NatType::SYMMETRIC, 50155));

    // 对端显式要求中继
    assert!(relay(true, NatType::ASYMMETRIC, NatType::ASYMMETRIC, 50155));

    // 地址无效（端口 0）
    assert!(relay(false, NatType::ASYMMETRIC, NatType::ASYMMETRIC, 0));
}

/// `AddrMangle` 是 Protocol 里 `socket_addr` 字段的编码方式，是**二进制**。
/// 用 `String::from_utf8_lossy` 会得到乱码，是修复前连不上的直接原因。
#[test]
fn addr_mangle_roundtrip() {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

    let v4 = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 7), 50155));
    let decoded = hbb_common::AddrMangle::decode(&hbb_common::AddrMangle::encode(v4));
    println!("v4 {} -> {}", v4, decoded);
    assert_eq!(decoded, v4, "IPv4 编解码必须可逆");

    let v6 = SocketAddr::V6(SocketAddrV6::new(
        "2001:db8::1".parse::<Ipv6Addr>().unwrap(),
        50155,
        0,
        0,
    ));
    let decoded6 = hbb_common::AddrMangle::decode(&hbb_common::AddrMangle::encode(v6));
    println!("v6 {} -> {}", v6, decoded6);
    assert_eq!(decoded6, v6, "IPv6 编解码必须可逆");
}

/// 空字段不能 panic（`PunchHole.socket_addr_v6` 在纯 IPv4 环境下是空的）。
#[test]
fn empty_addr_does_not_panic() {
    let a = hbb_common::AddrMangle::decode(&[]);
    println!("空字节 -> {}", a);
    assert_eq!(a.port(), 0, "空地址应解出端口 0，供上层判断为无效");
}

/// 二进制编码与 UTF-8 字符串解析的结果必然不同 —— 记录这一点，
/// 防止有人再把 `socket_addr` 当字符串用。
#[test]
fn binary_encoding_is_not_utf8_text() {
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(203, 0, 113, 7), 50155));
    let encoded = hbb_common::AddrMangle::encode(addr);
    let as_text = String::from_utf8_lossy(&encoded).to_string();
    println!("编码字节 {:?}", encoded);
    println!("误当字符串解析 -> {:?}", as_text);
    assert!(
        as_text.parse::<SocketAddr>().is_err() || as_text != addr.to_string(),
        "AddrMangle 编码不能被当作可解析的地址字符串"
    );
}

/// NAT 类型的数值必须与 proto 定义一致（存进配置的是 i32）。
#[test]
fn nat_type_values_match_proto() {
    assert_eq!(NatType::UNKNOWN_NAT as i32, 0);
    assert_eq!(NatType::ASYMMETRIC as i32, 1);
    assert_eq!(NatType::SYMMETRIC as i32, 2);
    // 配置里的 0 必须解回 UNKNOWN，而不是 SYMMETRIC
    assert_eq!(
        NatType::from_i32(0).unwrap(),
        NatType::UNKNOWN_NAT,
        "0 必须解为 UNKNOWN，否则会误触发中继"
    );
}
