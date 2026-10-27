use futures::{SinkExt, StreamExt};
use hbb_common::{
    config::{keys, Socks5Server, APP_NAME, OVERWRITE_SETTINGS},
    socket_client,
    tcp::DynTcpStream,
    tls::{upsert_tls_cache, TlsType},
    websocket::WsFramedStream,
    Stream,
};
use rustls_pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::timeout,
};
use tokio_rustls::{rustls, TlsAcceptor};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

const TIMEOUT: u64 = 5_000;
const PAYLOAD: &[u8] = b"RustDesk websocket proxy test";

fn tls_acceptor() -> TlsAcceptor {
    // This self-signed identity is only used by the local mock servers.
    let cert = CertificateDer::from_pem_slice(include_bytes!("data/websocket-cert.pem")).unwrap();
    let key = PrivateKeyDer::from_pem_slice(include_bytes!("data/websocket-key.pem")).unwrap();
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

async fn read_headers(stream: &mut DynTcpStream) -> String {
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        assert!(headers.len() < 4096);
        headers.push(stream.read_u8().await.unwrap());
    }
    String::from_utf8(headers).unwrap()
}

async fn echo(stream: DynTcpStream, path: &str, host: &str) {
    let mut ws =
        tokio_tungstenite::accept_hdr_async(stream, |request: &Request, response: Response| {
            assert_eq!(request.uri().to_string(), path);
            assert_eq!(request.headers()["host"], host);
            assert!(!request.headers().contains_key("proxy-authorization"));
            Ok(response)
        })
        .await
        .unwrap();
    let message = ws.next().await.unwrap().unwrap();
    assert!(message.is_binary());
    assert_eq!(message.into_data().as_ref(), PAYLOAD);
    ws.send(PAYLOAD.to_vec().into()).await.unwrap();
    ws.close(None).await.unwrap();
}

async fn exchange(mut stream: Stream) {
    stream.send_bytes(PAYLOAD.to_vec().into()).await.unwrap();
    let reply = stream.next_timeout(TIMEOUT).await.unwrap().unwrap();
    assert_eq!(reply.as_ref(), PAYLOAD);
    if matches!(&stream, Stream::WebSocket(_)) {
        assert!(timeout(Duration::from_millis(TIMEOUT), stream.next())
            .await
            .unwrap()
            .is_none());
    }
}

fn option(key: &str, value: &str) {
    OVERWRITE_SETTINGS
        .write()
        .unwrap()
        .insert(key.into(), value.into());
}

