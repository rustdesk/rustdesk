import crypto from 'node:crypto';
import nacl from 'tweetnacl';
import {
  type EncryptedFramedStream,
  KX_PARAMS_DOMAIN,
  kxVersionFor,
} from './bytesCodec.ts';
import {
  decodeIdPk,
  decodeKxParams,
  decodeMessage,
  decodeRendezvous,
  encodeMessage,
  encodeRendezvous,
} from './protos.ts';

export interface HostHash {
  salt: string;
  challenge: string;
}

/**
 * Decodes a standard base64 Ed25519 server public key (must be 32 bytes).
 */
export function rsPublicKey(keyB64: string): Uint8Array {
  const trimmed = keyB64.trim();
  const decoded = Buffer.from(trimmed, 'base64');
  if (decoded.length !== 32) {
    throw new Error('server public key must be 32 bytes of standard base64');
  }
  return new Uint8Array(decoded);
}

/**
 * Generates an ephemeral Curve25519 keypair + random 32-byte symmetric key,
 * sealing the symmetric key to theirPk with a 24-byte zero nonce (matching hbb_common).
 */
export function createSymmetricKeyMsg(theirPk: Uint8Array): {
  asymmetric: Uint8Array;
  symmetric: Uint8Array;
  key: Uint8Array;
} {
  const pair = nacl.box.keyPair();
  const key = new Uint8Array(crypto.randomBytes(32));
  const zeroNonce = new Uint8Array(24);
  const symmetric = nacl.box(key, zeroNonce, theirPk, pair.secretKey);
  return {
    asymmetric: pair.publicKey,
    symmetric,
    key,
  };
}

async function recvHash(stream: EncryptedFramedStream, permissions: { restart: boolean; privacy: boolean; clipboard: boolean }, timeoutMs = 10_000): Promise<HostHash> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const remaining = Math.max(50, deadline - Date.now());
    const raw = await stream.next(remaining);
    if (!raw) throw new Error('host closed the connection before sending Hash');
    const msg = decodeMessage(raw);
    if (msg.misc?.permission_info?.permission === 8) permissions.privacy = msg.misc.permission_info.enabled === true;
    if (msg.misc?.permission_info?.permission === 2) permissions.clipboard = msg.misc.permission_info.enabled === true;
    if (msg.misc?.permission_info?.permission === 5) permissions.restart = msg.misc.permission_info.enabled === true;
    if (msg.hash) {
      return {
        salt: String(msg.hash.salt ?? ''),
        challenge: String(msg.hash.challenge ?? ''),
      };
    }
  }
  throw new Error('timed out waiting for host Hash');
}

/**
 * Reads the initial Hash (direct plaintext) or SignedId (NaCl handshake) from the host.
 */
export async function readUntilHash(
  stream: EncryptedFramedStream,
  peerSignPk: Uint8Array | null,
  expectedId: string,
  permissions = { restart: true, privacy: true, clipboard: true },
): Promise<{ hash: HostHash; encrypted: boolean }> {
  const raw = await stream.next(10_000);
  if (!raw) throw new Error('host closed the connection during handshake');
  const msg = decodeMessage(raw);

  if (msg.hash) {
    return {
      hash: {
        salt: String(msg.hash.salt ?? ''),
        challenge: String(msg.hash.challenge ?? ''),
      },
      encrypted: false,
    };
  }

  if (msg.signed_id) {
    const signedBytes = new Uint8Array(msg.signed_id.id ?? []);
    let opened: { id: string; pk: Uint8Array; kx_version: number } | null = null;

    if (peerSignPk) {
      const verified = nacl.sign.open(signedBytes, peerSignPk);
      if (verified) {
        const parsed = decodeIdPk(verified);
        if (parsed.pk.length === 32 && (!expectedId || parsed.id === expectedId)) {
          opened = parsed;
        }
      }
    }

    if (!opened) {
      // Matches desktop client behavior: send empty PublicKey and continue unencrypted
      await stream.sendRaw(encodeMessage({ public_key: {} }));
      const hash = await recvHash(stream, permissions);
      return { hash, encrypted: false };
    }

    const { asymmetric, symmetric, key } = createSymmetricKeyMsg(opened.pk);
    const picked = kxVersionFor(opened.kx_version);
    await stream.sendRaw(
      encodeMessage({
        public_key: {
          asymmetric_value: Buffer.from(asymmetric),
          symmetric_value: Buffer.from(symmetric),
          kx_version: picked,
        },
      }),
    );

    stream.setNegotiatedKey(key, true, {
      initiatorPk: asymmetric,
      responderPk: opened.pk,
      advertised: opened.kx_version,
      picked,
    });

    const hash = await recvHash(stream, permissions);
    return { hash, encrypted: true };
  }

  throw new Error('expected Hash or SignedId, got another message');
}

