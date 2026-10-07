//! WebRTC transport for RustDesk streams.
//!
//! # Bumping the webrtc crate
//!
//! This module depends on webrtc-rs internals its public API does not promise. Re-verify each
//! against the new sources (last checked: webrtc 0.13, -data 0.11, -sctp 0.12), then run
//! `cargo test --features webrtc webrtc::tests`.
//!
//! - `DataChannel::write` parks on a full PendingQueue instead of buffering without bound.
//! - SCTP max message size is 65536: `MAX_FRAGMENT_PAYLOAD` + 1 must stay below it.
//! - `read_sctp` does not await after dequeuing, so a dropped `next()` cannot lose a fragment.
//! - `detach()` is an idempotent Arc clone that does not close on drop.
//! - `on_*` handlers live in the pc: one holding a strong `Arc<RTCPeerConnection>` leaks it.
//! - `close()` latches `is_closed` before its first await, so a cancelled close is unretryable.
//! - `Disconnected` is transient; only `Failed`/`Closed` are terminal.
//! - `RTCIceCandidatePair`'s `Display` layout — `is_relayed()` parses it because 0.13 keeps the
//!   candidates private. 0.17+ exposes them; switch and delete the parse.

use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::data::data_channel::DataChannel as DetachedDataChannel;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice::mdns::MulticastDnsMode;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_candidate_type::RTCIceCandidateType;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::policy::ice_transport_policy::RTCIceTransportPolicy;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use percent_encoding::percent_decode_str;
use tokio::sync::{mpsc, watch, Mutex, Semaphore};
use tokio::time::{timeout, timeout_at, Instant};
use url::Url;

use crate::config;
use crate::protobuf::Message;
use crate::rx_probe::RxProbe;
use crate::sodiumoxide::crypto::secretbox::Key;
use crate::ResultType;

#[derive(Clone, Debug, PartialEq, Eq)]
enum WebRTCConnectionState {
    Pending,
    Open,
    Closed(String),
}

/// A shared handle, not an owner: every clone points at the same `pc`, and `SESSIONS` holds one
/// of those clones. Hence no `Drop` — closing per clone would kill a live session as soon as a
/// losing race future is dropped, and closing on the last clone would wait on the very close
/// that evicts the cache entry. Ownership is `OffererGuard` until a connection attempt adopts
/// the stream and `Stream::WebRTC` after; holding one outside those two means closing it by hand.
pub struct WebRTCStream {
    pc: Arc<RTCPeerConnection>,
    stream: Arc<Mutex<Arc<RTCDataChannel>>>,
    state_notify: watch::Receiver<WebRTCConnectionState>,
    local_ice_rx: Arc<StdMutex<Option<mpsc::UnboundedReceiver<String>>>>,
    session_key: String,
    send_timeout: u64,
    // Built with Relay-only ICE policy (force_relay): every selected pair goes through TURN.
    relay_only: bool,
    // Detached data channel, cached after the first `detach()` so send/recv do not re-lock and
    // re-fetch it per message. Shared across clones; `detach()` is idempotent.
    detached: Arc<Mutex<Option<Arc<DetachedDataChannel>>>>,
    // Serialize a complete logical message across clones. Each fragment is a separate SCTP
    // message, so serializing only individual writes would allow two large messages to interleave.
    send_gate: Arc<Semaphore>,
    // Behind the Arc, not in the future, so a cancelled `next()` keeps the partial message.
    // SINGLE READER ONLY: reassembly spans calls and the fragment header carries no length or
    // sequence number, so two readers splice unrelated fragments into one wrong-but-valid message.
    recv_state: Arc<Mutex<RecvState>>,
    // True once the controller has completed the RustDesk identity binding (DTLS fingerprint
    // matched to the signed peer id, via `set_key`). DTLS always encrypts; this flag mirrors TCP's
    // "secured after key exchange" so key-less / unbound WebRTC is not shown as peer-authenticated.
    peer_verified: Arc<AtomicBool>,
    // Whether anyone still wants this pc — see `HandoffState`. Shared by every clone, the
    // `SESSIONS` entry included, because that question is about the pc, not about one handle.
    handoff: Arc<HandoffState>,
    // Taken once at construction: `pc.local_description()` grows with every gathered candidate
    // and this must not (why: `UDP_ENDPOINT_BUDGET`). Shared so cloning the struct stays cheap.
    local_endpoint: Arc<String>,
    // Counts arriving fragments, which `recv_state` cannot answer for: that lock is deliberately
    // held across the whole reassembly (see the field), so a sampler would block on it.
    rx_probe: RxProbe,
    // ICE has stopped hearing from the peer but the connection is not terminal yet. Kept out of
    // `state_notify` on purpose - see where it is set.
    disconnected: Arc<AtomicBool>,
    // The largest message this stream will reassemble, lowered while the peer is unauthenticated.
    // Shared by every clone, the `SESSIONS` entry included, because the receive path runs off a
    // clone and the bound is about the pc, not about one handle.
    max_recv_message: Arc<AtomicUsize>,
}

/// Whether a pc `new_inner` built is still wanted, shared by every `NewStreamHandoff` handed out
/// for it. `new_inner` caches the pc before its `new()` returns, so the builder's handoff and any
/// number of cache hits are in flight at once; a pc is abandoned only when the last of them is
/// dropped with none adopted, and closing on any single Drop would kill a live caller's session.
#[derive(Default)]
struct HandoffState {
    // Handoffs handed out and neither adopted nor dropped yet. Incremented under the `SESSIONS`
    // lock so a hit that is still arriving cannot be overtaken by the abandon-close below.
    pending: AtomicUsize,
    // Latched by the first `into_inner`: some caller owns the stream, and since `WebRTCStream`
    // has no `Drop` there is nothing that could un-own it. Abandoning any later hit is then a
    // no-op, which is what keeps a cached pc alive across a cancelled caller.
    adopted: AtomicBool,
    // Set under the `SESSIONS` lock once the abandon-close commits, so a hit that locks the map
    // afterwards (eviction only happens later, from the state handler) rejects the entry instead
    // of adopting a pc that is already going away.
    closing: AtomicBool,
}

#[derive(Default)]
struct RecvState {
    // Accumulated payload of the logical message currently being reassembled. Only multi-fragment
    // messages reach it; a message that arrives whole is split straight out of `scratch`.
    acc: BytesMut,
    // Read buffer, refilled in `SCRATCH_REFILL` chunks rather than per read. A whole-message
    // fragment is handed to the caller with `split_to`, which shares this allocation instead of
    // copying, so the buffer is consumed from the front and re-initialized only when what is left
    // can no longer hold one fragment — amortizing that cost over many small messages.
    scratch: BytesMut,
}

// The SCTP data channel's 65536-byte max message size is handled by
// splitting a logical message into fragments carrying a 1-byte header. Fragment payload is kept
// well under the limit so header+payload never reaches the exact-65536 boundary that the
// receiver's reassembly would truncate with data loss.
const MAX_FRAGMENT_PAYLOAD: usize = 60000;
/// Receive scratch size: must be >= 1 (fragment header) + `MAX_FRAGMENT_PAYLOAD` and fit the
/// negotiated SCTP max message size.
const RECV_BUF_SIZE: usize = 64 * 1024;
/// How much read buffer to initialize at a time. Whole messages are split out of it without
/// copying, so it is consumed rather than reused; refilling in chunks spreads the one cost that
/// remains — zeroing — across every small message that fits. Slices handed to the caller keep the
/// whole chunk alive, so this trades a bounded amount of retention for the copy.
const SCRATCH_REFILL: usize = 4 * RECV_BUF_SIZE;
/// Fragment header byte: more fragments follow for this logical message.
const FRAG_MORE: u8 = 1;
/// Fragment header byte: final (or only) fragment of a logical message.
const FRAG_END: u8 = 0;
/// Largest logical message this transport will send or reassemble, and the memory an
/// unauthenticated peer can make a receiver hold. Kept at parity with TCP on purpose: one session
/// moves between both paths, so a transport-specific ceiling would kill it on the other one.
const MAX_RECV_MESSAGE: usize = crate::bytes_codec::MAX_FRAME_LENGTH;
// Four networks, not four names: webrtc-ice queries each URL from its own socket, so entries
// sharing a host buy no redundancy - the two this list used to carry failed together, in the same
// millisecond, on a peer whose route to that one host was down. Two anycast, two unicast; the 443
// entry is for networks that pass no other UDP port.
static DEFAULT_ICE_SERVERS: [&str; 4] = [
    "stun:stun.cloudflare.com:3478",
    "stun:stun.l.google.com:19302",
    "stun:stun.antisip.com:3478",
    "stun:stun.nextcloud.com:443",
];

lazy_static::lazy_static! {
    static ref SESSIONS: Arc::<Mutex<HashMap<String, WebRTCStream>>> = Default::default();

    // The process-lifetime runtime that owns ALL WebRTC I/O. A pc's UDP sockets register with
    // the reactor — and its ICE/DTLS/SCTP pump tasks spawn on the runtime — that is current
    // while `new_inner` and the handlers it installs execute; if that were a session's
    // `#[tokio::main]` runtime (dropped the moment io_loop returns), the sockets and pumps
    // would die with it and even a close driven elsewhere would "succeed" without ever putting
    // close_notify on the wire — the peer then waits out ICE decay. So `new()` runs its body
    // here, and detached closes run here too: as independent tasks (a deadline-free `close()`
    // must not head-of-line block other sessions' teardown) that are never cancelled (a close
    // cancelled after `RTCPeerConnection::close` latches `is_closed` is unretryable, stranding
    // the pc in SESSIONS). Data-plane futures (send/recv/close) may be polled from any caller
    // runtime — cross-runtime polling is fine; only the OWNING driver must stay alive. Two
    // workers so one busy session cannot starve the rest. This is the one deliberate exception
    // to AGENTS.md's no-runtime-in-libraries rule: a single lazily-built runtime for I/O whose
    // lifetime exceeds any caller's.
    static ref WEBRTC_RT: Option<tokio::runtime::Runtime> = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("webrtc-io")
        .enable_all()
        .build()
        .map_err(|err| log::error!("failed to build the WebRTC I/O runtime: {}", err))
        .ok();
}

/// Hands a stream from `new_inner` (running detached on `WEBRTC_RT`) back to its `new()`
/// caller. If that caller was cancelled while awaiting, the runtime drops the task's output —
/// this guard — and an unanswered offerer never reaches a terminal ICE state on its own, so
/// without the Drop-close its pc, sockets and pump tasks would sit in SESSIONS forever. Claims
/// are counted in the shared `HandoffState`, so only the last one dropped with the stream still
/// unadopted closes it; a hit another caller is already using is left alone.
struct NewStreamHandoff {
    stream: Option<WebRTCStream>,
}

impl NewStreamHandoff {
    /// Take a claim on `stream`. Call under the `SESSIONS` lock: a claim registered after the map
    /// is unlocked can land behind the abandon-close of the claim that was last outstanding.
    fn claim(stream: WebRTCStream) -> Self {
        stream.handoff.pending.fetch_add(1, Ordering::AcqRel);
        Self {
            stream: Some(stream),
        }
    }

    fn into_inner(mut self) -> Option<WebRTCStream> {
        let stream = self.stream.take()?;
        // Before releasing the claim, or a concurrent Drop can see the count reach zero without
        // seeing that this caller took ownership.
        stream.handoff.adopted.store(true, Ordering::Release);
        stream.handoff.pending.fetch_sub(1, Ordering::AcqRel);
        Some(stream)
    }
}

impl Drop for NewStreamHandoff {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            let last = stream.handoff.pending.fetch_sub(1, Ordering::AcqRel) == 1;
            if last && !stream.handoff.adopted.load(Ordering::Acquire) {
                stream.close_if_abandoned();
            }
        }
    }
}

impl Clone for WebRTCStream {
    fn clone(&self) -> Self {
        WebRTCStream {
            pc: self.pc.clone(),
            stream: self.stream.clone(),
            state_notify: self.state_notify.clone(),
            local_ice_rx: self.local_ice_rx.clone(),
            session_key: self.session_key.clone(),
            send_timeout: self.send_timeout,
            relay_only: self.relay_only,
            detached: self.detached.clone(),
            send_gate: self.send_gate.clone(),
            recv_state: self.recv_state.clone(),
            peer_verified: self.peer_verified.clone(),
            handoff: self.handoff.clone(),
            local_endpoint: self.local_endpoint.clone(),
            rx_probe: self.rx_probe.clone(),
            disconnected: self.disconnected.clone(),
            max_recv_message: self.max_recv_message.clone(),
        }
    }
}