async fn proxy_round_trip(
    scheme: &str,
    url: &str,
    tls_type: TlsType,
    auth: bool,
    configured: bool,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = Socks5Server {
        proxy: format!("{scheme}://{}", listener.local_addr().unwrap()),
        username: if auth { "user".into() } else { String::new() },
        password: if auth { "pass".into() } else { String::new() },
    };
    upsert_tls_cache(&proxy.proxy, TlsType::Rustls, true);
    upsert_tls_cache(url, tls_type, true);
    let target = url::Url::parse(url).unwrap();
    let host = target.host_str().unwrap().to_owned();
    let domain = target.domain().map(str::to_owned);
    let port = target.port_or_known_default().unwrap();
    let authority = match target.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    };
    let path = format!(
        "{}{}",
        target.path(),
        target.query().map(|q| format!("?{q}")).unwrap_or_default()
    );
    let wss = target.scheme() == "wss";
    let scheme = scheme.to_owned();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut stream = DynTcpStream(Box::new(stream));
        if scheme == "https" {
            stream = DynTcpStream(Box::new(tls_acceptor().accept(stream).await.unwrap()));
        }
        if scheme == "socks5" {
            let mut greeting = [0; 3];
            stream.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [5, 1, 0]);
            stream.write_all(&[5, 0]).await.unwrap();
            let mut request = [0; 5];
            stream.read_exact(&mut request).await.unwrap();
            assert_eq!(&request[..4], &[5, 1, 0, 3]);
            let mut name = vec![0; request[4] as usize];
            stream.read_exact(&mut name).await.unwrap();
            assert_eq!(name, host.as_bytes());
            assert_eq!(stream.read_u16().await.unwrap(), port);
            stream
                .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
                .await
                .unwrap();
        } else {
            let headers = read_headers(&mut stream).await;
            assert!(headers.starts_with(&format!("CONNECT {host}:{port} HTTP/1.1\r\n")));
            assert!(headers.contains(&format!("Host: {host}:{port}\r\n")));
            assert_eq!(
                headers.contains("Proxy-Authorization: Basic dXNlcjpwYXNz\r\n"),
                auth
            );
            stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await
                .unwrap();
        }
        stream.flush().await.unwrap();
        if wss {
            let tls = tls_acceptor().accept(stream).await.unwrap();
            assert_eq!(tls.get_ref().1.server_name(), domain.as_deref());
            stream = DynTcpStream(Box::new(tls));
        }
        echo(stream, &path, &authority).await;
    });
    option(keys::OPTION_PROXY_URL, &proxy.proxy);
    option(keys::OPTION_PROXY_USERNAME, &proxy.username);
    option(keys::OPTION_PROXY_PASSWORD, &proxy.password);
    let stream = if configured {
        option(keys::OPTION_ALLOW_WEBSOCKET, "Y");
        option("custom-rendezvous-server", target.host_str().unwrap());
        option(
            "api-server",
            if wss {
                "https://websocket-target.invalid"
            } else {
                "http://websocket-target.invalid"
            },
        );
        socket_client::connect_tcp("websocket-target.invalid:21116", TIMEOUT)
            .await
            .unwrap()
    } else {
        WsFramedStream::new(url, None, TIMEOUT)
            .await
            .map(Stream::WebSocket)
            .unwrap()
    };
    exchange(stream).await;
    timeout(Duration::from_millis(TIMEOUT), server)
        .await
        .unwrap()
        .unwrap();
}

async fn rejected_or_stalled_proxy(stalled: bool, configured: bool) {
    let direct = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = Socks5Server {
        proxy: format!("http://{}", listener.local_addr().unwrap()),
        ..Default::default()
    };
    option(keys::OPTION_PROXY_URL, &proxy.proxy);
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut stream = DynTcpStream(Box::new(stream));
        read_headers(&mut stream).await;
        if !stalled {
            stream
                .write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n")
                .await
                .unwrap();
        }
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).await.unwrap(), 0);
    });
    let result = timeout(Duration::from_millis(TIMEOUT), async {
        let url = format!("wss://{}/ws/id", direct.local_addr().unwrap());
        if configured {
            socket_client::connect_tcp(url, 200).await
        } else {
            WsFramedStream::new(url, None, 200)
                .await
                .map(Stream::WebSocket)
        }
    })
    .await
    .unwrap();
    let error = result.err().expect("the proxy must fail");
    if stalled {
        assert!(error
            .downcast_ref::<tokio::time::error::Elapsed>()
            .is_some());
    } else {
        assert!(error.to_string().contains("407"), "{}", error);
    }
    assert!(timeout(Duration::from_millis(50), direct.accept())
        .await
        .is_err());
    timeout(Duration::from_millis(TIMEOUT), server)
        .await
        .unwrap()
        .unwrap();
}