/**
 * Verifies the rendezvous server's signed IdPk for the peer.
 */
export function peerSignKey(
  serverKeyB64: string,
  signedPk: Uint8Array | undefined,
  expectedPeerId: string,
): Uint8Array | null {
  if (!serverKeyB64.trim() || !signedPk || signedPk.length === 0) {
    return null;
  }
  const rsPk = rsPublicKey(serverKeyB64);
  const verified = nacl.sign.open(signedPk, rsPk);
  if (!verified) {
    throw new Error('identity server signature verification failed');
  }
  const opened = decodeIdPk(verified);
  const normalizedExpected = expectedPeerId.replace(/\s+/g, '');
  if (opened.id !== normalizedExpected) {
    throw new Error(`identity server signed a different id (${opened.id})`);
  }
  if (opened.pk.length !== 32) {
    throw new Error('peer sign public key length is invalid');
  }
  return opened.pk;
}

/**
 * Performs optional KeyExchange encryption with hbbs when server public key is provided.
 */
export async function secureRendezvous(
  stream: EncryptedFramedStream,
  keyB64: string,
  probeTimeoutMs = 700,
): Promise<boolean> {
  if (!keyB64.trim()) return false;
  const rsPk = rsPublicKey(keyB64);
  let raw: Buffer | null;
  try {
    raw = await stream.next(probeTimeoutMs);
  } catch (err) {
    // Open-source hbbs servers do not initiate KeyExchange on TCP 21116;
    // they wait for the client to send PunchHoleRequest first while still
    // signing peer keys with the server key.
    if (err instanceof Error && /timed out/i.test(err.message)) {
      return false;
    }
    throw err;
  }
  if (!raw) throw new Error('rendezvous server closed connection during key exchange');
  const msg = decodeRendezvous(raw);
  const ex = msg.key_exchange;
  if (!ex) {
    return false;
  }
  const keys: Buffer[] = ex.keys ?? [];
  if (keys.length !== 1) {
    throw new Error(`expected 1 key in KeyExchange, got ${keys.length}`);
  }
  const their = nacl.sign.open(new Uint8Array(keys[0]), rsPk);
  if (!their || their.length !== 32) {
    throw new Error('key mismatch: rendezvous server key signature is invalid');
  }

  const version = Number(ex.version ?? 0);
  const signedParams: Buffer | undefined = ex.signed_params;
  if ((their[31] & 0x80) !== 0 || (signedParams && signedParams.length > 0)) {
    if (!signedParams || signedParams.length === 0) {
      throw new Error('missing signed_params in rendezvous KeyExchange');
    }
    const openedParams = nacl.sign.open(new Uint8Array(signedParams), rsPk);
    if (!openedParams || openedParams.length < KX_PARAMS_DOMAIN.length) {
      throw new Error('invalid signed_params signature in rendezvous KeyExchange');
    }
    const prefix = Buffer.from(openedParams.subarray(0, KX_PARAMS_DOMAIN.length));
    if (!prefix.equals(KX_PARAMS_DOMAIN)) {
      throw new Error('invalid signed_params domain prefix');
    }
    const params = decodeKxParams(openedParams.subarray(KX_PARAMS_DOMAIN.length));
    if (
      params.version !== version ||
      !Buffer.from(params.pk).equals(Buffer.from(their))
    ) {
      throw new Error('signed_params mismatch in rendezvous KeyExchange');
    }
  }

  const { asymmetric, symmetric, key } = createSymmetricKeyMsg(their);
  const picked = kxVersionFor(version);
  await stream.sendRaw(
    encodeRendezvous({
      key_exchange: {
        keys: [Buffer.from(asymmetric), Buffer.from(symmetric)],
        version: picked,
      },
    }),
  );

  stream.setNegotiatedKey(key, true, {
    initiatorPk: asymmetric,
    responderPk: their,
    advertised: version,
    picked,
  });

  return true;
}
