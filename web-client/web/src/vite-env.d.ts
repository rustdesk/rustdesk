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

interface RdView {
  scale: number;
  panX: number;
  panY: number;
  width: number;
  height: number;
}

interface Window {
  __rd?: RdStats;
  __rdView?: RdView;
}
