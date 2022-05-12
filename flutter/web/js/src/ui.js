import "./style.css";
import "./connection";
import * as globals from "./globals";

const app = document.querySelector('#app');

if (app) {
  app.innerHTML = `
  <div id="connect" style="text-align: center"><table style="display: inline-block">
    <tr><td><span>Host: </span></td><td><input id="host" /></td></tr>
    <tr><td><span>Key: </span></td><td><input id="key" /></td></tr>
    <tr><td><span>Id: </span></td><td><input id="id" /></td></tr>
    <tr><td></td><td><button onclick="connect();">Connect</button></td></tr>
  </table></div>
  <div id="password" style="display: none;">
    <input type="password" id="password" />
    <button id="confirm" onclick="confirm()">Confirm</button>
    <button id="cancel" onclick="cancel();">Cancel</button>
  </div>
  <div id="status" style="display: none;">
    <div id="text" style="line-height: 2em"></div>
    <button id="cancel" onclick="cancel();">Cancel</button>
  </div>
  <div id="canvas" style="display: none;">
    <button id="cancel" onclick="cancel();">Cancel</button>
    <canvas id="player"></canvas>
    <canvas id="test-yuv-decoder-canvas"></canvas>
  </div>
`;

  let player;
  window.init();

  document.body.onload = () => {
    const host = document.querySelector('#host');
    host.value = localStorage.getItem('custom-rendezvous-server');
    const id = document.querySelector('#id');
    id.value = localStorage.getItem('id');
    const key = document.querySelector('#key');
    key.value = localStorage.getItem('key');
    player = YUVCanvas.attach(document.getElementById('player'));
    // globals.sendOffCanvas(document.getElementById('player'));
  };

  window.connect = () => {
    const host = document.querySelector('#host');
    localStorage.setItem('custom-rendezvous-server', host.value);
    const id = document.querySelector('#id');
    localStorage.setItem('id', id.value);
    const key = document.querySelector('#key');
    localStorage.setItem('key', key.value);
    const func = async () => {
      const conn = globals.newConn();
      conn.setMsgbox(msgbox);
      conn.setDraw((f) => {
        /*
        if (!(document.getElementById('player').width > 0)) {
          document.getElementById('player').width = f.format.displayWidth;
          document.getElementById('player').height = f.format.displayHeight;
        }
        */
        globals.draw(f);
        player.drawFrame(f);
      });
      document.querySelector('div#status').style.display = 'block';
      document.querySelector('div#connect').style.display = 'none';
      document.querySelector('div#text').innerHTML = 'Connecting ...';
      await conn.start(id.value);
    };
    func();
  }

  function msgbox(type, title, text) {
    if (!globals.getConn()) return;
    if (type == 'input-password') {
      document.querySelector('div#status').style.display = 'none';
      document.querySelector('div#password').style.display = 'block';
    } else if (!type) {
      document.querySelector('div#canvas').style.display = 'block';
      document.querySelector('div#password').style.display = 'none';
      document.querySelector('div#status').style.display = 'none';
    } else if (type == 'error') {
      document.querySelector('div#status').style.display = 'block';
      document.querySelector('div#canvas').style.display = 'none';
      document.querySelector('div#text').innerHTML = '<div style="color: red; font-weight: bold;">' + text + '</div>';
    } else {
      document.querySelector('div#password').style.display = 'none';
      document.querySelector('div#status').style.display = 'block';
      document.querySelector('div#text').innerHTML = '<div style="font-weight: bold;">' + text + '</div>';
    }
  }

  window.cancel = () => {
    globals.close();
    document.querySelector('div#connect').style.display = 'block';
    document.querySelector('div#password').style.display = 'none';
    document.querySelector('div#status').style.display = 'none';
    document.querySelector('div#canvas').style.display = 'none';
  }

  window.confirm = () => {
    const password = document.querySelector('input#password').value;
    if (password) {
      document.querySelector('div#password').style.display = 'none';
      globals.getConn().login(password);
    }
  }
}