import Connection from "./connection";
import PORT from "./connection";
import _sodium from "libsodium-wrappers";
import { CursorData } from "./message";
import { loadVp9 } from "./codec";
import { checkIfRetry, version } from "./gen_js_from_hbb";
import { initZstd, translate } from "./common";
import PCMPlayer from "pcm-player";

window.curConn = undefined;
window.isMobile = () => {
  return /(android|bb\d+|meego).+mobile|avantgo|bada\/|blackberry|blazer|compal|elaine|fennec|hiptop|iemobile|ip(hone|od)|ipad|iris|kindle|Android|Silk|lge |maemo|midp|mmp|netfront|opera m(ob|in)i|palm( os)?|phone|p(ixi|re)\/|plucker|pocket|psp|series(4|6)0|symbian|treo|up\.(browser|link)|vodafone|wap|windows (ce|phone)|xda|xiino/i.test(navigator.userAgent)
    || /1207|6310|6590|3gso|4thp|50[1-6]i|770s|802s|a wa|abac|ac(er|oo|s\-)|ai(ko|rn)|al(av|ca|co)|amoi|an(ex|ny|yw)|aptu|ar(ch|go)|as(te|us)|attw|au(di|\-m|r |s )|avan|be(ck|ll|nq)|bi(lb|rd)|bl(ac|az)|br(e|v)w|bumb|bw\-(n|u)|c55\/|capi|ccwa|cdm\-|cell|chtm|cldc|cmd\-|co(mp|nd)|craw|da(it|ll|ng)|dbte|dc\-s|devi|dica|dmob|do(c|p)o|ds(12|\-d)|el(49|ai)|em(l2|ul)|er(ic|k0)|esl8|ez([4-7]0|os|wa|ze)|fetc|fly(\-|_)|g1 u|g560|gene|gf\-5|g\-mo|go(\.w|od)|gr(ad|un)|haie|hcit|hd\-(m|p|t)|hei\-|hi(pt|ta)|hp( i|ip)|hs\-c|ht(c(\-| |_|a|g|p|s|t)|tp)|hu(aw|tc)|i\-(20|go|ma)|i230|iac( |\-|\/)|ibro|idea|ig01|ikom|im1k|inno|ipaq|iris|ja(t|v)a|jbro|jemu|jigs|kddi|keji|kgt( |\/)|klon|kpt |kwc\-|kyo(c|k)|le(no|xi)|lg( g|\/(k|l|u)|50|54|\-[a-w])|libw|lynx|m1\-w|m3ga|m50\/|ma(te|ui|xo)|mc(01|21|ca)|m\-cr|me(rc|ri)|mi(o8|oa|ts)|mmef|mo(01|02|bi|de|do|t(\-| |o|v)|zz)|mt(50|p1|v )|mwbp|mywa|n10[0-2]|n20[2-3]|n30(0|2)|n50(0|2|5)|n7(0(0|1)|10)|ne((c|m)\-|on|tf|wf|wg|wt)|nok(6|i)|nzph|o2im|op(ti|wv)|oran|owg1|p800|pan(a|d|t)|pdxg|pg(13|\-([1-8]|c))|phil|pire|pl(ay|uc)|pn\-2|po(ck|rt|se)|prox|psio|pt\-g|qa\-a|qc(07|12|21|32|60|\-[2-7]|i\-)|qtek|r380|r600|raks|rim9|ro(ve|zo)|s55\/|sa(ge|ma|mm|ms|ny|va)|sc(01|h\-|oo|p\-)|sdk\/|se(c(\-|0|1)|47|mc|nd|ri)|sgh\-|shar|sie(\-|m)|sk\-0|sl(45|id)|sm(al|ar|b3|it|t5)|so(ft|ny)|sp(01|h\-|v\-|v )|sy(01|mb)|t2(18|50)|t6(00|10|18)|ta(gt|lk)|tcl\-|tdg\-|tel(i|m)|tim\-|t\-mo|to(pl|sh)|ts(70|m\-|m3|m5)|tx\-9|up(\.b|g1|si)|utst|v400|v750|veri|vi(rg|te)|vk(40|5[0-3]|\-v)|vm40|voda|vulc|vx(52|53|60|61|70|80|81|83|85|98)|w3c(\-| )|webc|whit|wi(g |nc|nw)|wmlb|wonu|x700|yas\-|your|zeto|zte\-/i.test(navigator.userAgent.substr(0, 4));
}

