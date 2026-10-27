use super::FlushBeforeRead;
use crate::{proxy::Proxy, tcp::DynTcpStream, tls::TlsType};
use futures::{SinkExt, StreamExt};
use rustls_pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{duplex, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufStream},
    time::timeout,
};
use tokio_native_tls::native_tls;
use tokio_rustls::{rustls, TlsAcceptor};
use tokio_socks::IntoTargetAddr;
use tokio_tungstenite::{accept_async, client_async_tls_with_config, Connector, WebSocketStream};

fn acceptor() -> TlsAcceptor {
    let cert =
        CertificateDer::from_pem_slice(include_bytes!("../../tests/data/websocket-cert.pem"))
            .unwrap();
    let key = PrivateKeyDer::from_pem_slice(include_bytes!("../../tests/data/websocket-key.pem"))
        .unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![cert], key)
    .unwrap();
    TlsAcceptor::from(Arc::new(config))
}

async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(
    ws: WebSocketStream<S>,
    sent: u8,
    received: u8,
) {
    let (mut writer, mut reader) = ws.split();
    let send = async {
        for sequence in 0..16 {
            writer
                .send(vec![sent + sequence; 256 * 1024].into())
                .await
                .unwrap();
        }
    };
    let receive = async {
        for sequence in 0..16 {
            let message = reader.next().await.unwrap().unwrap();
            assert!(message.is_binary());
            assert_eq!(
                message.into_data().as_ref(),
                vec![received + sequence; 256 * 1024]
            );
        }
    };
    tokio::join!(send, receive);
}

#[tokio::test]
async fn native_tls_wss_bidirectional_transfer_through_proxy_buffers() {
    for outer_tls in [TlsType::Plain, TlsType::Rustls, TlsType::NativeTls] {
        // A 4 KiB pipe forces backpressure while both sides send 4 MiB.
        let (client, server) = duplex(4096);
        let server = async {
            let mut stream = DynTcpStream(Box::new(server));
            if !matches!(outer_tls, TlsType::Plain) {
                stream = DynTcpStream(Box::new(acceptor().accept(stream).await.unwrap()));
            }
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                assert!(headers.len() < 4096);
                headers.push(stream.read_u8().await.unwrap());
            }
            assert!(headers.starts_with(b"CONNECT target.invalid:443 HTTP/1.1\r\n"));
            stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await
                .unwrap();
            stream.flush().await.unwrap();
            let stream = acceptor().accept(stream).await.unwrap();
            exchange(accept_async(stream).await.unwrap(), 64, 0).await;
        };
        let client = async {
            let proxy = Proxy::new("https://proxy.invalid:4443", 5_000).unwrap();
            let target = "target.invalid:443".into_target_addr().unwrap();
            let stream = match outer_tls {
                TlsType::Plain => {
                    DynTcpStream(Box::new(proxy.http_connect(client, &target).await.unwrap()))
                }
                TlsType::Rustls => DynTcpStream(Box::new(
                    proxy
                        .https_connect_rustls(client, &target, true)
                        .await
                        .unwrap(),
                )),
                TlsType::NativeTls => DynTcpStream(Box::new(
                    proxy
                        .https_connect_nativetls(client, &target, true)
                        .await
                        .unwrap(),
                )),
            };
            let connector = native_tls::TlsConnector::builder()
                .danger_accept_invalid_certs(true)
                .build()
                .unwrap();
            let (ws, _) = client_async_tls_with_config(
                "wss://target.invalid/ws/relay",
                FlushBeforeRead(stream),
                None,
                Some(Connector::NativeTls(connector)),
            )
            .await
            .unwrap();
            exchange(ws, 0, 64).await;
        };
        timeout(Duration::from_secs(20), async {
            tokio::join!(client, server)
        })
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn read_progresses_while_proxy_flush_is_blocked() {
    let (client, mut peer) = duplex(64);
    let mut client = FlushBeforeRead(DynTcpStream(Box::new(BufStream::new(client))));
    let payload = [0x5a; 4096];
    client.write_all(&payload).await.unwrap();
    peer.write_all(b"pong").await.unwrap();

    let mut reply = [0; 4];
    timeout(Duration::from_secs(1), client.read_exact(&mut reply))
        .await
        .expect("a blocked flush must not prevent receiving queued data")
        .unwrap();
    assert_eq!(&reply, b"pong");

    let mut received = [0; 4096];
    timeout(Duration::from_secs(1), async {
        let (flushed, read) = tokio::join!(client.flush(), peer.read_exact(&mut received));
        flushed.unwrap();
        read.unwrap();
    })
    .await
    .unwrap();
    assert_eq!(received, payload);
}
