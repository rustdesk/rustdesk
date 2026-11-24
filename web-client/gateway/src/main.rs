use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use rustdesk_web_gateway::{server, server::GatewayConfig};

#[derive(Parser, Debug)]
#[command(name = "rustdesk-web-gateway", about = "Browser WebSocket gateway for a RustDesk host")]
struct Args {
    /// Listen address for the page, health check, and WebSocket.
    #[arg(long, env = "WEB_GATEWAY_LISTEN", default_value = "127.0.0.1:21119")]
    listen: SocketAddr,

    /// Directory of the built web client. When omitted, a built `web/dist` next
    /// to the repository or `/app/web` is used if it is present.
    #[arg(long, env = "WEB_GATEWAY_STATIC")]
    static_dir: Option<PathBuf>,

    /// Optional access token. Empty means the gateway is open, which is the local default.
    #[arg(long, env = "GATEWAY_TOKEN")]
    token: Option<String>,
}

fn static_dir(explicit: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(dir) = explicit {
        return Some(dir);
    }
    for candidate in [
        PathBuf::from("web-client/web/dist"),
        PathBuf::from("web/dist"),
        PathBuf::from("/app/web"),
    ] {
        if candidate.join("index.html").is_file() {
            return Some(candidate);
        }
    }
    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if hbb_common::sodiumoxide::init().is_err() {
        anyhow::bail!("sodiumoxide init failed");
    }
    hbb_common::env_logger::Builder::from_env(
        hbb_common::env_logger::Env::default().default_filter_or("info"),
    )
    .init();
    let args = Args::parse();
    server::serve(
        args.listen,
        GatewayConfig {
            static_dir: static_dir(args.static_dir),
            token: args.token,
        },
    )
    .await?;
    Ok(())
}