export function isDesktop() {
  return !isMobile();
}

export function msgbox(type, title, text, link) {
  if (!type || (type == 'error' && !text)) return;
  const text2 = text.toLowerCase();
  var hasRetry = checkIfRetry(type, title, text) ? 'true' : '';
  onGlobalEvent(JSON.stringify({ name: 'msgbox', type, title, text, link: link ?? '', hasRetry }));
}

function jsonfyForDart(payload) {
  var tmp = {};
  for (const [key, value] of Object.entries(payload)) {
    if (!key) continue;
    if (value instanceof String || typeof value == 'string') {
      tmp[key] = value;
    } else if (value instanceof Uint8Array) {
      tmp[key] = '[' + value.toString() + ']';
    } else {
      tmp[key] = JSON.stringify(value);
    }
  }
  return tmp;
}

export function pushEvent(name, payload) {
  payload = jsonfyForDart(payload);
  payload.name = name;
  onGlobalEvent(JSON.stringify(payload));
}

let yuvWorker;
let yuvCanvas;
let gl;
let pixels;
let flipPixels;
let oldSize;
if (YUVCanvas.WebGLFrameSink.isAvailable()) {
  var canvas = document.createElement('canvas');
  yuvCanvas = YUVCanvas.attach(canvas, { webGL: true });
  gl = canvas.getContext("webgl");
} else {
  yuvWorker = new Worker("./yuv.js");
}
let testSpeed = [0, 0];

