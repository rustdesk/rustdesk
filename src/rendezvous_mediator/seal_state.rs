//! Which registration datagrams go sealed, and which received ones are acted on.
//!
//! A socket starts out probing: every registration goes sealed. Two unanswered in a row and the
//! server is taken for one without the seal: registrations go plaintext, and every fourth one
//! is sealed to notice an upgrade. A sealed datagram that opens makes the socket sealed, where
//! nothing goes plaintext and a plaintext answer to a registration is dropped as forged. A
//! missed answer while sealed is retried both under the session and with a hello, since a server
//! restart retires every session; the session stays for pushes until a welcome replaces it. Such
//! a socket never falls back: its server seals. Only a new socket, which may reach another
//! server, probes afresh. A server past its hello budget answers with a cookie, which only a
//! server that seals sends: the hello goes again with it and the socket is sealed.

use super::{Sink, TargetAddr};
use hbb_common::{
    anyhow::bail,
    bytes::BytesMut,
    config::{Config, RS_PUB_KEY},
    protobuf::Message as _,
    rendezvous_proto::{rendezvous_message, RendezvousMessage},
    sodiumoxide::crypto::sign,
    udp::FramedSocket,
    udp_seal::{self, ClientSeal, Cookie, OneShot},
    ResultType,
};
use std::sync::Mutex;

const PROBE_TRIES: u8 = 2;
const PROBE_EVERY: u8 = 4;

// The hosts whose registration is sealed now, their keys, cookies and sessions: a one-off datagram
// to one of them may go sealed too. Only a socket with a session or a cookie adds its host, so an
// older server never gets one.
static SEALED: Mutex<Vec<(String, sign::PublicKey, Option<Cookie>, Option<OneShot>)>> =
    Mutex::new(Vec::new());

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Probing(u8),
    Fallback(u8),
    Sealed,
    Resealing,
}

impl Mode {
    /// A missed answer is no sign the server stopped sealing.
    fn seals(self) -> bool {
        matches!(self, Mode::Sealed | Mode::Resealing)
    }
}

pub(super) struct SealState {
    host: String,
    server: Option<sign::PublicKey>,
    seal: Option<ClientSeal>,
    mode: Mode,
}

/// The device's signing key; the server has its public half from RegisterPk.
pub(super) fn device_key() -> Option<sign::SecretKey> {
    sign::SecretKey::from_slice(&Config::get_key_pair().0)
}

impl SealState {
    /// Without a key for this server nothing is sealed and every path runs as before. The public
    /// server's key stands in for an empty setting, and it is no key for any other server.
    pub(super) fn new(
        host: &str,
        key: &str,
        public_server: bool,
        device: Option<sign::SecretKey>,
    ) -> Self {
        let server = if key == RS_PUB_KEY && !public_server {
            None
        } else {
            crate::common::get_rs_pk(key)
        };
        Self {
            host: host.to_owned(),
            server,
            seal: server.and_then(|pk| ClientSeal::new(&pk, device)),
            mode: Mode::Probing(0),
        }
    }

    /// Before a register round sends; `timed_out` is whether the previous one went unanswered.
    pub(super) fn on_round(&mut self, timed_out: bool) {
        if self.seal.is_none() {
            return;
        }
        let mode = match self.mode {
            Mode::Probing(n) if timed_out && n + 1 >= PROBE_TRIES => Mode::Fallback(0),
            Mode::Probing(n) if timed_out => Mode::Probing(n + 1),
            Mode::Sealed if timed_out => Mode::Resealing,
            Mode::Fallback(r) => Mode::Fallback((r + 1) % PROBE_EVERY),
            mode => mode,
        };
        self.set_mode(mode);
    }

    /// A new socket may be a new path, so it is probed again.
    pub(super) fn on_rebind(&mut self) {
        if let Some(seal) = self.seal.as_mut() {
            seal.reset();
        }
        self.set_mode(Mode::Probing(0));
    }

