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
  pk = sodium.from_base64(pk, sodium.base64_variants.ORIGINAL);
  return sodium.crypto_sign_open(signed, pk);
}

window.startConn = startConn;