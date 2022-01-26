import Connection from "./connection";
import _sodium from "libsodium-wrappers";
import { ZSTDecoder } from 'zstddec';

const decompressor = new ZSTDDecoder();
await decompressor.init();

var currentFrame = undefined;
var events = [];

window.currentConnection = undefined;
window.getRgba = () => currentFrame;
window.getLanguage = () => navigator.language;

export function msgbox(type, title, text) {
  text = text.toLowerCase();
  var hasRetry = msgtype == "error"
    && title == "Connection Error"
    && !text.indexOf("offline") >= 0
    && !text.indexOf("exist") >= 0
    && !text.indexOf("handshake") >= 0
    && !text.indexOf("failed") >= 0
    && !text.indexOf("resolve") >= 0
    && !text.indexOf("mismatch") >= 0
    && !text.indexOf("manually") >= 0;
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
  window.currentConnection = conn;
}

export function getConn() {
  return window.currentConnection;
}

export function close() {
  getConn()?.close();
  setConn(undefined);
  currentFrame = undefined;
}

export function newConn() {
  window.currentConnection?.close();
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
  switch (name) {
    case 'connect': 
      newConn();
      break;
    case 'login':
      currentConnection.login(value.password, value.remember);
      break;
    case 'close':
      close();
      break;
    case 'refresh':
      currentConnection.refresh();
      break;
    case 'reconnect':
      currentConnection.reconnect();
      break;
    default:
      break;
  }
}

window.getByName = (name, value) => {
  switch (name) {
    case 'peers':
      return localStorage.getItem('peers');
      break;
    case 'event':
      if (events.length) {
        const e = events[0];
        events.splice(0, 1);
        return e;
      }
      break;
  }
}