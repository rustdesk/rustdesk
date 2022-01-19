import Websock from "./websock";
import * as message from "./message.js";
import * as rendezvous from "./rendezvous.js";
import { loadVp9, loadOpus } from "./codec";

const URI = "ws://207.148.17.15";
const PORT = 21118;
const licenceKey = "";

loadVp9();
loadOpus();

export default class Client {
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

  async connect(id: string) {
    const ws = new Websock(URI + ":" + PORT);
    this._ws = ws;
    this._id = id;
    await ws.open();
    const connType = rendezvous.ConnType.DEFAULT_CONN;
    const natType = rendezvous.NatType.SYMMETRIC;
    const punchHoleRequest = rendezvous.PunchHoleRequest.fromJSON({
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
    if (uri.indexOf(':') > 0) {
      const tmp = uri.split(':');
      const port = parseInt(tmp[1]);
      uri = tmp[0] + ':' + (port + 2);
    } else {
      uri += ':' + (PORT + 1);
    }
    const uuid = rr.uuid;
    const ws = new Websock('ws://' + uri);
    await ws.open();
    console.log('Connected to relay server')
    this._ws = ws;
    const requestRelay = rendezvous.RequestRelay.fromJSON({
      licenceKey,
      uuid,
    });
    ws.sendRendezvous({ requestRelay });
    await this.secure(pk);
  }

  async secure(pk: Uint8Array | undefined) {
    //
  }
}

async function testDelay() {
  const ws = new Websock(URI + ":" + PORT);
  await ws.open();
  console.log(ws.latency());
}

testDelay();
new Client().connect("124931507");
