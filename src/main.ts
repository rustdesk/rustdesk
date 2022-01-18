import './style.css';
import { loadVp9, loadOpus } from "./codec";

loadVp9();
loadOpus();

const app = document.querySelector<HTMLDivElement>('#app')!

app.innerHTML = `
  <h1>Hello Vite!</h1>
  <a href="https://vitejs.dev/guide/features.html" target="_blank">Documentation</a>
`
