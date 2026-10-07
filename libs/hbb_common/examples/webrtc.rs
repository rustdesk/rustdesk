extern crate hbb_common;

#[cfg(feature = "webrtc")]
use hbb_common::webrtc::WebRTCStream;

use std::io::Write;
use anyhow::Result;
use bytes::Bytes;
use clap::{Arg, Command};
use tokio::time::Duration;

#[cfg(not(feature = "webrtc"))]
#[tokio::main]
async fn main() -> Result<()> {
    println!(
        "The webrtc feature is not enabled. \
        Please enable the webrtc feature to run this example."
    );
    Ok(())
}

#[cfg(feature = "webrtc")]
#[tokio::main]
async fn main() -> Result<()> {
    let app = Command::new("webrtc-stream")
        .about("An example of webrtc stream using hbb_common and webrtc-rs")
        .arg(
            Arg::new("debug")
                .long("debug")
                .short('d')
                .action(clap::ArgAction::SetTrue)
                .help("Prints debug log information"),
        )
        .arg(
            Arg::new("offer")
                .long("offer")
                .short('o')
                .help("set offer from other endpoint"),
        );

    let matches = app.clone().get_matches();

    let debug = matches.contains_id("debug");
    if debug {
        println!("Debug log enabled");
        env_logger::Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{}:{} [{}] {} - {}",
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    record.level(),
                    chrono::Local::now().format("%H:%M:%S.%6f"),
                    record.args()
                )
            })
            .filter(Some("hbb_common"), log::LevelFilter::Debug)
            .init();
    }

    let remote_endpoint = if let Some(endpoint) = matches.get_one::<String>("offer") {
        endpoint.to_string()
    } else {
        "".to_string()
    };

    let webrtc_stream = WebRTCStream::new(&remote_endpoint, false, 30000).await?;
    // Print the offer to be sent to the other peer
    let local_endpoint = webrtc_stream.get_local_endpoint().await?;

    if remote_endpoint.is_empty() {
        println!();
        // Wait for the answer to be pasted
        println!(
            "Start new terminal run: \n{} \ncopy remote endpoint and paste here",
            format!(
                "cargo r --features webrtc --example webrtc -- --offer {}",
                local_endpoint
            )
        );
        // readline blocking
        let line = std::io::stdin()
            .lines()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No input received"))??;
        webrtc_stream.set_remote_endpoint(&line).await?;
    } else {
        println!(
            "Copy local endpoint and paste to the other peer: \n{}",
            local_endpoint
        );
    }

    let s1 = webrtc_stream.clone();
    tokio::spawn(async move {
        let _ = read_loop(s1).await;
    });

    let s2 = webrtc_stream.clone();
    tokio::spawn(async move {
        let _ = write_loop(s2).await;
    });

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
    };

    Ok(())
}

// read_loop shows how to read from the datachannel directly
#[cfg(feature = "webrtc")]
async fn read_loop(mut stream: WebRTCStream) -> Result<()> {
    loop {
        let Some(res) = stream.next().await else {
            println!("WebRTC stream closed; Exit the read_loop");
            return Ok(());
        };
        match res {
            Err(e) => {
                println!("WebRTC stream read error: {}; Exit the read_loop", e);
                return Ok(());
            }
            Ok(data) => {
                println!("Message from stream: {}", String::from_utf8(data.to_vec())?);
            }
        }
    }
}

// write_loop shows how to write to the webrtc stream directly
#[cfg(feature = "webrtc")]
async fn write_loop(mut stream: WebRTCStream) -> Result<()> {
    let mut result = Result::<()>::Ok(());
    while result.is_ok() {
        let timeout = tokio::time::sleep(Duration::from_secs(5));
        tokio::pin!(timeout);

        tokio::select! {
            _ = timeout.as_mut() =>{
                let message = webrtc::peer_connection::math_rand_alpha(15);
                result = stream.send_bytes(Bytes::from(message.clone())).await;
                println!("Sent '{message}' {}", result.is_ok());
            }
        };
    }
    println!("WebRTC stream write failed; Exit the write_loop");

    Ok(())
}
