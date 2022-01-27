import Connection from "./connection";
import _sodium from "libsodium-wrappers";
import * as zstd from 'zstddec';
import { CursorData } from "./message";
import { loadOpus, loadVp9 } from "./codec";

var decompressor;
var wasmExports;

var currentFrame = undefined;
var events = [];

window.curConn = undefined;
window.getRgba = () => {
  const tmp = currentFrame;
  currentFrame = undefined;
  return tmp || null;
}
window.getLanguage = () => navigator.language;

export function msgbox(type, title, text) {
  if (!events) return;
  if (!type) return;
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
  if (!events) return;
  payload.name = name;
  events.push(payload);
}

export function draw(frame) {
  currentFrame = I420ToARGB(frame);
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
  events = undefined;
}

export function newConn() {
  window.curConn?.close();
  events = [];
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

export async function decompress(compressedArray) {
  const MAX = 1024 * 1024 * 64;
  const MIN = 1024 * 1024;
  let n = 30 * compressedArray.length;
  if (n > MAX) {
    n = MAX;
  }
  if (n < MIN) {
    n = MIN;
  }
  try {
    if (!decompressor) {
      await initZstd();
    }
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
      startConn(String(value));
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
      if (events && events.length) {
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

window.init = async () => {
  await initZstd();
}

let yPtr, yPtrLen, uPtr, uPtrLen, vPtr, vPtrLen, outPtr, outPtrLen;
// let testSpeed = [0, 0];
export function I420ToARGB(yb) {
  if (!wasmExports) return;
  // testSpeed[0] += 1;
  const tm0 = new Date().getTime();
  const { malloc, free, memory } = wasmExports;
  const HEAPU8 = new Uint8Array(memory.buffer);
  let n = yb.y.bytes.length;
  if (yPtrLen != n) {
    if (yPtr) free(yPtr);
    yPtrLen = n;
    yPtr = malloc(n);
  }
  HEAPU8.set(yb.y.bytes, yPtr);
  n = yb.u.bytes.length;
  if (uPtrLen != n) {
    if (uPtr) free(uPtr);
    uPtrLen = n;
    uPtr = malloc(n);
  }
  HEAPU8.set(yb.u.bytes, uPtr);
  n = yb.v.bytes.length;
  if (vPtrLen != n) {
    if (vPtr) free(vPtr);
    vPtrLen = n;
    vPtr = malloc(n);
  }
  HEAPU8.set(yb.v.bytes, vPtr);
  const w = yb.format.width;
  const h = yb.format.height;
  n = w * h * 4;
  if (outPtrLen != n) {
    if (outPtr) free(outPtr);
    outPtrLen = n;
    outPtr = malloc(n);
  }
  // const res = wasmExports.I420ToARGB(yPtr, yb.y.stride, uPtr, yb.u.stride, vPtr, yb.v.stride, outPtr, w * 4, w, h);
  const res = wasmExports.AVX_YUV_to_RGBA(outPtr, yPtr, uPtr, vPtr, w, h);
  // const res = wasmExports.yuv420_rgb24_std(w, h, yPtr, uPtr, vPtr, yb.y.stride, yb.v.stride, outPtr, w * 4, 0);
  const out = HEAPU8.slice(outPtr, outPtr + n);
  /*
  testSpeed[1] += new Date().getTime() - tm0;
  if (testSpeed[0] > 30) {
    console.log(testSpeed[1] / testSpeed[0]);
    testSpeed = [0, 0];
  }
  */
  return out;
}

async function initZstd() {
  loadOpus(() => { });
  loadVp9(() => { });
  const response = await fetch('yuv.wasm');
  const file = await response.arrayBuffer();
  const wasm = await WebAssembly.instantiate(file);
  wasmExports = wasm.instance.exports;
  console.log('yuv ready');
  const tmp = new zstd.ZSTDDecoder();
  await tmp.init();
  console.log('zstd ready');
  decompressor = tmp;
}
