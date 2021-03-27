use tokio::{self, prelude::*};
use parity_tokio_ipc::Endpoint;

#[tokio::main]
async fn main() {
	let path = std::env::args().nth(1).expect("Run it with server path to connect as argument");

	let mut client = Endpoint::connect(&path).await
		.expect("Failed to connect client.");

	loop {
		let mut buf = [0u8; 4];
		println!("SEND: PING");
		client.write_all(b"ping").await.expect("Unable to write message to client");
		client.read_exact(&mut buf[..]).await.expect("Unable to read buffer");
		if let Ok("pong") = std::str::from_utf8(&buf[..]) {
			println!("RECEIVED: PONG");
		} else {
			break;
		}

		tokio::time::delay_for(std::time::Duration::from_secs(2)).await;
	}
}