    pub(super) fn round_sink<'a>(
        &'a mut self,
        socket: &'a mut FramedSocket,
        addr: &'a TargetAddr<'a>,
    ) -> Sink<'a> {
        let sealed = !matches!(self.mode, Mode::Fallback(r) if r != PROBE_EVERY - 1);
        match self.seal.as_mut() {
            Some(seal) if sealed => Sink::Sealed(socket, addr, seal, self.mode == Mode::Resealing),
            _ => Sink::Framed(socket, addr),
        }
    }

    /// A reply goes the way the datagram it answers came.
    pub(super) fn reply_sink<'a>(
        &'a mut self,
        socket: &'a mut FramedSocket,
        addr: &'a TargetAddr<'a>,
        sealed: bool,
    ) -> Sink<'a> {
        match self.seal.as_mut() {
            Some(seal) if sealed => Sink::Sealed(socket, addr, seal, false),
            _ => Sink::Framed(socket, addr),
        }
    }

    /// The bytes to parse and whether they came sealed, or `None` to drop the datagram.
    pub(super) fn open(&mut self, bytes: BytesMut) -> Option<(BytesMut, bool)> {
        let seal = match self.seal.as_mut() {
            Some(seal) if udp_seal::is_sealed(&bytes) => seal,
            _ => return Some((bytes, false)),
        };
        let payload = seal.open(&bytes)?;
        self.set_mode(Mode::Sealed);
        // A welcome may have brought a new session.
        self.publish();
        Some((BytesMut::from(&payload[..]), true))
    }

    /// Whether `bytes` was a cookie reply; one to the latest hello sends that hello again with it.
    pub(super) async fn on_cookie(
        &mut self,
        bytes: &[u8],
        socket: &mut FramedSocket,
        addr: &TargetAddr<'_>,
    ) -> ResultType<bool> {
        let Some(seal) = self.seal.as_mut().filter(|_| udp_seal::is_cookie(bytes)) else {
            return Ok(false);
        };
        if let Some(again) = seal.on_cookie(bytes) {
            let cookie = seal.cookie();
            socket.send_bytes(again.into(), addr.to_owned()).await?;
            self.set_mode(Mode::Sealed);
            if let Some(entry) = SEALED.lock().unwrap().iter_mut().find(|e| e.0 == self.host) {
                entry.2 = cookie;
            }
        }
        Ok(true)
    }

    /// Whether to act on a parsed message. Unless the socket fell back, every registration went
    /// sealed, so a plaintext answer to one is forged; once sealed, so is a plaintext push from a
    /// server that seals every push.
    pub(super) fn on_message(
        &mut self,
        sealed: bool,
        msg: &Option<rendezvous_message::Union>,
    ) -> bool {
        if sealed {
            return true;
        }
        let Some(seal) = self.seal.as_ref() else {
            return true;
        };
        match (self.mode, msg) {
            (Mode::Fallback(_), _) => true,
            (
                _,
                Some(rendezvous_message::Union::RegisterPeerResponse(_))
                | Some(rendezvous_message::Union::RegisterPkResponse(_)),
            ) => false,
            (Mode::Probing(_), _) => true,
            (Mode::Sealed | Mode::Resealing, _) => !seal.pushes_sealed(),
        }
    }

    fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        let was_sealed = self.mode.seals();
        self.mode = mode;
        match self.server {
            Some(_) if mode == Mode::Sealed => self.publish(),
            _ if was_sealed && !mode.seals() => self.forget_sealed(),
            _ => {}
        }
    }

    fn publish(&self) {
        let (Some(server), Some(seal)) = (self.server, self.seal.as_ref()) else {
            return;
        };
        let mut sealed = SEALED.lock().unwrap();
        sealed.retain(|e| e.0 != self.host);
        sealed.push((self.host.clone(), server, seal.cookie(), seal.one_shot()));
    }

    fn forget_sealed(&self) {
        SEALED.lock().unwrap().retain(|e| e.0 != self.host);
    }
}

impl Drop for SealState {
    fn drop(&mut self) {
        if self.mode.seals() {
            self.forget_sealed();
        }
    }
}

/// The datagram a registration goes out as: data under the session, where it carries the
/// device's proof, or with no session yet an empty hello, the registration following its welcome.
/// `hello` asks for one even under a session, as after a missed answer. A RegisterPk with a key
/// other than the one proofs are signed by means the key pair changed: `device` loads the new
/// one, and is called for nothing else.
fn seal_message(
    seal: &mut ClientSeal,
    msg: &RendezvousMessage,
    hello: bool,
    device: impl Fn() -> Option<sign::SecretKey>,
) -> ResultType<Vec<u8>> {
    let pk = &msg.register_pk().pk;
    if !pk.is_empty() && !seal.signs_as(pk) {
        seal.set_device(device().filter(|sk| sk.0[32..] == pk[..]));
    }
    let sealed = if hello || !seal.has_session() {
        seal.hello(&[])
    } else {
        seal.seal(&msg.write_to_bytes()?)
    };
    match sealed {
        Some(bytes) => Ok(bytes),
        None => bail!("failed to seal a datagram to the rendezvous server"),
    }
}

