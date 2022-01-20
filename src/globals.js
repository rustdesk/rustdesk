import Connection from "./connection";
import _sodium from "libsodium-wrappers";

window.currentConnection = undefined;

export function setConn(conn) {
  window.currentConnection = conn;
}

export function getConn() {
  return windows.currentConnection;
}

export async function startConn(id) {
  const conn = new Connection();
  setConn(conn);
  await conn.start('124931507');
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

export function encrypt(unsigned, nonce, key) {
  return sodium.crypto_secretbox_easy(unsigned, nonce, key);
}

export function decrypt(signed, nonce, key) {
  return sodium.crypto_secretbox_open_easy(signed, nonce, key);
}

window.startConn = startConn;