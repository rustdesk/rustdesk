//! 中继候选解析与本地优先级。
//!
//! ## 协议事实（已核对 rustdesk-server 源码）
//!
//! 协议里 `relay_server` 是**单值** `string`，没有列表概念：hbbs 自己在
//! `relay_servers: Vec<String>` 里选一台再下发
//! （OSS 走 `ROTATION_RELAY_SERVER` 轮询，Pro 走 GeoLite2 地理最近），
//! 客户端拿到的永远是单个主机名。
//!
//! 逗号分隔的多中继是**服务器侧**配置：hbbs 的 `relay-servers=` / `RELAY=`
//! 环境变量，由 `src/common.rs::get_servers()` 的 `split(',')` 解析。
//! 所以「服务器会下发逗号分隔列表」是不成立的，上游不做切分是**正确的**。
//!
//! 本模块存在的理由是另一件事：`--relay` 是**本客户端自己的**选项，
//! 运维可以借此在本机配几台后备（服务器只给一台，那一台可能挂了）。
//!
//! 另外锁定上游 `get_relay_server()` 的优先级：本地配置优先于服务器下发。
//!
//! 运行：cargo test --release --test relay_candidates -- --nocapture

// `sos_relay.rs` 刻意不依赖 crate 内任何模块（纯 std），
// 因此可以按路径直接 include，测的就是生产代码本身而非副本。
#[path = "../src/sos_relay.rs"]
mod sos_relay;

use sos_relay::{relay_candidates, resolve_relay_candidates as resolve};

#[test]
fn single_relay_is_kept_as_is() {
    assert_eq!(
        relay_candidates("relay.example.com"),
        vec!["relay.example.com".to_string()]
    );
}

#[test]
fn comma_separated_relays_are_split_in_order() {
    // 顺序即优先级，必须原样保留
    assert_eq!(
        relay_candidates("relay1.example.com,relay2.example.com,relay3.example.com"),
        vec![
            "relay1.example.com".to_string(),
            "relay2.example.com".to_string(),
            "relay3.example.com".to_string(),
        ]
    );
}

#[test]
fn whitespace_and_empty_items_are_dropped() {
    // 运维手填时常带空格，也常见结尾多一个逗号
    assert_eq!(
        relay_candidates("  relay1.example.com , , relay2.example.com ,"),
        vec![
            "relay1.example.com".to_string(),
            "relay2.example.com".to_string()
        ]
    );
    assert!(relay_candidates("").is_empty());
    assert!(relay_candidates("  ,  , ").is_empty());
}

#[test]
fn duplicates_are_removed_case_insensitively() {
    // 同一台被写两遍不该重复拨号；主机名大小写不敏感
    assert_eq!(
        relay_candidates("Relay.Example.com,relay.example.com,RELAY.EXAMPLE.COM"),
        vec!["Relay.Example.com".to_string()],
        "重复项应折叠成首个写法的单条"
    );
}

#[test]
fn ipv6_literal_with_port_survives() {
    // IPv6 字面量内部是冒号不是逗号，切分不应破坏它
    assert_eq!(
        relay_candidates("[2001:db8::1]:21117,relay2.example.com"),
        vec![
            "[2001:db8::1]:21117".to_string(),
            "relay2.example.com".to_string()
        ]
    );
}

#[test]
fn cli_relay_beats_server_provided() {
    // 与上游 get_relay_server 一致：本地配置优先
    assert_eq!(
        resolve("my.relay.local", "relay.from.server"),
        vec!["my.relay.local".to_string()]
    );
    // 即使本地也写了多个，也整体取代服务器下发
    assert_eq!(
        resolve("a.local,b.local", "relay.from.server"),
        vec!["a.local".to_string(), "b.local".to_string()]
    );
}

#[test]
fn server_provided_used_when_cli_is_empty() {
    assert_eq!(
        resolve("", "relay1.from.server,relay2.from.server"),
        vec![
            "relay1.from.server".to_string(),
            "relay2.from.server".to_string(),
        ]
    );
}

#[test]
fn cli_with_only_separators_falls_through_to_server() {
    // `--relay ",,"` 之类的无效输入不能被当成「用户指定了」，
    // 否则会拿到空列表而不是回退到服务器下发
    assert_eq!(
        resolve(" , , ", "relay.from.server"),
        vec!["relay.from.server".to_string()]
    );
}

#[test]
fn both_empty_yields_empty_list_for_caller_to_fallback() {
    // 空列表由调用方（sos_rendezvous）补「信令服务器 +1」兜底
    assert!(resolve("", "").is_empty());
}
