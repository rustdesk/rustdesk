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

impl KcpStream {
    // Engage KCP's built-in congestion control (nc=0) unless disabled by option: pure turbo
    // (nc=1) keeps blasting a full 1024-segment window through loss, which on constrained
    // links amplifies brief loss into a spiral users experience as stalls or drops. This is
    // sender-side only, so no wire negotiation is needed and either peer may run either
    // profile. Requires kcp-sys from the `rustdesk-patches` branch, which wires the config
    // factory into connection setup (on older revs the factory was stored but never consulted).
    fn apply_kcp_config(endpoint: &mut KcpEndpoint) {
        if crate::get_kcp_cc_enabled() {
            endpoint.set_kcp_config_factory(Box::new(|conv| {
                let mut config = kcp_sys::ffi_safe::KcpConfig::new_turbo(conv);
                config.nc = Some(0);
                config
            }));
        }
    }

    // One line per run of socket failures rather than per failure: these are absorbed as packet
    // loss and can repeat every 10ms, and debug output is written to the log file.
    const KCP_IO_FAIL_PERSIST: u64 = 500; // ~5s at the 10ms retry interval

    fn note_io_err(fail_run: &mut u64, what: &str, e: &std::io::Error) {
        *fail_run += 1;
        if *fail_run == 1 {
            log::debug!("KCP {} error (treated as loss): {:?}", what, e);
        } else if *fail_run % Self::KCP_IO_FAIL_PERSIST == 0 {
            // Still failing well past a transient ICMP: say so once per run length, else a
            // socket that never recovers is silent until KCP reaps it 60s later.
            log::warn!(
                "KCP {} failing persistently: {} consecutive errors, last: {:?}",
                what,
                fail_run,
                e
            );
        }
    }

    fn note_io_ok(fail_run: &mut u64) {
        if *fail_run >= Self::KCP_IO_FAIL_PERSIST {
            log::info!("KCP socket recovered after {} failed operations", fail_run);
        }
        *fail_run = 0;
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
            // A connected UDP socket surfaces ICMP port-unreachable as an error on
            // send/recv (WSAECONNRESET 10054 on Windows, ECONNREFUSED on Linux). For UDP
            // these are advisory: a stray ICMP from a NAT rebind glitch or a momentary
            // peer hiccup does not mean the path is dead, and KCP retransmits through it.
            // Treat socket errors as packet loss instead of tearing the session down;
            // a truly dead link is reaped by the KCP pong timeout / app-level timeouts.
            // The short sleep prevents a persistently failing socket from busy-spinning.
            //
            // Log by run, not per occurrence: the 10ms sleep means a persistently failing
            // socket would otherwise write ~100 lines a second into the log file. One line
            // when a run starts, one per PERSIST run length so a stuck socket stays visible
            // (the pong timeout takes 60s to reap it, and the session is frozen meanwhile),
            // and one when it recovers carrying the total.
            let mut fail_run = 0u64;
            loop {
                tokio::select! {
                    _ = &mut stop_receiver => {
                        log::debug!("KCP io loop received stop signal");
                        break;
                    }
                    Some(data) = output.recv() => {
                        match udp.send(&data.inner()).await {
                            Ok(_) => Self::note_io_ok(&mut fail_run),
                            Err(e) => {
                                Self::note_io_err(&mut fail_run, "send", &e);
                                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                            }
                        }
                    }
                    result = udp.recv_from(&mut buf) => {
                        match result {
                            Ok((size, _)) => {
                                Self::note_io_ok(&mut fail_run);
                                if size < std::mem::size_of::<KcpPacketHeader>() {
                                    continue;
                                }
                                input
                                    .send(BytesMut::from(&buf[..size]).into())
                                    .await.ok();
                            }
                            Err(e) => {
                                Self::note_io_err(&mut fail_run, "recv", &e);
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
