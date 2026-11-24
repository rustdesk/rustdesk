import crypto from 'node:crypto';
import {
  DEFAULT_DIRECT_PORT,
  RELAY_PORT,
  RENDEZVOUS_PORT,
  checkPort,
  directUsername,
  isValidRelayUuid,
  passwordResponse,
} from './auth.ts';
import { EncryptedFramedStream } from './bytesCodec.ts';
import {
  type CodecCaps,
  buildLoginOptions,
  buildRefreshVideoMessage,
} from './control.ts';
import {
  type HostHash,
  peerSignKey,
  readUntilHash,
  secureRendezvous,
} from './handshake.ts';
import {
  ConnTypeValues,
  NatTypeValues,
  PunchFailureValues,
  decodeAddrMangle,
  decodeMessage,
  decodeRendezvous,
  encodeMessage,
  encodeRendezvous,
} from './protos.ts';

export interface DirectTarget {
  host: string;
  port: number;
  password: string;
  quality?: string;
  fps?: number;
  preferCodec?: string;
}

export interface IdTarget {
  peerId: string;
  password: string;
  idServer: string;
  relayServer: string;
  key: string;
  forceRelay: boolean;
  quality?: string;
  fps?: number;
  preferCodec?: string;
}

export interface ConnectedHost {
  route: 'direct' | 'relay';
  privacyAllowed: boolean;
  clipboardAllowed: boolean;
  restartAllowed: boolean;
  stream: EncryptedFramedStream;
  peerInfo: Record<string, any>;
  username: string;
  encrypted: boolean;
}

async function loginHost(
  stream: EncryptedFramedStream,
  hash: HostHash,
  username: string,
  password: string,
  codecs: CodecCaps,
  encrypted: boolean,
  route: ConnectedHost['route'],
  permissions: { restart: boolean; privacy: boolean; clipboard: boolean },
  quality?: string,
  fps?: number,
  preferCodec?: string,
): Promise<ConnectedHost> {
  const loginOption = buildLoginOptions(codecs, quality, fps, preferCodec);
  const loginReq = {
    login_request: {
      username,
      password: passwordResponse(password, hash.salt, hash.challenge),
      my_id: 'web-client',
      my_name: 'Web Client',
      my_platform: 'Web',
      version: '1.5.3',
      option: loginOption,
    },
  };

  await stream.sendRaw(encodeMessage(loginReq));

  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    const remaining = Math.max(50, deadline - Date.now());
    const raw = await stream.next(remaining);
    if (!raw) throw new Error('host closed the connection before LoginResponse');
    const msg = decodeMessage(raw);
    if (msg.misc?.permission_info?.permission === 8) permissions.privacy = msg.misc.permission_info.enabled === true;
    if (msg.misc?.permission_info?.permission === 2) permissions.clipboard = msg.misc.permission_info.enabled === true;
    if (msg.misc?.permission_info?.permission === 5) permissions.restart = msg.misc.permission_info.enabled === true;
    if (msg.login_response) {
      if (msg.login_response.error !== undefined && msg.login_response.union === 'error') {
        const reason = String(msg.login_response.error || '').trim();
        throw new Error(reason || 'the host rejected the login');
      }
      if (msg.login_response.peer_info) {
        await stream.sendRaw(encodeMessage({ misc: { option: loginOption } }));
        await stream.sendRaw(encodeMessage(buildRefreshVideoMessage()));
        return {
          stream,
          route, restartAllowed: permissions.restart,
          privacyAllowed: permissions.privacy, clipboardAllowed: permissions.clipboard,
          peerInfo: msg.login_response.peer_info,
          username,
          encrypted,
        };
      }
    }
    if (msg.peer_info) {
      await stream.sendRaw(encodeMessage({ misc: { option: loginOption } }));
      await stream.sendRaw(encodeMessage(buildRefreshVideoMessage()));
      return {
        stream,
        route, restartAllowed: permissions.restart,
          privacyAllowed: permissions.privacy, clipboardAllowed: permissions.clipboard,
          peerInfo: msg.peer_info,
        username,
        encrypted,
      };
    }
    if (msg.auth_2fa) {
      throw new Error('The host requires two-factor authentication, which is not yet enabled on this gateway.');
    }
    if (msg.misc?.close_reason) {
      const reason = String(msg.misc.close_reason).trim();
      throw new Error(reason || 'the host closed the session');
    }
    if (msg.message_box?.text) {
      throw new Error(String(msg.message_box.text).trim());
    }
  }

  throw new Error('timed out waiting for LoginResponse');
}

export async function connectDirect(
  target: DirectTarget,
  codecs: CodecCaps,
): Promise<ConnectedHost> {
  const host = target.host.trim();
  if (!host) throw new Error('host address is empty');
  const port = target.port > 0 ? target.port : DEFAULT_DIRECT_PORT;
  const dial = checkPort(host, port);
  const username = await directUsername(host, dial.port);

  const stream = await EncryptedFramedStream.connect(dial.host, dial.port, 8_000);
  try {
    const permissions = { restart: true, privacy: true, clipboard: true };
    const { hash, encrypted } = await readUntilHash(stream, null, '', permissions);
    return await loginHost(
      stream,
      hash,
      username,
      target.password,
      codecs,
      encrypted,
      'direct',
      permissions,
      target.quality,
      target.fps,
      target.preferCodec,
    );
  } catch (err) {
    stream.close();
    throw err;
  }
}

