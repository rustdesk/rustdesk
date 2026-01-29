use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice::mdns::MulticastDnsMode;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::policy::ice_transport_policy::RTCIceTransportPolicy;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bytes::{Bytes, BytesMut};
use tokio::sync::watch;
use tokio::sync::Mutex;
use tokio::time::timeout;
use url::Url;

use crate::config;
use crate::protobuf::Message;
use crate::sodiumoxide::crypto::secretbox::Key;
use crate::ResultType;

pub struct WebRTCStream {
    pc: Arc<RTCPeerConnection>,
    stream: Arc<Mutex<Arc<RTCDataChannel>>>,
    state_notify: watch::Receiver<bool>,
    send_timeout: u64,
}

/// Standard maximum message size for WebRTC data channels (RFC 8831, 65535 bytes).
/// Most browsers, including Chromium, enforce this protocol limit.
const DATA_CHANNEL_BUFFER_SIZE: u16 = u16::MAX;

// use 3 public STUN servers to find out the NAT type, 2 must be the same address but different ports
// https://stackoverflow.com/questions/72805316/determine-nat-mapping-behaviour-using-two-stun-servers
// luckily nextcloud supports two ports for STUN
// unluckily webrtc-rs does not use the same port to do the STUN request
static DEFAULT_ICE_SERVERS: [&str; 3] = [
    "stun:stun.cloudflare.com:3478",
    "stun:stun.nextcloud.com:3478",
    "stun:stun.nextcloud.com:443",
];

lazy_static::lazy_static! {
    static ref SESSIONS: Arc::<Mutex<HashMap<String, WebRTCStream>>> = Default::default();
}