impl WebRTCStream {
    #[inline]
    fn get_remote_offer(endpoint: &str) -> ResultType<String> {
        // Ensure the endpoint starts with the "webrtc://" prefix
        if !endpoint.starts_with("webrtc://") {
            return Err(
                Error::new(ErrorKind::InvalidInput, "Invalid WebRTC endpoint format").into(),
            );
        }

        // Extract the Base64-encoded SDP part
        let encoded_sdp = &endpoint["webrtc://".len()..];
        // Decode the Base64 string
        let decoded_bytes = BASE64_STANDARD
            .decode(encoded_sdp)
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "Failed to decode Base64 SDP"))?;
        Ok(String::from_utf8(decoded_bytes).map_err(|_| {
            Error::new(
                ErrorKind::InvalidInput,
                "Failed to convert decoded bytes to UTF-8",
            )
        })?)
    }

    #[inline]
    fn sdp_to_endpoint(sdp: &str) -> String {
        let encoded_sdp = BASE64_STANDARD.encode(sdp);
        format!("webrtc://{}", encoded_sdp)
    }

    // Envelope JSON key carrying the local description's ICE transport policy, alongside the
    // RTCSessionDescription fields (see `local_endpoint`).
    // `'static` spelled out: an elided lifetime here is a warn-by-default future hard error on
    // the 1.75 toolchain CI pins (elided_lifetimes_in_associated_constant).
    const ICE_POLICY_KEY: &'static str = "ice_policy";
    const ICE_POLICY_ALL: &'static str = "all";

    #[inline]
    fn get_key_for_sdp(sdp: &RTCSessionDescription) -> ResultType<String> {
        let binding = sdp.unmarshal()?;
        let Some(fingerprint) = binding.attribute("fingerprint") else {
            // find fingerprint attribute in media descriptions
            for media in &binding.media_descriptions {
                if media.media_name.media != "application" {
                    continue;
                }
                if let Some(fp) = media
                    .attributes
                    .iter()
                    .find(|x| x.key == "fingerprint")
                    .and_then(|x| x.value.clone())
                {
                    return Ok(fp);
                }
            }
            return Err(anyhow::anyhow!("SDP fingerprint attribute not found"));
        };
        Ok(fingerprint.to_string())
    }

    /// Process-local SESSIONS-map key: the DTLS fingerprint prefixed by role. An offerer and an
    /// answerer that share a fingerprint (a single process connecting to its own id) would
    /// otherwise collide, handing the offerer back as the answerer. The wire-level `session_key`
    /// used for ICE-candidate routing stays the bare fingerprint so both peers still match.
    #[inline]
    fn cache_key(fingerprint: &str, is_offerer: bool) -> String {
        format!(
            "{}:{}",
            if is_offerer { "offer" } else { "answer" },
            fingerprint
        )
    }

    /// Reject a data channel from inside `on_data_channel`, where closing it directly does not
    /// work: webrtc-rs runs that handler to completion BEFORE `handle_open` binds the SCTP stream,
    /// so `close()` only flips the ready state and `handle_open` then sets it back to Open.
    fn close_unbound_channel(dc: Arc<RTCDataChannel>) {
        let dc_for_close = dc.clone();
        dc.on_open(Box::new(move || {
            let dc = dc_for_close.clone();
            Box::pin(async move {
                if let Err(err) = dc.close().await {
                    log::debug!("failed to close rejected data channel: {}", err);
                }
            })
        }));
    }

    /// Whether a cached `SESSIONS` entry may be handed to a caller asking for `force_relay`: it
    /// was built for the first caller, so its ICE policy may not match, and the state handler
    /// evicts late enough that dead entries linger. Both the lookup and the insert must use it.
    fn is_reusable_for(&self, force_relay: bool) -> bool {
        self.relay_only == force_relay
            && !self.handoff.closing.load(Ordering::Acquire)
            // ICE has stopped hearing from the peer on this one. Not terminal, so nothing else
            // rejects it, but handing it to a caller that is about to judge liveness starts them
            // off already suspecting a pc they never used.
            && !self.is_disconnected()
            && !matches!(
                *self.state_notify.borrow(),
                WebRTCConnectionState::Closed(_)
            )
    }

    #[inline]
    fn get_key_for_sdp_json(sdp_json: &str) -> ResultType<String> {
        if sdp_json.is_empty() {
            return Ok("".to_string());
        }
        let sdp = serde_json::from_str::<RTCSessionDescription>(sdp_json)?;
        Self::get_key_for_sdp(&sdp)
    }

    fn remove_sessions_for_peer(
        sessions: &mut HashMap<String, WebRTCStream>,
        pc: &Arc<RTCPeerConnection>,
    ) -> Vec<String> {
        let keys: Vec<String> = sessions
            .iter()
            .filter_map(|(key, session)| {
                Arc::ptr_eq(&session.pc, pc).then(|| key.clone())
            })
            .collect();
        for key in &keys {
            sessions.remove(key);
        }
        keys
    }

    #[inline]
    async fn get_key_for_peer(pc: &Arc<RTCPeerConnection>, is_local: bool) -> ResultType<String> {
        let Some(desc) = (match is_local {
            true => pc.local_description().await,
            false => pc.remote_description().await,
        }) else {
            return Err(anyhow::anyhow!("PeerConnection description is not set"));
        };
        Self::get_key_for_sdp(&desc)
    }

    #[inline]
    fn get_ice_server_from_url(url: &str) -> Option<RTCIceServer> {
        let u = Url::parse(url).ok()?;
        if !matches!(u.scheme(), "turn" | "turns" | "stun" | "stuns") {
            return None;
        }
        // Two accepted spellings parse differently: `turn://user:pass@host:port` has an authority
        // and is the only form that can carry credentials, while the RFC 7065 `turn:host:port` is
        // cannot-be-a-base, so `host_str()` is None and the whole `host:port` lands in the path.
        let (host, port) = match u.host_str() {
            Some(host) => (host.to_owned(), u.port().unwrap_or(3478)),
            None => Self::split_host_port(u.path())?,
        };
        if host.is_empty() {
            return None;
        }
        // `Url` hands userinfo back percent-encoded; the TURN server expects the value,
        // so `p%40ss` must reach it as `p@ss` for the long-term credential to verify.
        let username = percent_decode_str(u.username())
            .decode_utf8_lossy()
            .into_owned();
        let credential = percent_decode_str(u.password().unwrap_or_default())
            .decode_utf8_lossy()
            .into_owned();
        // A TURN server without credentials is not merely useless: webrtc-rs validates every
        // configured server in `new_peer_connection`, so one credential-less entry makes EVERY
        // peer connection fail — including plain non-relay ones that never wanted TURN. The
        // RFC 7065 spelling has nowhere to put credentials, so drop such entries here rather
        // than let them poison the whole configuration; `turn://user:pass@host:port` carries them.
        if matches!(u.scheme(), "turn" | "turns") && (username.is_empty() || credential.is_empty())
        {
            log::warn!(
                "Ignoring TURN server without credentials: {}:{}:{} (use {}://user:pass@host:port)",
                u.scheme(),
                host,
                port,
                u.scheme()
            );
            return None;
        }
        Some(RTCIceServer {
            urls: vec![format!("{}:{}:{}", u.scheme(), host, port)],
            username,
            credential,
            ..Default::default()
        })
    }

    /// Split `host[:port]` from an RFC 7065 URL path, defaulting the port. Bracketed IPv6
    /// literals keep their brackets, which is the form webrtc-rs re-parses.
    fn split_host_port(path: &str) -> Option<(String, u16)> {
        let rest = path.split(['?', '#']).next().unwrap_or_default();
        if rest.is_empty() {
            return None;
        }
        if let Some(after_open) = rest.strip_prefix('[') {
            let (host, tail) = after_open.split_once(']')?;
            let port = tail
                .strip_prefix(':')
                .and_then(|p| p.parse().ok())
                .unwrap_or(3478);
            return Some((format!("[{host}]"), port));
        }
        // An unbracketed IPv6 literal has no unambiguous split point — `2001:db8::1` would be cut
        // at its last colon into host `2001:db8:` port `1` — so require the bracketed form for
        // those rather than emit a host webrtc-ice cannot resolve.
        if rest.matches(':').count() > 1 {
            log::warn!("Ignoring ICE server {rest}: bracket IPv6 literals as [addr]:port");
            return None;
        }
        match rest.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() => match port.parse() {
                Ok(port) => Some((host.to_owned(), port)),
                // A port that is present but unusable is a typo, not a host: folding it back in
                // would produce `host:99999:3478`, which webrtc-ice rejects outright — taking
                // every peer connection down with it, not just this server.
                Err(_) => {
                    log::warn!("Ignoring ICE server {rest}: invalid port");
                    None
                }
            },
            _ => Some((rest.to_owned(), 3478)),
        }
    }

    /// Whether the ICE configuration contains a usable TURN server. A Relay-policy peer
    /// connection (force_relay) can only gather relay candidates, so without a TURN server it can
    /// never connect — callers use this to skip building a guaranteed-dead pc.
    pub fn has_turn_server() -> bool {
        // `get_ice_server_from_url` is what makes a bare scheme test sufficient: it drops entries
        // with no host and TURN entries with no credentials, i.e. exactly the ones that would
        // answer `true` here while being unusable — which is worse than answering `false`, since
        // the caller skips its "don't build a guaranteed-dead Relay-only pc" guard on our word.
        Self::get_ice_servers().iter().any(|s| {
            s.urls
                .iter()
                .any(|u| u.starts_with("turn:") || u.starts_with("turns:"))
        })
    }

    #[inline]
    fn get_cc_enabled() -> bool {
        let k = config::keys::OPTION_ALLOW_WEBRTC_CC;
        config::option2bool(k, &config::Config::get_option(k))
    }

    #[inline]
    fn get_ice_servers() -> Vec<RTCIceServer> {
        Self::parse_ice_servers(&config::Config::get_option(
            config::keys::OPTION_ICE_SERVERS,
        ))
    }

    /// The default UDP STUN servers as bare `host:port`, for application-level address probes.
    pub fn default_stun_servers() -> Vec<String> {
        DEFAULT_ICE_SERVERS
            .iter()
            .filter_map(|url| url.strip_prefix("stun:").map(str::to_owned))
            .collect()
    }

    /// Split out from `get_ice_servers` so parsing can be exercised without touching the
    /// process-global, on-disk-persisted option — the tests run in parallel threads of one
    /// process, so a test that rewrote it raced every peer connection another test was building.
    fn parse_ice_servers(cfg: &str) -> Vec<RTCIceServer> {
        let mut ice_servers = Vec::new();
        let mut has_stun = false;

        for url in cfg.split(',').map(str::trim) {
            if let Some(ice_server) = Self::get_ice_server_from_url(url) {
                // Detect STUN in user config
                if ice_server
                    .urls
                    .iter()
                    .any(|u| u.starts_with("stun:") || u.starts_with("stuns:"))
                {
                    has_stun = true;
                }

                ice_servers.push(ice_server);
            }
        }

        // If there is no STUN (either TURN-only or empty config) → prepend defaults
        if !has_stun {
            ice_servers.insert(
                0,
                RTCIceServer {
                    urls: DEFAULT_ICE_SERVERS.iter().map(|s| s.to_string()).collect(),
                    ..Default::default()
                },
            );
        }
        ice_servers
    }

    /// How many binding requests the answerer's ICE agent sends on a pair without a response
    /// before giving up on it: 74 at the 200ms check interval is ~14.8s, just inside the 15s
    /// persistence libwebrtc gives a pair (`CONNECTION_WRITE_TIMEOUT`), so both ends of a
    /// browser session keep checking for about the same time. The ICE crate's default of 7
    /// (~1.4s) is a server-side choice that assumes the other side keeps knocking; behind an
    /// address-restricted NAT those requests are also what keep this side's mapping open to
    /// the remote's checks, so stopping early closes the door on a pair that would have
    /// connected once the path cleared, however long the remote keeps trying. Bounded by
    /// `CONNECT_TIMEOUT` (18s).
    const ICE_MAX_BINDING_REQUESTS: u16 = 74;

    /// Built and driven on `WEBRTC_RT`: every socket and background task the pc creates must
    /// belong to a runtime that outlives the session (see `WEBRTC_RT`). A caller that abandons
    /// this future mid-await leaves the setup task to finish there; the abandoned result then
    /// closes a freshly built pc (and leaves a shared cache hit alone) — see `NewStreamHandoff`.
    pub async fn new(
        remote_endpoint: &str,
        force_relay: bool,
        ms_timeout: u64,
    ) -> ResultType<Self> {
        let Some(rt) = WEBRTC_RT.as_ref() else {
            return Err(anyhow::anyhow!("WebRTC I/O runtime unavailable"));
        };
        let remote_endpoint = remote_endpoint.to_owned();
        rt.spawn(Self::new_inner(remote_endpoint, force_relay, ms_timeout))
            .await
            .map_err(|err| anyhow::anyhow!("WebRTC setup task failed: {}", err))??
            .into_inner()
            .ok_or_else(|| anyhow::anyhow!("WebRTC setup handoff was empty"))
    }

    async fn new_inner(
        remote_endpoint: String,
        force_relay: bool,
        ms_timeout: u64,
    ) -> ResultType<NewStreamHandoff> {
        // The endpoint contains a Base64-encoded SDP with host addresses and live ICE
        // credentials. Log only its size so debug logs cannot disclose that information.
        log::debug!(
            "New webrtc stream (remote endpoint: {} bytes)",
            remote_endpoint.len()
        );
        let remote_offer = if remote_endpoint.is_empty() {
            "".into()
        } else {
            Self::get_remote_offer(&remote_endpoint)?
        };

        let mut key = Self::get_key_for_sdp_json(&remote_offer)?;
        let start_local_offer = remote_offer.is_empty();
        if !key.is_empty() {
            // Claim inside the lock: `close_if_abandoned` commits under it too, so this hit
            // either registers in time to call that close off, or finds the entry already rejected.
            let cached = {
                let sessions_lock = SESSIONS.lock().await;
                match sessions_lock.get(&Self::cache_key(&key, start_local_offer)) {
                    Some(session) if session.is_reusable_for(force_relay) => {
                        Some(NewStreamHandoff::claim(session.clone()))
                    }
                    Some(session) => {
                        log::debug!(
                            "Ignoring cached webrtc peer (relay_only {}, wanted {})",
                            session.relay_only,
                            force_relay
                        );
                        None
                    }
                    None => None,
                }
            };
            if let Some(handoff) = cached {
                log::debug!("Start webrtc with cached peer");
                return Ok(handoff);
            }
        }
        // Create a SettingEngine and enable Detach
        let mut s = SettingEngine::default();
        s.detach_data_channels();
        s.set_ice_multicast_dns_mode(MulticastDnsMode::Disabled);
        // Answerer only: `remote_endpoint` is empty when this side is the offerer. The
        // desktop controller keeps the crate default so it stays an unchanged comparison
        // point while the persistence experiment runs on the controlled side.
        if !remote_endpoint.is_empty() {
            s.set_ice_max_binding_requests(Some(Self::ICE_MAX_BINDING_REQUESTS));
        }
        // fe80::/10 can only be bound together with a scope id, which `IpAddr` cannot carry, so
        // gathering one never yields a candidate - only a failed bind and a warning per address.
        // Spelled out because `is_unicast_link_local` is not stable on our MSRV.
        s.set_ip_filter(Box::new(|ip: IpAddr| match ip {
            IpAddr::V6(v6) => v6.segments()[0] & 0xffc0 != 0xfe80,
            IpAddr::V4(_) => true,
        }));

        // KCP's nc=1 profile by default, with the same opt-in to a congestion window as
        // `allow-kcp-congestion-control`, for the reason given at `get_kcp_cc_enabled`. Set per
        // connection so the option takes effect on the next session, not the next start.
        webrtc::sctp::association::set_no_congestion_control(!Self::get_cc_enabled());

        // Create the API object
        let api = APIBuilder::new().with_setting_engine(s).build();

        // Prepare the configuration, get ICE servers from config
        let config = RTCConfiguration {
            ice_servers: Self::get_ice_servers(),
            ice_transport_policy: if force_relay {
                RTCIceTransportPolicy::Relay
            } else {
                RTCIceTransportPolicy::All
            },
            ..Default::default()
        };

        let (notify_tx, notify_rx) = watch::channel(WebRTCConnectionState::Pending);
        let (ice_tx, ice_rx) = mpsc::unbounded_channel::<String>();
        // Create a new RTCPeerConnection
        let pc = Arc::new(api.new_peer_connection(config).await?);
        // `on_*` handlers are never cleared — not by `close()`, not by `SESSIONS` eviction — so
        // this sender outlives the pc and would park a caller's ICE forwarder on `recv()` forever,
        // holding the pc with it. Gathering complete (`None`) is the agent's last word short of an
        // ICE restart, which nothing here does, so the sender goes with it and the forwarder ends
        // there rather than with the session; the terminal-state handler below drops this closure
        // for a pc that dies before it gets that far.
        let mut local_ice_tx = Some(ice_tx);
        pc.on_ice_candidate(Box::new(move |candidate| {
            let Some(candidate) = candidate else {
                local_ice_tx = None;
                return Box::pin(async {});
            };
            let Some(local_ice_tx) = local_ice_tx.clone() else {
                return Box::pin(async {});
            };
            Box::pin(async move {
                match candidate.to_json() {
                    Ok(candidate) => match serde_json::to_string(&candidate) {
                        Ok(candidate_json) => {
                            let _ = local_ice_tx.send(candidate_json);
                        }
                        Err(err) => {
                            log::warn!("failed to serialize local ICE candidate: {}", err);
                        }
                    },
                    Err(err) => {
                        log::warn!("failed to convert local ICE candidate to JSON: {}", err);
                    }
                }
            })
        }));

        let bootstrap_dc = if start_local_offer {
            let dc_open_notify = notify_tx.clone();
            // Create a data channel with label "bootstrap"
            let dc = match pc.create_data_channel("bootstrap", None).await {
                Ok(dc) => dc,
                Err(e) => {
                    // Close before propagating: the pc is live and would otherwise leak.
                    pc.close().await.ok();
                    return Err(e.into());
                }
            };
            dc.on_open(Box::new(move || {
                log::debug!("Local data channel bootstrap open.");
                let _ = dc_open_notify.send(WebRTCConnectionState::Open);
                Box::pin(async {})
            }));
            dc
        } else {
            // Wait for the data channel to be created by the remote peer
            // Here we create a dummy data channel to satisfy the type system
            Arc::new(RTCDataChannel::default())
        };

        let stream = Arc::new(Mutex::new(bootstrap_dc));
        if !start_local_offer {
            // Register data channel creation handling
            let dc_open_notify = notify_tx.clone();
            let stream_for_dc = stream.clone();
            // The remote may open any number of channels; only the first is ever bound.
            let dc_bound = Arc::new(AtomicBool::new(false));
            pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
                let d_label = dc.label().to_owned();
                let dc_open_notify2 = dc_open_notify.clone();
                let stream_for_dc_clone = stream_for_dc.clone();
                let dc_bound = dc_bound.clone();
                Box::pin(async move {
                    // Reassembly is sound only on an ordered, fully-reliable channel: the fragment
                    // header has no sequence number, so a reorder or a loss silently merges
                    // messages. webrtc-rs takes these from the unauthenticated REMOTE's DCEP OPEN.
                    if !dc.ordered()
                        || dc.max_retransmits().is_some()
                        || dc.max_packet_lifetime().is_some()
                    {
                        log::warn!(
                            "Rejecting WebRTC data channel {}: not ordered and fully reliable",
                            d_label
                        );
                        Self::close_unbound_channel(dc);
                        return;
                    }
                    // Bind the first channel only. `detached` caches the first detached handle
                    // for the life of the stream, so rebinding would leave teardown closing a
                    // channel that send/recv no longer use; worse, a second channel's `on_open`
                    // would push Open onto the same watch and re-arm a session already latched
                    // Closed — and that watch gates both `send_bytes_inner` and `next()`.
                    if dc_bound.swap(true, Ordering::SeqCst) {
                        log::warn!("Ignoring extra WebRTC data channel {}", d_label);
                        Self::close_unbound_channel(dc);
                        return;
                    }
                    log::debug!("Remote data channel {} ready", d_label);
                    let mut stream_lock = stream_for_dc_clone.lock().await;
                    *stream_lock = dc.clone();
                    drop(stream_lock);
                    dc.on_open(Box::new(move || {
                        let _ = dc_open_notify2.send(WebRTCConnectionState::Open);
                        Box::pin(async {})
                    }));
                })
            }));
        }

        // This will notify you when the peer has connected/disconnected
        let stream_for_close = stream.clone();
        // Weak, not strong: a handler stored inside the pc that captured a strong
        // `Arc<RTCPeerConnection>` forms a pc -> internal -> handler -> pc cycle that `close()`
        // never breaks (it only fires the handler) and no `Drop` clears, permanently leaking every
        // pc and the ICE-candidate sender's forwarding task. Upgrade inside the handler; if the pc
        // is already gone there is nothing left in SESSIONS to evict.
        let pc_for_close = Arc::downgrade(&pc);
        // Deliberately NOT a `WebRTCConnectionState` variant: that enum gates send and receive
        // readiness (`wait_for_connect_result`) and session-cache admissibility, so a fourth state
        // would have to be given a meaning at each of those. `Disconnected` is transient and must
        // change neither, so it is reported beside them instead of through them.
        let disconnected = Arc::new(AtomicBool::new(false));
        let disconnected_for_state = disconnected.clone();
        pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            let stream_for_close2 = stream_for_close.clone();
            let on_connection_notify = notify_tx.clone();
            let pc_for_close2 = pc_for_close.clone();
            let disconnected_for_state = disconnected_for_state.clone();
            Box::pin(async move {
                log::debug!("WebRTC session peer connection state: {}", s);
                Self::note_health(&disconnected_for_state, s);
                match s {
                    // `Disconnected` is a transient, recoverable ICE state (webrtc-ice fires it
                    // after ~5s without consent and returns to `Connected` when traffic resumes).
                    // Only tear down on the terminal states so a short network blip (Wi-Fi roam,
                    // sleep/wake, cell handover) does not permanently kill an established session.
                    RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed => {
                        let _ =
                            on_connection_notify.send(WebRTCConnectionState::Closed(s.to_string()));
                        log::debug!("WebRTC session closing due to {}", s);
                        let stream = stream_for_close2.lock().await.clone();
                        let _ = stream.close().await;
                        log::debug!("WebRTC session stream closed");

                        let Some(pc_for_close2) = pc_for_close2.upgrade() else {
                            return;
                        };

                        // Nothing else ever drops this sender (`close()` clears no handler), so
                        // replace the handler to close the channel: a caller's forwarder loop then
                        // ends instead of parking on `recv()` and holding this pc alive.
                        pc_for_close2.on_ice_candidate(Box::new(|_| Box::pin(async {})));

                        // By pc identity, not by re-deriving the key: a duplicate offer shares
                        // the key with the live winner, and this must not evict that one.
                        let mut sessions_lock = SESSIONS.lock().await;
                        for key in Self::remove_sessions_for_peer(
                            &mut sessions_lock,
                            &pc_for_close2,
                        ) {
                            log::debug!("WebRTC session removed key: {}", key);
                        }
                    }
                    _ => {}
                }
            })
        }));

        // Trickle ICE: local-only work, no gathering wait — the controlled side awaits answer
        // creation inline on its punch-reply critical path. A failure below leaves a live pc whose
        // state handler only fires on a terminal ICE state, so close before propagating.
        // Encode each endpoint before `set_local_description`, which is what starts gathering, so
        // there is nothing for `trickle_endpoint` to strip in the first place.
        let offer_answer: ResultType<(String, String)> = async {
            if start_local_offer {
                let sdp = pc.create_offer(None).await?;
                let endpoint = Self::trickle_endpoint(&sdp, force_relay)?;
                pc.set_local_description(sdp.clone()).await?;
                // SDP carries host/srflx IPs and ICE ufrag/pwd; log only its size, not the body.
                log::debug!(
                    "local offer SDP built ({} bytes, endpoint {} bytes)",
                    sdp.sdp.len(),
                    endpoint.len()
                );
                let k = Self::get_key_for_sdp(&sdp)?;
                log::debug!("Start webrtc with local key: {}", k);
                Ok((k, endpoint))
            } else {
                let sdp = serde_json::from_str::<RTCSessionDescription>(&remote_offer)?;
                pc.set_remote_description(sdp.clone()).await?;
                let answer = pc.create_answer(None).await?;
                let endpoint = Self::trickle_endpoint(&answer, force_relay)?;
                pc.set_local_description(answer).await?;
                log::debug!(
                    "remote offer SDP received ({} bytes), local answer endpoint {} bytes",
                    sdp.sdp.len(),
                    endpoint.len()
                );
                let k = Self::get_key_for_sdp(&sdp)?;
                log::debug!("Start webrtc with remote key: {}", k);
                Ok((k, endpoint))
            }
        }
        .await;
        let (new_key, local_endpoint) = match offer_answer {
            Ok(x) => x,
            Err(e) => {
                pc.close().await.ok();
                return Err(e);
            }
        };
        // Only the UDP leg has a ceiling, and which leg carries this is the rendezvous server's
        // to decide, so this reports rather than refuses: over a TCP/WS route a larger endpoint is
        // delivered fine, and failing here would cost WebRTC for no reason.
        if local_endpoint.len() > Self::UDP_ENDPOINT_BUDGET {
            log::warn!(
                "WebRTC endpoint is {} bytes, over the {} byte UDP budget; a peer reached over UDP \
                 may never receive it",
                local_endpoint.len(),
                Self::UDP_ENDPOINT_BUDGET
            );
        }
        key = new_key;

        let webrtc_stream = Self {
            pc,
            stream,
            state_notify: notify_rx,
            local_ice_rx: Arc::new(StdMutex::new(Some(ice_rx))),
            session_key: key.clone(),
            send_timeout: ms_timeout,
            relay_only: force_relay,
            detached: Arc::new(Mutex::new(None)),
            send_gate: Arc::new(Semaphore::new(1)),
            recv_state: Arc::new(Mutex::new(RecvState::default())),
            peer_verified: Arc::new(AtomicBool::new(false)),
            handoff: Default::default(),
            local_endpoint: Arc::new(local_endpoint),
            rx_probe: RxProbe::new(),
            disconnected,
            max_recv_message: Arc::new(AtomicUsize::new(MAX_RECV_MESSAGE)),
        };
        // Insert into the session cache, but never `await pc.close()` while holding this lock:
        // `close()` fires the peer-connection-state handler inline, which itself locks SESSIONS,
        // self-deadlocking the whole process. Resolve any duplicate off-lock.
        let cache_key = Self::cache_key(&key, start_local_offer);
        // Claim inside the lock, the insert included: with the pc cached but unclaimed, a hit
        // could take the only claim on it and release it again, closing it under this caller.
        let mut duplicate = None;
        let handoff = {
            let mut final_lock = SESSIONS.lock().await;
            // Same admissibility test as the lookup above, or that lookup is dead code: an entry
            // rejected there is still in the map when we get here, so returning it unconditionally
            // would discard the pc we just built precisely because the cached one was unusable.
            match final_lock.get(&cache_key) {
                Some(session) if session.is_reusable_for(force_relay) => {
                    duplicate = Some(webrtc_stream.clone());
                    NewStreamHandoff::claim(session.clone())
                }
                _ => {
                    final_lock.insert(cache_key, webrtc_stream.clone());
                    NewStreamHandoff::claim(webrtc_stream)
                }
            }
        };
        if let Some(loser) = duplicate {
            // A concurrent `new()` already cached an equivalent stream; discard this pc's
            // resources (on the closer thread — awaiting here would strand the pc if this
            // `new()` were cancelled mid-close) and return the cached one.
            loser.close_detached();
        }
        Ok(handoff)
    }

    /// One-shot endpoint: waits for ICE gathering so the SDP already carries the candidates.
    /// The wait is deliberately unbounded — a deadline here would return a half-gathered SDP and
    /// defeat the contract; callers needing one should wrap the call or use the trickle variant,
    /// which both rustdesk signaling paths do. Remaining callers are the loopback tests and
    /// `examples/webrtc.rs`, neither of which bounds it.
    #[inline]
    pub async fn get_local_endpoint(&self) -> ResultType<String> {
        // Preserve the original one-shot endpoint contract: callers that only exchange this SDP
        // do not have a separate path for `take_local_ice_rx`, so their endpoint must contain the
        // gathered host/srflx/relay candidates.
        let mut gather_complete = self.pc.gathering_complete_promise().await;
        let _gathering_channel_closed = gather_complete.recv().await;
        let Some(local_desc) = self.pc.local_description().await else {
            return Err(anyhow::anyhow!("Local desc is not set"));
        };
        Self::encode_endpoint(local_desc.sdp_type, &local_desc.sdp, self.relay_only)
    }

    /// The offer/answer exactly as it was created, for callers that signal candidates via
    /// `take_local_ice_rx`. Taken at construction, so this cannot fail and cannot block — unlike
    /// `get_local_endpoint`, which reads the live description and waits for gathering.
    ///
    /// Deliberately not `pc.local_description()`, which runs `populate_local_candidates` and so
    /// returns the SDP plus every candidate gathered up to that moment — and callers read this a
    /// network round trip after `new`, by which time gathering has filled it in. Why the size
    /// matters: `UDP_ENDPOINT_BUDGET`.
    #[inline]
    pub fn local_endpoint(&self) -> &str {
        self.local_endpoint.as_str()
    }

    /// The size a trickle endpoint must stay under for the UDP leg of its route to be safe. It is
    /// the one signaling message that can grow: a peer registered over UDP receives it inside
    /// `PunchHole` — with the mangled addresses and the permission blobs — as a single datagram,
    /// and one that needs IP fragmentation is dropped without a trace on paths that discard
    /// fragments. The candidates that follow are one small message each and never approach it.
    ///
    /// Not an MTU calculation: `PunchHole`'s other fields are not bounded here, so a value near
    /// this number could still fragment. It is a tripwire on a quantity `trickle_endpoint` pins at
    /// ~700 bytes, sized to leave that headroom while still catching a regression that lets
    /// candidates back in — as a log line rather than another silent drop. Nothing enforces it: a
    /// TCP/WS leg has no packet ceiling.
    const UDP_ENDPOINT_BUDGET: usize = 1024;

    /// The endpoint a trickling peer signals: the session parameters needed to start ICE and DTLS
    /// (ice-ufrag, ice-pwd, fingerprint, setup, the sctp m-line), without the candidates — those
    /// ride `take_local_ice_rx` and arrive as individual `IceCandidate` messages.
    ///
    /// Candidates are the only part of a local description that grows. Stripping them makes the
    /// size a property of the value rather than of when it was taken, so it cannot drift with
    /// gathering however this is later refactored.
    fn trickle_endpoint(local_desc: &RTCSessionDescription, relay_only: bool) -> ResultType<String> {
        let mut sdp = String::with_capacity(local_desc.sdp.len());
        for line in local_desc.sdp.lines() {
            // `end-of-candidates` would tell the remote agent to stop waiting for the trickle.
            if line.starts_with("a=candidate:") || line.starts_with("a=end-of-candidates") {
                continue;
            }
            sdp.push_str(line);
            sdp.push_str("\r\n");
        }
        Self::encode_endpoint(local_desc.sdp_type, &sdp, relay_only)
    }

    /// Base64 envelope of a local description, carrying its ICE transport policy unless the pc is
    /// Relay-only (see `ICE_POLICY_KEY`).
    /// Built from the two fields a session description serializes rather than from the value, so
    /// a caller that rewrote the SDP cannot leave a stale `parsed` tree riding along with it.
    fn encode_endpoint(sdp_type: RTCSdpType, sdp: &str, relay_only: bool) -> ResultType<String> {
        let mut envelope = serde_json::Map::new();
        envelope.insert("type".to_owned(), serde_json::to_value(sdp_type)?);
        envelope.insert("sdp".to_owned(), serde_json::Value::from(sdp));
        if !relay_only {
            // Rides in the envelope, not a proto field the rendezvous server would forward,
            // because it is the offer's own property: the receiver must know whether
            // force_relay was policy (stay Relay-only) or transport. Older peers ignore it.
            envelope.insert(
                Self::ICE_POLICY_KEY.to_owned(),
                serde_json::Value::from(Self::ICE_POLICY_ALL),
            );
        }
        Ok(Self::sdp_to_endpoint(&serde_json::to_string(&envelope)?))
    }

    /// Whether the peer's endpoint declares it was built with ICE transport policy `all`
    /// (W3C RTCIceTransportPolicy), i.e. it gathers host/srflx/relay candidates and a direct
    /// pair may form even though the request carries force_relay. Absent key, foreign format
    /// or parse failure all mean "not declared" — the old Relay-only reading.
    pub fn endpoint_declares_all_ice(endpoint: &str) -> bool {
        let Ok(sdp_json) = Self::get_remote_offer(endpoint) else {
            return false;
        };
        serde_json::from_str::<serde_json::Value>(&sdp_json)
            .ok()
            .and_then(|v| {
                v.get(Self::ICE_POLICY_KEY)?
                    .as_str()
                    .map(|p| p == Self::ICE_POLICY_ALL)
            })
            .unwrap_or(false)
    }

    #[inline]
    pub async fn set_remote_endpoint(&self, endpoint: &str) -> ResultType<()> {
        let offer = Self::get_remote_offer(endpoint)?;
        log::debug!("WebRTC set remote sdp ({} bytes)", offer.len());
        let sdp = serde_json::from_str::<RTCSessionDescription>(&offer)?;
        self.pc.set_remote_description(sdp).await?;
        Ok(())
    }

    /// DTLS certificate fingerprint of the local description (this endpoint's own cert).
    #[inline]
    pub async fn local_dtls_fingerprint(&self) -> ResultType<String> {
        Self::get_key_for_peer(&self.pc, true).await
    }

    /// DTLS certificate fingerprint of the remote description (the peer's cert). webrtc-rs
    /// verifies the negotiated peer certificate against this fingerprint during the DTLS
    /// handshake, so once the channel is open a matching fingerprint identifies the peer's cert.
    #[inline]
    pub async fn remote_dtls_fingerprint(&self) -> ResultType<String> {
        Self::get_key_for_peer(&self.pc, false).await
    }

    /// Whether the connection runs through a TURN relay; feeds the UI's direct/relayed flag.
    /// Under Relay policy the answer is known by construction, so that arm returns `Some(true)`
    /// without consulting ICE — including before a pair is selected, unlike the `None` the other
    /// arm returns then. Both callers ask post-connection, where the arms agree; making the relay
    /// arm await a pair would only add a stats round trip.
    pub async fn is_relayed(&self) -> Option<bool> {
        if self.relay_only {
            return Some(true);
        }
        let dtls = self.pc.sctp().transport();
        let pair = dtls.ice_transport().get_selected_candidate_pair().await?;

        // Not the stats report's `nominated` flag: webrtc-ice never clears it per checklist entry,
        // so after a pair switch several entries carry it and map order decides the answer.
        // 0.13 keeps the candidates private, so read the types out of `Display` by position.
        let relay = RTCIceCandidateType::Relay.to_string();
        Some(
            pair.to_string()
                .split(" <-> ")
                .filter_map(|side| side.split_whitespace().nth(2))
                .any(|typ| typ == relay),
        )
    }

    /// Whether the nominated pair reaches the peer over IPv6 — `None` before one is selected.
    /// The remote side on purpose: it is the address the peer is actually reached at, which the
    /// rendezvous-observed address the session is otherwise identified by cannot report.
    pub async fn is_remote_ipv6(&self) -> Option<bool> {
        let dtls = self.pc.sctp().transport();
        let pair = dtls.ice_transport().get_selected_candidate_pair().await?;
        // Same `Display` parse as `is_relayed`, one token over: a side renders as
        // `(remote) <protocol> <typ> <address>:<port>`, and `protocol` is only udp/tcp, so the
        // family has to come from the address — an IPv6 literal is the only one with two colons.
        let pair = pair.to_string();
        let remote = pair.split(" <-> ").nth(1)?;
        Some(remote.split_whitespace().nth(3)?.matches(':').count() > 1)
    }

    #[inline]
    pub fn take_local_ice_rx(&self) -> Option<mpsc::UnboundedReceiver<String>> {
        self.local_ice_rx.lock().ok().and_then(|mut rx| rx.take())
    }

    #[inline]
    pub async fn add_remote_ice_candidate(&self, candidate_json: &str) -> ResultType<()> {
        if candidate_json.is_empty() {
            return Ok(());
        }
        let candidate = serde_json::from_str::<RTCIceCandidateInit>(candidate_json)?;
        self.pc.add_ice_candidate(candidate).await?;
        Ok(())
    }

    #[inline]
    pub fn session_key(&self) -> &str {
        &self.session_key
    }

    pub async fn wait_connected(&mut self, ms: u64) -> ResultType<()> {
        if ms > 0 {
            match timeout(Duration::from_millis(ms), self.wait_for_connect_result()).await {
                Ok(result) => result?,
                Err(_) => return Err(anyhow::anyhow!("WebRTC wait_connected timeout")),
            }
        } else {
            self.wait_for_connect_result().await?;
        }
        Ok(())
    }

    /// Explicitly tear down the peer connection. Dropping the handle is not enough: `SESSIONS`
    /// holds a clone, so the pc and its ICE/DTLS resources survive until it happens to reach a
    /// terminal ICE state. Closing fires the state handler, which evicts the `SESSIONS` entry —
    /// callers that abandon a stream (e.g. an offerer that lost the transport race) must call it.
    #[inline]
    pub async fn close(&self) {
        self.pc.close().await.ok();
    }

    /// Tear the pc down as an independent task on `WEBRTC_RT` — the runtime that owns the
    /// pc's sockets and pump tasks, so the close can actually reach the wire no matter which
    /// caller runtimes have died — keeping `keep` alive until the teardown finishes. Callable
    /// from any context, `Drop` on a runtime-less thread included, and never cancelled, so
    /// `close()`'s `is_closed` latch is only ever set by a close that finishes. The send path
    /// passes its permit as `keep`.
    pub fn close_detached_with<T: Send + 'static>(&self, keep: T) {
        // If the runtime never built, no pc exists either (`new` fails first); nothing to close.
        let Some(rt) = WEBRTC_RT.as_ref() else {
            return;
        };
        let pc = self.pc.clone();
        rt.spawn(async move {
            if let Err(err) = pc.close().await {
                log::debug!("WebRTC close failed: {}", err);
            }
            drop(keep);
        });
    }

    #[inline]
    pub fn close_detached(&self) {
        self.close_detached_with(());
    }

    /// Close a pc no caller ever took. Spawned on `WEBRTC_RT` like `close_detached`, but the
    /// decision is remade under the `SESSIONS` lock that a cache hit claims under: either the hit
    /// registers first and this call stands down, or `closing` commits first and
    /// `is_reusable_for` rejects the entry, which the state handler evicts only later.
    fn close_if_abandoned(&self) {
        let Some(rt) = WEBRTC_RT.as_ref() else {
            return;
        };
        let stream = self.clone();
        rt.spawn(async move {
            {
                let _sessions_lock = SESSIONS.lock().await;
                if stream.handoff.pending.load(Ordering::Acquire) != 0
                    || stream.handoff.adopted.load(Ordering::Acquire)
                {
                    return;
                }
                stream.handoff.closing.store(true, Ordering::Release);
            }
            // Off-lock: `close()` fires the state handler inline and that handler locks SESSIONS.
            if let Err(err) = stream.pc.close().await {
                log::debug!("WebRTC close of an abandoned peer failed: {}", err);
            }
        });
    }

    #[inline]
    pub fn set_raw(&mut self) {
        // not-supported
    }

    /// See `Stream::set_max_packet_length`. Clamped to `MAX_RECV_MESSAGE`: the send path checks
    /// against that constant, so a bound above it would only accept what no peer will ever send.
    #[inline]
    pub fn set_max_packet_length(&mut self, n: usize) {
        self.max_recv_message
            .store(n.min(MAX_RECV_MESSAGE), Ordering::Relaxed);
    }

    #[inline]
    pub fn local_addr(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
    }

    #[inline]
    pub fn set_send_timeout(&mut self, ms: u64) {
        self.send_timeout = ms;
    }

    /// See `Stream::rx_progress`.
    #[inline]
    pub fn rx_progress(&self) -> u64 {
        self.rx_probe.get()
    }

    /// Track whether ICE still hears from the peer. `Disconnected` is raised about 5s after it
    /// stops and is cleared again when traffic resumes, so it is a hint to be acted on by whoever
    /// asks, never a terminal state; the terminal ones are handled by the caller of this and
    /// deliberately leave the flag alone, since a closed session has no use for a hint.
    fn note_health(disconnected: &AtomicBool, s: RTCPeerConnectionState) {
        match s {
            RTCPeerConnectionState::Disconnected => disconnected.store(true, Ordering::SeqCst),
            RTCPeerConnectionState::Connected => disconnected.store(false, Ordering::SeqCst),
            _ => {}
        }
    }

    /// See `Stream::webrtc_disconnected`.
    #[inline]
    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn set_key(&mut self, _key: Key) {
        // WebRTC traffic is DTLS-encrypted regardless; the secretbox key is unused.
        // Callers invoke set_key only after the controller has bound the DTLS fingerprint to the
        // verified peer identity (or the controlled side has completed the matching handshake).
        // Mark peer-verified so is_secured() matches TCP's post-key-exchange meaning.
        self.peer_verified.store(true, Ordering::Release);
    }

    #[inline]
    pub fn is_secured(&self) -> bool {
        self.peer_verified.load(Ordering::Acquire)
    }

    #[inline]
    pub async fn send(&mut self, msg: &impl Message) -> ResultType<()> {
        self.send_raw(msg.write_to_bytes()?).await
    }

    #[inline]
    pub async fn send_raw(&mut self, msg: Vec<u8>) -> ResultType<()> {
        self.send_bytes(Bytes::from(msg)).await
    }

    #[inline]
    async fn wait_for_connect_result(&mut self) -> ResultType<()> {
        loop {
            match self.state_notify.borrow().clone() {
                WebRTCConnectionState::Open => return Ok(()),
                WebRTCConnectionState::Closed(reason) => {
                    return Err(anyhow::anyhow!("WebRTC connection closed: {}", reason));
                }
                WebRTCConnectionState::Pending => {}
            }
            self.state_notify.changed().await?;
        }
    }

    /// Fetch (and cache) the detached data channel. `detach()` is idempotent and returns a
    /// clone of the same underlying channel, so caching it just avoids re-locking per message.
    async fn detached_dc(&self) -> ResultType<Arc<DetachedDataChannel>> {
        {
            let cache = self.detached.lock().await;
            if let Some(dc) = cache.as_ref() {
                return Ok(dc.clone());
            }
        }
        let raw = self.stream.lock().await.clone();
        let dc = raw.detach().await?;
        let mut cache = self.detached.lock().await;
        // Another task may have cached it while we were detaching.
        if let Some(existing) = cache.as_ref() {
            return Ok(existing.clone());
        }
        *cache = Some(dc.clone());
        Ok(dc)
    }

    /// NOT cancel-safe: dropping this future mid-message (e.g. wrapping it in `select!`/`timeout`)
    /// can leave a partial fragment sequence on the wire, corrupting reassembly of every later
    /// message on this stream. A caller that abandons a send must treat the stream as dead and
    /// close it; the built-in `send_timeout` path below already does (it closes the pc).
    pub async fn send_bytes(&mut self, bytes: Bytes) -> ResultType<()> {
        let send_timeout = self.send_timeout;
        let send_gate = self.send_gate.clone();
        // Bound the WHOLE send (wait-for-open, queueing behind another clone, every write) by
        // send_timeout: otherwise a write parks indefinitely on SCTP backpressure and
        // connection.rs's timer never runs. That parking is also what bounds sender memory —
        // PendingQueue admits 128 KiB and inflight is cwnd/rwnd-capped — so this timeout is the
        // TCP-send-timeout equivalent. See the checklist entry on send backpressure.
        if send_timeout > 0 {
            // Waiting for the connection to open is not this message's progress. Charging it to
            // the send clock meant that on the first send after signaling, a slow ICE/DTLS
            // completion ate most of the budget and the write inherited the remainder — then
            // timed out and closed a peer connection that was an RTT from working. Give it its
            // own budget and start the send clock once the channel is actually usable.
            timeout(
                Duration::from_millis(send_timeout),
                self.wait_for_connect_result(),
            )
            .await
            .map_err(|_| Error::new(ErrorKind::TimedOut, "WebRTC connect timeout"))??;

            let gate_deadline = Instant::now() + Duration::from_millis(send_timeout);
            let send_permit = match timeout_at(gate_deadline, send_gate.acquire_owned()).await {
                Ok(Ok(permit)) => permit,
                Ok(Err(err)) => {
                    return Err(Error::new(
                        ErrorKind::BrokenPipe,
                        format!("WebRTC send gate closed: {}", err),
                    )
                    .into());
                }
                Err(_) => {
                    // Deliberately no teardown: this task never held the permit, so all that
                    // happened is that another clone is legitimately mid-message. Closing from
                    // here would abort a healthy sender's fragment sequence — precisely the
                    // corruption the permit exists to prevent.
                    return Err(Error::new(ErrorKind::TimedOut, "WebRTC send gate timeout").into());
                }
            };
            // Fresh budget once the permit is held: queueing behind another clone's message is
            // not this message's progress, so charging it here meant a caller that only just lost
            // the gate race got a few milliseconds to write in — and then tore the whole peer
            // connection down for missing them. The narrower the miss, the more certain the
            // teardown, which is precisely backwards.
            let write_deadline = Instant::now() + Duration::from_millis(send_timeout);
            let mut desynced = false;
            match timeout_at(write_deadline, self.send_bytes_inner(bytes, &mut desynced)).await {
                // A write that failed part-way leaves the same orphaned prefix a timeout does, so
                // it needs the same teardown — and the same handoff of the permit, or a waiting
                // clone appends its message to that prefix while the close is still in flight.
                Ok(res) => {
                    if desynced {
                        self.close_detached_with(send_permit);
                    }
                    res
                }
                Err(_) => {
                    // Hand the logical-message permit to the teardown so no waiting clone can
                    // append a new message after a partially-written fragment sequence. Holding
                    // it across an awaited close would drop both on cancellation.
                    self.close_detached_with(send_permit);
                    Err(Error::new(ErrorKind::TimedOut, "WebRTC send timeout").into())
                }
            }
        } else {
            let send_permit = send_gate.acquire_owned().await.map_err(|err| {
                Error::new(
                    ErrorKind::BrokenPipe,
                    format!("WebRTC send gate closed: {}", err),
                )
            })?;
            let mut desynced = false;
            let res = self.send_bytes_inner(bytes, &mut desynced).await;
            if desynced {
                self.close_detached_with(send_permit);
            }
            res
        }
    }

    /// `desynced` is set when the failure left a partial fragment sequence on the wire, i.e. when
    /// the stream can no longer be made consistent and the caller must close it.
    async fn send_bytes_inner(&mut self, bytes: Bytes, desynced: &mut bool) -> ResultType<()> {
        // Same bound the receiver enforces, so we never emit a message a same-version peer
        // would have to kill the connection over.
        if bytes.len() > MAX_RECV_MESSAGE {
            return Err(Error::new(ErrorKind::InvalidInput, "Overflow").into());
        }
        self.wait_for_connect_result().await?;
        let dc = self.detached_dc().await?;
        let data = bytes.as_ref();
        let mut offset = 0;
        // Always emit at least one fragment (a lone FRAG_END header for an empty message), so a
        // zero-length data-channel message — which the receiver cannot distinguish from EOF — is
        // never sent.
        let mut wrote_any = false;
        loop {
            let end = (offset + MAX_FRAGMENT_PAYLOAD).min(data.len());
            let is_last = end >= data.len();
            let chunk = &data[offset..end];
            let mut framed = BytesMut::with_capacity(1 + chunk.len());
            framed.put_u8(if is_last { FRAG_END } else { FRAG_MORE });
            framed.put_slice(chunk);
            if let Err(err) = dc.write(&framed.freeze()).await {
                // Fragments already on the wire leave the peer an unterminated prefix, and this
                // framing has no length for it to notice with, so the next message is appended
                // to it. Callers wrap sends in `allow_err!`, so the error alone is not enough.
                *desynced = wrote_any;
                return Err(err.into());
            }
            wrote_any = true;
            offset = end;
            if is_last {
                break;
            }
        }
        Ok(())
    }

    #[inline]
    pub async fn next(&mut self) -> Option<Result<BytesMut, Error>> {
        if let Err(err) = self.wait_for_connect_result().await {
            self.close_detached();
            return Some(Err(Error::new(ErrorKind::Other, err.to_string())));
        }
        let dc = match self.detached_dc().await {
            Ok(dc) => dc,
            Err(err) => {
                self.close_detached();
                return Some(Err(Error::new(ErrorKind::Other, err.to_string())));
            }
        };
        // Held across `dc.read().await` on purpose, against the usual "no locks across await"
        // rule: it is what makes the single reader this reassembly requires (see the field) an
        // exclusion rather than a convention. Releasing it around the read would admit exactly
        // the second reader that corrupts `acc`. The cost — a would-be second reader blocking
        // until a packet arrives — is a state the design does not permit anyway.
        let mut st = self.recv_state.lock().await;
        loop {
            let RecvState { acc, scratch } = &mut *st;
            if scratch.len() < RECV_BUF_SIZE {
                scratch.resize(SCRATCH_REFILL, 0);
            }
            let n = match dc.read(&mut scratch[..RECV_BUF_SIZE]).await {
                Ok(n) => n,
                Err(err) => {
                    // Release the partial message, as the framing-violation paths below do: the
                    // buffer can hold up to the cap and `recv_state` outlives this call through
                    // the `SESSIONS` clone.
                    *acc = BytesMut::new();
                    self.close_detached();
                    return Some(Err(Error::new(
                        ErrorKind::Other,
                        format!("data channel read error: {}", err),
                    )));
                }
            };
            // Before the framing checks below: a fragment that turns out to be malformed still
            // proves the peer sent it, which is all this counter claims.
            if n > 0 {
                self.rx_probe.add(n);
            }
            if n == 0 {
                // Not exclusively a stream reset: webrtc-data maps the empty-message PPIDs to
                // n == 0 as well and `read` drops the flag that would separate them. Both mean
                // nothing frameable follows, so treat them alike but not as a clean remote close.
                log::debug!("WebRTC data channel ended (reset or empty message)");
                *acc = BytesMut::new();
                self.close_detached();
                return None;
            }
            // Both would otherwise read as "more fragments". The payload-less FRAG_MORE is the
            // nastier one: it adds nothing to `acc`, so the cap below never trips and the loop
            // spins for as long as the peer keeps sending.
            let header = scratch[0];
            let bad = match header {
                FRAG_END => None,
                FRAG_MORE if n > 1 => None,
                FRAG_MORE => Some("FRAG_MORE fragment carries no payload".to_owned()),
                other => Some(format!(
                    "fragment header {other} is neither FRAG_END nor FRAG_MORE"
                )),
            };
            if let Some(why) = bad {
                *acc = BytesMut::new();
                self.close_detached();
                return Some(Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("WebRTC {why}"),
                )));
            }
            // Bound BEFORE growing: this framing carries no length, so an oversize message is only
            // discovered by accumulating it, and checking after the append would let `BytesMut`'s
            // reallocate-and-copy hold the old and the new buffer at once.
            if acc.len() + (n - 1) > self.max_recv_message.load(Ordering::Relaxed) {
                // Release the buffer, don't just truncate it: by definition it is near the cap
                // here, and `recv_state` outlives this call through the `SESSIONS` clone.
                *acc = BytesMut::new();
                self.close_detached();
                return Some(Err(Error::new(
                    ErrorKind::InvalidData,
                    "WebRTC reassembled message exceeded maximum frame size",
                )));
            }
            // A message that arrived whole is handed over as a slice of the read buffer: no
            // allocation and no copy, which is every input event and most audio packets.
            if header == FRAG_END && acc.is_empty() {
                scratch.advance(1);
                return Some(Ok(scratch.split_to(n - 1)));
            }
            acc.extend_from_slice(&scratch[1..n]);
            if header == FRAG_END {
                // Split, not take: the caller has dropped the message by the time the next
                // fragment lands, so `reserve` reclaims this allocation instead of regrowing it
                // from nothing for every fragmented frame.
                let msg = acc.split();
                return Some(Ok(msg));
            }
        }
    }

    #[inline]
    pub async fn next_timeout(&mut self, ms: u64) -> Option<Result<BytesMut, Error>> {
        match timeout(Duration::from_millis(ms), self.next()).await {
            Ok(res) => res,
            Err(_) => None,
        }
    }
}

