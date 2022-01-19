import * as message from "./message.js";
import * as rendezvous from "./rendezvous.js";
import * as sha256 from "fast-sha256";

type Keys = "message" | "open" | "close" | "error";

export default class Websock {
  _websocket: WebSocket;
  _eventHandlers: { [key in Keys]: Function };
  _buf: Uint8Array[];
  _status: any;

  constructor(uri: string) {
    this._eventHandlers = {
      message: (_: any) => {},
      open: () => {},
      close: () => {},
      error: () => {},
    };
    this._status = "";
    this._buf = [];
    this._websocket = new WebSocket(uri);
    this._websocket.onmessage = this._recv_message.bind(this);
    this._websocket.binaryType = "arraybuffer";
  }

  sendMessage(data: any) {
    this._websocket.send(
      message.Message.encode(message.Message.fromJSON(data)).finish()
    );
  }

  sendRendezvous(data: any) {
    this._websocket.send(
      rendezvous.RendezvousMessage.encode(
        rendezvous.RendezvousMessage.fromJSON(data)
      ).finish()
    );
  }

  parseMessage(data: Uint8Array) {
    return message.Message.decode(data);
  }

  parseRendezvous(data: Uint8Array) {
    return rendezvous.RendezvousMessage.decode(data);
  }

  // Event Handlers
  off(evt: Keys) {
    this._eventHandlers[evt] = () => {};
  }

  on(evt: Keys, handler: Function) {
    this._eventHandlers[evt] = handler;
  }

  async open(timeout: number = 12000): Promise<Websock> {
    return new Promise((resolve, reject) => {
      setTimeout(() => {
        if (this._status != "open") {
          reject(this._status || "timeout");
        }
      }, timeout);
      this._websocket.onopen = () => {
        this._status = "open";
        console.debug(">> WebSock.onopen");
        if (this._websocket?.protocol) {
          console.info(
            "Server choose sub-protocol: " + this._websocket.protocol
          );
        }

        this._eventHandlers.open();
        console.debug("<< WebSock.onopen");
        resolve(this);
      };
      this._websocket.onclose = (e) => {
        this._status = e;
        console.debug(">> WebSock.onclose");
        this._eventHandlers.close(e);
        console.debug("<< WebSock.onclose");
        reject(e);
      };
      this._websocket.onerror = (e) => {
        this._status = e;
        console.debug(">> WebSock.onerror: " + e);
        this._eventHandlers.error(e);
        console.debug("<< WebSock.onerror: " + e);
        reject(e);
      };
    });
  }

  async next(timeout = 12000): Promise<Uint8Array> {
    let func = (
      resolve: (value: Uint8Array) => void,
      reject: (reason: any) => void,
      tm0: number
    ) => {
      if (this._buf.length) {
        resolve(this._buf[0]);
        this._buf.splice(0, 1);
      } else {
        if (this._status != 'open') {
          reject(this._status);
          return;
        }
        if (new Date().getTime() > tm0 + timeout) {
          reject("timeout");
        } else {
          setTimeout(() => func(resolve, reject, tm0), 1);
        }
      }
    };
    return new Promise((resolve, reject) => {
      func(resolve, reject, new Date().getTime());
    });
  }

  close() {
    if (this._websocket) {
      if (
        this._websocket.readyState === WebSocket.OPEN ||
        this._websocket.readyState === WebSocket.CONNECTING
      ) {
        console.info("Closing WebSocket connection");
        this._websocket.close();
      }

      this._websocket.onmessage = () => {};
    }
  }

  _recv_message(e: any) {
    if (e.data instanceof window.ArrayBuffer) {
      let bytes = new Uint8Array(e.data);
      this._buf.push(bytes);
    }
    this._eventHandlers.message(e.data);
  }

  hash(datas: [Uint8Array]): Uint8Array {
    const hasher = new sha256.Hash();
    datas.forEach((data) => hasher.update(data));
    return hasher.digest();
  }
}

let ws = new Websock("ws://207.148.17.15:21118");
await ws.open();
console.log("ws connected");
// let punchHole = rendezvous.PunchHoleRequest.fromJSON({ id: '' });
// ws.send_rendezvous(rendezvous.RendezvousMessage.fromJSON({ punchHole }));
let testNatRequest = rendezvous.TestNatRequest.fromJSON({ serial: 0 });
ws.sendRendezvous({ testNatRequest });
let msg = ws.parseRendezvous(await ws.next());
console.log(msg);