/**
 * Reads the next non-KeyExchange RendezvousMessage from hbbs (mirrors rustdesk::get_next_nonkeyexchange_msg).
 */
async function nextNonKeyExchange(
  stream: EncryptedFramedStream,
  timeoutMs: number,
): Promise<Record<string, any> | null> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const remaining = Math.max(50, deadline - Date.now());
    const raw = await stream.next(remaining);
    if (!raw) return null;
    const msg = decodeRendezvous(raw);
    if (msg.key_exchange) continue;
    return msg;
  }
  return null;
}

/**
 * Coordinates a relay tunnel via hbbs (RequestRelay -> RelayResponse) and then connects to hbbr:21117.
 * Mirrors rustdesk/src/client.rs::request_relay + create_relay so NATed peers work seamlessly.
 */
async function requestRelayViaRendezvous(
  target: IdTarget,
  idDial: { host: string; port: number },
  suggestedRelayServer: string,
  secure: boolean,
  idServerSecured: boolean,
): Promise<{ peerStream: EncryptedFramedStream; pkBytes?: Uint8Array }> {
  const peerId = target.peerId.replace(/\s+/g, '');
  const relaySocket = await EncryptedFramedStream.connect(idDial.host, idDial.port, 10_000);
  try {
    if (idServerSecured && target.key.trim()) {
      await secureRendezvous(relaySocket, target.key, 1_500);
    }

    const uuid = crypto.randomUUID();
    const relayServer = target.relayServer.trim() || suggestedRelayServer.trim();

    await relaySocket.sendRaw(
      encodeRendezvous({
        request_relay: {
          id: peerId,
          uuid,
          relay_server: relayServer,
          secure,
          licence_key: target.key.trim(),
          conn_type: ConnTypeValues.DEFAULT_CONN,
        },
      }),
    );

    const resp = await nextNonKeyExchange(relaySocket, 12_000);
    if (!resp || !resp.relay_response) {
      throw new Error('id server did not confirm relay request');
    }
    const rr = resp.relay_response;
    const refuse = String(rr.refuse_reason ?? '').trim();
    if (refuse) throw new Error(refuse);

    const finalUuid = String(rr.uuid || uuid);
    if (!isValidRelayUuid(finalUuid)) {
      throw new Error('invalid relay uuid');
    }

    const finalRelayHost =
      target.relayServer.trim() ||
      String(rr.relay_server ?? '').trim() ||
      suggestedRelayServer.trim() ||
      idDial.host;
    if (!finalRelayHost) throw new Error('relay server is empty');

    const defaultRelayPort =
      idDial.port > 0 && idDial.port < 65535 ? idDial.port + 1 : RELAY_PORT;

    const peerStream = await connectRelayEndpoint(
      finalRelayHost,
      peerId,
      finalUuid,
      target.key.trim(),
      defaultRelayPort,
    );
    const pkBytes = rr.pk ? new Uint8Array(rr.pk) : undefined;
    relaySocket.close();
    return { peerStream, pkBytes };
  } catch (err) {
    relaySocket.close();
    throw err;
  }
}

async function connectRelayEndpoint(
  relayHost: string,
  peerId: string,
  uuid: string,
  licenceKey: string,
  defaultRelayPort = RELAY_PORT,
): Promise<EncryptedFramedStream> {
  const relayDial = checkPort(relayHost, defaultRelayPort);
  const peerStream = await EncryptedFramedStream.connect(relayDial.host, relayDial.port, 10_000);
  try {
    await peerStream.sendRaw(
      encodeRendezvous({
        request_relay: {
          id: peerId,
          uuid,
          licence_key: licenceKey,
          conn_type: ConnTypeValues.DEFAULT_CONN,
        },
      }),
    );
    return peerStream;
  } catch (err) {
    peerStream.close();
    throw err;
  }
}