impl Clone for WebRTCStream {
    fn clone(&self) -> Self {
        WebRTCStream {
            pc: self.pc.clone(),
            stream: self.stream.clone(),
            state_notify: self.state_notify.clone(),
            send_timeout: self.send_timeout,
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

    #[inline]
    fn get_key_for_sdp_json(sdp_json: &str) -> ResultType<String> {
        if sdp_json.is_empty() {
            return Ok("".to_string());
        }
        let sdp = serde_json::from_str::<RTCSessionDescription>(&sdp_json)?;
        Self::get_key_for_sdp(&sdp)
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
        // standard url format with turn scheme: turn://user:pass@host:port
        match Url::parse(url) {
            Ok(u) => {
                if u.scheme() == "turn"
                    || u.scheme() == "turns"
                    || u.scheme() == "stun"
                    || u.scheme() == "stuns"
                {
                    Some(RTCIceServer {
                        urls: vec![format!(
                            "{}:{}:{}",
                            u.scheme(),
                            u.host_str().unwrap_or_default(),
                            u.port().unwrap_or(3478)
                        )],
                        username: u.username().to_string(),
                        credential: u.password().unwrap_or_default().to_string(),
                        ..Default::default()
                    })
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    #[inline]
    fn get_ice_servers() -> Vec<RTCIceServer> {
        let mut ice_servers = Vec::new();
        let cfg = config::Config::get_option(config::keys::OPTION_ICE_SERVERS);

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

    pub async fn new(
        remote_endpoint: &str,
        force_relay: bool,
        ms_timeout: u64,
    ) -> ResultType<Self> {
        log::debug!("New webrtc stream to endpoint: {}", remote_endpoint);
        let remote_offer = if remote_endpoint.is_empty() {
            "".into()
        } else {
            Self::get_remote_offer(remote_endpoint)?
        };

        let mut key = Self::get_key_for_sdp_json(&remote_offer)?;
        let sessions_lock = SESSIONS.lock().await;
        if let Some(cached_stream) = sessions_lock.get(&key) {
            if !key.is_empty() {
                log::debug!("Start webrtc with cached peer");
                return Ok(cached_stream.clone());
            }
        }
        drop(sessions_lock);

        let start_local_offer = remote_offer.is_empty();
        // Create a SettingEngine and enable Detach
        let mut s = SettingEngine::default();
        s.detach_data_channels();
        s.set_ice_multicast_dns_mode(MulticastDnsMode::Disabled);

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

        let (notify_tx, notify_rx) = watch::channel(false);
        // Create a new RTCPeerConnection
        let pc = Arc::new(api.new_peer_connection(config).await?);
        let bootstrap_dc = if start_local_offer {
            let dc_open_notify = notify_tx.clone();
            // Create a data channel with label "bootstrap"
            let dc = pc.create_data_channel("bootstrap", None).await?;
            dc.on_open(Box::new(move || {
                log::debug!("Local data channel bootstrap open.");
                let _ = dc_open_notify.send(true);
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
            pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
                let d_label = dc.label().to_owned();
                let dc_open_notify2 = dc_open_notify.clone();
                let stream_for_dc_clone = stream_for_dc.clone();
                log::debug!("Remote data channel {} ready", d_label);
                Box::pin(async move {
                    let mut stream_lock = stream_for_dc_clone.lock().await;
                    *stream_lock = dc.clone();
                    drop(stream_lock);
                    dc.on_open(Box::new(move || {
                        let _ = dc_open_notify2.send(true);
                        Box::pin(async {})
                    }));
                })
            }));
        }

        // This will notify you when the peer has connected/disconnected
        let stream_for_close = stream.clone();
        let pc_for_close = pc.clone();
        pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            let stream_for_close2 = stream_for_close.clone();
            let on_connection_notify = notify_tx.clone();
            let pc_for_close2 = pc_for_close.clone();
            Box::pin(async move {
                log::debug!("WebRTC session peer connection state: {}", s);
                match s {
                    RTCPeerConnectionState::Disconnected
                    | RTCPeerConnectionState::Failed
                    | RTCPeerConnectionState::Closed => {
                        let _ = on_connection_notify.send(true);
                        log::debug!("WebRTC session closing due to disconnected");
                        let _ = stream_for_close2.lock().await.close().await;
                        log::debug!("WebRTC session stream closed");

                        let mut sessions_lock = SESSIONS.lock().await;
                        match Self::get_key_for_peer(&pc_for_close2, start_local_offer).await {
                            Ok(k) => {
                                sessions_lock.remove(&k);
                                log::debug!("WebRTC session removed key: {}", k);
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to extract key for peer during session cleanup: {:?}",
                                    e
                                );
                                // Fallback: try to remove any session associated with this peer connection
                                let keys_to_remove: Vec<String> = sessions_lock
                                    .iter()
                                    .filter_map(|(key, session)| {
                                        if Arc::ptr_eq(&session.pc, &pc_for_close2) {
                                            Some(key.clone())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                for k in keys_to_remove {
                                    sessions_lock.remove(&k);
                                    log::debug!("WebRTC session removed by fallback key: {}", k);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            })
        }));

        // process offer/answer
        if start_local_offer {
            let sdp = pc.create_offer(None).await?;
            let mut gather_complete = pc.gathering_complete_promise().await;
            pc.set_local_description(sdp.clone()).await?;
            let _ = gather_complete.recv().await;

            log::debug!("local offer:\n{}", sdp.sdp);
            // get local sdp key
            key = Self::get_key_for_sdp(&sdp)?;
            log::debug!("Start webrtc with local key: {}", key);
        } else {
            let sdp = serde_json::from_str::<RTCSessionDescription>(&remote_offer)?;
            pc.set_remote_description(sdp.clone()).await?;
            let answer = pc.create_answer(None).await?;
            let mut gather_complete = pc.gathering_complete_promise().await;
            pc.set_local_description(answer).await?;
            let _ = gather_complete.recv().await;

            log::debug!("remote offer:\n{}", sdp.sdp);
            // get remote sdp key
            key = Self::get_key_for_sdp(&sdp)?;
            log::debug!("Start webrtc with remote key: {}", key);
        }

        let mut final_lock = SESSIONS.lock().await;
        if let Some(session) = final_lock.get(&key) {
            pc.close().await.ok();
            return Ok(session.clone());
        }

        let webrtc_stream = Self {
            pc,
            stream,
            state_notify: notify_rx,
            send_timeout: ms_timeout,
        };
        final_lock.insert(key, webrtc_stream.clone());
        Ok(webrtc_stream)
    }

    #[inline]
    pub async fn get_local_endpoint(&self) -> ResultType<String> {
        if let Some(local_desc) = self.pc.local_description().await {
            let sdp = serde_json::to_string(&local_desc)?;
            let endpoint = Self::sdp_to_endpoint(&sdp);
            Ok(endpoint)
        } else {
            Err(anyhow::anyhow!("Local desc is not set"))
        }
    }

    #[inline]
    pub async fn set_remote_endpoint(&self, endpoint: &str) -> ResultType<()> {
        let offer = Self::get_remote_offer(endpoint)?;
        log::debug!("WebRTC set remote sdp: {}", offer);
        let sdp = serde_json::from_str::<RTCSessionDescription>(&offer)?;
        self.pc.set_remote_description(sdp).await?;
        Ok(())
    }

    #[inline]
    pub fn set_raw(&mut self) {
        // not-supported
    }

    #[inline]
    pub fn local_addr(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
    }

    #[inline]
    pub fn set_send_timeout(&mut self, ms: u64) {
        self.send_timeout = ms;
    }

    #[inline]
    pub fn set_key(&mut self, _key: Key) {
        // not-supported
        // WebRTC uses built-in DTLS encryption for secure communication.
        // DTLS handles key exchange and encryption automatically, so explicit key management is not required.
    }

    #[inline]
    pub fn is_secured(&self) -> bool {
        true
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
    async fn wait_for_connect_result(&mut self) {
        if *self.state_notify.borrow() {
            return;
        }
        let _ = self.state_notify.changed().await;
    }

    pub async fn send_bytes(&mut self, bytes: Bytes) -> ResultType<()> {
        if self.send_timeout > 0 {
            match timeout(
                Duration::from_millis(self.send_timeout),
                self.wait_for_connect_result(),
            )
            .await
            {
                Ok(_) => {}
                Err(_) => {
                    self.pc.close().await.ok();
                    return Err(Error::new(
                        ErrorKind::TimedOut,
                        "WebRTC send wait for connect timeout",
                    )
                    .into());
                }
            }
        } else {
            self.wait_for_connect_result().await;
        }
        let stream = self.stream.lock().await.clone();
        stream.send(&bytes).await?;
        Ok(())
    }

    #[inline]
    pub async fn next(&mut self) -> Option<Result<BytesMut, Error>> {
        self.wait_for_connect_result().await;
        let stream = self.stream.lock().await.clone();

        // TODO reuse buffer?
        let mut buffer = BytesMut::zeroed(DATA_CHANNEL_BUFFER_SIZE as usize);
        let dc = stream.detach().await.ok()?;
        let n = match dc.read(&mut buffer).await {
            Ok(n) => n,
            Err(err) => {
                self.pc.close().await.ok();
                return Some(Err(Error::new(
                    ErrorKind::Other,
                    format!("data channel read error: {}", err),
                )));
            }
        };
        if n == 0 {
            self.pc.close().await.ok();
            return Some(Err(Error::new(
                ErrorKind::Other,
                "data channel read exited with 0 bytes",
            )));
        }
        buffer.truncate(n);
        Some(Ok(buffer))
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
    use crate::config;
    use crate::webrtc::WebRTCStream;
    use crate::webrtc::DEFAULT_ICE_SERVERS;
    use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

    #[test]
    fn test_webrtc_ice_url() {
        assert_eq!(
            WebRTCStream::get_ice_server_from_url("turn://example.com:3478")
                .unwrap_or_default()
                .urls[0],
            "turn:example.com:3478"
        );

        assert_eq!(
            WebRTCStream::get_ice_server_from_url("turn://example.com")
                .unwrap_or_default()
                .urls[0],
            "turn:example.com:3478"
        );

        assert_eq!(
            WebRTCStream::get_ice_server_from_url("turn://123@example.com")
                .unwrap_or_default()
                .username,
            "123"
        );

        assert_eq!(
            WebRTCStream::get_ice_server_from_url("turn://123@example.com")
                .unwrap_or_default()
                .credential,
            ""
        );

        assert_eq!(
            WebRTCStream::get_ice_server_from_url("turn://123:321@example.com")
                .unwrap_or_default()
                .credential,
            "321"
        );

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

        config::Config::set_option("ice-servers".to_string(), "".to_string());
        assert_eq!(
            WebRTCStream::get_ice_servers()[0].urls[0],
            DEFAULT_ICE_SERVERS[0].to_string()
        );

        config::Config::set_option(
            "ice-servers".to_string(),
            ",stun://example.com,turn://example.com,sdf".to_string(),
        );
        assert_eq!(
            WebRTCStream::get_ice_servers()[0].urls[0],
            "stun:example.com:3478"
        );
        assert_eq!(
            WebRTCStream::get_ice_servers()[1].urls[0],
            "turn:example.com:3478"
        );
        assert_eq!(WebRTCStream::get_ice_servers().len(), 2);
        config::Config::set_option(
            "ice-servers".to_string(),
            "".to_string(),
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

        assert!(
            WebRTCStream::new("", false, 10000).await.is_ok(),
            "local webrtc endpoint should ok"
        );

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
}
