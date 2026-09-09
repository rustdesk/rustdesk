use hbb_common::{
    anyhow,
    bytes::{Bytes, BytesMut},
    bytes_codec::BytesCodec,
    config, log,
    tcp::{DynTcpStream, FramedStream},
    tokio::{self, net::UdpSocket, sync::mpsc, sync::oneshot},
    tokio_util, ResultType, Stream,
};
use kcp_sys::{
    endpoint::KcpEndpoint,
    packet_def::{KcpPacket, KcpPacketHeader},
    stream,
};
use std::{net::SocketAddr, sync::Arc};

pub struct KcpStream {
    _endpoint: KcpEndpoint,
    stop_sender: Option<oneshot::Sender<()>>,
}

const KCP_IO_ERR_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
static KCP_SEND_ERR_LOG: hbb_common::log_throttle::LogThrottle =
    hbb_common::log_throttle::LogThrottle::new(KCP_IO_ERR_LOG_INTERVAL);
static KCP_RECV_ERR_LOG: hbb_common::log_throttle::LogThrottle =
    hbb_common::log_throttle::LogThrottle::new(KCP_IO_ERR_LOG_INTERVAL);

impl KcpStream {
    // Opt in to KCP's built-in congestion window (nc=0) instead of the pure turbo profile
    // (nc=1) that has always shipped; see `get_kcp_cc_enabled` for why this is not the default.
    // Sender-side only, so no wire negotiation is needed and either peer may run either profile.
    // Requires kcp-sys from the `rustdesk-patches` branch, which wires the config factory into
    // connection setup (on older revs the factory was stored but never consulted).
    fn apply_kcp_config(endpoint: &mut KcpEndpoint) {
        if crate::get_kcp_cc_enabled() {
            endpoint.set_kcp_config_factory(Box::new(|conv| {
                let mut config = kcp_sys::ffi_safe::KcpConfig::new_turbo(conv);
                config.nc = Some(0);
                config
            }));
        }
    }

    fn create_framed(stream: stream::KcpStream, local_addr: Option<SocketAddr>) -> Stream {
        Stream::Tcp(FramedStream(
            tokio_util::codec::Framed::new(DynTcpStream(Box::new(stream)), BytesCodec::new()),
            local_addr.unwrap_or(config::Config::get_any_listen_addr(true)),
            None,
            0,
        ))
    }

    pub async fn accept(
        udp_socket: Arc<UdpSocket>,
        timeout: std::time::Duration,
        init_packet: Option<BytesMut>,
    ) -> ResultType<(Self, Stream)> {
        let mut endpoint = KcpEndpoint::new();
        Self::apply_kcp_config(&mut endpoint);
        endpoint.run().await;

        let (input, output) = (
            endpoint.input_sender(),
            endpoint
                .output_receiver()
                .ok_or_else(|| anyhow::anyhow!("Failed to get output receiver"))?,
        );
        let (stop_sender, stop_receiver) = oneshot::channel();
        if let Some(packet) = init_packet {
            if packet.len() >= std::mem::size_of::<KcpPacketHeader>() {
                input.send(packet.into()).await?;
            }
        }
        Self::kcp_io(udp_socket.clone(), input, output, stop_receiver).await;

        let conn_id = tokio::time::timeout(timeout, endpoint.accept()).await??;
        if let Some(stream) = stream::KcpStream::new(&endpoint, conn_id) {
            Ok((
                Self {
                    _endpoint: endpoint,
                    stop_sender: Some(stop_sender),
                },
                Self::create_framed(stream, udp_socket.local_addr().ok()),
            ))
        } else {
            Err(anyhow::anyhow!("Failed to create KcpStream"))
        }
    }

    pub async fn connect(
        udp_socket: Arc<UdpSocket>,
        timeout: std::time::Duration,
    ) -> ResultType<(Self, Stream)> {
        let mut endpoint = KcpEndpoint::new();
        Self::apply_kcp_config(&mut endpoint);
        endpoint.run().await;

        let (input, output) = (
            endpoint.input_sender(),
            endpoint
                .output_receiver()
                .ok_or_else(|| anyhow::anyhow!("Failed to get output receiver"))?,
        );
        let (stop_sender, stop_receiver) = oneshot::channel();
        Self::kcp_io(udp_socket.clone(), input, output, stop_receiver).await;

        let conn_id = endpoint.connect(timeout, 0, 0, Bytes::new()).await?;
        if let Some(stream) = stream::KcpStream::new(&endpoint, conn_id) {
            Ok((
                Self {
                    _endpoint: endpoint,
                    stop_sender: Some(stop_sender),
                },
                Self::create_framed(stream, udp_socket.local_addr().ok()),
            ))
        } else {
            Err(anyhow::anyhow!("Failed to create KcpStream"))
        }
    }

