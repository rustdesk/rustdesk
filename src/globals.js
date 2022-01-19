window.currentConnection = undefined;

export function setConn(conn) {
  window.currentConnection = conn;
}

export function getConn() {
  return windows.currentConnection;
}