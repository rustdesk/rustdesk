import Connection from "./connection";
import _sodium from "libsodium-wrappers";
import * as zstd from 'zstddec';
import { CursorData } from "./message";

const decompressor = new zstd.ZSTDDecoder();

var currentFrame = undefined;
var events = [];

window.curConn = undefined;
window.getRgba = () => currentFrame;
window.getLanguage = () => navigator.language;

export function msgbox(type, title, text) {
  const text2 = text.toLowerCase();
  var hasRetry = type == "error"
    && title == "Connection Error"
    && text2.indexOf("offline") < 0
    && text2.indexOf("exist") < 0
    && text2.indexOf("handshake") < 0
    && text2.indexOf("failed") < 0
    && text2.indexOf("resolve") < 0
    && text2.indexOf("mismatch") < 0
    && text2.indexOf("manually") < 0;
  events.push({ name: 'msgbox', type, title, text, hasRetry });
}

export function pushEvent(name, payload) {
  payload.name = name;
  events.push(payload);
}

export function draw(frame) {
  currentFrame = frame;
}

export function setConn(conn) {
  window.curConn = conn;
}

export function getConn() {
  return window.curConn;
}

export async function startConn(id) {
  try {
    await curConn.start(id);
  } catch (e) {
    console.log(e);
    msgbox('error', 'Error', String(e));
  }
}

export function close() {
  getConn()?.close();
  setConn(undefined);
  currentFrame = undefined;
}

export function newConn() {
  window.curConn?.close();
  const conn = new Connection();
  setConn(conn);
  return conn;
}

let sodium;
export async function verify(signed, pk) {
  if (!sodium) {
    await _sodium.ready;
    sodium = _sodium;
  }
  if (typeof pk == 'string') {
    pk = decodeBase64(pk);
  }
  return sodium.crypto_sign_open(signed, pk);
}

export function decodeBase64(pk) {
  return sodium.from_base64(pk, sodium.base64_variants.ORIGINAL);
}

export function genBoxKeyPair() {
  const pair = sodium.crypto_box_keypair();
  const sk = pair.privateKey;
  const pk = pair.publicKey;
  return [sk, pk];
}

export function genSecretKey() {
  return sodium.crypto_secretbox_keygen();
}

export function seal(unsigned, theirPk, ourSk) {
  const nonce = Uint8Array.from(Array(24).fill(0));
  return sodium.crypto_box_easy(unsigned, nonce, theirPk, ourSk);
}

function makeOnce(value) {
  var byteArray = Array(24).fill(0);

  for (var index = 0; index < byteArray.length && value > 0; index++) {
    var byte = value & 0xff;
    byteArray[index] = byte;
    value = (value - byte) / 256;
  }

  return Uint8Array.from(byteArray);
};

export function encrypt(unsigned, nonce, key) {
  return sodium.crypto_secretbox_easy(unsigned, makeOnce(nonce), key);
}

export function decrypt(signed, nonce, key) {
  return sodium.crypto_secretbox_open_easy(signed, makeOnce(nonce), key);
}

export function decompress(compressedArray) {
  const MAX = 1024 * 1024 * 64;
  const MIN = 1024 * 1024;
  let n = 30 * data.length;
  if (n > MAX) {
    n = MAX;
  }
  if (n < MIN) {
    n = MIN;
  }
  try {
    return decompressor.decode(compressedArray, n);
  } catch (e) {
    console.error('decompress failed: ' + e);
  }
}

window.setByName = (name, value) => {
  try {
    value = JSON.parse(value);
  } catch (e) { }
  switch (name) {
    case 'connect':
      newConn();
      startConn(value);
      break;
    case 'login':
      curConn.login(value.password, value.remember || false);
      break;
    case 'close':
      close();
      break;
    case 'refresh':
      curConn.refresh();
      break;
    case 'reconnect':
      curConn.reconnect();
      break;
    case 'toggle_option':
      curConn.toggleOption(value);
      break;
    case 'image_quality':
      curConn.setImageQuality(value);
      break;
    case 'lock_screen':
      curConn.lockScreen();
      break;
    case 'ctrl_alt_del':
      curConn.ctrlAltDe();
      break;
    case 'switch_display':
      curConn.switchDisplay(value);
      break;
    case 'remove':
      const peers = JSON.parse(localStorage.getItem('peers') || '{}');
      delete peers[value];
      localStorage.setItem('peers', JSON.stringify(peers));
      break;
    case 'input_key':
      curConn.inputKey(value.name, value.alt || false, value.ctrl || false, value.shift || false, value.command || false);
      break;
    case 'input_string':
      curConn.inputString(value);
      break;
    case 'send_mouse':
      let mask = 0;
      switch (value.type) {
        case 'down':
          mask = 1;
          break;
        case 'up':
          mask = 2;
          break;
        case 'wheel':
          mask = 3;
          break;
      }
      switch (value.buttons) {
        case 'left':
          mask |= 1 << 3;
          break;
        case 'right':
          mask |= 2 << 3;
          break;
        case 'wheel':
          mask |= 4 << 3;
      }
      curConn.inputMouse(mask, value.x || 0, value.y || 0, value.alt || false, value.ctrl || false, value.shift || false, value.command || false);
      break;
    case 'option':
      localStorage.setItem(value.name, value.value);
      break;
    case 'peer_option':
      curConn.setPeerOption(value.name, value.value);
      break;
    case 'input_os_password':
      curConn.inputOsPassword(value, true);
      break;
    default:
      break;
  }
}

window.getByName = (name, arg) => {
  try {
    arg = JSON.parse(arg);
  } catch (e) { }
  switch (name) {
    case 'peers':
      return localStorage.getItem('peers') || '[]';
      break;
    case 'remote_id':
      return localStorage.getItem('remote-id') || '';
      break;
    case 'remember':
      return curConn.getRemember();
      break;
    case 'event':
      if (events.length) {
        const e = events[0];
        events.splice(0, 1);
        return JSON.stringify(e);
      }
      break;
    case 'toggle_option':
      return curConn.getOption(arg);
      break;
    case 'option':
      return localStorage.getItem(arg);
      break;
    case 'image_quality':
      return curConn.getImageQuality();
      break;
    case 'translate':
      return arg.text;
      break;
    case 'peer_option':
      return curConn.getOption(arg);
      break;
    case 'test_if_valid_server':
      break;
  }
  return '';
}

window.init = () => {
  decompressor.init();
}