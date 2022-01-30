import Connection from "./connection";
import _sodium from "libsodium-wrappers";
import { CursorData } from "./message";
import { loadOpus, loadVp9 } from "./codec";
import { checkIfRetry, version } from "./gen_js_from_hbb";
import { initZstd, translate } from "./common";

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
  if (!type || (type == 'error' && !text)) return;
  const text2 = text.toLowerCase();
  var hasRetry = checkIfRetry(type, title, text);
  events.push({ name: 'msgbox', type, title, text, hasRetry });
}

function jsonfyForDart(payload) {
  var tmp = {};
  for (const [key, value] of Object.entries(payload)) {
    if (!key) continue;
    tmp[key] = value instanceof Uint8Array ? '[' + value.toString() + ']' : JSON.stringify(value);
  }
  return tmp;
}

export function pushEvent(name, payload) {
  if (!events) return;
  payload = jsonfyForDart(payload);
  payload.name = name;
  events.push(payload);
}

const yuvWorker = new Worker("./yuv.js");

yuvWorker.onmessage = (e) => {
  currentFrame = e.data;
}

export function draw(frame) {
  yuvWorker.postMessage(frame);
}

export function setConn(conn) {
  window.curConn = conn;
}

export function getConn() {
  return window.curConn;
}

export async function startConn(id) {
  try {
    currentFrame = undefined;
    events = [];
    setByName('remote_id', id);
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

window.setByName = (name, value) => {
  switch (name) {
    case 'remote_id':
      localStorage.setItem('remote-id', value);
      break;
    case 'connect':
      newConn();
      startConn(value);
      break;
    case 'login':
      value = JSON.parse(value);
      curConn.setRemember(value.remember == 'true');
      curConn.login(value.password);
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
      curConn.ctrlAltDel();
      break;
    case 'switch_display':
      curConn.switchDisplay(value);
      break;
    case 'remove':
      const peers = getPeers();
      delete peers[value];
      localStorage.setItem('peers', JSON.stringify(peers));
      break;
    case 'input_key':
      value = JSON.parse(value);
      curConn.inputKey(value.name, value.alt || false, value.ctrl || false, value.shift || false, value.command || false);
      break;
    case 'input_string':
      curConn.inputString(value);
      break;
    case 'send_mouse':
      let mask = 0;
      value = JSON.parse(value);
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
      curConn.inputMouse(mask, parseInt(value.x || '0'), parseInt(value.y || '0'), value.alt || false, value.ctrl || false, value.shift || false, value.command || false);
      break;
    case 'option':
      value = JSON.parse(value);
      localStorage.setItem(value.name, value.value);
      break;
    case 'peer_option':
      value = JSON.parse(value);
      curConn.setOption(value.name, value.value);
      break;
    case 'input_os_password':
      curConn.inputOsPassword(value);
      break;
    default:
      break;
  }
}

window.getByName = (name, arg) => {
  let v = _getByName(name, arg);
  if (typeof v == 'string' || v instanceof String) return v;
  if (v == undefined || v == null) return '';
  return JSON.stringify(v);
}

function getPeersForDart() {
  const peers = [];
  for (const [key, value] of Object.entries(getPeers())) {
    if (!key) continue;
    const tm = value['tm'];
    const info = values['info'];
    if (!tm || !info) continue;
    peers.push([tm, id, info]);
  }
  return peers.sort().reverse().map(x => x.slice(1));
}

function _getByName(name, arg) {
  switch (name) {
    case 'peers':
      return getPeersForDart();
    case 'remote_id':
      return localStorage.getItem('remote-id');
    case 'remember':
      return curConn.getRemember();
    case 'event':
      if (events && events.length) {
        const e = events[0];
        events.splice(0, 1);
        return JSON.stringify(e);
      }
      break;
    case 'toggle_option':
      return curConn.getOption(arg) || false;
    case 'option':
      return localStorage.getItem(arg);
    case 'image_quality':
      return curConn.getImageQuality();
    case 'translate':
      arg = JSON.parse(arg);
      return translate(arg.locale, arg.text);
    case 'peer_option':
      return curConn.getOption(arg);
    case 'test_if_valid_server':
      break;
    case 'version':
      return version;
  }
  return '';
}

window.init = async () => {
  loadOpus(() => { });
  loadVp9(() => { });
  await initZstd();
}

export function getPeers() {
  try {
    return JSON.parse(localStorage.getItem('peers')) || {};
  } catch (e) {
    return {};
  }
}