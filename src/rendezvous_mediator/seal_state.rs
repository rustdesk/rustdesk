//! Which registration datagrams go sealed, and which received ones are acted on.
//!
//! A socket starts out probing: every registration goes sealed. Two unanswered in a row and the
//! server is taken for one without the seal: registrations go plaintext, and every fourth one
//! is sealed to notice an upgrade. A sealed datagram that opens makes the socket sealed, where
//! nothing goes plaintext and a plaintext answer to a registration is dropped as forged. A
//! missed answer while sealed is retried both under the session and with a hello, since a server
//! restart retires every session; the session stays for pushes until a welcome replaces it. Such
//! a socket never falls back: its server seals. Only a new socket, which may reach another
//! server, probes afresh.

use super::{Sink, TargetAddr};
use hbb_common::{
    anyhow::bail,
    bytes::BytesMut,
    config::{Config, RS_PUB_KEY},
    protobuf::Message as _,
    rendezvous_proto::{rendezvous_message, RendezvousMessage},
    sodiumoxide::crypto::sign,
    udp::FramedSocket,
    udp_seal::{self, ClientSeal},
    ResultType,
};
use std::sync::Mutex;

const PROBE_TRIES: u8 = 2;
const PROBE_EVERY: u8 = 4;

// The hosts whose registration is sealed now, and their keys: a one-off datagram to one of them
// may go sealed too. Only a socket with a session adds its host, so an older server never gets one.
static SEALED: Mutex<Vec<(String, sign::PublicKey)>> = Mutex::new(Vec::new());

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
        Some((BytesMut::from(&payload[..]), true))
    }

    /// Whether to act on a parsed message. Unless the socket fell back, every registration went
    /// sealed, so a plaintext answer to one is forged; once sealed, so is a plaintext push from a
    /// server that seals every push. A sealed answer
    /// to a RegisterPk means the server has the device's key now, which it may not have had
    /// when the hello came, so the next RegisterPeer goes as a signed hello it can check.
    pub(super) fn on_message(
        &mut self,
        sealed: bool,
        msg: &Option<rendezvous_message::Union>,
    ) -> bool {
        if sealed {
            if let (Some(seal), Some(rendezvous_message::Union::RegisterPkResponse(_))) =
                (self.seal.as_mut(), msg)
            {
                seal.require_signed_hello();
            }
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
            Some(server) if mode == Mode::Sealed => {
                let mut sealed = SEALED.lock().unwrap();
                sealed.retain(|(host, _)| *host != self.host);
                sealed.push((self.host.clone(), server));
            }
            _ if was_sealed && !mode.seals() => self.forget_sealed(),
            _ => {}
        }
    }

    fn forget_sealed(&self) {
        SEALED
            .lock()
            .unwrap()
            .retain(|(host, _)| *host != self.host);
    }
}

impl Drop for SealState {
    fn drop(&mut self) {
        if self.mode.seals() {
            self.forget_sealed();
        }
    }
}

