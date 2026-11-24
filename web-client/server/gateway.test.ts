import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import net from 'node:net';
import { zstdCompressSync } from 'node:zlib';
import nacl from 'tweetnacl';
import {
  DEFAULT_DIRECT_PORT,
  RELAY_PORT,
  RENDEZVOUS_PORT,
  checkPort,
  directUsername,
  isValidRelayUuid,
  passwordResponse,
  sanitizeHostInput,
  tokenMatches,
} from './auth.ts';
import {
  BytesFrameDecoder,
  EncryptedFramedStream,
  KX_VERSION_LATEST,
  StreamCipher,
  encodeBytesFrame,
  formatNetError,
} from './bytesCodec.ts';
import {
  MOUSE_BUTTON_LEFT,
  MOUSE_TYPE_DOWN,
  MOUSE_TYPE_WHEEL,
  buildLoginOptions,
  buildVideoReceivedMessage,
  humanizeError,
  packMouseMask,
  selectCodecs,
  toHostMessage,
} from './control.ts';
import { CursorShapes } from './cursor.ts';
import { readUntilHash } from './handshake.ts';
import {
  BoolOptionValues,
  ChromaValues,
  ControlKeyValues,
  KeyboardModeValues,
  PreferCodecValues,
  decodeMessage,
  decodeRendezvous,
  encodeIdPk,
  encodeMessage,
  encodeRendezvous,
} from './protos.ts';
import { connectViaId } from './rustdeskClient.ts';
import {
  CODEC_VP8,
  HEADER_LEN,
  decodeRdv1Frame,
  encodeRdv1Frame,
  vp8KeyframeSize,
} from './video.ts';

test('passwordResponse matches the RustDesk host formula test vector', () => {
  const actual = passwordResponse('pw', 'salt', 'b').toString('hex');
  assert.equal(
    actual,
    '803235e4d1ed7843e834f5e27655ea12b7ab5c194226dd10e6aebcc9d3dc5ec1',
  );
});

test('directUsername keeps IPv4 with port and formats bare IPv4/IPv6', async () => {
  assert.equal(await directUsername('127.0.0.1:21118', 21118), '127.0.0.1:21118');
  assert.equal(await directUsername('127.0.0.1', 21118), '127.0.0.1:21118');
  assert.equal(await directUsername('::1', 21118), '[::1]:21118');
});

test('direct port constants match RustDesk host listener', () => {
  assert.equal(DEFAULT_DIRECT_PORT, 21118);
  assert.equal(DEFAULT_DIRECT_PORT, RELAY_PORT + 1);
  assert.equal(DEFAULT_DIRECT_PORT, RENDEZVOUS_PORT + 2);
});

test('login options advertise only probed codecs and I420 chroma', () => {
  const caps = selectCodecs({
    vp8: true,
    vp9: false,
    h264: true,
    h265: false,
    av1: false,
    reported: true,
  });
  const opts = buildLoginOptions(caps) as any;
  assert.equal(opts.supported_decoding.ability_vp8, 1);
  assert.equal(opts.supported_decoding.ability_vp9, 0);
  assert.equal(opts.supported_decoding.ability_h264, 1);
  assert.equal(opts.supported_decoding.ability_h265, 0);
  assert.equal(opts.supported_decoding.ability_av1, 0);
  assert.equal(opts.supported_decoding.i444, undefined);
  assert.equal(opts.supported_decoding.prefer_chroma, ChromaValues.I420);
});

test('unprobed codecs default to software VP8 and VP9 only', () => {
  const caps = selectCodecs(undefined);
  assert.equal(caps.vp8, true);
  assert.equal(caps.vp9, true);
  assert.equal(caps.h264, false);
  assert.equal(caps.h265, false);
  assert.equal(caps.av1, false);
});

test('RDV1 header roundtrip preserves metadata, negative pts, and payload', () => {
  assert.equal(HEADER_LEN, 32);
  const payload = new Uint8Array([1, 2, 3, 4, 5]);
  const encoded = encodeRdv1Frame({
    codec: CODEC_VP8,
    key: true,
    display: 1,
    width: 1280,
    height: 720,
    pts: -42n,
    gatewayUnixMs: 1700000000123n,
    data: payload,
  });
  assert.equal(encoded.byteLength, HEADER_LEN + payload.byteLength);

  const decoded = decodeRdv1Frame(encoded);
  assert.ok(decoded);
  assert.equal(decoded.codec, CODEC_VP8);
  assert.equal(decoded.key, true);
  assert.equal(decoded.display, 1);
  assert.equal(decoded.width, 1280);
  assert.equal(decoded.height, 720);
  assert.equal(decoded.pts, -42n);
  assert.equal(decoded.gatewayUnixMs, 1700000000123n);
  assert.deepEqual(Array.from(decoded.data), [1, 2, 3, 4, 5]);
});