    async fn kcp_io(
        udp_socket: Arc<UdpSocket>,
        input: mpsc::Sender<KcpPacket>,
        mut output: mpsc::Receiver<KcpPacket>,
        mut stop_receiver: oneshot::Receiver<()>,
    ) {
        let udp = udp_socket.clone();
        tokio::spawn(async move {
            let mut buf = vec![0; 1500];
            // Socket errors are ICMP unreachable on a connected UDP socket — advisory, and
            // routine while a hole forms — so treat them as loss and let KCP's pong timeout reap
            // a link that is really dead. One throttle PER DIRECTION: the error is reported once
            // and cleared, so send-ok/recv-err alternates and a shared counter never fires.
            loop {
                tokio::select! {
                    _ = &mut stop_receiver => {
                        log::debug!("KCP io loop received stop signal");
                        break;
                    }
                    Some(data) = output.recv() => {
                        if let Err(e) = udp.send(&data.inner()).await {
                            if let Some(n) = KCP_SEND_ERR_LOG.due() {
                                log::debug!("KCP send error x{n} (treated as loss), last: {e}");
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                    }
                    result = udp.recv_from(&mut buf) => {
                        match result {
                            Ok((size, _)) => {
                                if size < std::mem::size_of::<KcpPacketHeader>() {
                                    continue;
                                }
                                input
                                    .send(BytesMut::from(&buf[..size]).into())
                                    .await.ok();
                            }
                            Err(e) => {
                                if let Some(n) = KCP_RECV_ERR_LOG.due() {
                                    log::debug!("KCP recv error x{n} (treated as loss), last: {e}");
                                }
                                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                            }
                        }
                    }
                    else => {
                        log::debug!("KCP endpoint input closed");
                        break;
                    }
                }
            }
        });
    }
}

impl Drop for KcpStream {
    fn drop(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    async fn connected_pair() -> (Arc<UdpSocket>, Arc<UdpSocket>) {
        let a = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let b = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        a.connect(b.local_addr().unwrap()).await.unwrap();
        b.connect(a.local_addr().unwrap()).await.unwrap();
        (Arc::new(a), Arc::new(b))
    }

    async fn establish() -> ((KcpStream, Stream), (KcpStream, Stream)) {
        let (a, b) = connected_pair().await;
        let (accept_res, connect_res) = tokio::join!(
            KcpStream::accept(b, Duration::from_secs(5), None),
            KcpStream::connect(a, Duration::from_secs(5))
        );
        (
            connect_res.expect("connect over loopback"),
            accept_res.expect("accept over loopback"),
        )
    }

    // The full client path over real loopback sockets: handshake through the kcp_io
    // pumps, framed data both ways, then a graceful close. The endpoint guard stays
    // alive across the stream drop so the FIN can go out, and the peer's framed
    // stream must end (BrokenPipe from the kcp reader) instead of hanging.
    #[tokio::test]
    async fn test_kcp_stream_loopback_roundtrip_and_close() {
        let ((_guard_a, mut stream_a), (_guard_b, mut stream_b)) = establish().await;

        stream_a
            .send_bytes(Bytes::from_static(b"ping"))
            .await
            .unwrap();
        let got = stream_b.next_timeout(5000).await.unwrap().unwrap();
        assert_eq!(&got[..], b"ping");

        stream_b
            .send_bytes(Bytes::from_static(b"pong"))
            .await
            .unwrap();
        let got = stream_a.next_timeout(5000).await.unwrap().unwrap();
        assert_eq!(&got[..], b"pong");

        drop(stream_a);
        match stream_b.next_timeout(10_000).await {
            None | Some(Err(_)) => {}
            Some(Ok(data)) => panic!("unexpected data after close: {:?}", data),
        }
    }

    // A writer that queues many frames and closes immediately must not cost the
    // reader any of them: every frame arrives intact, in order, before end-of-stream.
    // This is the client-side pin for the kcp-sys close-tail-drain semantics, through
    // the real BytesCodec framing rustdesk sessions use.
    #[tokio::test]
    async fn test_kcp_stream_close_delivers_all_frames() {
        let ((_guard_a, mut tx), (_guard_b, mut rx)) = establish().await;

        const N: usize = 50;
        let payload = vec![7u8; 32 * 1024];
        for _ in 0..N {
            tx.send_bytes(Bytes::from(payload.clone())).await.unwrap();
        }
        drop(tx);

        let mut got = 0usize;
        loop {
            match rx.next_timeout(10_000).await {
                Some(Ok(data)) => {
                    assert_eq!(data.len(), payload.len(), "frame boundary broken");
                    assert!(data.iter().all(|&b| b == 7), "frame content corrupted");
                    got += 1;
                }
                // BrokenPipe (kcp reader end) or timeout-None both end the stream.
                None | Some(Err(_)) => break,
            }
        }
        assert_eq!(got, N, "graceful close lost frames");
    }

    // Socket errors on the connected UDP socket (ICMP unreachable after the peer
    // vanishes) are advisory: the io loop must treat them as loss - keep accepting
    // writes, keep running - rather than tearing the session down. Whether the OS
    // actually surfaces ECONNREFUSED here is platform-dependent; either way the
    // session must stay alive for this window.
    #[tokio::test]
    async fn test_kcp_io_treats_socket_errors_as_loss() {
        let ((_guard_a, mut stream_a), (guard_b, stream_b)) = establish().await;

        // Kill the peer entirely: endpoint stops, socket closes.
        drop(stream_b);
        drop(guard_b);
        tokio::time::sleep(Duration::from_millis(50)).await;

        for _ in 0..10 {
            stream_a
                .send_bytes(Bytes::from_static(b"into the void"))
                .await
                .expect("socket errors must be treated as loss, not stream failure");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    // The connect deadline must hold when nothing answers: no hang, prompt error.
    #[tokio::test]
    async fn test_kcp_connect_timeout_without_peer() {
        let (a, _b) = connected_pair().await;
        let start = tokio::time::Instant::now();
        let res = KcpStream::connect(a, Duration::from_millis(600)).await;
        assert!(res.is_err(), "connect must fail with no peer endpoint");
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "connect did not honor its deadline"
        );
    }
}