export function draw(display, frame) {
  if (yuvWorker) {
    // frame's (y/u/v).bytes already detached, can not transferrable any more.
    yuvWorker.postMessage({ display, frame });
  } else {
    var tm0 = new Date().getTime();
    yuvCanvas.drawFrame(frame);
    var width = canvas.width;
    var height = canvas.height;
    var size = width * height * 4;
    if (size != oldSize) {
      pixels = new Uint8Array(size);
      flipPixels = new Uint8Array(size);
      oldSize = size;
    }
    gl.readPixels(0, 0, width, height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    const row = width * 4;
    const end = (height - 1) * row;
    for (let i = 0; i < size; i += row) {
      flipPixels.set(pixels.subarray(i, i + row), end - i);
    }
    onRgba(display, flipPixels);
    testSpeed[1] += new Date().getTime() - tm0;
    testSpeed[0] += 1;
    if (testSpeed[0] > 30) {
      console.log('gl: ' + parseInt('' + testSpeed[1] / testSpeed[0]));
      testSpeed = [0, 0];
    }
  }
  /*
  var testCanvas = document.getElementById("test-yuv-decoder-canvas");
  if (testCanvas && currentFrame) {
    var ctx = testCanvas.getContext("2d");
    testCanvas.width = frame.format.displayWidth;
    testCanvas.height = frame.format.displayHeight;
    var img = ctx.createImageData(testCanvas.width, testCanvas.height);
    img.data.set(currentFrame);
    ctx.putImageData(img, 0, 0);
  }
  */
}

export function sendOffCanvas(c) {
  let canvas = c.transferControlToOffscreen();
  yuvWorker.postMessage({ canvas }, [canvas]);
}

export function setConn(conn) {
  window.curConn = conn;
}

export function getConn() {
  return window.curConn;
}

export async function startConn(id) {
  setByName('remote_id', id);
  await curConn.start(id);
}

export function close() {
  getConn()?.close();
  setConn(undefined);
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
      curConn.setRemember(value.remember);
      curConn.login({
        os_login: {
          username: value.os_username,
          password: value.os_password,
        },
        password: value.password,
      });
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
    case 'toggle_privacy_mode':
      curConn.togglePrivacyMode(value);
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
      curConn.inputKey(value.name, value.down == 'true', value.press == 'true', value.alt == 'true', value.ctrl == 'true', value.shift == 'true', value.command == 'true');
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
      curConn.inputMouse(mask, parseInt(value.x || '0'), parseInt(value.y || '0'), value.alt == 'true', value.ctrl == 'true', value.shift == 'true', value.command == 'true');
      break;
    case 'send_2fa':
      curConn.send2fa(value);
      break;
    case 'option':
      value = JSON.parse(value);
      localStorage.setItem(value.name, value.value);
      break;
    case 'options':
      value = JSON.parse(value);
      for (const [key, value] of Object.entries(value)) {
        localStorage.setItem(key, value);
      }
      break;
    case 'option:local':
    case 'option:flutter:local':
    case 'option:flutter:peer':
      value = JSON.parse(value);
      localStorage.setItem(name + ':' + value.name, value.value);
      break;
    case 'option:session':
      value = JSON.parse(value);
      curConn.setOption(value.name, value.value);
      break;
    case 'option:peer':
      setPeerOption(value);
      break;
    case 'input_os_password':
      curConn.inputOsPassword(value);
      break;
    case 'check_conn_status':
      curConn.checkConnStatus();
      break;
    case 'session_add_sync':
      return sessionAdd(value);
    case 'session_start':
      sessionStart(value);
      break;
    case 'session_close':
      sessionClose(value);
      break;
    case 'elevate_with_logon':
      curConn.elevateWithLogon(value);
      break;
    case 'forget':
      curConn.setRemember(false);
      break;
    case 'peer_has_password':
      const options = getPeers()[value] || {};
      return (options['password'] ?? '') !== '';
    case 'peer_exists':
      return !(!getPeers()[value]);
    case 'restart':
      curConn.restart();
      break;
    case 'fav':
      return localStorage.setItem('fav', value);
    case 'query_onlines':
      queryOnlines(value);
      break;
    case 'change_prefer_codec':
      curConn.changePreferCodec(value);
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

function _getByName(name, arg) {
  switch (name) {
    case 'remote_id':
      return localStorage.getItem('remote-id');
    case 'remember':
      return curConn.getRemember();
    case 'toggle_option':
      return curConn.getOption(arg) || false;
    case 'option':
      return localStorage.getItem(arg);
    case 'options':
      const keys = [
        'custom-rendezvous-server',
        'relay-server',
        'api-server',
        'key'
      ];
      const obj = {};
      keys.forEach(key => {
        const v = localStorage.getItem(key);
        if (v) obj[key] = v;
      });
      return JSON.stringify(obj);
    case 'option:local':
    case 'option:flutter:local':
    case 'option:flutter:peer':
      return localStorage.getItem(name + ':' + arg);
    case 'image_quality':
      return curConn.getImageQuality();
    case 'translate':
      arg = JSON.parse(arg);
      return translate(arg.locale, arg.text);
    case 'option:session':
      return curConn.getOption(arg);
    case 'option:peer':
      return getPeerOption(arg);
    case 'option:toggle':
      return curConn.getToggleOption(arg);
    case 'get_conn_status':
      if (curConn) {
        return curConn.getStatus();
      } else {
        return JSON.stringify({ status_num: 0 });
      }
    case 'test_if_valid_server':
      break;
    case 'version':
      return version;
    case 'load_recent_peers':
      loadRecentPeers();
      break;
    case 'load_fav_peers':
      loadFavPeers();
      break;
    case 'fav':
      return localStorage.getItem('fav') ?? '[]';
    case 'load_recent_peers_sync':
      return JSON.stringify({
        peers: JSON.stringify(getRecentPeers())
      });
    case 'api_server':
      return getApiServer();
    case 'is_using_public_server':
      return !localStorage.getItem('custom-rendezvous-server');
    case 'get_version_number':
      return getVersionNumber(arg);
    case 'audit_server':
      return getAuditServer(arg);
    case 'alternative_codecs':
      return getAlternativeCodecs();
  }
  return '';
}

let opusWorker = new Worker("./libopus.js");
let pcmPlayer;

export function initAudio(channels, sampleRate) {
  pcmPlayer = newAudioPlayer(channels, sampleRate);
  opusWorker.postMessage({ channels, sampleRate });
}

export function playAudio(packet) {
  opusWorker.postMessage(packet, [packet.buffer]);
}

window.init = async () => {
  if (yuvWorker) {
    yuvWorker.onmessage = (e) => {
      onRgba(e.data.display, e.data.frame);
    }
  }
  opusWorker.onmessage = (e) => {
    pcmPlayer.feed(e.data);
  }
  loadVp9(() => { });
  await initZstd();
  console.log('init done');
}

export function getPeers() {
  return getJsonObj('peers');
}

export function getJsonObj(key) {
  try {
    return JSON.parse(localStorage.getItem(key)) || {};
  } catch (e) {
    return {};
  }
}

function newAudioPlayer(channels, sampleRate) {
  return new PCMPlayer({
    channels,
    sampleRate,
    flushingTime: 2000
  });
}

export function copyToClipboard(text) {
  if (window.clipboardData && window.clipboardData.setData) {
    // Internet Explorer-specific code path to prevent textarea being shown while dialog is visible.
    return window.clipboardData.setData("Text", text);
  }
  else if (document.queryCommandSupported && document.queryCommandSupported("copy")) {
    var textarea = document.createElement("textarea");
    textarea.textContent = text;
    textarea.style.position = "fixed";  // Prevent scrolling to bottom of page in Microsoft Edge.
    document.body.appendChild(textarea);
    textarea.select();
    try {
      return document.execCommand("copy");  // Security exception may be thrown by some browsers.
    }
    catch (ex) {
      console.warn("Copy to clipboard failed.", ex);
      // return prompt("Copy to clipboard: Ctrl+C, Enter", text);
    }
    finally {
      document.body.removeChild(textarea);
    }
  }
}

// Dup to the function in hbb_common, lib.rs
// Maybe we need to move this function to js part.
export function getVersionNumber(v) {
  try {
    let versions = v.split('-');

    let n = 0;

    // The first part is the version number.
    // 1.1.10 -> 1001100, 1.2.3 -> 1001030, multiple the last number by 10
    // to leave space for patch version.
    if (versions.length > 0) {
      let last = 0;
      for (let x of versions[0].split('.')) {
        last = parseInt(x) || 0;
        n = n * 1000 + last;
      }
      n -= last;
      n += last * 10;
    }

    if (versions.length > 1) {
      n += parseInt(versions[1]) || 0;
    }

    // Ignore the rest

    return n;
  }
  catch (e) {
    console.error('Failed to parse version number: "' + v + '" ' + e.message);
    return 0;
  }
}

function getPeerOption(value) {
  try {
    const obj = JSON.parse(value);
    const options = getPeers()[obj.id] || {};
    return options[obj.name] || '';
  }
  catch (e) {
    console.error('Failed to get peer option: "' + value + '", ' + e.message);
  }
}

function setPeerOption(value) {
  try {
    const obj = JSON.parse(value);
    const id = obj.id;
    const name = obj.name;
    const value = obj.value;
    const peers = getPeers();
    const options = peers[id] || {};

    if (value == undefined) {
      delete options[name];
    } else {
      options[name] = value;
    }
    options["tm"] = new Date().getTime();
    peers[id] = options;
    localStorage.setItem("peers", JSON.stringify(peers));
  }
  catch (e) {
    console.error('Failed to set peer option: "' + value + '", ' + e.message);
  }
}

// ========================== peers begin ==========================
function getRecentPeers() {
  const peers = [];
  for (const [id, value] of Object.entries(getPeers())) {
    if (!id) continue;
    const tm = value['tm'];
    const info = value['info'];
    const cardInfo = {
      id: id,
      username: info['username'] || '',
      hostname: info['hostname'] || '',
      platform: info['platform'] || '',
      alias: value.alias || '',
    };
    if (!tm || !cardInfo) continue;
    peers.push([tm, id, cardInfo]);
  }
  return peers.sort().reverse().map(x => x[2]);
}

function loadRecentPeers() {
  const peersRecent = getRecentPeers();
  if (peersRecent) {
    onRegisteredEvent(JSON.stringify({ name: 'load_recent_peers', peers: JSON.stringify(peersRecent) }));
  }
}

function loadFavPeers() {
  try {
    const fav = localStorage.getItem('fav') ?? '[]';
    const favs = JSON.parse(fav);
    const peersFav = getRecentPeers().filter(x => favs.includes(x.id));
    if (peersFav) {
      onRegisteredEvent(JSON.stringify({ name: 'load_fav_peers', peers: JSON.stringify(peersFav) }));
    }
  } catch (e) {
    console.error('Failed to load fav peers: ' + e.message);
  }
}

export function queryOnlines(value) {
  // TODO: implement this
}
// ========================== peers end ===========================

// ========================== session begin ==========================
function sessionAdd(value) {
  try {
    const data = JSON.parse(value);
    window.curConn?.close();
    const conn = new Connection();
    setConn(conn);
    return '';
  } catch (e) {
    return e.message;
  }
}

function sessionStart(value) {
  try {
    const conn = getConn();
    if (!conn) {
      return;
    }

    const data = JSON.parse(value);
    if (data['id']) {
      startConn(data['id']);
    } else {
      msgbox('error', 'Error', 'No id found in session data ' + value, '');
    }
  } catch (e) {
    // TODO: better error handling
    msgbox('error', 'Error', e.message, '');
  }
}

function sessionClose(value) {
  close();
}
// ========================== session end ===========================

// ========================== settings begin ==========================
function increasePort(host, offset) {
  function isIPv6(str) {
    const ipv6Pattern = /^([0-9a-fA-F]{0,4}:){1,7}[0-9a-fA-F]{0,4}$/;
    return ipv6Pattern.test(str);
  }

  if (isIPv6(host)) {
    if (host.startsWith('[')) {
      let tmp = host.split(']:');
      if (tmp.length === 2) {
        let port = parseInt(tmp[1]) || 0;
        if (port > 0) {
          return `${tmp[0]}]:${port + offset}`;
        }
      }
    }
  } else if (host.includes(':')) {
    let tmp = host.split(':');
    if (tmp.length === 2) {
      let port = parseInt(tmp[1]) || 0;
      if (port > 0) {
        return `${tmp[0]}:${port + offset}`;
      }
    }
  }
  return host;
}

function getAlternativeCodecs() {
  return JSON.stringify({
    vp8: 1,
    av1: 0,
    h264: 1,
    h265: 1,
  });
}
// ========================== settings end ===========================

// ========================== server begin ==========================
function getApiServer() {
  const api_server = localStorage.getItem('api-server');
  if (api_server) {
    return api_server;
  }

  const custom_rendezvous_server = localStorage.getItem('custom-rendezvous-server');
  if (custom_rendezvous_server) {
    let s = increasePort(custom_rendezvous_server, -2);
    if (s == custom_rendezvous_server) {
      return `http://${s}:${PORT - 2}`;
    } else {
      return `http://${s}`;
    }
  }
  return 'https://admin.rustdesk.com';
}

function getAuditServer(typ) {
  if (!localStorage.getItem("access_token")) {
    return '';
  }
  const api_server = getApiServer();
  if (!api_server || api_server.includes('rustdesk.com')) {
    return '';
  }
  return api_server + '/api/audit/' + typ;
}
// ========================== server end ============================
