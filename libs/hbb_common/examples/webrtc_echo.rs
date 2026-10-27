// Temporary e2e helper for the web client: answers a browser offer read from
// stdin, bridges ICE candidates over stdin/stdout as JSON lines, then echoes
// every message it receives.
extern crate hbb_common;

#[cfg(feature = "webrtc")]
#[hbb_common::tokio::main]
async fn main() -> hbb_common::anyhow::Result<()> {
    use hbb_common::{
        anyhow::anyhow,
        serde_json::{self, json, Value},
        tokio,
        webrtc::WebRTCStream,
    };
    use std::io::BufRead;

    let mut offer = String::new();
    std::io::stdin().lock().read_line(&mut offer)?;
    if offer.trim().is_empty() {
        return Err(anyhow!("no offer"));
    }
    let mut stream = WebRTCStream::new(offer.trim(), false, 30000).await?;
    let out = json!({
        "answer": stream.local_endpoint(),
        "session_key": stream.session_key(),
        "local_fingerprint": stream.local_dtls_fingerprint().await?,
        "remote_fingerprint": stream.remote_dtls_fingerprint().await?,
    });
    println!("{}", out);
    let mut ice_rx = stream
        .take_local_ice_rx()
        .ok_or_else(|| anyhow!("no ice rx"))?;
    tokio::spawn(async move {
        while let Some(c) = ice_rx.recv().await {
            println!("{}", json!({ "candidate": c }));
        }
    });
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines().flatten() {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let for_ice = stream.clone();
    tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(c) = v.get("candidate").and_then(Value::as_str) {
                if let Err(e) = for_ice.add_remote_ice_candidate(c).await {
                    eprintln!("add_remote_ice_candidate failed: {e}");
                }
            }
        }
    });
    stream.wait_connected(30000).await?;
    println!(
        "{}",
        json!({
            "connected": true,
            "relayed": stream.is_relayed().await,
            "remote_ipv6": stream.is_remote_ipv6().await,
        })
    );
    while let Some(res) = stream.next().await {
        let bytes = res?;
        let len = bytes.len();
        stream.send_bytes(bytes.freeze()).await?;
        eprintln!("echoed {len} bytes");
    }
    stream.close().await;
    Ok(())
}

#[cfg(not(feature = "webrtc"))]
fn main() {}