test('RDV1 rejects short or foreign magic frames', () => {
  assert.equal(decodeRdv1Frame(new Uint8Array(10)), null);
  const bad = new Uint8Array(40);
  bad.set([0x58, 0x58, 0x58, 0x58], 0);
  assert.equal(decodeRdv1Frame(bad), null);
});

test('vp8KeyframeSize parses uncompressed keyframe header and rejects interframes', () => {
  // Keyframe: bit0=0, start code 9d 01 2a, 1280 (0x0500) LE, 720 (0x02d0) LE
  const kf = new Uint8Array([0x10, 0x00, 0x00, 0x9d, 0x01, 0x2a, 0x00, 0x05, 0xd0, 0x02]);
  assert.deepEqual(vp8KeyframeSize(kf), { width: 1280, height: 720 });

  const inter = new Uint8Array([0x11, 0x00, 0x00, 0x9d, 0x01, 0x2a, 0x00, 0x05, 0xd0, 0x02]);
  assert.equal(vp8KeyframeSize(inter), null);
});

test('mouse mask packing matches RustDesk bit layout', () => {
  assert.equal(packMouseMask('down', 'left'), (MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN);
  assert.equal(packMouseMask('wheel', 'right'), MOUSE_TYPE_WHEEL);
  const wheelMsg = toHostMessage({ type: 'mouse', action: 'wheel', x: 0, y: -1 }) as any;
  assert.equal(wheelMsg.mouse_event.mask, 3);
  assert.equal(wheelMsg.mouse_event.y, -1);
});

test('text and Escape become legacy KeyEvents and video_received is a Misc flag', () => {
  const txt = toHostMessage({ type: 'text', text: '你好' }) as any;
  assert.equal(txt.key_event.mode, KeyboardModeValues.Legacy);
  assert.equal(txt.key_event.down, true);
  assert.equal(txt.key_event.press, true);
  assert.equal(txt.key_event.seq, '你好');

  const esc = toHostMessage({ type: 'key', action: 'down', control: 'Escape' }) as any;
  assert.equal(esc.key_event.control_key, ControlKeyValues.Escape);
  assert.equal(esc.key_event.down, true);

  const ack = buildVideoReceivedMessage() as any;
  assert.equal(ack.misc.video_received, true);
});

test('humanizeError produces plain-language errors including distinct DNS failures', () => {
  assert.match(humanizeError(new Error('Wrong Password')), /rejected the password/i);
  assert.match(humanizeError(new Error('ECONNREFUSED 127.0.0.1:21118')), /Cannot reach the server or host/i);
  assert.match(humanizeError(new Error('timed out')), /did not answer in time/i);
  assert.match(
    humanizeError(new Error('DNS lookup failed for "bad.rustdesk.test" (ENOTFOUND)')),
    /DNS lookup failed for "bad\.rustdesk\.test"/i,
  );
});

test('sanitizeHostInput and checkPort normalize pasted URLs, fullwidth colons, and trailing paths', () => {
  assert.equal(sanitizeHostInput('  https://rs.example.com:21116/path?a=1  '), 'rs.example.com:21116');
  assert.equal(sanitizeHostInput('http://hbbs.example.com/'), 'hbbs.example.com');
  assert.equal(sanitizeHostInput('rs.example.com：31116'), 'rs.example.com:31116');
  assert.deepEqual(checkPort('https://rs.example.com/', 21116), { host: 'rs.example.com', port: 21116 });
  assert.deepEqual(checkPort('rs.example.com：31116', 21116), { host: 'rs.example.com', port: 31116 });
  assert.deepEqual(checkPort('[2001:db8::1]:21117', 21116), { host: '2001:db8::1', port: 21117 });
});

test('formatNetError unwraps Node AggregateError with empty top-level message', () => {
  const agg = new AggregateError([new Error('connect ECONNREFUSED 127.0.0.1:21116')], '');
  assert.equal(formatNetError(agg), 'connect ECONNREFUSED 127.0.0.1:21116');
});

test('gateway token check is optional and constant-time exact', () => {
  assert.equal(tokenMatches('', ''), true);
  assert.equal(tokenMatches('   ', 'anything'), true);
  assert.equal(tokenMatches('secret-token', 'secret-token'), true);
  assert.equal(tokenMatches(' secret-token ', 'secret-token '), true);
  assert.equal(tokenMatches('secret-token', 'wrong-token'), false);
  assert.equal(tokenMatches('secret-token', ''), false);
});

test('relay UUID validator enforces allowed charset and max length', () => {
  assert.equal(isValidRelayUuid('abc_123-XYZ'), true);
  assert.equal(isValidRelayUuid(''), false);
  assert.equal(isValidRelayUuid('bad/uuid'), false);
  assert.equal(isValidRelayUuid('a'.repeat(129)), false);
});

test('BytesCodec handles 1-byte, 2-byte, and 3-byte varint frames across chunk splits', () => {
  const decoder = new BytesFrameDecoder();
  const small = Buffer.from('hello');
  const medium = Buffer.alloc(300, 0xab);
  const large = Buffer.alloc(20_000, 0xcd);
  const combined = Buffer.concat([
    encodeBytesFrame(small),
    encodeBytesFrame(medium),
    encodeBytesFrame(large),
  ]);

  const out: Buffer[] = [];
  for (let i = 0; i < combined.length; i += 7) {
    out.push(...decoder.push(combined.subarray(i, i + 7)));
  }
  assert.equal(out.length, 3);
  assert.deepEqual(out[0], small);
  assert.deepEqual(out[1], medium);
  assert.deepEqual(out[2], large);
});

test('NaCl SignedId handshake with kx_version=1 encrypts the Hash challenge over TCP', async () => {
  const peerSignPair = nacl.sign.keyPair();
  const peerBoxPair = nacl.box.keyPair();
  const expectedId = '987654321';

  const server = net.createServer(async (socket) => {
    const srvStream = new EncryptedFramedStream(socket);
    const idPkBytes = encodeIdPk(expectedId, peerBoxPair.publicKey, KX_VERSION_LATEST);
    const signedId = nacl.sign(idPkBytes, peerSignPair.secretKey);
    await srvStream.sendRaw(encodeMessage({ signed_id: { id: Buffer.from(signedId) } }));

    const pkRaw = await srvStream.next(5000);
    assert.ok(pkRaw);
    const pkMsg = decodeMessage(pkRaw);
    assert.ok(pkMsg.public_key);
    const asym = new Uint8Array(pkMsg.public_key.asymmetric_value);
    const sym = new Uint8Array(pkMsg.public_key.symmetric_value);
    const picked = Number(pkMsg.public_key.kx_version);
    assert.equal(picked, KX_VERSION_LATEST);

    const key = nacl.box.open(sym, new Uint8Array(24), asym, peerBoxPair.secretKey);
    assert.ok(key);

    srvStream.setNegotiatedKey(key, false, {
      initiatorPk: asym,
      responderPk: peerBoxPair.publicKey,
      advertised: KX_VERSION_LATEST,
      picked,
    });

    await srvStream.sendRaw(encodeMessage({ hash: { salt: 's1', challenge: 'c1' } }));
  });

  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const addr = server.address() as net.AddressInfo;

  try {
    const client = await EncryptedFramedStream.connect('127.0.0.1', addr.port, 5000);
    const { hash, encrypted } = await readUntilHash(client, peerSignPair.publicKey, expectedId);
    assert.equal(encrypted, true);
    assert.equal(client.isSecured(), true);
    assert.deepEqual(hash, { salt: 's1', challenge: 'c1' });
    client.close();
  } finally {
    server.close();
  }
});

test('StreamCipher kx_version=1 derives directional split keys', () => {
  const shared = new Uint8Array(crypto.randomBytes(32));
  const transcript = {
    initiatorPk: new Uint8Array(32).fill(1),
    responderPk: new Uint8Array(32).fill(2),
    advertised: 1,
    picked: 1,
  };
  const initiator = StreamCipher.fromNegotiated(shared, true, transcript);
  const responder = StreamCipher.fromNegotiated(shared, false, transcript);

  const sealed = initiator.seal(Buffer.from('ping'));
  assert.equal(Buffer.from(responder.open(sealed)).toString('utf8'), 'ping');

  const reply = responder.seal(Buffer.from('pong'));
  assert.equal(Buffer.from(initiator.open(reply)).toString('utf8'), 'pong');
});

test('gateway requests remote cursor updates and decodes and caches native cursor shapes', () => {
  const capabilities = { vp8: true, vp9: false, h264: false, h265: false, av1: false, reported: true };
  assert.equal(buildLoginOptions(capabilities).show_remote_cursor, BoolOptionValues.Yes);

  const pixels = Buffer.from([20, 40, 60, 255, 10, 30, 50, 128]);
  const id = { low: -1, high: -1, unsigned: true };
  const frame = encodeMessage({
    cursor_data: { id, width: 2, height: 1, hotx: 1, colors: zstdCompressSync(pixels) },
  });
  assert.equal(frame[0], 0x62);
  const cursorId = encodeMessage({ cursor_id: id });
  assert.equal(cursorId[0], 0x70);

  const shapes = new CursorShapes();
  const shape = shapes.decode(decodeMessage(frame).cursor_data);
  assert.deepEqual(shape, { width: 2, height: 1, hotX: 1, hotY: 0, rgba: pixels.toString('base64') });
  const decodedId = decodeMessage(cursorId).cursor_id;
  assert.equal(decodedId, '18446744073709551615');
  assert.deepEqual(shapes.get(decodedId), shape);
});

test('gateway falls back when a cursor bitmap is malformed or exceeds its declared dimensions', () => {
  const shapes = new CursorShapes();
  assert.equal(shapes.decode({ width: 1, height: 1, colors: Buffer.from('bad data') }), null);
  assert.equal(shapes.decode({ width: 1, height: 1, colors: zstdCompressSync(Buffer.alloc(8)) }), null);
});

test('canonical RustDesk protobuf field wire tags match upstream message.proto and rendezvous.proto', () => {
  // Message.key_event MUST be field 15 (wire tag = (15<<3)|2 = 0x7a)
  const keyBuf = encodeMessage(toHostMessage({ type: 'key', action: 'down', control: 'Escape' })!);
  assert.equal(keyBuf[0], (15 << 3) | 2, 'Message.key_event must be field 15');

  // Message.clipboard MUST be field 16 (varint tag = (16<<3)|2 = 0x82 0x01)
  const clipBuf = encodeMessage(toHostMessage({ type: 'clipboard', text: 'hi' })!);
  assert.equal(clipBuf[0], 0x82);
  assert.equal(clipBuf[1], 0x01);

  // Message.misc MUST be field 19 (varint tag = (19<<3)|2 = 0x9a 0x01)
  // Misc.video_received MUST be field 12 (tag = (12<<3)|0 = 0x60)
  const ackBuf = encodeMessage(buildVideoReceivedMessage());
  assert.equal(ackBuf[0], 0x9a);
  assert.equal(ackBuf[1], 0x01);
  assert.equal(ackBuf[3], (12 << 3) | 0, 'Misc.video_received must be field 12');

  // Message.cursor_position MUST be field 13 (tag = (13<<3)|2 = 0x6a)
  const cursorBuf = encodeMessage({ cursor_position: { x: 10, y: 20 } });
  assert.equal(cursorBuf[0], (13 << 3) | 2, 'Message.cursor_position must be field 13');

  // IdPk.kx_version MUST be field 4 (tag = (4<<3)|0 = 0x20)
  const idPkBuf = encodeIdPk('123', new Uint8Array(32).fill(7), 1);
  assert.ok(idPkBuf.includes(Buffer.from([0x20, 0x01])), 'IdPk.kx_version must be field 4');

  // LoginRequest.version MUST be field 11 (tag = (11<<3)|2 = 0x5a)
  const lrBuf = encodeMessage({
    login_request: {
      username: '127.0.0.1:21118',
      version: '1.5.3',
    },
  });
  assert.ok(lrBuf.includes(Buffer.from([0x5a, 0x05])), 'LoginRequest.version must be field 11');
});

test('AddrMangle roundtrips IPv4 (including trimmed zero bytes) and IPv6 with little-endian port', async () => {
  const { encodeAddrMangle, decodeAddrMangle } = await import('./protos.ts');

  const v4 = encodeAddrMangle('203.0.113.42', 21118, 0x12345678);
  assert.ok(v4.length <= 16 && v4.length > 0);
  assert.deepEqual(decodeAddrMangle(v4), { host: '203.0.113.42', port: 21118 });

  const v4Local = encodeAddrMangle('127.0.0.1', 54321, 1);
  assert.deepEqual(decodeAddrMangle(v4Local), { host: '127.0.0.1', port: 54321 });

  const v6 = encodeAddrMangle('2001:db8::1', 21118);
  assert.equal(v6.length, 18);
  assert.equal(v6.readUInt16LE(16), 21118);
  assert.deepEqual(decodeAddrMangle(v6), { host: '2001:db8:0:0:0:0:0:1', port: 21118 });

  assert.equal(decodeAddrMangle(new Uint8Array(0)), null);
  assert.equal(decodeAddrMangle(new Uint8Array(17)), null);
});

test('adversarial tampering: corrupted NaCl ciphertext and forged peer IdPk are rejected', async () => {
  const { peerSignKey } = await import('./handshake.ts');
  const rsPair = nacl.sign.keyPair();
  const attackerPair = nacl.sign.keyPair();
  const rsB64 = Buffer.from(rsPair.publicKey).toString('base64');

  // Forged signature by attacker key
  const forged = nacl.sign(encodeIdPk('111222333', new Uint8Array(32).fill(9), 1), attackerPair.secretKey);
  assert.throws(() => peerSignKey(rsB64, forged, '111222333'), /signature verification failed/i);

  // Valid signature for a DIFFERENT peer ID (MITM peer swap attack)
  const swapped = nacl.sign(encodeIdPk('999888777', new Uint8Array(32).fill(9), 1), rsPair.secretKey);
  assert.throws(() => peerSignKey(rsB64, swapped, '111222333'), /signed a different id/i);

  // Corrupted secretbox ciphertext
  const shared = new Uint8Array(crypto.randomBytes(32));
  const transcript = {
    initiatorPk: new Uint8Array(32).fill(1),
    responderPk: new Uint8Array(32).fill(2),
    advertised: 1,
    picked: 1,
  };
  const tx = StreamCipher.fromNegotiated(shared, true, transcript);
  const rx = StreamCipher.fromNegotiated(shared, false, transcript);
  const sealed = new Uint8Array(tx.seal(Buffer.from('secret')));
  sealed[sealed.length - 1] ^= 0xff;
  assert.throws(() => rx.open(sealed), /decryption failed/i);

  // Oversized clipboard (> 256 KiB) rejected cleanly
  assert.equal(toHostMessage({ type: 'clipboard', text: 'x'.repeat(300 * 1024) }), null);
});

test('adversarial NAT traversal: connectViaId automatically falls back from unreachable PunchHole to hbbs->hbbr encrypted relay', async () => {
  const { connectViaId } = await import('./rustdeskClient.ts');
  const {
    decodeRendezvous,
    encodeAddrMangle,
    encodeRendezvous,
  } = await import('./protos.ts');

  const rsSign = nacl.sign.keyPair();
  const peerSign = nacl.sign.keyPair();
  const peerBox = nacl.box.keyPair();
  const peerId = '555666777';
  const password = 'test-permanent-password';
  const rsKeyB64 = Buffer.from(rsSign.publicKey).toString('base64');
  const signedPeerPk = nacl.sign(encodeIdPk(peerId, peerSign.publicKey, 0), rsSign.secretKey);

  // 1. Mock hbbr relay + RustDesk host behind NAT
  let relaySeenUuid = '';
  let postLoginOptionReceived = false;
  let refreshReceived = false;
  const hbbrServer = net.createServer(async (socket) => {
    const s = new EncryptedFramedStream(socket);
    const reqRaw = await s.next(5000);
    assert.ok(reqRaw);
    const req = decodeRendezvous(reqRaw);
    assert.ok(req.request_relay);
    assert.equal(req.request_relay.id, peerId);
    relaySeenUuid = req.request_relay.uuid;

    // Now the relayed peer speaks standard RustDesk Message protocol (SignedId -> PublicKey -> encrypted Hash -> LoginRequest -> LoginResponse)
    const idPk = encodeIdPk(peerId, peerBox.publicKey, KX_VERSION_LATEST);
    const signedId = nacl.sign(idPk, peerSign.secretKey);
    await s.sendRaw(encodeMessage({ signed_id: { id: Buffer.from(signedId) } }));

    const pkRaw = await s.next(5000);
    assert.ok(pkRaw);
    const pkMsg = decodeMessage(pkRaw);
    const asym = new Uint8Array(pkMsg.public_key.asymmetric_value);
    const sym = new Uint8Array(pkMsg.public_key.symmetric_value);
    const picked = Number(pkMsg.public_key.kx_version);
    const symKey = nacl.box.open(sym, new Uint8Array(24), asym, peerBox.secretKey);
    assert.ok(symKey);

    s.setNegotiatedKey(symKey, false, {
      initiatorPk: asym,
      responderPk: peerBox.publicKey,
      advertised: KX_VERSION_LATEST,
      picked,
    });

    await s.sendRaw(encodeMessage({ hash: { salt: 'natSalt', challenge: 'natChall' } }));

    const lrRaw = await s.next(5000);
    assert.ok(lrRaw);
    const lrMsg = decodeMessage(lrRaw);
    assert.equal(lrMsg.login_request.username, peerId);
    assert.equal(
      Buffer.from(lrMsg.login_request.password).toString('hex'),
      passwordResponse(password, 'natSalt', 'natChall').toString('hex'),
    );

    await s.sendRaw(
      encodeMessage({
        login_response: {
          peer_info: {
            username: 'alice',
            hostname: 'nat-workstation',
            platform: 'Linux',
            version: '1.5.3',
            current_display: 0,
            displays: [{ x: 0, y: 0, width: 1920, height: 1080, name: 'DP-1', online: true }],
          },
        },
      }),
    );

    for (let i = 0; i < 3; i += 1) {
      const nextRaw = await s.next(5000);
      if (!nextRaw) break;
      const m = decodeMessage(nextRaw);
      if (m.misc?.option) postLoginOptionReceived = true;
      if (m.misc?.refresh_video) {
        refreshReceived = true;
        break;
      }
    }
  });

  await new Promise<void>((resolve) => hbbrServer.listen(0, '127.0.0.1', resolve));
  const hbbrPort = (hbbrServer.address() as net.AddressInfo).port;

  // 2. Mock hbbs server: completes KeyExchange on first socket, answers PunchHoleRequest with unreachable 127.0.0.1:1,
  //    then answers follow-up RequestRelay with RelayResponse pointing to mock hbbrPort.
  let hbbsConnCount = 0;
  const hbbsAuthServer = net.createServer(async (socket) => {
    hbbsConnCount += 1;
    const isFirst = hbbsConnCount === 1;
    const s = new EncryptedFramedStream(socket);

    if (isFirst) {
      const srvBox = nacl.box.keyPair();
      const signedBoxPk = nacl.sign(srvBox.publicKey, rsSign.secretKey);
      await s.sendRaw(
        encodeRendezvous({
          key_exchange: {
            keys: [Buffer.from(signedBoxPk)],
            version: 0,
          },
        }),
      );
      const kxRespRaw = await s.next(5000);
      assert.ok(kxRespRaw);
      const kxResp = decodeRendezvous(kxRespRaw);
      const asym = new Uint8Array(kxResp.key_exchange.keys[0]);
      const sym = new Uint8Array(kxResp.key_exchange.keys[1]);
      const k = nacl.box.open(sym, new Uint8Array(24), asym, srvBox.secretKey);
      assert.ok(k);
      s.setNegotiatedKey(k, false, {
        initiatorPk: asym,
        responderPk: srvBox.publicKey,
        advertised: 0,
        picked: 0,
      });
    }

    const reqRaw = await s.next(5000);
    assert.ok(reqRaw);
    const msg = decodeRendezvous(reqRaw);
    if (msg.punch_hole_request) {
      await s.sendRaw(
        encodeRendezvous({
          punch_hole_response: {
            socket_addr: encodeAddrMangle('127.0.0.1', 1),
            pk: Buffer.from(signedPeerPk),
            relay_server: `127.0.0.1:${hbbrPort}`,
          },
        }),
      );
    } else if (msg.request_relay) {
      await s.sendRaw(
        encodeRendezvous({
          relay_response: {
            uuid: msg.request_relay.uuid,
            relay_server: `127.0.0.1:${hbbrPort}`,
            pk: Buffer.from(signedPeerPk),
          },
        }),
      );
    }
  });

  await new Promise<void>((resolve) => hbbsAuthServer.listen(0, '127.0.0.1', resolve));
  const hbbsPort = (hbbsAuthServer.address() as net.AddressInfo).port;

  try {
    const conn = await connectViaId(
      {
        peerId,
        password,
        idServer: `127.0.0.1:${hbbsPort}`,
        relayServer: '',
        key: rsKeyB64,
        forceRelay: false,
      },
      selectCodecs({ vp8: true, vp9: true, h264: true, h265: false, av1: false, reported: true }),
    );
    assert.equal(conn.encrypted, true);
    assert.equal(conn.peerInfo.hostname, 'nat-workstation');
    assert.ok(relaySeenUuid.length > 0, 'must coordinate a relay UUID with hbbr');
    // Allow host server loop to observe post-login Option + RefreshVideo
    await new Promise((r) => setTimeout(r, 50));
    assert.equal(postLoginOptionReceived, true, 'must send post-login Misc.option after LoginResponse');
    assert.equal(refreshReceived, true, 'must send RefreshVideo after LoginResponse');
    conn.stream.close();
  } finally {
    hbbsAuthServer.close();
    hbbrServer.close();
  }
});

test('open-source hbbs compatibility: connects when hbbs sends no KeyExchange and peerId contains spaces', async () => {
  const rsSign = nacl.sign.keyPair();
  const rsKeyB64 = Buffer.from(rsSign.publicKey).toString('base64');
  const peerSign = nacl.sign.keyPair();
  const peerBox = nacl.box.keyPair();
  const peerId = '187685134';
  const password = 'temp-oss-password';
  const salt = 's12345';
  const challenge = 'c67890';

  const signedPeerPk = nacl.sign(encodeIdPk(peerId, peerSign.publicKey, 0), rsSign.secretKey);
  const peerSignedId = nacl.sign(encodeIdPk(peerId, peerBox.publicKey, 1), peerSign.secretKey);

  let seenPunchId = '';

  const hbbrServer = net.createServer(async (socket) => {
    const s = new EncryptedFramedStream(socket);
    const reqRaw = await s.next(5000);
    assert.ok(reqRaw);
    const req = decodeRendezvous(reqRaw);
    assert.equal(req.request_relay.id, peerId);

    await s.sendRaw(encodeMessage({ signed_id: { id: Buffer.from(peerSignedId) } }));
    const pkRaw = await s.next(5000);
    assert.ok(pkRaw);
    const pkMsg = decodeMessage(pkRaw);
    const asym = new Uint8Array(pkMsg.public_key.asymmetric_value);
    const sym = new Uint8Array(pkMsg.public_key.symmetric_value);
    const symKey = nacl.box.open(sym, new Uint8Array(24), asym, peerBox.secretKey);
    assert.ok(symKey);

    s.setNegotiatedKey(symKey, false, {
      initiatorPk: asym,
      responderPk: peerBox.publicKey,
      advertised: 1,
      picked: 1,
    });
    await s.sendRaw(encodeMessage({ hash: { salt, challenge } }));
    const loginRaw = await s.next(5000);
    assert.ok(loginRaw);
    const login = decodeMessage(loginRaw);
    assert.equal(login.login_request.username, peerId);
    assert.deepEqual(
      Buffer.from(login.login_request.password),
      passwordResponse(password, salt, challenge),
    );
    await s.sendRaw(
      encodeMessage({
        login_response: {
          peer_info: {
            hostname: 'oss-hbbs-peer',
            platform: 'Mac OS',
            username: 'tester',
            version: '1.5.3',
            displays: [{ width: 1920, height: 1200, name: '1', online: true }],
          },
        },
      }),
    );
  });

  await new Promise<void>((resolve) => hbbrServer.listen(0, '127.0.0.1', resolve));
  const hbbrPort = (hbbrServer.address() as net.AddressInfo).port;

  // Open-source hbbs: does NOT send KeyExchange on TCP connect; waits for PunchHoleRequest
  const hbbsServer = net.createServer(async (socket) => {
    const s = new EncryptedFramedStream(socket);
    const reqRaw = await s.next(5000);
    assert.ok(reqRaw);
    const msg = decodeRendezvous(reqRaw);
    assert.ok(msg.punch_hole_request);
    seenPunchId = msg.punch_hole_request.id;
    await s.sendRaw(
      encodeRendezvous({
        relay_response: {
          uuid: '5e7ded90-c408-4449-bbba-205d848985fd',
          relay_server: `127.0.0.1:${hbbrPort}`,
          pk: Buffer.from(signedPeerPk),
          version: '1.5.3',
        },
      }),
    );
  });

  await new Promise<void>((resolve) => hbbsServer.listen(0, '127.0.0.1', resolve));
  const hbbsPort = (hbbsServer.address() as net.AddressInfo).port;

  try {
    const conn = await connectViaId(
      {
        peerId: '187 685 134',
        password,
        idServer: `127.0.0.1:${hbbsPort}`,
        relayServer: '',
        key: rsKeyB64,
        forceRelay: false,
      },
      selectCodecs({ vp8: true, vp9: true, h264: true, h265: false, av1: false, reported: true }),
    );
    assert.equal(seenPunchId, '187685134');
    assert.equal(conn.encrypted, true);
    assert.equal(conn.peerInfo.hostname, 'oss-hbbs-peer');
    conn.stream.close();
  } finally {
    hbbsServer.close();
    hbbrServer.close();
  }
});

test('detectNonRustDeskProtocol identifies HTTP, HTML, TLS, and SSH banners while ignoring binary frames', async () => {
  const { detectNonRustDeskProtocol } = await import('./bytesCodec.ts');
  assert.equal(detectNonRustDeskProtocol(Buffer.from('HTTP/1.1 400 Bad Request\r\n\r\n')), 'http');
  assert.equal(detectNonRustDeskProtocol(Buffer.from('<!DOCTYPE html><html>')), 'http');
  assert.equal(detectNonRustDeskProtocol(Buffer.from('SSH-2.0-OpenSSH_9.6\r\n')), 'ssh');
  assert.equal(detectNonRustDeskProtocol(Buffer.from([0x16, 0x03, 0x03, 0x00, 0x20])), 'tls');
  assert.equal(detectNonRustDeskProtocol(encodeBytesFrame(encodeMessage({ hash: { salt: 'a', challenge: 'b' } }))), null);
});

test('decodeMessage and decodeRendezvous wrap protobuf wire errors into clear non-RustDesk diagnostics', () => {
  assert.throws(
    () => decodeRendezvous(Buffer.from('TTP/1.1 400 Bad Re')),
    /Not a RustDesk ID server/i,
  );
  assert.throws(
    () => decodeMessage(Buffer.from('TTP/1.1 400 Bad Re')),
    /Not a RustDesk/i,
  );
  assert.match(
    humanizeError(new Error('invalid wire type 4 at offset 1')),
    /Not a RustDesk server/i,
  );
  assert.match(
    humanizeError(new Error('Not a RustDesk server: received an HTTP response')),
    /HTTP\/HTTPS response/i,
  );
});

test('connectViaId against an HTTP server fails cleanly with Not a RustDesk server instead of invalid wire type', async () => {
  const httpLikeServer = net.createServer((socket) => {
    socket.once('data', () => {
      socket.end('HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\n\r\nBad Request');
    });
  });

  await new Promise<void>((resolve) => httpLikeServer.listen(0, '127.0.0.1', resolve));
  const port = (httpLikeServer.address() as net.AddressInfo).port;

  try {
    await assert.rejects(
      () =>
        connectViaId(
          {
            peerId: '123456789',
            password: 'secret',
            idServer: `127.0.0.1:${port}`,
            relayServer: '',
            key: '',
            forceRelay: false,
          },
          selectCodecs(undefined),
        ),
      (err: unknown) => {
        const msg = err instanceof Error ? err.message : String(err);
        assert.match(msg, /Not a RustDesk/i);
        assert.match(msg, /HTTP/i);
        assert.doesNotMatch(msg, /invalid wire type/i);
        return true;
      },
    );
  } finally {
    httpLikeServer.close();
  }
});

test('buildLoginOptions forwards custom quality, fps, and codec negotiation safely', () => {
  const allCaps = selectCodecs({
    vp8: true,
    vp9: true,
    h264: true,
    h265: false,
    av1: true,
    reported: true,
  });

  // Auto mode avoids CPU-bound software AV1 and explicitly prefers H264
  const autoOpts = buildLoginOptions(allCaps, 'best', 60, 'auto') as any;
  assert.equal(autoOpts.custom_fps, 60);
  assert.equal(autoOpts.image_quality, 4); // ImageQualityValues.Best
  assert.equal(autoOpts.supported_decoding.ability_av1, 0);
  assert.equal(autoOpts.supported_decoding.ability_h264, 1);
  assert.equal(autoOpts.supported_decoding.prefer, PreferCodecValues.H264);

  // When H264 is unavailable, auto prefers VP8
  const vpOnlyCaps = selectCodecs({
    vp8: true,
    vp9: true,
    h264: false,
    h265: false,
    av1: true,
    reported: true,
  });
  const vpAutoOpts = buildLoginOptions(vpOnlyCaps, 'balanced', 30, 'auto') as any;
  assert.equal(vpAutoOpts.supported_decoding.ability_av1, 0);
  assert.equal(vpAutoOpts.supported_decoding.prefer, PreferCodecValues.VP8);

  // Explicit AV1 selection enables AV1 and requests PreferCodec.AV1
  const av1Opts = buildLoginOptions(allCaps, 'balanced', 30, 'av1') as any;
  assert.equal(av1Opts.supported_decoding.ability_av1, 1);
  assert.equal(av1Opts.supported_decoding.prefer, PreferCodecValues.AV1);

  // Live quality message forwards custom_fps, image_quality, and supported_decoding.prefer
  const liveMsg = toHostMessage(
    { type: 'quality', quality: 'balanced', fps: 30, codec: 'vp8' },
    allCaps,
  ) as any;
  assert.equal(liveMsg.misc.option.custom_fps, 30);
  assert.equal(liveMsg.misc.option.image_quality, 3);
  assert.equal(liveMsg.misc.option.supported_decoding.prefer, PreferCodecValues.VP8);
  assert.equal(liveMsg.misc.option.supported_decoding.ability_vp8, 1);
  assert.equal(liveMsg.misc.option.supported_decoding.ability_av1, 0);
});