/// The datagram a registration goes out as. A hello can be replayed from any address, and a
/// RegisterPk in one, its uuid and key unchanged, would move the route there; so a hello carries
/// a RegisterPeer or nothing, and anything else waits for the session. A session from an empty
/// hello is replaced at the next RegisterPeer by a signed hello, which ties it to this device;
/// pushes still come under it until the welcome.
/// `hello` asks for one even under a session, as after a missed answer. A RegisterPk with a key
/// other than the one hellos are signed by means the key pair changed: `device` loads the new
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
    let hello = hello || (msg.has_register_peer() && seal.needs_signed_hello());
    let payload = if msg.has_register_peer() || (seal.has_session() && !hello) {
        msg.write_to_bytes()?
    } else {
        Vec::new()
    };
    let sealed = if hello {
        seal.hello(&payload)
    } else {
        seal.seal(&payload)
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
/// host's registration is: an older server would drop it, and the punch with it.
pub(super) fn seal_one_shot(host: &str, data: Vec<u8>) -> Vec<u8> {
    let server = SEALED
        .lock()
        .unwrap()
        .iter()
        .find(|(h, _)| h == host)
        .map(|(_, pk)| *pk);
    match server
        .and_then(|pk| ClientSeal::new(&pk, None))
        .and_then(|mut seal| seal.seal(&data))
    {
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
    fn a_register_pk_with_a_new_device_key_signs_the_next_hello_with_it() {
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
        seal_message(&mut seal, &register_pk, false, || Some(new_sk.clone())).unwrap();
        seal.require_signed_hello();
        let again = seal_message(&mut seal, &register_peer, false, || unreachable!()).unwrap();
        let (opened, _) = server.open(&again, || 2).unwrap();
        assert!(opened.signed_by(new_pk.as_ref()).is_some());
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
    fn a_register_pk_waits_for_the_session_and_a_signed_hello_follows() {
        let (pk, sk) = sign::gen_keypair();
        let server = ServerKey::new(&sk, [1u8; 32]).unwrap();
        let (device_pk, device_sk) = sign::gen_keypair();
        let mut seal = ClientSeal::new(&pk, Some(device_sk)).unwrap();
        let register_pk = message(|m| m.set_register_pk(RegisterPk::default()));
        let hello = seal_message(&mut seal, &register_pk, false, || None).unwrap();
        let (opened, mut reply) = server.open(&hello, || 1).unwrap();
        assert!(
            opened.payload.is_empty(),
            "a replayable hello carries no RegisterPk"
        );
        let welcome = server.seal(&mut reply, 0, b"", || 0).unwrap();
        seal.open(&welcome).unwrap();
        let data = seal_message(&mut seal, &register_pk, false, || None).unwrap();
        let (opened, _) = server.open(&data, || 2).unwrap();
        assert!(opened.pn.is_some());
        let parsed = RendezvousMessage::parse_from_bytes(&opened.payload).unwrap();
        assert!(parsed.has_register_pk());
        let register_peer = message(|m| m.set_register_peer(RegisterPeer::default()));
        let again = seal_message(&mut seal, &register_peer, false, || None).unwrap();
        assert!(udp_seal::is_hello(&again));
        let (opened, _) = server.open(&again, || 3).unwrap();
        assert!(opened.signed_by(device_pk.as_ref()).is_some());
        assert!(RendezvousMessage::parse_from_bytes(&opened.payload)
            .unwrap()
            .has_register_peer());
    }

    #[test]
    fn a_register_pk_answered_under_a_signed_session_brings_a_signed_hello() {
        let (mut state, server) = state("after-pk");
        let register_peer = message(|m| m.set_register_peer(RegisterPeer::default()));
        let hello =
            seal_message(state.seal.as_mut().unwrap(), &register_peer, false, || None).unwrap();
        let (_, mut reply) = server.open(&hello, || 1).unwrap();
        let welcome = server.seal(&mut reply, 0, b"", || 0).unwrap();
        state.open(BytesMut::from(&welcome[..])).unwrap();
        let seal = state.seal.as_mut().unwrap();
        assert!(!udp_seal::is_hello(
            &seal_message(seal, &register_peer, false, || None).unwrap()
        ));
        let answer = Some(rendezvous_message::Union::RegisterPkResponse(
            Default::default(),
        ));
        assert!(state.on_message(true, &answer));
        let seal = state.seal.as_mut().unwrap();
        assert!(udp_seal::is_hello(
            &seal_message(seal, &register_peer, false, || None).unwrap()
        ));
    }

    #[test]
    fn a_signed_hello_keeps_the_session_it_replaces_until_welcomed() {
        let (mut state, server) = state("re-hello");
        welcomed(&mut state, &server, CAP_PUSHES_SEALED);
        let register_peer = message(|m| m.set_register_peer(RegisterPeer::default()));
        let hello =
            seal_message(state.seal.as_mut().unwrap(), &register_peer, false, || None).unwrap();
        assert!(udp_seal::is_hello(&hello));
        let push = server.seal(&mut Reply::push(1), 0, b"", || 7).unwrap();
        assert!(
            state.open(BytesMut::from(&push[..])).is_some(),
            "a push under the old session still opens"
        );
        let punch = Some(rendezvous_message::Union::PunchHole(Default::default()));
        assert!(
            !state.on_message(false, &punch),
            "a plaintext push is still forged"
        );
    }

    #[test]
    fn a_punch_goes_sealed_only_while_its_server_is() {
        let data = b"\x0a\x00".to_vec();
        assert_eq!(seal_one_shot("punch", data.clone()), data);
        let (mut state, server) = state("punch");
        welcomed(&mut state, &server, 0);
        let (mut other, other_server) = self::state("punch-other");
        welcomed(&mut other, &other_server, 0);
        let sealed = seal_one_shot("punch", data.clone());
        assert!(udp_seal::is_hello(&sealed));
        let (opened, _) = server.open(&sealed, || 9).unwrap();
        assert_eq!(opened.payload, data);
        assert_eq!(seal_one_shot("another-host", data.clone()), data);
        state.on_round(true);
        assert!(
            udp_seal::is_hello(&seal_one_shot("punch", data.clone())),
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