/// The datagrams a round's registration goes out as: under the session, and with `rehello` a
/// hello too, which a server that restarted needs and one that only lost an answer does not.
pub(super) fn seal_round(
    seal: &mut ClientSeal,
    msg: &RendezvousMessage,
    rehello: bool,
    device: impl Fn() -> Option<sign::SecretKey>,
) -> ResultType<Vec<Vec<u8>>> {
    let mut datagrams = vec![seal_message(seal, msg, false, &device)?];
    if rehello && !udp_seal::is_hello(&datagrams[0]) {
        datagrams.push(seal_message(seal, msg, true, &device)?);
    }
    Ok(datagrams)
}

/// A one-off datagram to `host`, the PunchHoleSent of a UDP punch, goes sealed only while that
/// host's registration is: an older server would drop it, and the punch with it. It goes under the
/// registration's session; with none yet, as right after a cookie, it goes as a hello, 0-RTT data
/// without forward secrecy (RFC 8446 §2.3).
pub(super) fn seal_one_shot(host: &str, data: Vec<u8>) -> Vec<u8> {
    let server = SEALED
        .lock()
        .unwrap()
        .iter()
        .find(|e| e.0 == host)
        .map(|e| (e.1, e.2, e.3.clone()));
    let sealed = server.and_then(|(pk, cookie, one_shot)| match one_shot {
        Some(one_shot) => one_shot.seal(&data),
        None => {
            let mut seal = ClientSeal::new(&pk, None)?;
            seal.set_cookie(cookie);
            seal.seal(&data)
        }
    });
    match sealed {
        Some(sealed) => sealed,
        None => data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::{
        rendezvous_proto::{RegisterPeer, RegisterPk},
        tokio,
        udp_seal::{Reply, ServerKey, CAP_PUSHES_SEALED},
    };

    // Each test names its own host: the sealed host is process-wide and tests run in parallel.
    fn state(host: &str) -> (SealState, ServerKey) {
        let (pk, sk) = sign::gen_keypair();
        let device = Some(sign::gen_keypair().1);
        (
            SealState::new(host, &crate::encode64(pk.0), false, device),
            ServerKey::new(&sk, [1u8; 32]).unwrap(),
        )
    }

    fn welcomed(state: &mut SealState, server: &ServerKey, caps: u8) {
        let hello = state.seal.as_mut().unwrap().seal(b"").unwrap();
        let (_, mut reply) = server.open(&hello, || 1).unwrap();
        let welcome = server.seal(&mut reply, caps, b"", || 0).unwrap();
        assert!(state.open(BytesMut::from(&welcome[..])).unwrap().1);
    }

    async fn rounds(state: &mut SealState, timeouts: &[bool]) -> Vec<bool> {
        let mut socket = FramedSocket::new("127.0.0.1:0").await.unwrap();
        let addr = TargetAddr::Ip("127.0.0.1:9".parse().unwrap());
        let mut sealed = Vec::new();
        for &timed_out in timeouts {
            state.on_round(timed_out);
            sealed.push(matches!(
                state.round_sink(&mut socket, &addr),
                Sink::Sealed(..)
            ));
        }
        sealed
    }

    fn message(set: impl FnOnce(&mut RendezvousMessage)) -> RendezvousMessage {
        let mut msg = RendezvousMessage::new();
        set(&mut msg);
        msg
    }

    #[tokio::test]
    async fn two_unanswered_hellos_fall_back_and_every_fourth_round_probes() {
        let (mut state, _) = state("fall-back");
        let timeouts = [false, true, true, false, false, false, false, false];
        assert_eq!(
            rounds(&mut state, &timeouts).await,
            [true, true, false, false, false, true, false, false]
        );
    }

    #[tokio::test]
    async fn without_a_key_for_this_server_nothing_is_sealed() {
        for key in ["", RS_PUB_KEY] {
            let mut state = SealState::new("no-key", key, false, None);
            assert_eq!(rounds(&mut state, &[false, true, true]).await, [false; 3]);
            let bytes = BytesMut::from(&udp_seal::MAGIC[..]);
            assert_eq!(state.open(bytes.clone()), Some((bytes, false)));
        }
        let mut public = SealState::new("public", RS_PUB_KEY, true, None);
        assert_eq!(rounds(&mut public, &[false]).await, [true]);
    }

    #[test]
    fn once_sealed_plaintext_answers_are_dropped_and_pushes_follow_the_server() {
        for caps in [0, CAP_PUSHES_SEALED] {
            let (mut state, server) = state("answers");
            let answer = Some(rendezvous_message::Union::RegisterPkResponse(
                Default::default(),
            ));
            let push = Some(rendezvous_message::Union::PunchHole(Default::default()));
            welcomed(&mut state, &server, caps);
            assert!(state.on_message(true, &answer));
            assert!(!state.on_message(false, &answer));
            assert_eq!(state.on_message(false, &push), caps == 0);
        }
    }

    #[tokio::test]
    async fn while_probing_plaintext_answers_are_forged_but_fallback_takes_them() {
        let (mut state, _) = state("probing");
        let answer = Some(rendezvous_message::Union::RegisterPkResponse(
            Default::default(),
        ));
        let push = Some(rendezvous_message::Union::PunchHole(Default::default()));
        assert!(!state.on_message(false, &answer));
        assert!(state.on_message(false, &push));
        rounds(&mut state, &[false, true, true]).await;
        assert!(
            state.on_message(false, &answer),
            "an older server answers in plaintext"
        );
        let mut off = SealState::new("probing-off", "", false, None);
        assert!(
            off.on_message(false, &answer),
            "without a key nothing changes"
        );
    }

    #[test]
    fn a_register_pk_with_a_new_device_key_proves_it_with_that_key() {
        let (pk, sk) = sign::gen_keypair();
        let server = ServerKey::new(&sk, [1u8; 32]).unwrap();
        let mut seal = ClientSeal::new(&pk, Some(sign::gen_keypair().1)).unwrap();
        let (new_pk, new_sk) = sign::gen_keypair();
        let register_peer = message(|m| m.set_register_peer(RegisterPeer::default()));
        let hello = seal_message(&mut seal, &register_peer, false, || unreachable!()).unwrap();
        let (_, mut reply) = server.open(&hello, || 1).unwrap();
        seal.open(&server.seal(&mut reply, 0, b"", || 0).unwrap())
            .unwrap();
        let register_pk = message(|m| {
            m.set_register_pk(RegisterPk {
                pk: new_pk.0.to_vec().into(),
                ..Default::default()
            })
        });
        let data = seal_message(&mut seal, &register_pk, false, || Some(new_sk.clone())).unwrap();
        let (opened, _) = server.open(&data, || unreachable!()).unwrap();
        assert!(opened.proven_by(new_pk.as_ref()));
    }

    async fn registration(state: &mut SealState, msg: &RendezvousMessage) -> Vec<Vec<u8>> {
        let mut socket = FramedSocket::new("127.0.0.1:0").await.unwrap();
        let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = TargetAddr::Ip(server.local_addr().unwrap());
        state
            .round_sink(&mut socket, &addr)
            .send(msg)
            .await
            .unwrap();
        let mut buf = [0u8; 2048];
        let mut datagrams = Vec::new();
        let wait = std::time::Duration::from_millis(100);
        while let Ok(Ok(n)) = tokio::time::timeout(wait, server.recv(&mut buf)).await {
            datagrams.push(buf[..n].to_vec());
        }
        datagrams
    }

    #[tokio::test]
    async fn a_missed_answer_while_sealed_retries_under_the_session_and_with_a_hello() {
        let (mut state, server) = state("missed");
        let register_peer = message(|m| m.set_register_peer(RegisterPeer::default()));
        let hello = registration(&mut state, &register_peer).await;
        let (_, mut reply) = server.open(&hello[0], || 1).unwrap();
        let welcome = server
            .seal(&mut reply, CAP_PUSHES_SEALED, b"", || 0)
            .unwrap();
        assert!(state.open(BytesMut::from(&welcome[..])).unwrap().1);
        assert_eq!(registration(&mut state, &register_peer).await.len(), 1);
        state.on_round(true);
        let retry = registration(&mut state, &register_peer).await;
        assert_eq!(retry.len(), 2);
        assert!(
            server.open(&retry[0], || 2).unwrap().0.pn.is_some(),
            "a server that only lost an answer takes it without the hello budget"
        );
        assert!(
            udp_seal::is_hello(&retry[1]),
            "a restarted one takes the hello"
        );
        let punch = Some(rendezvous_message::Union::PunchHole(Default::default()));
        assert!(
            !state.on_message(false, &punch),
            "a plaintext push is still forged"
        );
        let push = server.seal(&mut Reply::push(1), 0, b"", || 7).unwrap();
        assert!(
            state.open(BytesMut::from(&push[..])).is_some(),
            "a push under the session still opens"
        );
    }

    #[tokio::test]
    async fn a_cookie_sends_the_hello_again_and_the_socket_stays_sealed() {
        let (mut state, server) = state("cookie");
        let register_peer = message(|m| m.set_register_peer(RegisterPeer::default()));
        let hello = registration(&mut state, &register_peer).await;
        let reply = udp_seal::cookie_reply(&hello[0], [5u8; 16]).unwrap();
        let mut socket = FramedSocket::new("127.0.0.1:0").await.unwrap();
        let peer = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = TargetAddr::Ip(peer.local_addr().unwrap());
        assert!(state.on_cookie(&reply, &mut socket, &addr).await.unwrap());
        let mut buf = [0u8; 2048];
        let wait = std::time::Duration::from_millis(500);
        let n = tokio::time::timeout(wait, peer.recv(&mut buf))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(udp_seal::hello_cookie(&buf[..n]), Some([5u8; 16]));
        assert!(server.open(&buf[..n], || 1).is_some());
        assert_eq!(
            rounds(&mut state, &[true; 4]).await,
            [true; 4],
            "only a server that seals sends a cookie"
        );
        let punch = seal_one_shot("cookie", b"\x0a\x00".to_vec());
        assert_eq!(udp_seal::hello_cookie(&punch), Some([5u8; 16]));
    }

    #[tokio::test]
    async fn a_socket_that_sealed_does_not_fall_back_until_it_rebinds() {
        let (mut state, server) = state("no-fall-back");
        welcomed(&mut state, &server, 0);
        let answer = Some(rendezvous_message::Union::RegisterPkResponse(
            Default::default(),
        ));
        assert_eq!(rounds(&mut state, &[true; 6]).await, [true; 6]);
        assert!(
            !state.on_message(false, &answer),
            "its server seals, so a plaintext answer is forged"
        );
        state.on_rebind();
        assert_eq!(
            rounds(&mut state, &[false, true, true]).await,
            [true, true, false]
        );
        assert!(
            state.on_message(false, &answer),
            "a new socket may reach an older server"
        );
    }

    #[test]
    fn a_registration_never_rides_a_hello_and_goes_proven_under_the_session() {
        let (pk, sk) = sign::gen_keypair();
        let server = ServerKey::new(&sk, [1u8; 32]).unwrap();
        let (device_pk, device_sk) = sign::gen_keypair();
        let mut seal = ClientSeal::new(&pk, Some(device_sk)).unwrap();
        let register_peer = message(|m| m.set_register_peer(RegisterPeer::default()));
        let register_pk = message(|m| m.set_register_pk(RegisterPk::default()));
        let mut reply = None;
        for msg in [&register_peer, &register_pk] {
            let hello = seal_message(&mut seal, msg, false, || None).unwrap();
            let (opened, r) = server.open(&hello, || 1).unwrap();
            assert!(opened.pn.is_none() && opened.payload.is_empty());
            reply = Some(r);
        }
        let welcome = server.seal(&mut reply.unwrap(), 0, b"", || 0).unwrap();
        seal.open(&welcome).unwrap();
        for msg in [&register_peer, &register_pk] {
            let data = seal_message(&mut seal, msg, false, || None).unwrap();
            let (opened, _) = server.open(&data, || unreachable!()).unwrap();
            assert!(opened.proven_by(device_pk.as_ref()));
            assert_eq!(
                RendezvousMessage::parse_from_bytes(&opened.payload).unwrap(),
                *msg
            );
        }
    }

    #[test]
    fn a_punch_goes_under_the_session_only_while_its_server_seals() {
        let data = b"\x0a\x00".to_vec();
        assert_eq!(seal_one_shot("punch", data.clone()), data);
        let (mut state, server) = state("punch");
        welcomed(&mut state, &server, 0);
        let (mut other, other_server) = self::state("punch-other");
        welcomed(&mut other, &other_server, 0);
        let sealed = seal_one_shot("punch", data.clone());
        let (opened, _) = server.open(&sealed, || unreachable!()).unwrap();
        assert_eq!((opened.key_id, &opened.payload), (1, &data));
        assert_eq!(seal_one_shot("another-host", data.clone()), data);
        let hello = state.seal.as_mut().unwrap().hello(b"").unwrap();
        let (_, mut reply) = server.open(&hello, || 2).unwrap();
        let welcome = server.seal(&mut reply, 0, b"", || 0).unwrap();
        state.open(BytesMut::from(&welcome[..])).unwrap();
        let sealed = seal_one_shot("punch", data.clone());
        assert_eq!(
            server.open(&sealed, || unreachable!()).unwrap().0.key_id,
            2,
            "the session a welcome brought"
        );
        state.on_round(true);
        assert!(
            server
                .open(&seal_one_shot("punch", data.clone()), || unreachable!())
                .is_some(),
            "a missed answer is no sign the server stopped sealing"
        );
        state.on_rebind();
        assert_eq!(seal_one_shot("punch", data.clone()), data);
        welcomed(&mut state, &server, 0);
        state.on_round(true);
        drop(state);
        assert_eq!(seal_one_shot("punch", data.clone()), data);
    }
}
