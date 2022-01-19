import "./style.css";
import "./connection";

const app = document.querySelector("#app");

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
    <button id="confirm" id="confirm()">Confirm</button>
    <button id="cancel" onclick="cancel();">Cancel</button>
  </div>
`;

  document.body.onload = () => {
    const host = document.querySelector('#host');
    host.value = localStorage.getItem('host');
    const id = document.querySelector('#id');
    id.value = localStorage.getItem('id');
    const key = document.querySelector('#key');
    key.value = localStorage.getItem('key');
  };

  window.connect = () => {
    const host = document.querySelector('#host');
    localStorage.setItem('host', host.value);
    const id = document.querySelector('#id');
    localStorage.setItem('id', id.value);
    const key = document.querySelector('#key');
    localStorage.setItem('key', key.value);
    document.querySelector('div#connect').style.display = 'none';
    document.querySelector('div#password').style.display = 'block';
  }

  window.cancel = () => {
    document.querySelector('div#connect').style.display = 'block';
    document.querySelector('div#password').style.display = 'none';
  }

  window.confirm = () => {
    //
  }

}