async fn direct_round_trip(wss: bool) {
    OVERWRITE_SETTINGS
        .write()
        .unwrap()
        .remove(keys::OPTION_PROXY_URL);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("{}://{addr}/ws/id", if wss { "wss" } else { "ws" });
    upsert_tls_cache(&url, TlsType::Rustls, true);
    let server = tokio::spawn(async move {
        let (stream, client_addr) = listener.accept().await.unwrap();
        let mut stream = DynTcpStream(Box::new(stream));
        if wss {
            stream = DynTcpStream(Box::new(tls_acceptor().accept(stream).await.unwrap()));
        }
        echo(stream, "/ws/id", &addr.to_string()).await;
        client_addr
    });
    let stream = socket_client::connect_tcp(url, TIMEOUT).await.unwrap();
    let local_addr = stream.local_addr();
    exchange(stream).await;
    let client_addr = timeout(Duration::from_millis(TIMEOUT), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(local_addr, client_addr);
}

async fn tcp_proxy_round_trip() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = Socks5Server {
        proxy: format!("http://{}", listener.local_addr().unwrap()),
        ..Default::default()
    };
    option(keys::OPTION_PROXY_URL, &proxy.proxy);
    option(keys::OPTION_PROXY_USERNAME, "");
    option(keys::OPTION_PROXY_PASSWORD, "");
    let server = tokio::spawn(async move {
        let (stream, addr) = listener.accept().await.unwrap();
        let mut stream = DynTcpStream(Box::new(stream));
        let headers = read_headers(&mut stream).await;
        assert!(headers.starts_with("CONNECT tcp-target.invalid:21116 HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .unwrap();
        let mut stream = hbb_common::tcp::FramedStream::from(stream, addr);
        let message = stream.next().await.unwrap().unwrap();
        assert_eq!(message.as_ref(), PAYLOAD);
        stream.send_bytes(message.freeze()).await.unwrap();
    });
    let stream = socket_client::connect_tcp_local("tcp-target.invalid:21116", None, TIMEOUT)
        .await
        .unwrap();
    exchange(stream).await;
    timeout(Duration::from_millis(TIMEOUT), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn websocket_proxy_connections() {
    // Keep configuration changes inside this integration test process.
    *APP_NAME.write().unwrap() = format!("RustDesk WebSocket Test {}", std::process::id());
    option(keys::OPTION_ALLOW_INSECURE_TLS_FALLBACK, "Y");
    for scheme in ["http", "https"] {
        for url in ["ws://[::1]:21119", "wss://[::1]:21119"] {
            proxy_round_trip(scheme, url, TlsType::Rustls, false, false).await;
        }
    }
    for (scheme, url, tls_type, auth, configured) in [
        (
            "http",
            "ws://websocket-target.invalid/ws/id",
            TlsType::Plain,
            false,
            true,
        ),
        (
            "http",
            "wss://websocket-target.invalid/ws/id",
            TlsType::Rustls,
            true,
            true,
        ),
        (
            "http",
            "wss://websocket-target.invalid:8443/ws/relay?test=1",
            TlsType::NativeTls,
            true,
            false,
        ),
        (
            "https",
            "ws://websocket-target.invalid/ws/id",
            TlsType::Plain,
            true,
            false,
        ),
        (
            "https",
            "wss://websocket-target.invalid/ws/relay",
            TlsType::Rustls,
            true,
            false,
        ),
        (
            "https",
            "wss://websocket-target.invalid/ws/relay",
            TlsType::NativeTls,
            true,
            false,
        ),
        (
            "socks5",
            "ws://websocket-target.invalid/ws/id",
            TlsType::Plain,
            false,
            false,
        ),
        (
            "socks5",
            "wss://websocket-target.invalid/ws/relay",
            TlsType::Rustls,
            false,
            false,
        ),
        (
            "socks5",
            "wss://websocket-target.invalid/ws/relay",
            TlsType::NativeTls,
            false,
            false,
        ),
    ] {
        proxy_round_trip(scheme, url, tls_type, auth, configured).await;
    }
    for scheme in ["http", "https", "socks5"] {
        for (url, tls_type) in [
            (
                "ws://websocket-target.invalid:8080/ws/id?test=1",
                TlsType::Plain,
            ),
            (
                "wss://websocket-target.invalid:8443/ws/relay?test=1",
                TlsType::Rustls,
            ),
            (
                "wss://websocket-target.invalid:8443/ws/relay?test=1",
                TlsType::NativeTls,
            ),
        ] {
            proxy_round_trip(scheme, url, tls_type, scheme != "socks5", false).await;
        }
    }
    for configured in [false, true] {
        rejected_or_stalled_proxy(false, configured).await;
        rejected_or_stalled_proxy(true, configured).await;
    }
    tcp_proxy_round_trip().await;

    for wss in [false, true] {
        direct_round_trip(wss).await;
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    for scheme in ["wss", "ws"] {
        let url = format!("{scheme}://{addr}/ws/id");
        let error = WsFramedStream::new(url, None, TIMEOUT).await.err().unwrap();
        assert!(error.to_string().starts_with("WebSocket error:"));
    }
}
