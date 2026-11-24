export const HEADER_LEN = 32;

const CODEC_NAMES: Record<number, string> = {
  1: "vp8",
  2: "vp9",
  3: "h264",
  4: "h265",
  5: "av1",
};

export interface VideoPacket {
  codec: number;
  codecName: string;
  key: boolean;
  display: number;
  width: number;
  height: number;
  pts: number;
  gatewayUnixMs: number;
  data: Uint8Array;
}

export function decodeFrame(buf: ArrayBuffer): VideoPacket | null {
  if (buf.byteLength < HEADER_LEN) return null;
  const view = new DataView(buf);
  if (
    view.getUint8(0) !== 0x52 ||
    view.getUint8(1) !== 0x44 ||
    view.getUint8(2) !== 0x56 ||
    view.getUint8(3) !== 0x31
  ) {
    return null;
  }
  const codec = view.getUint8(4);
  return {
    codec,
    codecName: CODEC_NAMES[codec] ?? "unknown",
    key: (view.getUint8(5) & 1) !== 0,
    display: view.getUint16(6, true),
    width: view.getUint32(8, true),
    height: view.getUint32(12, true),
    pts: Number(view.getBigInt64(16, true)),
    gatewayUnixMs: Number(view.getBigUint64(24, true)),
    data: new Uint8Array(buf, HEADER_LEN),
  };
}

export interface DisplayInfo {
  index: number;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  online: boolean;
}

export interface PeerMessage {
  type: "peer";
  hostname: string;
  platform: string;
  username: string;
  version: string;
  currentDisplay: number;
  displays: DisplayInfo[];
  encrypted: boolean;
}

export interface StatusMessage {
  type: "status";
  state: string;
  detail?: string;
}

export type ServerMessage =
  | { type: "hello"; service: string }
  | StatusMessage
  | PeerMessage
  | { type: "display"; display: number; width: number; height: number }
  | { type: "clipboard"; text: string }
  | { type: "cursor"; x: number; y: number }
  | { type: "error"; message: string }
  | { type: "rtt"; ms: number };

export function parseServerMessage(text: string): ServerMessage | null {
  try {
    const value = JSON.parse(text) as ServerMessage;
    if (!value || typeof value !== "object" || !("type" in value)) return null;
    return value;
  } catch {
    return null;
  }
}

export function wsUrl(): string {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}/ws`;
}