pub fn is_webrtc_endpoint(endpoint: &str) -> bool {
    // use sdp base64 json string as endpoint, or prefix webrtc:
    endpoint.starts_with("webrtc://")
}

#[cfg(test)]
mod tests {
    use crate::webrtc::WebRTCStream;
    use crate::webrtc::{NewStreamHandoff, DEFAULT_ICE_SERVERS, FRAG_MORE, SESSIONS, WEBRTC_RT};
    use bytes::{BufMut, Bytes, BytesMut};
    use std::{collections::HashMap, sync::Arc, time::Duration};
    use tokio::sync::Barrier;
    use tokio::time::timeout;
    use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

    async fn new_test_stream() -> WebRTCStream {
        WebRTCStream::new("", false, 10000).await.unwrap()
    }

    #[test]
    fn test_webrtc_ice_url() {
        let turn = WebRTCStream::get_ice_server_from_url("turn://123:321@example.com:3478")
            .expect("credentialed turn is usable");
        assert_eq!(turn.urls[0], "turn:example.com:3478");
        assert_eq!(turn.username, "123");
        assert_eq!(turn.credential, "321");

        assert_eq!(
            WebRTCStream::get_ice_server_from_url("turn://123:321@example.com")
                .unwrap_or_default()
                .urls[0],
            "turn:example.com:3478"
        );

        // Userinfo arrives percent-encoded from the URL parser; the TURN server wants the
        // value, so a password written `p%40ss` authenticates as `p@ss`.
        let encoded =
            WebRTCStream::get_ice_server_from_url("turn://us%20er:p%40s%2Fs%25@example.com")
                .expect("encoded credentials are usable");
        assert_eq!(encoded.username, "us er");
        assert_eq!(encoded.credential, "p@s/s%");

        // TURN without both halves of the credential is dropped rather than passed on:
        // webrtc-rs validates every configured server when the peer connection is built, so
        // one such entry fails EVERY connection, including those that never wanted TURN.
        for missing in [
            "turn://example.com:3478",
            "turn://example.com",
            "turn://123@example.com",
            "turns://example.com:5349",
            "turn:example.com:3478",
        ] {
            assert_eq!(
                WebRTCStream::get_ice_server_from_url(missing),
                None,
                "credential-less {missing} must not reach the configuration"
            );
        }

        // STUN needs no credentials, so both spellings stay usable.
        assert_eq!(
            WebRTCStream::get_ice_server_from_url("stun://example.com:3478")
                .unwrap_or_default()
                .urls[0],
            "stun:example.com:3478"
        );

        assert_eq!(
            WebRTCStream::get_ice_server_from_url("http://123:123@example.com:3478"),
            None
        );

        // RFC 7065 spelling (`scheme:host:port`, no authority) — what STUN/TURN docs hand out.
        // `url` cannot-be-a-base's it, so host and port come out of the path; getting this wrong
        // produced a hostless "stun::3478" no ICE agent can resolve.
        for (input, expected) in [
            ("stun:example.com:19302", "stun:example.com:19302"),
            ("stun:example.com", "stun:example.com:3478"),
            ("stun:[2001:db8::1]:19302", "stun:[2001:db8::1]:19302"),
            ("stun:[2001:db8::1]", "stun:[2001:db8::1]:3478"),
            (
                "stun:example.com:19302?transport=udp",
                "stun:example.com:19302",
            ),
        ] {
            assert_eq!(
                WebRTCStream::get_ice_server_from_url(input)
                    .unwrap_or_default()
                    .urls[0],
                expected,
                "parsing {input}"
            );
        }

        // Malformed entries are dropped, never repaired into something webrtc-ice will choke
        // on: a folded-in bad port ("host:99999:3478") or an unbracketed IPv6 split at its last
        // colon fails peer-connection construction outright, taking every server down with it.
        for bad in [
            "stun:",
            "stun:example.com:99999",
            "stun:example.com:abc",
            "stun:2001:db8::1",
        ] {
            assert_eq!(
                WebRTCStream::get_ice_server_from_url(bad),
                None,
                "malformed {bad} must be dropped"
            );
        }
    }

