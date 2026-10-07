use crate::{bail, config, tcp, websocket, ResultType};
#[cfg(feature = "webrtc")]
use crate::webrtc;
use sodiumoxide::crypto::secretbox::Key;
use std::net::SocketAddr;
use tokio::net::TcpStream;

// support Websocket and tcp.
pub enum Stream {
    #[cfg(feature = "webrtc")]
    WebRTC(webrtc::WebRTCStream),
    WebSocket(websocket::WsFramedStream),
    Tcp(tcp::FramedStream),
}

impl Stream {
    #[inline]
    pub fn set_send_timeout(&mut self, ms: u64) {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.set_send_timeout(ms),
            Stream::WebSocket(s) => s.set_send_timeout(ms),
            Stream::Tcp(s) => s.set_send_timeout(ms),
        }
    }

    #[inline]
    pub fn set_raw(&mut self) {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.set_raw(),
            Stream::WebSocket(s) => s.set_raw(),
            Stream::Tcp(s) => s.set_raw(),
        }
    }

    /// The largest message the peer may send. Lowered before authorization, where nothing a peer
    /// legitimately sends is large, so an unauthenticated connection cannot make us hold more than
    /// that. Each transport clamps `n` to its own ceiling, so `usize::MAX` restores the default.
    ///
    /// Set it before the first read of untrusted data. Lowering it later is not a way to constrain
    /// a message already in progress: what that does to one is up to the transport.
    #[inline]
    pub fn set_max_packet_length(&mut self, n: usize) {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.set_max_packet_length(n),
            Stream::WebSocket(s) => s.set_max_packet_length(n),
            Stream::Tcp(s) => s.codec_mut().set_max_packet_length(n),
        }
    }

    #[inline]
    pub async fn send_bytes(&mut self, bytes: bytes::Bytes) -> ResultType<()> {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.send_bytes(bytes).await,
            Stream::WebSocket(s) => s.send_bytes(bytes).await,
            Stream::Tcp(s) => s.send_bytes(bytes).await,
        }
    }

    #[inline]
    pub async fn send_raw(&mut self, bytes: Vec<u8>) -> ResultType<()> {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.send_raw(bytes).await,
            Stream::WebSocket(s) => s.send_raw(bytes).await,
            Stream::Tcp(s) => s.send_raw(bytes).await,
        }
    }

    #[inline]
    pub fn set_key(&mut self, key: Key) {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.set_key(key),
            Stream::WebSocket(s) => s.set_key(key),
            Stream::Tcp(s) => s.set_key(key),
        }
    }

    /// The stream key for the key exchange version in `t.picked`: version 0 is `set_key`, and
    /// version 1 derives one key per direction from the transcript. Any other version is refused
    /// rather than run as one of these. `is_initiator` is the side that sent the sealed key.
    #[inline]
    pub fn set_negotiated_key(
        &mut self,
        key: Key,
        is_initiator: bool,
        t: &tcp::KxTranscript,
    ) -> ResultType<()> {
        match t.picked {
            0 => {
                self.set_key(key);
                Ok(())
            }
            1 => match self {
                #[cfg(feature = "webrtc")]
                Stream::WebRTC(_) => bail!("key exchange version 1 does not apply to WebRTC"),
                Stream::WebSocket(s) => s.set_key_split(key, is_initiator, t),
                Stream::Tcp(s) => s.set_key_split(key, is_initiator, t),
            },
            v => bail!("unsupported key exchange version {}", v),
        }
    }

    /// An opaque token that changes whenever bytes have arrived from the peer, a fragment of a
    /// message that is still incomplete included. Compare successive samples; the value itself
    /// means nothing. `None` where the transport reports no such thing.
    ///
    /// A message reaches the caller only once it is whole, so `next()` alone cannot distinguish a
    /// peer sending a large one — a clipboard image is the case that occurs — from one that died
    /// mid-message. Sampling this alongside the last received message tells them apart.
    ///
    /// TCP and WebSocket answer `None` on purpose: this change leaves their existing close, error
    /// and 30s-timeout behaviour exactly as it was, and giving them a value would alter it.
    #[inline]
    pub fn rx_progress(&self) -> Option<u64> {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => Some(s.rx_progress()),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// Whether ICE has stopped hearing from the peer on a WebRTC transport. Transient and
    /// recoverable — it says the peer is worth suspecting, never that the session is over — and
    /// always false for transports this change leaves alone.
    ///
    /// Maintained by ICE's own task, so unlike anything derived from the session's read loop it
    /// stays accurate while that loop is busy sending.
    #[inline]
    pub fn webrtc_disconnected(&self) -> bool {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.is_disconnected(),
            #[allow(unreachable_patterns)]
            _ => false,
        }
    }

    #[inline]
    pub fn is_secured(&self) -> bool {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.is_secured(),
            Stream::WebSocket(s) => s.is_secured(),
            Stream::Tcp(s) => s.is_secured(),
        }
    }

    /// Whether this is a WebRTC transport. Used to enforce the WebRTC-only DTLS fingerprint
    /// binding (fail closed) in the secure handshake.
    #[inline]
    pub fn is_webrtc(&self) -> bool {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(_) => true,
            #[allow(unreachable_patterns)]
            _ => false,
        }
    }

    /// Close the underlying WebRTC peer connection if this is a WebRTC stream; no-op otherwise.
    /// A WebRTC pc is kept alive by the global session cache and its cleanup handler only fires on
    /// a terminal ICE state, so it must be closed explicitly at session end. TCP/WebSocket streams
    /// release their resources on drop and need nothing here.
    ///
    /// Deliberately not `async`: most callers sit in a `select!` arm or a future the UI can
    /// abandon, and an awaited close that loses that race is unretryable — `close()` latches
    /// `is_closed` before its first await, so every later attempt early-returns while the state
    /// handler that would evict the session never runs. With no await point here there is
    /// nothing to cancel; the teardown runs to completion on the WebRTC I/O runtime
    /// (see `webrtc::WEBRTC_RT`) that owns the pc's sockets and pump tasks, so it reaches the
    /// wire even after a caller's own runtime dies on return.
    #[inline]
    pub fn close_webrtc(&self) {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.close_detached(),
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }

    /// Whether an established WebRTC transport runs through a TURN relay (used for the UI's
    /// direct/relayed flag). `None` for non-WebRTC transports, and for a non-relay-policy pc
    /// before ICE selects a pair; a Relay-policy pc answers `Some(true)` straight away — see
    /// `WebRTCStream::is_relayed`.
    #[inline]
    pub async fn webrtc_relayed(&self) -> Option<bool> {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.is_relayed().await,
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// Whether an established WebRTC transport reaches the peer over IPv6 (used to name the
    /// transport in the UI). `None` for non-WebRTC transports and before ICE selects a pair —
    /// every other transport already carries the family in the label it was raced under.
    #[inline]
    pub async fn webrtc_remote_ipv6(&self) -> Option<bool> {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.is_remote_ipv6().await,
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// DTLS certificate fingerprint for a WebRTC stream (`local`=true for this endpoint's own
    /// cert, false for the peer's), used to bind the channel to the signed peer identity.
    /// Returns None for non-WebRTC transports, which authenticate via the secretbox key exchange.
    #[inline]
    #[cfg_attr(not(feature = "webrtc"), allow(unused_variables))]
    pub async fn dtls_fingerprint(&self, local: bool) -> Option<String> {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => {
                if local {
                    s.local_dtls_fingerprint().await.ok()
                } else {
                    s.remote_dtls_fingerprint().await.ok()
                }
            }
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    #[inline]
    pub async fn next_timeout(
        &mut self,
        timeout: u64,
    ) -> Option<Result<bytes::BytesMut, std::io::Error>> {
        match self {
            #[cfg(feature = "webrtc")]
            Stream::WebRTC(s) => s.next_timeout(timeout).await,
            Stream::WebSocket(s) => s.next_timeout(timeout).await,
            Stream::Tcp(s) => s.next_timeout(timeout).await,
        }
    }

    /// establish connect from websocket
    #[inline]
    pub async fn connect_websocket(
        url: impl AsRef<str>,
        local_addr: Option<SocketAddr>,
        proxy_conf: Option<&config::Socks5Server>,
        timeout_ms: u64,
    ) -> ResultType<Self> {
        let ws_stream =
            websocket::WsFramedStream::new(url, local_addr, proxy_conf, timeout_ms).await?;
        log::debug!("WebSocket connection established");
        Ok(Self::WebSocket(ws_stream))
    }

    /// send message
    #[inline]
    pub async fn send(&mut self, msg: &impl protobuf::Message) -> ResultType<()> {
        match self {
            #[cfg(feature = "webrtc")]
            Self::WebRTC(s) => s.send(msg).await,
            Self::WebSocket(ws) => ws.send(msg).await,
            Self::Tcp(tcp) => tcp.send(msg).await,
        }
    }

    /// receive message
    #[inline]
    pub async fn next(&mut self) -> Option<Result<bytes::BytesMut, std::io::Error>> {
        match self {
            #[cfg(feature = "webrtc")]
            Self::WebRTC(s) => s.next().await,
            Self::WebSocket(ws) => ws.next().await,
            Self::Tcp(tcp) => tcp.next().await,
        }
    }

    #[inline]
    pub fn local_addr(&self) -> SocketAddr {
        match self {
            #[cfg(feature = "webrtc")]
            Self::WebRTC(s) => s.local_addr(),
            Self::WebSocket(ws) => ws.local_addr(),
            Self::Tcp(tcp) => tcp.local_addr(),
        }
    }

    #[inline]
    pub fn from(stream: TcpStream, stream_addr: SocketAddr) -> Self {
        Self::Tcp(tcp::FramedStream::from(stream, stream_addr))
    }
}

/// Owning the stream owns the transport, WebRTC included.
///
/// A peer connection outlives its handle — the session cache holds a clone, and the handler that
/// evicts it only fires on a terminal ICE state — so it has to be closed explicitly. Doing that
/// at each exit path made it an obligation every `return`, `break` and `?` had to remember, and
/// the long-lived side never did: `server::connection` ends its ~15 exits by dropping the stream.
/// Nothing warned, because a missed close leaks silently and only under WebRTC.
///
/// `Stream` is not `Clone`, so dropping it really is the end of the transport and there is no
/// second owner to surprise. Explicit `close_webrtc()` calls remain valid — they close sooner
/// than scope end — but they are now an optimization rather than the thing correctness rests on.
impl Drop for Stream {
    fn drop(&mut self) {
        self.close_webrtc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_set_negotiated_key_refuses_unknown_versions() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut stream = Stream::from(TcpStream::connect(addr).await.unwrap(), addr);
        let t = |picked| tcp::KxTranscript {
            initiator_pk: &[1u8; 32],
            responder_pk: &[2u8; 32],
            advertised: 1,
            picked,
        };
        let mut set = |picked| stream.set_negotiated_key(Key([0u8; 32]), true, &t(picked));
        assert!(set(2).is_err());
        assert!(set(1).is_ok());
    }
}
