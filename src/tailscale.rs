//! Lists the machines of the local Tailscale network by asking the `tailscale` CLI.
//!
//! Tailscale documents `status --json` as "format subject to change", so only a handful of
//! fields are read and every one of them is optional; a missing field degrades the listing
//! instead of failing it.
//!
//! Tailscale peers are reachable by their `100.x` address, which RustDesk accepts as a peer id
//! for direct IP connections, so no extra rendezvous is needed.

use serde_derive::Deserialize;
use std::{
    process::Command,
    sync::Mutex,
    time::{Duration, Instant},
};

const CACHE_TTL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscalePeer {
    /// Tailscale IPv4 address, usable directly as a connection id.
    pub ip: String,
    pub hostname: String,
    pub os: String,
    pub online: bool,
}

#[derive(Deserialize)]
struct Status {
    #[serde(rename = "BackendState", default)]
    backend_state: String,
    #[serde(rename = "Peer", default)]
    peer: Option<std::collections::HashMap<String, Node>>,
}

#[derive(Deserialize)]
struct Node {
    #[serde(rename = "HostName", default)]
    host_name: String,
    #[serde(rename = "OS", default)]
    os: String,
    #[serde(rename = "Online", default)]
    online: bool,
    #[serde(rename = "TailscaleIPs", default)]
    ips: Vec<String>,
}

fn parse_status(json: &[u8]) -> Result<Vec<TailscalePeer>, serde_json::Error> {
    let status: Status = serde_json::from_slice(json)?;
    if status.backend_state != "Running" {
        return Ok(Vec::new());
    }
    let mut peers: Vec<TailscalePeer> = status
        .peer
        .unwrap_or_default()
        .into_values()
        .filter_map(|n| {
            let ip = n.ips.into_iter().find(|ip| !ip.contains(':'))?;
            Some(TailscalePeer {
                ip,
                hostname: n.host_name,
                os: n.os,
                online: n.online,
            })
        })
        .collect();
    // Online machines first, then by name.
    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()))
    });
    Ok(peers)
}

/// Where the CLI may live. Tailscale documents that the macOS standalone variant installs a
/// launcher in `/usr/local/bin`, while the App Store variant only ships the CLI inside the app
/// bundle; elsewhere it is expected on `PATH`.
const CLI_CANDIDATES: &[&str] = &[
    "tailscale",
    #[cfg(target_os = "macos")]
    "/usr/local/bin/tailscale",
    #[cfg(target_os = "macos")]
    "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
];

fn run_cli() -> Option<Vec<u8>> {
    // There is no CLI on Android and iOS.
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return None;
    }
    for cli in CLI_CANDIDATES {
        let mut cmd = Command::new(cli);
        // The macOS GUI binary acts as the CLI only when told so from a non-terminal process.
        cmd.env("TAILSCALE_BE_CLI", "1").args(["status", "--json"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        match cmd.output() {
            Ok(o) if o.status.success() => return Some(o.stdout),
            // Installed but not running or not logged in: no point trying other locations.
            Ok(_) => return None,
            Err(_) => continue,
        }
    }
    None
}

fn query() -> Vec<TailscalePeer> {
    let Some(stdout) = run_cli() else {
        return Vec::new();
    };
    let output = stdout;
    match parse_status(&output) {
        Ok(peers) => peers,
        Err(e) => {
            hbb_common::log::warn!("Failed to parse tailscale status: {}", e);
            Vec::new()
        }
    }
}

/// Returns the peers of the Tailscale network, cached for a few seconds because the UI
/// refreshes the peer list often and each query spawns a process.
pub fn get_peers() -> Vec<TailscalePeer> {
    static CACHE: Mutex<Option<(Instant, Vec<TailscalePeer>)>> = Mutex::new(None);
    if let Ok(cache) = CACHE.lock() {
        if let Some((at, peers)) = cache.as_ref() {
            if at.elapsed() < CACHE_TTL {
                return peers.clone();
            }
        }
    }
    let peers = query();
    if let Ok(mut cache) = CACHE.lock() {
        *cache = Some((Instant::now(), peers.clone()));
    }
    peers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_sorts_peers() {
        let json = br#"{"BackendState":"Running","Peer":{
            "a":{"HostName":"zeta","OS":"linux","Online":false,"TailscaleIPs":["100.1.1.1","fd7a::1"]},
            "b":{"HostName":"Alpha","OS":"macOS","Online":true,"TailscaleIPs":["fd7a::2","100.1.1.2"]},
            "c":{"HostName":"noip","OS":"linux","Online":true,"TailscaleIPs":["fd7a::3"]}}}"#;
        let peers = parse_status(json).unwrap();
        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].hostname, "Alpha");
        assert_eq!(peers[0].ip, "100.1.1.2");
        assert!(!peers[1].online);
    }

    #[test]
    fn stopped_backend_yields_nothing() {
        let json = br#"{"BackendState":"Stopped","Peer":{"a":{"HostName":"x","TailscaleIPs":["100.1.1.1"],"Online":true}}}"#;
        assert!(parse_status(json).unwrap().is_empty());
    }
}
