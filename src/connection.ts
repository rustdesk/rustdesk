import Websock from './websock';
import * as message from './message.js';
import * as rendezvous from './rendezvous.js';
import { loadVp9, loadOpus } from './codec';
import * as globals from './globals';

const PORT = 21116;
const HOST = 'rs-sg.rustdesk.com';
const licenceKey = '';
const SCHEMA = 'ws://';

export default class Connection {
  _msgs: any[];
  _ws: Websock | undefined;
  _interval: any;
  _id: string;

  constructor() {
    this._msgs = [];
    this._id = '';
    this._interval = setInterval(() => {
      while (this._msgs.length) {
        this._ws?.sendMessage(this._msgs[0]);
        this._msgs.splice(0, 1);
      }
    }, 1);
  }

  close() {
    clearInterval(this._interval);
    this._ws?.close();
  }

  async start(id: string) {
    const ws = new Websock(getDefaultUri());
    this._ws = ws;
    this._id = id;
    await ws.open();
    const connType = rendezvous.ConnType.DEFAULT_CONN;
    const natType = rendezvous.NatType.SYMMETRIC;
    const punchHoleRequest = rendezvous.PunchHoleRequest.fromPartial({
      id,
      licenceKey,
      connType,
      natType,
    });
    ws.sendRendezvous({ punchHoleRequest });
    const msg = ws.parseRendezvous(await ws.next());
    const phr = msg.punchHoleResponse;
    const rr = msg.relayResponse;
    if (phr) {
      if (phr.failure != rendezvous.PunchHoleResponse_Failure.UNKNOWN) {
        switch (phr?.failure) {
          case rendezvous.PunchHoleResponse_Failure.ID_NOT_EXIST:
            break;
        }
        ws.close();
      }
    } else if (rr) {
      await this.connectRelay(rr);
    }
  }

  async connectRelay(rr: rendezvous.RelayResponse) {
    const pk = rr.pk;
    let uri = rr.relayServer;
    if (uri) {
      uri = getrUriFromRs(uri);
    } else {
      uri = getDefaultUri(true);
    }
    const uuid = rr.uuid;
    const ws = new Websock(uri);
    await ws.open();
    console.log('Connected to relay server');
    this._ws = ws;
    const requestRelay = rendezvous.RequestRelay.fromPartial({
      licenceKey,
      uuid,
    });
    ws.sendRendezvous({ requestRelay });
    await this.secure(pk);
  }

  async secure(pk: Uint8Array | undefined) {
    if (pk) {
      const RS_PK = 'OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=';
      try {
        pk = await globals.verify(pk, RS_PK).catch();
        if (pk?.length != 32) {
          pk = undefined;
        }
      } catch (e) {
        console.error(e);
        pk = undefined;
      }
      if (!pk) console.error('Handshake failed: invalid public key from rendezvous server');
    }
    if (!pk) {
      // send an empty message out in case server is setting up secure and waiting for first message
      await this._ws?.sendMessage({});
      return;
    }
    const msg = this._ws?.parseMessage(await this._ws?.next());
    let signedId: any = msg?.signedId;
    if (!signedId) {
      console.error("Handshake failed: invalid message type");
      await this._ws?.sendMessage({});
      return;
    }
    try {
      signedId = await globals.verify(signedId.id, Uint8Array.from(pk!));
    } catch (e) {
      console.error(e);
      // fall back to non-secure connection in case pk mismatch
      console.error("pk mismatch, fall back to non-secure");
      const publicKey = message.PublicKey.fromPartial({});
      await this._ws?.sendMessage({ publicKey });
      return;
    }
    signedId = new TextDecoder().decode(signedId!);
    const tmp = signedId.split('\0');
    const id = tmp[0];
    let theirPk = tmp[1];
    if (id != this._id!) {
      console.error("Handshake failed: sign failure");
      await this._ws?.sendMessage({});
      return;
    }
    theirPk = globals.decodeBase64(theirPk);
    if (theirPk.length != 32) {
      console.error("Handshake failed: invalid public box key length from peer");
      await this._ws?.sendMessage({});
      return;
    }
    const [mySk, asymmetricValue] = globals.genBoxKeyPair();
    const secretKey = globals.genSecretKey();
    const symmetricValue = globals.seal(secretKey, theirPk, mySk);
    const publicKey = message.PublicKey.fromPartial({ asymmetricValue, symmetricValue });
    await this._ws?.sendMessage({ publicKey });
    this._ws?.setSecretKey(secretKey)
  }
}

async function testDelay() {
  const ws = new Websock(getDefaultUri(false));
  await ws.open();
  console.log(ws.latency());
}

function getDefaultUri(isRelay: Boolean = false): string {
  const host = localStorage.getItem('host');
  return SCHEMA + (host || HOST) + ':' + (PORT + (isRelay ? 3 : 2));
}

function getrUriFromRs(uri: string): string {
  if (uri.indexOf(':') > 0) {
    const tmp = uri.split(':');
    const port = parseInt(tmp[1]);
    uri = tmp[0] + ':' + (port + 2);
  } else {
    uri += ':' + (PORT + 3);
  }
  return SCHEMA + uri;
}