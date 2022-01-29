import Websock from "./websock";
import * as message from "./message.js";
import * as rendezvous from "./rendezvous.js";
import { loadVp9, loadOpus } from "./codec";
import * as sha256 from "fast-sha256";
import * as globals from "./globals";

const PORT = 21116;
const HOST = "rs-sg.rustdesk.com";
const SCHEMA = "ws://";

type MsgboxCallback = (type: string, title: string, text: string) => void;
type DrawCallback = (data: Uint8Array) => void;

export default class Connection {
  _msgs: any[];
  _ws: Websock | undefined;
  _interval: any;
  _id: string;
  _hash: message.Hash | undefined;
  _msgbox: MsgboxCallback;
  _draw: DrawCallback;
  _peerInfo: message.PeerInfo | undefined;
  _firstFrame: Boolean | undefined;
  _videoDecoder: any;
  _audioDecoder: any;
  _password: string | undefined;
  _options: any;

  constructor() {
    this._msgbox = globals.msgbox;
    this._draw = globals.draw;
    this._msgs = [];
    this._id = "";
  }

  async start(id: string) {
    try {
      this._options =
        JSON.parse(localStorage.getItem("peers") || "{}")[id] || {};
    } catch (e) {
      this._options = {};
    }
    this._interval = setInterval(() => {
      while (this._msgs.length) {
        this._ws?.sendMessage(this._msgs[0]);
        this._msgs.splice(0, 1);
      }
    }, 1);
    loadVp9((decoder: any) => {
      this._videoDecoder = decoder;
      console.log("vp9 loaded");
      console.log(decoder);
    });
    loadOpus((decoder: any) => {
      this._audioDecoder = decoder;
      console.log("opus loaded");
    });
    const uri = getDefaultUri();
    const ws = new Websock(uri);
    this._ws = ws;
    this._id = id;
    console.log(
      new Date() + ": Conntecting to rendezvoous server: " + uri + ", for " + id
    );
    await ws.open();
    console.log(new Date() + ": Connected to rendezvoous server");
    const connType = rendezvous.ConnType.DEFAULT_CONN;
    const natType = rendezvous.NatType.SYMMETRIC;
    const punchHoleRequest = rendezvous.PunchHoleRequest.fromPartial({
      id,
      licenceKey: localStorage.getItem("key") || undefined,
      connType,
      natType,
    });
    ws.sendRendezvous({ punchHoleRequest });
    const msg = ws.parseRendezvous(await ws.next());
    ws.close();
    console.log(new Date() + ": Got relay response");
    const phr = msg.punchHoleResponse;
    const rr = msg.relayResponse;
    if (phr) {
      if (phr.failure != rendezvous.PunchHoleResponse_Failure.UNRECOGNIZED) {
        switch (phr?.failure) {
          case rendezvous.PunchHoleResponse_Failure.ID_NOT_EXIST:
            this.msgbox("error", "Error", "ID does not exist");
            break;
          case rendezvous.PunchHoleResponse_Failure.OFFLINE:
            this.msgbox("error", "Error", "Remote desktop is offline");
            break;
          case rendezvous.PunchHoleResponse_Failure.LICENSE_MISMATCH:
            this.msgbox("error", "Error", "Key mismatch");
            break;
          default:
            if (phr?.otherFailure) {
              this.msgbox("error", "Error", phr?.otherFailure);
            }
        }
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
    console.log(new Date() + ": Connecting to relay server: " + uri);
    const ws = new Websock(uri);
    await ws.open();
    console.log(new Date() + ": Connected to relay server");
    this._ws = ws;
    const requestRelay = rendezvous.RequestRelay.fromPartial({
      licenceKey: localStorage.getItem("key") || undefined,
      uuid,
    });
    ws.sendRendezvous({ requestRelay });
    const secure = (await this.secure(pk)) || false;
    globals.pushEvent("connection_ready", { secure, direct: false });
    await this.msgLoop();
  }

  async secure(pk: Uint8Array | undefined) {
    if (pk) {
      const RS_PK = "OeVuKk5nlHiXp+APNn0Y3pC1Iwpwn44JGqrQCsWqmBw=";
      try {
        pk = await globals.verify(pk, RS_PK).catch();
        if (pk?.length != 32) {
          pk = undefined;
        }
      } catch (e) {
        console.error(e);
        pk = undefined;
      }
      if (!pk)
        console.error(
          "Handshake failed: invalid public key from rendezvous server"
        );
    }
    if (!pk) {
      // send an empty message out in case server is setting up secure and waiting for first message
      this._ws?.sendMessage({});
      return;
    }
    const msg = this._ws?.parseMessage(await this._ws?.next());
    let signedId: any = msg?.signedId;
    if (!signedId) {
      console.error("Handshake failed: invalid message type");
      this._ws?.sendMessage({});
      return;
    }
    try {
      signedId = await globals.verify(signedId.id, Uint8Array.from(pk!));
    } catch (e) {
      console.error(e);
      // fall back to non-secure connection in case pk mismatch
      console.error("pk mismatch, fall back to non-secure");
      const publicKey = message.PublicKey.fromPartial({});
      this._ws?.sendMessage({ publicKey });
      return;
    }
    signedId = new TextDecoder().decode(signedId!);
    const tmp = signedId.split("\0");
    const id = tmp[0];
    let theirPk = tmp[1];
    if (id != this._id!) {
      console.error("Handshake failed: sign failure");
      this._ws?.sendMessage({});
      return;
    }
    theirPk = globals.decodeBase64(theirPk);
    if (theirPk.length != 32) {
      console.error(
        "Handshake failed: invalid public box key length from peer"
      );
      this._ws?.sendMessage({});
      return;
    }
    const [mySk, asymmetricValue] = globals.genBoxKeyPair();
    const secretKey = globals.genSecretKey();
    const symmetricValue = globals.seal(secretKey, theirPk, mySk);
    const publicKey = message.PublicKey.fromPartial({
      asymmetricValue,
      symmetricValue,
    });
    this._ws?.sendMessage({ publicKey });
    this._ws?.setSecretKey(secretKey);
    return true;
  }

  async msgLoop() {
    while (true) {
      const msg = this._ws?.parseMessage(await this._ws?.next());
      if (msg?.hash) {
        this._hash = msg?.hash;
        if (!this._password)
          this.msgbox("input-password", "Password Required", "");
        this.login(this._password);
      } else if (msg?.testDelay) {
        const testDelay = msg?.testDelay;
        if (!testDelay.fromClient) {
          this._ws?.sendMessage({ testDelay });
        }
      } else if (msg?.loginResponse) {
        const r = msg?.loginResponse;
        if (r.error) {
          this.msgbox("error", "Error", r.error);
        } else if (r.peerInfo) {
          this.handlePeerInfo(r.peerInfo);
        }
      } else if (msg?.videoFrame) {
        this.handleVideoFrame(msg?.videoFrame!);
      } else if (msg?.clipboard) {
        const cb = msg?.clipboard;
        if (cb.compress) cb.content = await globals.decompress(cb.content)!;
        globals.pushEvent("clipboard", cb);
      } else if (msg?.cursorData) {
        const cd = msg?.cursorData;
        cd.colors = await globals.decompress(cd.colors)!;
        globals.pushEvent("cursor_data", cd);
      } else if (msg?.cursorId) {
        globals.pushEvent("cursor_id", { id: msg?.cursorId });
      } else if (msg?.cursorPosition) {
        globals.pushEvent("cursor_position", msg?.cursorPosition);
      } else if (msg?.misc) {
        this.handleMisc(msg?.misc);
      } else if (msg?.audioFrame) {
        //
      }
    }
  }

  msgbox(type_: string, title: string, text: string) {
    this._msgbox?.(type_, title, text);
  }

  draw(frame: any) {
    this._draw?.(frame);
  }

  close() {
    this._msgs = [];
    clearInterval(this._interval);
    this._ws?.close();
    this._videoDecoder?.close();
    this._audioDecoder?.close();
  }

  refresh() {
    const misc = message.Misc.fromPartial({
      refreshVideo: true,
    });
    this._ws?.sendMessage({ misc });
  }

  setMsgbox(callback: MsgboxCallback) {
    this._msgbox = callback;
  }

  setDraw(callback: DrawCallback) {
    this._draw = callback;
  }

  login(password: string | undefined, _remember: Boolean = false) {
    this._password = password;
    if (password) {
      const salt = this._hash?.salt;
      let p = hash([password, salt!]);
      const challenge = this._hash?.challenge;
      p = hash([p, challenge!]);
      this.msgbox("connecting", "Connecting...", "Logging in...");
      this._sendLoginMessage(p);
    } else {
      this._sendLoginMessage();
    }
  }

  async reconnect() {
    this.close();
    await this.start(this._id);
  }

  _sendLoginMessage(password: Uint8Array | undefined = undefined) {
    const loginRequest = message.LoginRequest.fromPartial({
      username: this._id!,
      myId: "web", // to-do
      myName: "web", // to-do
      password,
      option: this.getOptionMessage(),
    });
    this._ws?.sendMessage({ loginRequest });
  }

  getOptionMessage(): message.OptionMessage | undefined {
    let n = 0;
    const msg = message.OptionMessage.fromPartial({});
    const q = this.getImageQualityEnum(this._options["image-quality"], true);
    const yes = message.OptionMessage_BoolOption.Yes;
    if (q != undefined) {
      msg.imageQuality = q;
      n += 1;
    }
    if (this._options["show-remote-cursor"]) {
      msg.showRemoteCursor = yes;
      n += 1;
    }
    if (this._options["lock-after-session-end"]) {
      msg.lockAfterSessionEnd = yes;
      n += 1;
    }
    if (this._options["privacy-mode"]) {
      msg.privacyMode = yes;
      n += 1;
    }
    if (this._options["disable-audio"]) {
      msg.disableAudio = yes;
      n += 1;
    }
    if (this._options["disable-clipboard"]) {
      msg.disableClipboard = yes;
      n += 1;
    }
    return n > 0 ? msg : undefined;
  }

  handleVideoFrame(vf: message.VideoFrame) {
    if (!this._firstFrame) {
      this.msgbox("", "", "");
      this._firstFrame = true;
    }
    if (vf.vp9s) {
      const dec = this._videoDecoder;
      // dec.sync();
      vf.vp9s.frames.forEach((f) => {
        dec.processFrame(f.data.slice(0).buffer, (ok: any) => {
          if (ok && dec.frameBuffer) {
            this.draw(dec.frameBuffer);
          }
        });
      });
    }
  }

  handlePeerInfo(pi: message.PeerInfo) {
    this._peerInfo = pi;
    if (pi.displays.length == 0) {
      this.msgbox("error", "Remote Error", "No Display");
      return;
    }
    this.msgbox("success", "Successful", "Connected, waiting for image...");
    globals.pushEvent("peer_info", pi);
  }

  handleMisc(misc: message.Misc) {
    if (misc.audioFormat) {
      //
    } else if (misc.permissionInfo) {
      const p = misc.permissionInfo;
      console.info("Change permission " + p.permission + " -> " + p.enabled);
      let name;
      switch (p.permission) {
        case message.PermissionInfo_Permission.Keyboard:
          name = "keyboard";
          break;
        case message.PermissionInfo_Permission.Clipboard:
          name = "clipboard";
          break;
        case message.PermissionInfo_Permission.Audio:
          name = "audio";
          break;
        default:
          return;
      }
      globals.pushEvent("permission", { [name]: p.enabled });
    } else if (misc.switchDisplay) {
      globals.pushEvent("switch_display", misc.switchDisplay);
    } else if (misc.closeReason) {
      this.msgbox("error", "Connection Error", misc.closeReason);
    }
  }

  getRemember(): any {
    return this._options["remember"];
  }

  getOption(name: string): any {
    return this._options[name];
  }

  setOption(name: string, value: any) {
    this._options[name] = value;
  }

  inputKey() {
    // name: string, x: number, y: number, alt: Boolean, ctrl: Boolean, shift: Boolean, command: Boolean) {
  }

  inputString(seq: string) {
    const keyEvent = message.KeyEvent.fromPartial({ seq });
    this._ws?.sendMessage({ keyEvent });
  }

  inputMouse(
    mask: number,
    x: number,
    y: number,
    alt: Boolean,
    ctrl: Boolean,
    shift: Boolean,
    command: Boolean
  ) {
    const mouseEvent = message.MouseEvent.fromPartial({ mask, x, y });
    if (alt) mouseEvent.modifiers.push(message.ControlKey.Alt);
    if (ctrl) mouseEvent.modifiers.push(message.ControlKey.Control);
    if (shift) mouseEvent.modifiers.push(message.ControlKey.Shift);
    if (command) mouseEvent.modifiers.push(message.ControlKey.Meta);
    this._ws?.sendMessage({ mouseEvent });
  }

  toggleOption(name: string) {
    const v = !this._options[name];
    const option = message.OptionMessage.fromPartial({});
    const v2 = v
      ? message.OptionMessage_BoolOption.Yes
      : message.OptionMessage_BoolOption.No;
    switch (name) {
      case "show-remote-cursor":
        option.showRemoteCursor = v2;
        break;
      case "disable-audio":
        option.disableAudio = v2;
        break;
      case "disable-clipboard":
        option.disableClipboard = v2;
        break;
      case "lock-after-session-end":
        option.lockAfterSessionEnd = v2;
        break;
      case "privacy-mode":
        option.privacyMode = v2;
        break;
      case "block-input":
        option.blockInput = message.OptionMessage_BoolOption.Yes;
        break;
      case "unblock-input":
        option.blockInput = message.OptionMessage_BoolOption.No;
        break;
      default:
        return;
    }
    if (name.indexOf("block-input") < 0) this.setOption(name, v);
    const misc = message.Misc.fromPartial({ option });
    this._ws?.sendMessage({ misc });
  }

  getImageQualityEnum(
    value: string,
    ignoreDefault: Boolean
  ): message.ImageQuality | undefined {
    switch (value) {
      case "low":
        return message.ImageQuality.Low;
      case "best":
        return message.ImageQuality.Best;
      case "balanced":
        return ignoreDefault ? undefined : message.ImageQuality.Balanced;
      default:
        return undefined;
    }
  }

  setImageQuality(value: string) {
    this.setOption("image-quality", value);
    const imageQuality = this.getImageQualityEnum(value, false);
    if (imageQuality == undefined) return;
    const option = message.OptionMessage.fromPartial({ imageQuality });
    const misc = message.Misc.fromPartial({ option });
    this._ws?.sendMessage({ misc });
  }
}

// @ts-ignore
async function testDelay() {
  const ws = new Websock(getDefaultUri(false));
  await ws.open();
  console.log(ws.latency());
}

function getDefaultUri(isRelay: Boolean = false): string {
  const host = localStorage.getItem("custom-rendezvous-server");
  return SCHEMA + (host || HOST) + ":" + (PORT + (isRelay ? 3 : 2));
}

function getrUriFromRs(uri: string): string {
  if (uri.indexOf(":") > 0) {
    const tmp = uri.split(":");
    const port = parseInt(tmp[1]);
    uri = tmp[0] + ":" + (port + 2);
  } else {
    uri += ":" + (PORT + 3);
  }
  return SCHEMA + uri;
}

function hash(datas: (string | Uint8Array)[]): Uint8Array {
  const hasher = new sha256.Hash();
  datas.forEach((data) => {
    if (typeof data == "string") {
      data = new TextEncoder().encode(data);
    }
    return hasher.update(data);
  });
  return hasher.digest();
}