    // Parsing is exercised through `parse_ice_servers`, never by rewriting the global
    // `ice-servers` option: `set_option` persists to the real config file and the tests share
    // one process, so mutating it raced every peer connection the loopback tests were building.
    #[test]
    fn test_webrtc_ice_server_list() {
        assert_eq!(
            WebRTCStream::parse_ice_servers("")[0].urls[0],
            DEFAULT_ICE_SERVERS[0].to_string()
        );

        // Unusable entries drop out of the list; the rest of the config still applies.
        let parsed = WebRTCStream::parse_ice_servers(
            ",stun://example.com,turn://u:p@example.com,turn://nocreds.example.com,sdf",
        );
        assert_eq!(parsed[0].urls[0], "stun:example.com:3478");
        assert_eq!(parsed[1].urls[0], "turn:example.com:3478");
        assert_eq!(parsed.len(), 2);

        // TURN-only config still gets the default STUN servers prepended.
        let turn_only = WebRTCStream::parse_ice_servers("turn://u:p@example.com:3478");
        assert_eq!(turn_only[0].urls[0], DEFAULT_ICE_SERVERS[0].to_string());
        assert_eq!(turn_only[1].urls[0], "turn:example.com:3478");
    }

    #[test]
    fn test_endpoint_ice_policy_declaration() {
        // An envelope with the marker declares full ICE; everything else — no marker,
        // wrong value, foreign scheme, garbage — reads as the old Relay-only semantics.
        let marked =
            WebRTCStream::sdp_to_endpoint(r#"{"type":"offer","sdp":"v=0","ice_policy":"all"}"#);
        assert!(WebRTCStream::endpoint_declares_all_ice(&marked));

        let unmarked = WebRTCStream::sdp_to_endpoint(r#"{"type":"offer","sdp":"v=0"}"#);
        assert!(!WebRTCStream::endpoint_declares_all_ice(&unmarked));

        let wrong =
            WebRTCStream::sdp_to_endpoint(r#"{"type":"offer","sdp":"v=0","ice_policy":"relay"}"#);
        assert!(!WebRTCStream::endpoint_declares_all_ice(&wrong));

        assert!(!WebRTCStream::endpoint_declares_all_ice(""));
        assert!(!WebRTCStream::endpoint_declares_all_ice(
            "webrtc://not-base64!"
        ));
        assert!(!WebRTCStream::endpoint_declares_all_ice(
            "https://example.com"
        ));

        // The marker must be invisible to the plain RTCSessionDescription parse old peers do.
        let sdp_json = WebRTCStream::get_remote_offer(&marked).unwrap();
        serde_json::from_str::<
            webrtc::peer_connection::sdp::session_description::RTCSessionDescription,
        >(&sdp_json)
        .expect("extra envelope key must not break RTCSessionDescription parsing");
    }

    /// The live description once it has gathered something. Polls instead of awaiting
    /// `gathering_complete_promise`: a stream sits in the global `SESSIONS` for the whole wait, and
    /// waiting out gathering there is long enough to trip `test_cancelled_new_does_not_leak_the_pc`
    /// when the suite runs its tests in parallel. One candidate is all these tests need.
    /// Returns `None` on timeout rather than panicking: the caller must get to `close()` before
    /// it asserts, or a stranded pc fails `test_cancelled_new_does_not_leak_the_pc` as well and
    /// one root failure is reported as two.
    async fn first_gathered_description(stream: &WebRTCStream) -> Option<RTCSessionDescription> {
        timeout(Duration::from_secs(10), async {
            loop {
                if let Some(desc) = stream.pc.local_description().await {
                    if desc.sdp.contains("a=candidate:") {
                        return desc;
                    }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .ok()
    }

    /// The signalled endpoint and the live description must diverge. `pc.local_description()`
    /// appends every candidate gathered so far; the endpoint the peer is handed must not grow with
    /// them, because the rendezvous server delivers it to a UDP-registered peer as one datagram
    /// and a fragmented one is dropped without a trace on paths that discard fragments.
    #[tokio::test]
    async fn test_stored_endpoint_does_not_grow_with_the_live_description() {
        let offerer = WebRTCStream::new("", false, 20000).await.unwrap();
        let live = first_gathered_description(&offerer).await;
        let endpoint = offerer.local_endpoint().to_owned();
        offerer.close().await;

        // Some(_) is the assertion that the live description grew: the helper only returns once it
        // carries a candidate, so the endpoint below really had something to grow by.
        live.expect("no ICE candidate gathered within 10s");
        assert!(
            !WebRTCStream::get_remote_offer(&endpoint)
                .unwrap()
                .contains("a=candidate"),
            "the signalled endpoint grew candidates alongside the live description"
        );
        assert!(
            endpoint.len() <= WebRTCStream::UDP_ENDPOINT_BUDGET,
            "signalled endpoint is {} bytes, over the {} byte budget",
            endpoint.len(),
            WebRTCStream::UDP_ENDPOINT_BUDGET
        );
    }

    /// The split itself: candidates out, everything ICE and DTLS need to start in. A stripped
    /// endpoint the peer cannot consume would trade a silent drop for a silent failure.
    #[tokio::test]
    async fn test_trickle_endpoint_splits_candidates_off() {
        let offerer = WebRTCStream::new("", false, 20000).await.unwrap();
        let gathered = first_gathered_description(&offerer).await;
        offerer.close().await;
        let gathered = gathered.expect("no ICE candidate gathered within 10s");

        let endpoint = WebRTCStream::trickle_endpoint(&gathered, false).unwrap();
        let json = WebRTCStream::get_remote_offer(&endpoint).unwrap();
        assert!(!json.contains("a=candidate:"), "candidate survived the split");
        assert!(
            !json.contains("a=end-of-candidates"),
            "end-of-candidates would stop the remote agent waiting for the trickle"
        );
        for keep in [
            "m=application",
            "a=ice-ufrag:",
            "a=ice-pwd:",
            "a=fingerprint:",
            "a=setup:",
            "a=mid:",
            "a=sctp-port:",
        ] {
            assert!(json.contains(keep), "trickle endpoint dropped {}", keep);
        }
        assert!(
            endpoint.len() <= WebRTCStream::UDP_ENDPOINT_BUDGET,
            "stripped endpoint is {} bytes",
            endpoint.len()
        );
        // `parsed` is #[serde(skip)], so deserializing alone would accept any string as the SDP
        // body. Unmarshal it, which is also what the peer does, and require the fingerprint the
        // session key is derived from to have survived.
        let desc: RTCSessionDescription =
            serde_json::from_str(&json).expect("a stripped endpoint must deserialize");
        WebRTCStream::get_key_for_sdp(&desc)
            .expect("a stripped endpoint must still parse as SDP and keep its fingerprint");

        // `encode_endpoint`, which `get_local_endpoint` also goes through, holds the opposite
        // contract: carry whatever it is handed, candidates included. (What `get_local_endpoint`
        // adds on top — the gathering wait — is covered by test_webrtc_loopback_gathered_endpoints.)
        let kept = WebRTCStream::encode_endpoint(gathered.sdp_type, &gathered.sdp, false).unwrap();
        assert!(
            WebRTCStream::get_remote_offer(&kept)
                .unwrap()
                .contains("a=candidate:"),
            "encode_endpoint dropped the candidates it was handed"
        );
    }

    #[test]
    fn test_webrtc_session_key() {
        let mut sdp_str = "".to_owned();
        assert_eq!(
            WebRTCStream::get_key_for_sdp(
                &RTCSessionDescription::offer(sdp_str).unwrap_or_default()
            )
            .unwrap_or_default(),
            ""
        );

        sdp_str = "\
v=0
o=- 7400546379179479477 208696200 IN IP4 0.0.0.0
s=-
t=0 0
a=fingerprint:sha-256 97:52:D6:1F:1E:87:6C:DA:B8:21:95:64:A5:85:89:FA:02:71:C7:4D:B3:FD:25:92:40:FB:6B:65:24:3C:79:88
a=group:BUNDLE 0
a=extmap-allow-mixed
m=application 9 UDP/DTLS/SCTP webrtc-datachannel
c=IN IP4 0.0.0.0
a=setup:actpass
a=mid:0
a=sendrecv
a=sctp-port:5000
a=ice-ufrag:RMWjjpXfpXbDPdMz
a=ice-pwd:BtIqlWHfwhsJdFiBROeLuEbNmYfHxRfT".to_owned();
        assert_eq!(
            WebRTCStream::get_key_for_sdp(
                &RTCSessionDescription::offer(sdp_str).unwrap_or_default()
            ).unwrap_or_default(),
            "sha-256 97:52:D6:1F:1E:87:6C:DA:B8:21:95:64:A5:85:89:FA:02:71:C7:4D:B3:FD:25:92:40:FB:6B:65:24:3C:79:88"
        );

        sdp_str = "\
v=0
o=- 7400546379179479477 208696200 IN IP4 0.0.0.0
s=-
t=0 0
a=group:BUNDLE 0
a=extmap-allow-mixed
m=application 9 UDP/DTLS/SCTP webrtc-datachannel
c=IN IP4 0.0.0.0
a=fingerprint:sha-256 97:52:D6:1F:1E:87:6C:DA:B8:21:95:64:A5:85:89:FA:02:71:C7:4D:B3:FD:25:92:40:FB:6B:65:24:3C:79:88
a=setup:actpass
a=mid:0
a=sendrecv
a=sctp-port:5000
a=ice-ufrag:RMWjjpXfpXbDPdMz
a=ice-pwd:BtIqlWHfwhsJdFiBROeLuEbNmYfHxRfT".to_owned();
        assert_eq!(
            WebRTCStream::get_key_for_sdp(
                &RTCSessionDescription::offer(sdp_str).unwrap_or_default()
            ).unwrap_or_default(),
            "sha-256 97:52:D6:1F:1E:87:6C:DA:B8:21:95:64:A5:85:89:FA:02:71:C7:4D:B3:FD:25:92:40:FB:6B:65:24:3C:79:88"
        );

        sdp_str = "\
v=0
o=- 7400546379179479477 208696200 IN IP4 0.0.0.0
s=-
t=0 0
a=group:BUNDLE 0
a=extmap-allow-mixed
m=application 9 UDP/DTLS/SCTP webrtc-datachannel
c=IN IP4 0.0.0.0
a=setup:actpass
a=mid:0
a=sendrecv
a=sctp-port:5000
a=ice-ufrag:RMWjjpXfpXbDPdMz
a=ice-pwd:BtIqlWHfwhsJdFiBROeLuEbNmYfHxRfT"
            .to_owned();
        assert!(
            WebRTCStream::get_key_for_sdp(
                &RTCSessionDescription::offer(sdp_str).unwrap_or_default()
            )
            .is_err(),
            "can not find fingerprint attribute"
        );

        sdp_str = "\
v=0
o=- 7400546379179479477 208696200 IN IP4 0.0.0.0
s=-
t=0 0
a=group:BUNDLE 0
a=extmap-allow-mixed
m=audio 9 UDP/DTLS/SCTP webrtc-datachannel
c=IN IP4 0.0.0.0
a=fingerprint:sha-256 97:52:D6:1F:1E:87:6C:DA:B8:21:95:64:A5:85:89:FA:02:71:C7:4D:B3:FD:25:92:40:FB:6B:65:24:3C:79:88
a=setup:actpass
a=mid:0
a=sendrecv
a=sctp-port:5000
a=ice-ufrag:RMWjjpXfpXbDPdMz
a=ice-pwd:BtIqlWHfwhsJdFiBROeLuEbNmYfHxRfT".to_owned();
        assert!(
            WebRTCStream::get_key_for_sdp(
                &RTCSessionDescription::offer(sdp_str).unwrap_or_default()
            )
            .is_err(),
            "can not find datachannel fingerprint attribute"
        );

        assert!(
            WebRTCStream::get_key_for_sdp(
                &RTCSessionDescription::offer("".to_owned()).unwrap_or_default()
            )
            .is_err(),
            "invalid sdp should error"
        );

        assert!(
            WebRTCStream::get_key_for_sdp_json("{}").is_err(),
            "empty sdp json should error"
        );

        assert!(
            WebRTCStream::get_key_for_sdp_json("{ss}").is_err(),
            "invalid sdp json should error"
        );

        let endpoint = "webrtc://eyJ0eXBlIjoiYW5zd2VyIiwic2RwIjoidj0wXHJcbm89LSA0MTA1NDk3NTY2NDgyMTQzODEwIDYwMzk1NzQw\
MCBJTiBJUDQgMC4wLjAuMFxyXG5zPS1cclxudD0wIDBcclxuYT1maW5nZXJwcmludDpzaGEtMjU2IDYxOjYwOjc0OjQwOjI4OkNFOjBCOjBDOjc1OjRCOj\
EwOjlBOkVFOjc3OkY1OjQ0OjU3Ojg0OjUxOkRCOjA0OjkyOjRBOjEwOjFDOjRFOjVGOjdFOkYxOkIzOjcxOjIyXHJcbmE9Z3JvdXA6QlVORExFIDBcclxu\
YT1leHRtYXAtYWxsb3ctbWl4ZWRcclxubT1hcHBsaWNhdGlvbiA5IFVEUC9EVExTL1NDVFAgd2VicnRjLWRhdGFjaGFubmVsXHJcbmM9SU4gSVA0IDAuMC\
4wLjBcclxuYT1zZXR1cDphY3RpdmVcclxuYT1taWQ6MFxyXG5hPXNlbmRyZWN2XHJcbmE9c2N0cC1wb3J0OjUwMDBcclxuYT1pY2UtdWZyYWc6SHlnU1Rr\
V2RsRlpHRG1XWlxyXG5hPWljZS1wd2Q6SkJneFZWaGZveVhHdHZha1VWcnBQeHVOSVpMU3llS1pcclxuYT1jYW5kaWRhdGU6OTYzOTg4MzQ4IDEgdWRwID\
IxMzA3MDY0MzEgMTkyLjE2OC4xLjIgNjQwMDcgdHlwIGhvc3RcclxuYT1jYW5kaWRhdGU6OTYzOTg4MzQ4IDIgdWRwIDIxMzA3MDY0MzEgMTkyLjE2OC4x\
LjIgNjQwMDcgdHlwIGhvc3RcclxuYT1jYW5kaWRhdGU6MTg2MTA0NTE5MCAxIHVkcCAxNjk0NDk4ODE1IDE0LjIxMi42OC4xMiAyNzAwNCB0eXAgc3JmbH\
ggcmFkZHIgMC4wLjAuMCBycG9ydCA2NDAwOFxyXG5hPWNhbmRpZGF0ZToxODYxMDQ1MTkwIDIgdWRwIDE2OTQ0OTg4MTUgMTQuMjEyLjY4LjEyIDI3MDA0\
IHR5cCBzcmZseCByYWRkciAwLjAuMC4wIHJwb3J0IDY0MDA4XHJcbmE9ZW5kLW9mLWNhbmRpZGF0ZXNcclxuIn0=".to_owned();
        assert_eq!(
            WebRTCStream::get_key_for_sdp_json(
                &WebRTCStream::get_remote_offer(&endpoint).unwrap_or_default()
            ).unwrap_or_default(),
            "sha-256 61:60:74:40:28:CE:0B:0C:75:4B:10:9A:EE:77:F5:44:57:84:51:DB:04:92:4A:10:1C:4E:5F:7E:F1:B3:71:22"
        );
    }

    #[tokio::test]
    async fn test_webrtc_new_stream() {
        let mut endpoint = "webrtc://sdfsdf".to_owned();
        assert!(
            WebRTCStream::new(&endpoint, false, 10000).await.is_err(),
            "invalid webrtc endpoint should error"
        );

        endpoint = "wss://sdfsdf".to_owned();
        assert!(
            WebRTCStream::new(&endpoint, false, 10000).await.is_err(),
            "invalid webrtc endpoint should error"
        );

        let stream = WebRTCStream::new("", false, 10000)
            .await
            .expect("local webrtc endpoint should ok");
        // A raw WebRTCStream has no Drop: close it, or its session stays cached in SESSIONS
        // and pollutes cross-test assertions about the cache.
        stream.close().await;

        endpoint = "webrtc://eyJ0eXBlIjoiYW5zd2VyIiwic2RwIjoidj0wXHJcbm89LSA0MTA1NDk3NTY2NDgyMTQzODEwIDYwMzk1NzQw\
MCBJTiBJUDQgMC4wLjAuMFxyXG5zPS1cclxudD0wIDBcclxuYT1maW5nZXJwcmludDpzaGEtMjU2IDYxOjYwOjc0OjQwOjI4OkNFOjBCOjBDOjc1OjRCOj\
EwOjlBOkVFOjc3OkY1OjQ0OjU3Ojg0OjUxOkRCOjA0OjkyOjRBOjEwOjFDOjRFOjVGOjdFOkYxOkIzOjcxOjIyXHJcbmE9Z3JvdXA6QlVORExFIDBcclxu\
YT1leHRtYXAtYWxsb3ctbWl4ZWRcclxubT1hcHBsaWNhdGlvbiA5IFVEUC9EVExTL1NDVFAgd2VicnRjLWRhdGFjaGFubmVsXHJcbmM9SU4gSVA0IDAuMC\
4wLjBcclxuYT1zZXR1cDphY3RpdmVcclxuYT1taWQ6MFxyXG5hPXNlbmRyZWN2XHJcbmE9c2N0cC1wb3J0OjUwMDBcclxuYT1pY2UtdWZyYWc6SHlnU1Rr\
V2RsRlpHRG1XWlxyXG5hPWljZS1wd2Q6SkJneFZWaGZveVhHdHZha1VWcnBQeHVOSVpMU3llS1pcclxuYT1jYW5kaWRhdGU6OTYzOTg4MzQ4IDEgdWRwID\
IxMzA3MDY0MzEgMTkyLjE2OC4xLjIgNjQwMDcgdHlwIGhvc3RcclxuYT1jYW5kaWRhdGU6OTYzOTg4MzQ4IDIgdWRwIDIxMzA3MDY0MzEgMTkyLjE2OC4x\
LjIgNjQwMDcgdHlwIGhvc3RcclxuYT1jYW5kaWRhdGU6MTg2MTA0NTE5MCAxIHVkcCAxNjk0NDk4ODE1IDE0LjIxMi42OC4xMiAyNzAwNCB0eXAgc3JmbH\
ggcmFkZHIgMC4wLjAuMCBycG9ydCA2NDAwOFxyXG5hPWNhbmRpZGF0ZToxODYxMDQ1MTkwIDIgdWRwIDE2OTQ0OTg4MTUgMTQuMjEyLjY4LjEyIDI3MDA0\
IHR5cCBzcmZseCByYWRkciAwLjAuMC4wIHJwb3J0IDY0MDA4XHJcbmE9ZW5kLW9mLWNhbmRpZGF0ZXNcclxuIn0=".to_owned();
        assert!(
            WebRTCStream::new(&endpoint, false, 10000).await.is_err(),
            "connect to an 'answer' webrtc endpoint should error"
        );
    }

    #[tokio::test]
    async fn test_session_cleanup_only_removes_matching_peer() {
        let key = "test-session-cleanup".to_owned();
        let old_key = "test-session-cleanup-old".to_owned();
        let old = new_test_stream().await;
        let replacement = new_test_stream().await;
        let mut sessions = HashMap::new();
        sessions.insert(key.clone(), replacement.clone());

        assert!(WebRTCStream::remove_sessions_for_peer(&mut sessions, &old.pc).is_empty());
        assert!(Arc::ptr_eq(
            &sessions.get(&key).unwrap().pc,
            &replacement.pc,
        ));

        sessions.insert(old_key.clone(), old.clone());
        assert_eq!(
            WebRTCStream::remove_sessions_for_peer(&mut sessions, &old.pc),
            vec![old_key.clone()],
        );
        assert!(!sessions.contains_key(&old_key));
        assert!(Arc::ptr_eq(
            &sessions.get(&key).unwrap().pc,
            &replacement.pc,
        ));

        assert_eq!(
            WebRTCStream::remove_sessions_for_peer(&mut sessions, &replacement.pc),
            vec![key.clone()],
        );
        assert!(!sessions.contains_key(&key));

        old.pc.close().await.ok();
        replacement.pc.close().await.ok();
    }

    #[tokio::test]
    async fn test_webrtc_wait_connected_timeout() {
        let mut stream = WebRTCStream::new("", false, 100).await.unwrap();
        let err = stream.wait_connected(10).await.unwrap_err();
        assert!(err.to_string().contains("timeout"));
        stream.close().await; // see test_webrtc_new_stream: no Drop on a raw WebRTCStream
    }

    async fn connect_loopback() -> (WebRTCStream, WebRTCStream) {
        let mut offerer = WebRTCStream::new("", false, 20000).await.unwrap();
        let offer = offerer.local_endpoint().to_owned();
        let answerer = WebRTCStream::new(&offer, false, 20000).await.unwrap();
        let answer = answerer.local_endpoint().to_owned();
        offerer.set_remote_endpoint(&answer).await.unwrap();

        // Bridge trickle candidates directly between the two peers, both directions.
        let mut off_ice = offerer.take_local_ice_rx().unwrap();
        let mut ans_ice = answerer.take_local_ice_rx().unwrap();
        let answerer_for_ice = answerer.clone();
        let offerer_for_ice = offerer.clone();
        tokio::spawn(async move {
            while let Some(c) = off_ice.recv().await {
                let _ = answerer_for_ice.add_remote_ice_candidate(&c).await;
            }
        });
        tokio::spawn(async move {
            while let Some(c) = ans_ice.recv().await {
                let _ = offerer_for_ice.add_remote_ice_candidate(&c).await;
            }
        });

        offerer.wait_connected(20000).await.unwrap();
        let mut answerer = answerer;
        answerer.wait_connected(20000).await.unwrap();
        (offerer, answerer)
    }

    // next()'s teardown must not be cancellable: an awaited close that loses the select! race
    // against a consumer's timer strands the pc (is_closed already latched) and its SESSIONS
    // entry. `close_detached` is non-async, so what needs proving is that the handoff completes.
    #[tokio::test]
    async fn test_close_detached_completes_without_the_caller() {
        let (offerer, answerer) = connect_loopback().await;
        let key = format!("offer:{}", offerer.session_key());
        assert!(
            SESSIONS.lock().await.contains_key(&key),
            "offerer should be cached while live"
        );

        offerer.close_detached();
        drop(offerer);

        for _ in 0..200 {
            if !SESSIONS.lock().await.contains_key(&key) {
                answerer.close().await;
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("detached close never evicted the peer connection from SESSIONS");
    }

    // Whole messages are split out of the read buffer instead of copied, so the buffer is
    // consumed from the front and periodically re-initialized. Alternating whole and fragmented
    // messages exercises that against the accumulator path, and running well past one refill
    // exercises the refill itself — a boundary the fixed-size buffer never had.
    #[tokio::test]
    async fn test_webrtc_mixed_message_sizes_survive_scratch_refill() {
        let (mut offerer, mut answerer) = connect_loopback().await;

        // Sized either side of MAX_FRAGMENT_PAYLOAD, and enough rounds to consume several
        // SCRATCH_REFILL chunks.
        let sizes = [1usize, 64, 60_000, 60_001, 130_000, 7];
        for round in 0..40u8 {
            for &len in &sizes {
                let payload = Bytes::from(vec![round; len]);
                offerer.send_bytes(payload.clone()).await.unwrap();
                let got = timeout(Duration::from_secs(10), answerer.next())
                    .await
                    .expect("receiver starved")
                    .expect("stream ended")
                    .expect("read failed");
                assert_eq!(got.len(), len, "round {round}, len {len}");
                assert!(
                    got.iter().all(|&b| b == round),
                    "round {}, len {}: content or boundary corrupted",
                    round,
                    len
                );
            }
        }

        offerer.close().await;
        answerer.close().await;
    }

    // Owning the Stream owns the peer connection: dropping it must close the pc and evict the
    // session, without the owner having to remember to. Every exit path used to carry that
    // obligation, and the controlled side never honoured it.
    #[tokio::test]
    async fn dropping_the_stream_closes_the_peer_connection() {
        let offerer = WebRTCStream::new("", false, 20000).await.unwrap();
        let key = format!("offer:{}", offerer.session_key());
        assert!(
            SESSIONS.lock().await.contains_key(&key),
            "offerer should be cached while live"
        );

        // No explicit close anywhere: the stream simply goes out of scope, as it does on the
        // exits that forget.
        drop(crate::Stream::WebRTC(offerer));

        for _ in 0..200 {
            if !SESSIONS.lock().await.contains_key(&key) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("dropping the stream left the peer connection in SESSIONS");
    }

    // A replayed offer asking for a different ICE policy must not be handed the cached peer
    // connection. The lookup and the insert-time duplicate check read the same key, so the test
    // fails if either one stops applying `is_reusable_for` — which is how the guard was dead
    // code: the lookup rejected the entry, and the insert handed back the very same one.
    #[tokio::test]
    async fn test_cached_peer_is_not_reused_across_ice_policies() {
        let offerer = WebRTCStream::new("", false, 20000).await.unwrap();
        let offer = offerer.local_endpoint().to_owned();

        let all_ice = WebRTCStream::new(&offer, false, 20000).await.unwrap();
        assert!(!all_ice.relay_only);

        let relay_only = WebRTCStream::new(&offer, true, 20000).await.unwrap();
        assert!(
            relay_only.relay_only,
            "a Relay-only request was answered with the cached All-policy peer connection"
        );

        relay_only.close().await;
        all_ice.close().await;
        offerer.close().await;
    }

    // The local-candidate channel must close when the pc does. Its only sender lives inside the
    // on_ice_candidate handler, which close() does not clear, so without the teardown the
    // receiver stays open forever — and the forwarder loop this API asks callers to write holds
    // a stream clone while parked on recv(), keeping the pc alive past its own close. Gathering
    // complete drops the sender as well, and may beat the close here; a pc closed before that
    // has only the teardown, which is what this pins.
    #[tokio::test]
    async fn test_local_ice_channel_closes_with_the_peer_connection() {
        let offerer = WebRTCStream::new("", false, 20000).await.unwrap();
        let mut ice_rx = offerer.take_local_ice_rx().unwrap();

        // Exactly the loop every consumer writes, holding a clone of the stream.
        let forwarder_stream = offerer.clone();
        let forwarder = tokio::spawn(async move {
            while let Some(candidate) = ice_rx.recv().await {
                let _ = forwarder_stream.add_remote_ice_candidate(&candidate).await;
            }
        });

        offerer.close().await;
        drop(offerer);

        tokio::time::timeout(Duration::from_secs(20), forwarder)
            .await
            .expect("forwarder task outlived the peer connection")
            .unwrap();
    }

    // The channel must close when gathering does, with the pc still open: nothing can follow the
    // agent's last candidate, and a forwarder parked on recv() until the session ends holds its
    // signaling connection for that whole time — long after the server's idle close, as a socket
    // left in CLOSE_WAIT.
    #[tokio::test]
    async fn test_local_ice_channel_closes_when_gathering_completes() {
        let offerer = WebRTCStream::new("", false, 20000).await.unwrap();
        let mut ice_rx = offerer.take_local_ice_rx().unwrap();
        let drained = timeout(Duration::from_secs(20), async {
            while ice_rx.recv().await.is_some() {}
        })
        .await;
        // Close before asserting, or a failure here strands the pc and fails
        // `test_cancelled_new_does_not_leak_the_pc` as well.
        offerer.close().await;
        drained.expect("local ICE channel outlived gathering");
    }

    // Extra channels must not displace the bound one: the newcomer's on_open would push Open onto
    // the watch that gates send and recv, re-arming a session already latched Closed. Only the
    // bind-once guard is exercised; the ordered+reliable check cannot decide anything here.
    #[tokio::test]
    async fn test_webrtc_answerer_binds_only_the_first_data_channel() {
        use webrtc::data_channel::data_channel_init::RTCDataChannelInit;

        let mut offerer = WebRTCStream::new("", false, 20000).await.unwrap();
        // Replace the bootstrap channel's siblings: an unordered one and a duplicate.
        let unordered = offerer
            .pc
            .create_data_channel(
                "unordered",
                Some(RTCDataChannelInit {
                    ordered: Some(false),
                    ..Default::default()
                }),
            )
            .await
            .unwrap();
        let duplicate = offerer
            .pc
            .create_data_channel("duplicate", None)
            .await
            .unwrap();

        let offer = offerer.local_endpoint().to_owned();
        let answerer = WebRTCStream::new(&offer, false, 20000).await.unwrap();
        let answer = answerer.local_endpoint().to_owned();
        offerer.set_remote_endpoint(&answer).await.unwrap();

        let mut off_ice = offerer.take_local_ice_rx().unwrap();
        let mut ans_ice = answerer.take_local_ice_rx().unwrap();
        let answerer_for_ice = answerer.clone();
        let offerer_for_ice = offerer.clone();
        tokio::spawn(async move {
            while let Some(c) = off_ice.recv().await {
                let _ = answerer_for_ice.add_remote_ice_candidate(&c).await;
            }
        });
        tokio::spawn(async move {
            while let Some(c) = ans_ice.recv().await {
                let _ = offerer_for_ice.add_remote_ice_candidate(&c).await;
            }
        });

        offerer.wait_connected(20000).await.unwrap();
        let mut answerer = answerer;
        answerer.wait_connected(20000).await.unwrap();

        // The bootstrap channel still carries data: the rejected siblings did not displace it.
        let payload = Bytes::from_static(b"bootstrap still bound");
        offerer.send_bytes(payload.clone()).await.unwrap();
        let got = tokio::time::timeout(Duration::from_secs(10), answerer.next())
            .await
            .expect("answerer starved")
            .expect("stream ended")
            .expect("read failed");
        assert_eq!(&got[..], &payload[..]);

        drop(unordered);
        drop(duplicate);
        offerer.close().await;
        answerer.close().await;
    }

    // One-shot callers exchange only the endpoints and never consume `take_local_ice_rx`.
    #[tokio::test]
    async fn test_webrtc_loopback_gathered_endpoints() {
        let connect = async {
            let mut offerer = WebRTCStream::new("", false, 20000).await.unwrap();
            let offer = offerer.get_local_endpoint().await.unwrap();
            let mut answerer = WebRTCStream::new(&offer, false, 20000).await.unwrap();
            let answer = answerer.get_local_endpoint().await.unwrap();
            offerer.set_remote_endpoint(&answer).await.unwrap();

            offerer.wait_connected(20000).await.unwrap();
            answerer.wait_connected(20000).await.unwrap();
            offerer.close().await;
            answerer.close().await;
        };
        timeout(Duration::from_secs(40), connect)
            .await
            .expect("gathered-endpoint WebRTC loopback did not complete in time");
    }

    // The state mapping the health flag is built from. ICE raises `Disconnected` only on a real
    // consent failure, which an in-process loopback cannot produce, so this drives the mapping
    // directly; the loopback test below covers the wiring from the flag out to callers.
    #[test]
    fn test_note_health_maps_only_the_transient_states() {
        use crate::webrtc::RTCPeerConnectionState;
        use std::sync::atomic::{AtomicBool, Ordering};
        let flag = AtomicBool::new(false);

        WebRTCStream::note_health(&flag, RTCPeerConnectionState::Disconnected);
        assert!(flag.load(Ordering::SeqCst), "Disconnected must raise the hint");

        WebRTCStream::note_health(&flag, RTCPeerConnectionState::Connected);
        assert!(!flag.load(Ordering::SeqCst), "Connected must clear it");

        // The terminal states have their own path and must not touch the hint: a session that
        // ends has no use for one, and clearing it here would hide a peer that went silent first.
        for terminal in [
            RTCPeerConnectionState::Failed,
            RTCPeerConnectionState::Closed,
        ] {
            WebRTCStream::note_health(&flag, RTCPeerConnectionState::Disconnected);
            WebRTCStream::note_health(&flag, terminal);
            assert!(
                flag.load(Ordering::SeqCst),
                "{} must leave the hint as it found it",
                terminal
            );
        }
    }

    // Pins the wiring and the polarity of the health flag: a pair that just connected must not
    // look suspect.
    #[tokio::test]
    async fn test_webrtc_disconnected_is_false_on_a_live_pair() {
        let body = async {
            let (offerer, answerer) = connect_loopback().await;
            assert!(!offerer.is_disconnected());
            assert!(!answerer.is_disconnected());
            offerer.close().await;
            answerer.close().await;
        };
        timeout(Duration::from_secs(30), body)
            .await
            .expect("WebRTC loopback did not complete in time");
    }

    // A message far above the fragment size must show as activity while it is still arriving:
    // `next()` yields nothing until the last fragment, so a receiver watching only completed
    // messages cannot tell a peer mid-transfer from one that died.
    #[tokio::test]
    async fn test_webrtc_rx_progress_moves_before_a_large_message_completes() {
        let body = async {
            let (mut offerer, mut answerer) = connect_loopback().await;

            offerer.send_raw(b"warmup".to_vec()).await.unwrap();
            assert_eq!(&answerer.next().await.unwrap().unwrap()[..], b"warmup");
            let baseline = answerer.rx_progress();

            // Large enough that it cannot land inside one poll of the loop below, so a probe
            // that only counted whole messages would leave the counter untouched throughout.
            let big = vec![0x5Au8; 4 * 1024 * 1024];
            let expected = big.len();
            let sender = tokio::spawn(async move { offerer.send_raw(big).await.map(|_| offerer) });

            // `next_timeout` returning None leaves the partial message in place, so the counter
            // is read at a point where nothing has been delivered. No select and no sleeping
            // sibling: the assertion cannot turn on which branch a poll happened to take.
            let mut moved_early = false;
            let mut got = None;
            for _ in 0..600 {
                match answerer.next_timeout(20).await {
                    Some(msg) => {
                        got = Some(msg);
                        break;
                    }
                    None => {
                        if answerer.rx_progress() > baseline {
                            moved_early = true;
                        }
                    }
                }
            }

            assert!(
                moved_early,
                "rx_progress must advance while the message is still incomplete"
            );
            assert_eq!(
                got.expect("message never completed").unwrap().len(),
                expected
            );
            let offerer = sender.await.unwrap().expect("send");
            // `WebRTCStream` has no `Drop`, so both pcs stay in `SESSIONS` unless closed here;
            // `test_cancelled_new_does_not_leak_the_pc` scans that map and would report them.
            offerer.close().await;
            answerer.close().await;
        };
        timeout(Duration::from_secs(60), body)
            .await
            .expect("WebRTC loopback did not complete in time");
    }

    // The bound `Stream::set_max_packet_length` lowers before authorization: a message above it
    // must end the connection as it arrives, not be reassembled. Only the receiver's bound moves,
    // so the sender still puts it on the wire, exactly as an unauthenticated peer would.
    #[tokio::test]
    async fn test_webrtc_max_packet_length_bounds_reassembly() {
        let body = async {
            let (mut offerer, mut answerer) = connect_loopback().await;
            answerer.set_max_packet_length(16 * 1024);

            offerer.send_raw(vec![0xCDu8; 8 * 1024]).await.unwrap();
            let got = answerer.next().await.unwrap().unwrap();
            assert_eq!(got.len(), 8 * 1024, "a message under the bound still arrives");

            offerer.send_raw(vec![0xCDu8; 200_000]).await.unwrap();
            match timeout(Duration::from_secs(10), answerer.next()).await {
                Ok(Some(Err(e))) => assert!(
                    e.to_string().contains("maximum frame size"),
                    "unexpected error: {}",
                    e
                ),
                Ok(Some(Ok(b))) => panic!("expected a refusal, got {} bytes", b.len()),
                Ok(None) => panic!("expected a refusal, got EOF"),
                Err(_) => panic!("answerer.next() hung on an oversize message"),
            }

            offerer.close().await;
            answerer.close().await;
        };
        timeout(Duration::from_secs(60), body)
            .await
            .expect("WebRTC loopback did not complete in time");
    }

    // The other half of the same lifecycle: what was refused while the bound was low arrives once
    // it is lifted, which is what authorization does on the server.
    #[tokio::test]
    async fn test_webrtc_max_packet_length_restores() {
        let body = async {
            let (mut offerer, mut answerer) = connect_loopback().await;
            answerer.set_max_packet_length(16 * 1024);

            offerer.send_raw(vec![0xCDu8; 8 * 1024]).await.unwrap();
            let got = answerer.next().await.unwrap().unwrap();
            assert_eq!(got.len(), 8 * 1024);

            answerer.set_max_packet_length(usize::MAX);
            offerer.send_raw(vec![0xCDu8; 200_000]).await.unwrap();
            let got = timeout(Duration::from_secs(10), answerer.next())
                .await
                .expect("answerer.next() hung after the bound was lifted")
                .unwrap()
                .unwrap();
            assert_eq!(got.len(), 200_000, "lifting the bound lets a large message through again");

            offerer.close().await;
            answerer.close().await;
        };
        timeout(Duration::from_secs(60), body)
            .await
            .expect("WebRTC loopback did not complete in time");
    }

    // In-process offerer<->answerer loopback exercising the send/next data plane that the framing,
    // empty-message, and EOF fixes live in. Connects over host candidates (works offline; any
    // configured/default STUN just fails in the background without blocking the host pair).
    #[tokio::test]
    async fn test_webrtc_loopback_roundtrip() {
        let connect = async {
            let (mut offerer, mut answerer) = connect_loopback().await;

            // Host-candidate loopback is direct, never TURN-relayed.
            assert_eq!(offerer.is_relayed().await, Some(false));

            // Small message.
            offerer.send_raw(b"hello".to_vec()).await.unwrap();
            let got = answerer.next().await.unwrap().unwrap();
            assert_eq!(&got[..], b"hello");

            // Empty message: must round-trip as an empty frame, not be seen as EOF.
            offerer.send_raw(Vec::new()).await.unwrap();
            let got = answerer.next().await.unwrap().unwrap();
            assert_eq!(got.len(), 0, "empty message must not be treated as EOF");

            // Payload far above the 64KB single-message cap: must be fragmented and reassembled.
            let big = vec![0xABu8; 200_000];
            offerer.send_raw(big.clone()).await.unwrap();
            let got = answerer.next().await.unwrap().unwrap();
            assert_eq!(
                got.len(),
                big.len(),
                "large message must survive fragmentation"
            );
            assert_eq!(&got[..], &big[..]);

            // Reverse direction.
            answerer.send_raw(b"world".to_vec()).await.unwrap();
            let got = offerer.next().await.unwrap().unwrap();
            assert_eq!(&got[..], b"world");

            // Peer close: the other side observes a clean EOF (None) or a close error, never a hang.
            offerer.close().await;
            match timeout(Duration::from_secs(10), answerer.next()).await {
                Ok(None) | Ok(Some(Err(_))) => {}
                Ok(Some(Ok(b))) => panic!("expected EOF after peer close, got {} bytes", b.len()),
                Err(_) => panic!("answerer.next() hung after peer close"),
            }
            answerer.close().await;
        };
        timeout(Duration::from_secs(40), connect)
            .await
            .expect("webrtc loopback did not complete in time");
    }

    // A client session runs on its own `#[tokio::main(flavor = "current_thread")]` runtime that
    // is dropped the moment io_loop returns, so a close spawned or awaited there can be killed
    // before (or worse, after) `close()` latches `is_closed`. Pin the production sequence over
    // `Stream::WebRTC`: `close_webrtc()` from a thread with no runtime at all, then Drop; the
    // peer must still see EOF — the closer thread, not any caller runtime, delivers it.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_end_close_reaches_the_peer() {
        let (offerer, mut answerer) = connect_loopback().await;
        let stream = crate::Stream::WebRTC(offerer);
        tokio::task::spawn_blocking(move || {
            stream.close_webrtc();
            drop(stream);
        })
        .await
        .expect("session thread");

        match timeout(Duration::from_secs(5), answerer.next()).await {
            Ok(None) | Ok(Some(Err(_))) => {}
            Ok(Some(Ok(b))) => panic!("expected EOF after session close, got {} bytes", b.len()),
            Err(_) => panic!("peer never observed the session-end close"),
        }
        answerer.close().await;
    }

    // The shared count, not any one handoff, decides: with a second claim outstanding on a pc
    // no one has adopted, dropping the first must leave it open for that caller, and dropping
    // the last must close it — an unanswered offerer never reaches a terminal ICE state itself.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_last_abandoned_claim_closes_an_unadopted_pc() {
        let builder = WEBRTC_RT
            .as_ref()
            .expect("WebRTC I/O runtime")
            .spawn(WebRTCStream::new_inner(String::new(), false, 20000))
            .await
            .expect("setup task")
            .expect("offerer builds");
        let stream = builder
            .stream
            .as_ref()
            .expect("a fresh handoff carries its stream")
            .clone();
        let key = format!("offer:{}", stream.session_key());
        let hit = NewStreamHandoff::claim(stream);

        drop(builder);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            SESSIONS.lock().await.contains_key(&key),
            "a claim was still outstanding, so the pc had to stay open"
        );

        drop(hit);
        for _ in 0..200 {
            if !SESSIONS.lock().await.contains_key(&key) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("the last abandoned claim never closed the unadopted pc");
    }

    // `new_inner` caches the pc BEFORE its own `new()` returns, so a concurrent caller can hit
    // that cache and adopt the stream while the builder's handoff is still outstanding. Closing
    // on the builder's Drop then tears down a connection another caller is already using, so
    // abandonment has to be a property of the pc, not of one handoff.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_abandoned_builder_handoff_spares_an_adopted_pc() {
        // Exactly what `new()` does, minus the adoption: the setup task runs on WEBRTC_RT and
        // hands back a fresh, still-unadopted handoff.
        let builder = WEBRTC_RT
            .as_ref()
            .expect("WebRTC I/O runtime")
            .spawn(WebRTCStream::new_inner(String::new(), false, 20000))
            .await
            .expect("setup task")
            .expect("offerer builds");
        let stream = builder
            .stream
            .as_ref()
            .expect("a fresh handoff carries its stream")
            .clone();
        let key = format!("offer:{}", stream.session_key());
        assert!(
            SESSIONS.lock().await.contains_key(&key),
            "new_inner must cache the pc it built"
        );

        // The concurrent `new()`: a cache hit that takes the stream and goes on using it.
        let adopted = NewStreamHandoff::claim(stream)
            .into_inner()
            .expect("a cache hit hands the stream back");
        // ...and now the builder's `new()` is cancelled, so the runtime drops its handoff.
        drop(builder);

        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            SESSIONS.lock().await.contains_key(&key),
            "abandoning the builder's handoff closed a pc another caller had already adopted"
        );
        adopted.close_detached();
    }

    // End-to-end: cancel `new()` itself after its first poll (the spawn is already in flight)
    // and every session it transiently created must be closed and evicted again.
    //
    // The cancelled attempt's own key is unknowable here — its DTLS fingerprint is generated
    // inside the task that was abandoned — so what is watched for is a key that OUTLASTS the
    // window, not an instant with no new keys at all. Concurrent tests each close what they
    // create, so their keys come and go and drop out of the running intersection; a leaked pc
    // never does. Waiting for an empty difference instead made this test depend on the suite
    // having an idle moment, which `--test-threads=2` never gives it: one lane is this test for
    // the whole wait while the other keeps starting sessions.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_cancelled_new_does_not_leak_the_pc() {
        use std::collections::HashSet;
        let before: HashSet<String> = SESSIONS.lock().await.keys().cloned().collect();
        // Zero timeout: polls the future exactly once (spawning new_inner), then cancels it —
        // usually. The setup task runs on its own runtime and can finish inside that single poll,
        // and then `new()` hands back a live stream instead. `WebRTCStream` has no `Drop`, so
        // discarding that one is itself a leak, and this test would go on to report it as the
        // cancelled attempt's. Close what comes back and try for a real cancellation. Fewer test
        // threads make the setup task likelier to win, which is why this surfaced under
        // `--test-threads=2` and not at the default.
        let mut cancelled = false;
        for _ in 0..20 {
            match timeout(Duration::ZERO, WebRTCStream::new("", false, 20000)).await {
                Err(_) => {
                    cancelled = true;
                    break;
                }
                Ok(Ok(stream)) => stream.close().await,
                Ok(Err(_)) => {}
            }
        }
        assert!(cancelled, "new() never lost the race with its own cancellation");
        // Let the detached setup task finish (and insert its session) before sampling, or the
        // first sample is taken before the leak has formed and every later one intersects to
        // nothing.
        tokio::time::sleep(Duration::from_millis(500)).await;
        const ATTEMPTS: usize = 600; // 30s
        let mut persisted: Option<HashSet<String>> = None;
        for attempt in 1..=ATTEMPTS {
            let now: HashSet<String> = SESSIONS.lock().await.keys().cloned().collect();
            let new_keys: HashSet<String> = now.difference(&before).cloned().collect();
            persisted = Some(match persisted {
                None => new_keys,
                Some(prev) => prev.intersection(&new_keys).cloned().collect(),
            });
            let persisted_keys = persisted.as_ref().map_or(0, HashSet::len);
            if persisted_keys == 0 {
                return;
            }
            // Positional, not an inline `{persisted:?}`: on edition 2018 a lone-literal `panic!`
            // does not go through format_args and would print the placeholder verbatim.
            assert!(
                attempt < ATTEMPTS,
                "cancelled new() left a pc cached in SESSIONS (persisted: {:?})",
                persisted
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    // The production shape of a controller session end: the offerer was created on a session's
    // own current-thread runtime, the session queues its detached close and the runtime is
    // destroyed immediately after (io_loop returning drops it). The pc's sockets and its
    // ICE/DTLS/SCTP pump tasks must not die with that runtime, or the queued close completes
    // without ever putting close_notify on the wire and the peer waits out ICE decay. Gathered
    // (non-trickle) endpoints keep the cross-runtime signaling to two string handoffs.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_close_survives_creator_runtime_destruction() {
        // tokio oneshots for every handoff: their sends are synchronous, and the session side
        // awaits them INSIDE block_on — a current_thread runtime only drives its tasks while
        // being block_on-driven, and the offerer's SCTP/ICE pumps must stay live until the
        // answerer has confirmed the channel is open.
        let (offer_tx, offer_rx) = tokio::sync::oneshot::channel::<String>();
        let (answer_tx, answer_rx) = tokio::sync::oneshot::channel::<String>();
        let (go_tx, go_rx) = tokio::sync::oneshot::channel::<()>();
        let session = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("session runtime");
            let offerer = rt.block_on(async {
                let mut offerer = WebRTCStream::new("", false, 20000).await.unwrap();
                offer_tx
                    .send(offerer.get_local_endpoint().await.unwrap())
                    .unwrap();
                let answer = answer_rx.await.unwrap();
                offerer.set_remote_endpoint(&answer).await.unwrap();
                offerer.wait_connected(20000).await.unwrap();
                // Keep driving the runtime until the answerer confirms open, so the EOF
                // assertion — not connection setup — is what discriminates.
                go_rx.await.unwrap();
                offerer
            });
            // The session ends: queue the close, then destroy the runtime the pc was created
            // on — exactly what io_loop returning does.
            drop(crate::Stream::WebRTC(offerer));
            drop(rt);
        });

        let offer = offer_rx.await.expect("offer from session thread");
        let mut answerer = WebRTCStream::new(&offer, false, 20000).await.unwrap();
        answer_tx
            .send(answerer.get_local_endpoint().await.unwrap())
            .expect("answer to session thread");
        answerer.wait_connected(20000).await.unwrap();
        go_tx.send(()).expect("release session thread");
        session.join().expect("session thread");

        match timeout(Duration::from_secs(5), answerer.next()).await {
            Ok(None) | Ok(Some(Err(_))) => {}
            Ok(Some(Ok(b))) => panic!("expected EOF after session close, got {} bytes", b.len()),
            Err(_) => panic!("peer never observed the close after the creator runtime died"),
        }
        answerer.close().await;
    }

    // The peer-initiated end: `next()` hits EOF and fires its own `close_detached` before the
    // session's final close (here via `Stream`'s Drop) is issued. Both land on the serialized
    // closer thread, so the first runs to completion and the second is a benign no-op — the pc
    // must end up closed and evicted, never stranded by one closer cancelling the other.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_eof_close_then_drop_still_evicts() {
        let (mut offerer, answerer) = connect_loopback().await;
        let key = format!("offer:{}", offerer.session_key());
        answerer.close().await;
        match timeout(Duration::from_secs(5), offerer.next()).await {
            Ok(None) | Ok(Some(Err(_))) => {} // EOF path fired close_detached internally
            Ok(Some(Ok(b))) => panic!("expected EOF after peer close, got {} bytes", b.len()),
            Err(_) => panic!("offerer.next() hung after peer close"),
        }
        drop(crate::Stream::WebRTC(offerer)); // second close via Drop

        for _ in 0..200 {
            if !SESSIONS.lock().await.contains_key(&key) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("pc still cached after EOF close + Drop close");
    }

    // Both framing violations must end the stream. The empty-FRAG_MORE case is the one that
    // cannot be caught downstream: it adds nothing to the accumulator, so the MAX_FRAME_LENGTH
    // cap never trips and `next()` would otherwise spin for as long as the peer keeps writing.
    // The bad frame is injected mid-message so the branch's `acc` reset is exercised too.
    #[tokio::test]
    async fn test_webrtc_rejects_bad_fragment_framing() {
        // (raw frame, expected substring)
        let cases: [(&[u8], &str); 2] = [
            (&[0x2A, b'x'], "fragment header 42"),
            (&[FRAG_MORE], "carries no payload"),
        ];
        for (frame, want) in cases {
            let connect = async {
                let (offerer, mut answerer) = connect_loopback().await;
                let dc = offerer.detached_dc().await.unwrap();

                // Leave a partial message in the accumulator first, so the rejection has
                // something to discard. `send_bytes` only ever emits valid headers, so the bad
                // frame itself has to be written through the raw channel below it.
                let mut lead = BytesMut::with_capacity(1 + 8);
                lead.put_u8(FRAG_MORE);
                lead.put_slice(b"leading!");
                dc.write(&lead.freeze()).await.unwrap();
                dc.write(&bytes::Bytes::copy_from_slice(frame))
                    .await
                    .unwrap();

                let err = answerer
                    .next()
                    .await
                    .expect("bad framing must surface as an error, not EOF")
                    .expect_err("bad framing must be rejected");
                assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
                assert!(err.to_string().contains(want), "unexpected error: {}", err);

                offerer.close().await;
                answerer.close().await;
            };
            timeout(Duration::from_secs(40), connect)
                .await
                .expect("webrtc loopback did not complete in time");
        }
    }

    #[tokio::test]
    async fn test_webrtc_concurrent_large_sends_preserve_boundaries() {
        let connect = async {
            let (offerer, mut answerer) = connect_loopback().await;
            let mut sender_a = offerer.clone();
            let mut sender_b = offerer.clone();
            let expected_a = vec![0xAA; 200_000];
            let expected_b = vec![0xBB; 200_000];
            let payload_a = expected_a.clone();
            let payload_b = expected_b.clone();
            let barrier = Arc::new(Barrier::new(3));

            let barrier_a = barrier.clone();
            let send_a = tokio::spawn(async move {
                barrier_a.wait().await;
                sender_a.send_raw(payload_a).await
            });
            let barrier_b = barrier.clone();
            let send_b = tokio::spawn(async move {
                barrier_b.wait().await;
                sender_b.send_raw(payload_b).await
            });

            barrier.wait().await;
            let receive = async {
                let first = answerer.next().await.unwrap().unwrap();
                let second = answerer.next().await.unwrap().unwrap();
                (first, second)
            };
            let (send_a, send_b, (first, second)) = tokio::join!(send_a, send_b, receive);
            send_a.unwrap().unwrap();
            send_b.unwrap().unwrap();

            let boundaries_preserved = (first.as_ref() == expected_a.as_slice()
                && second.as_ref() == expected_b.as_slice())
                || (first.as_ref() == expected_b.as_slice()
                    && second.as_ref() == expected_a.as_slice());
            assert!(boundaries_preserved, "concurrent messages were interleaved");

            offerer.close().await;
            answerer.close().await;
        };
        timeout(Duration::from_secs(40), connect)
            .await
            .expect("concurrent WebRTC sends did not complete in time");
    }
}
