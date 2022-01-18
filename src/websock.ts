import * as message from "./message.js";
import * as rendezvous from "./rendezvous.js";
import * as sha256 from "fast-sha256";

type Keys = "message" | "open" | "close" | "error";

export default class Websock {
  _websocket: WebSocket;
  _eventHandlers: { [key in Keys]: Function };

  send_message(msg: message.Message) {
    this._websocket.send(message.Message.encode(msg).finish());
  }

  send_rendezvous(msg: rendezvous.RendezvousMessage) {
    this._websocket.send(rendezvous.RendezvousMessage.encode(msg).finish());
  }

  // Event Handlers
  off(evt: Keys) {
    this._eventHandlers[evt] = () => {};
  }

  on(evt: Keys, handler: Function) {
    this._eventHandlers[evt] = handler;
  }

  constructor(uri: string, protocols: string) {
    this._eventHandlers = {
      message: (_: string) => {},
      open: () => {},
      close: () => {},
      error: () => {},
    };

    this._websocket = new WebSocket(uri, protocols);

    this._websocket.onmessage = this._recv_message.bind(this);
    this._websocket.binaryType = "arraybuffer";
    this._websocket.onopen = () => {
      console.debug(">> WebSock.onopen");
      if (this._websocket.protocol) {
        console.info("Server choose sub-protocol: " + this._websocket.protocol);
      }

      this._eventHandlers.open();
      console.debug("<< WebSock.onopen");
    };
    this._websocket.onclose = (e) => {
      console.debug(">> WebSock.onclose");
      this._eventHandlers.close(e);
      console.debug("<< WebSock.onclose");
    };
    this._websocket.onerror = (e) => {
      console.debug(">> WebSock.onerror: " + e);
      this._eventHandlers.error(e);
      console.debug("<< WebSock.onerror: " + e);
    };
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
    }
  }

  hash(datas: [Uint8Array]): Uint8Array {
    const hasher = new sha256.Hash();
    datas.forEach((data) => hasher.update(data));
    return hasher.digest();
  }
}
