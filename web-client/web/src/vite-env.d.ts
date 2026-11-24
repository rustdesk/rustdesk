/// <reference types="vite/client" />

interface RdStats {
  state: string;
  codec: string;
  width: number;
  height: number;
  fps: number;
  delayMs: number;
  bitrate: number;
  decoded: number;
}

interface Window {
  __rd?: RdStats;
}