export async function connectViaId(
  target: IdTarget,
  codecs: CodecCaps,
): Promise<ConnectedHost> {
  const peerId = target.peerId.replace(/\s+/g, '');
  const idServer = target.idServer.trim();
  if (!peerId) throw new Error('peer id is empty');
  if (!idServer) throw new Error('id server is empty');

  const idDial = checkPort(idServer, RENDEZVOUS_PORT);
  const idSocket = await EncryptedFramedStream.connect(idDial.host, idDial.port, 10_000);

  let peerStream: EncryptedFramedStream | null = null;
  let idServerSecured = false;
  try {
    if (target.key.trim()) {
      idServerSecured = await secureRendezvous(idSocket, target.key);
    }

    await idSocket.sendRaw(
      encodeRendezvous({
        punch_hole_request: {
          id: peerId,
          licence_key: target.key.trim(),
          nat_type: target.forceRelay ? NatTypeValues.SYMMETRIC : NatTypeValues.UNKNOWN_NAT,
          conn_type: ConnTypeValues.DEFAULT_CONN,
          force_relay: target.forceRelay,
          version: '1.5.3',
        },
      }),
    );

    const deadline = Date.now() + 20_000;
    while (Date.now() < deadline) {
      const remaining = Math.max(50, deadline - Date.now());
      const msg = await nextNonKeyExchange(idSocket, remaining);
      if (!msg) throw new Error('id server closed the connection');

      if (msg.punch_hole_response) {
        const ph = msg.punch_hole_response;
        const addrBytes: Buffer | undefined = ph.socket_addr;
        const relayHint = String(ph.relay_server ?? '').trim();

        if (!addrBytes || addrBytes.length === 0) {
          const other = String(ph.other_failure ?? '').trim();
          if (other) throw new Error(other);
          switch (ph.failure) {
            case PunchFailureValues.ID_NOT_EXIST:
              throw new Error('ID does not exist');
            case PunchFailureValues.OFFLINE:
              throw new Error('remote desktop is offline');
            case PunchFailureValues.LICENSE_MISMATCH:
              throw new Error('key mismatch');
            case PunchFailureValues.LICENSE_OVERUSE:
              throw new Error('key overuse');
            default:
              throw new Error('punch hole failed');
          }
        }

        const signedPk = ph.pk ? new Uint8Array(ph.pk) : undefined;
        const signPk = peerSignKey(target.key, signedPk, peerId);

        // Try direct TCP if forceRelay is false and address decoded cleanly;
        // if NAT blocks direct TCP (standard Internet case), fall back automatically to hbbr relay.
        if (!target.forceRelay) {
          const decodedAddr = decodeAddrMangle(new Uint8Array(addrBytes));
          if (decodedAddr) {
            try {
              peerStream = await EncryptedFramedStream.connect(
                decodedAddr.host,
                decodedAddr.port,
                3_500,
              );
              const permissions = { restart: true, privacy: true, clipboard: true };
              const { hash, encrypted } = await readUntilHash(peerStream, signPk, peerId, permissions);
              idSocket.close();
              return await loginHost(
                peerStream,
                hash,
                peerId,
                target.password,
                codecs,
                encrypted,
                'direct',
                permissions,
                target.quality,
                target.fps,
                target.preferCodec,
              );
            } catch {
              if (peerStream) {
                peerStream.close();
                peerStream = null;
              }
            }
          }
        }

        // Seamless automatic relay fallback via hbbs -> hbbr (mirrors desktop RustDesk client)
        idSocket.close();
        const relay = await requestRelayViaRendezvous(
          target,
          idDial,
          relayHint,
          Boolean(signPk),
          idServerSecured,
        );
        peerStream = relay.peerStream;
        const finalSignPk =
          signPk ?? peerSignKey(target.key, relay.pkBytes, peerId);
        const permissions = { restart: true, privacy: true, clipboard: true };
        const { hash, encrypted } = await readUntilHash(peerStream, finalSignPk, peerId, permissions);
        return await loginHost(
          peerStream,
          hash,
          peerId,
          target.password,
          codecs,
          encrypted,
          'relay',
          permissions,
          target.quality,
          target.fps,
          target.preferCodec,
        );
      }

      if (msg.relay_response) {
        const rr = msg.relay_response;
        const refuse = String(rr.refuse_reason ?? '').trim();
        if (refuse) throw new Error(refuse);

        const uuid = String(rr.uuid ?? '');
        if (!isValidRelayUuid(uuid)) {
          throw new Error('invalid relay uuid');
        }

        const relayHost =
          target.relayServer.trim() || String(rr.relay_server ?? '').trim() || idDial.host;
        if (!relayHost) throw new Error('relay server is empty');

        const defaultRelayPort =
          idDial.port > 0 && idDial.port < 65535 ? idDial.port + 1 : RELAY_PORT;

        peerStream = await connectRelayEndpoint(
          relayHost,
          peerId,
          uuid,
          target.key.trim(),
          defaultRelayPort,
        );
        const pkBytes = rr.pk ? new Uint8Array(rr.pk) : undefined;
        const signPk = peerSignKey(target.key, pkBytes, peerId);
        const permissions = { restart: true, privacy: true, clipboard: true };
        const { hash, encrypted } = await readUntilHash(peerStream, signPk, peerId, permissions);
        idSocket.close();
        return await loginHost(
          peerStream,
          hash,
          peerId,
          target.password,
          codecs,
          encrypted,
          'relay',
          permissions,
          target.quality,
          target.fps,
          target.preferCodec,
        );
      }
    }

    throw new Error('timed out waiting for rendezvous response');
  } catch (err) {
    idSocket.close();
    if (peerStream) peerStream.close();
    throw err;
  }
}
