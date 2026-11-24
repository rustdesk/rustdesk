export const HEADER_LEN = 32;

export const CODEC_NAMES: Record<number, string> = {
  1: 'vp8',
  2: 'vp9',
  3: 'h264',
  4: 'h265',
  5: 'av1',
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

export interface DisplayInfo {
  index: number;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  online: boolean;
  cursorEmbedded?: boolean;
}

export interface PeerEncodingInfo {
  vp8: boolean;
  h264: boolean;
  h265: boolean;
  av1: boolean;
}

export interface PeerMessage {
  route?: 'direct' | 'relay';
  privacyAllowed?: boolean;
  clipboardAllowed?: boolean;
  restartAllowed?: boolean;
  type: 'peer';
  hostname: string;
  platform: string;
  username: string;
  version: string;
  currentDisplay: number;
  displays: DisplayInfo[];
  encoding?: PeerEncodingInfo | null;
  encrypted: boolean;
}

export interface CursorShape {
  width: number;
  height: number;
  hotX: number;
  hotY: number;
  rgba: string;
}

export type ServerMessage =
  | { type: 'permissions'; privacyAllowed: boolean; clipboardAllowed: boolean }
  | { type: 'privacyResult'; enabled?: boolean; failed: boolean }
  | { type: 'restartPermission'; allowed: boolean }
  | { type: 'restartResult'; sent: boolean }
  | { type: 'audioDenied' }
  | { type: 'audioFormat'; sampleRate: number; channels: number }
  | { type: 'audioFrame'; packet: string }
  | { type: 'focusedDisplay'; display: number }
  | { type: 'resolutions'; display: number; sizes: { width: number; height: number }[] }
  | { type: 'hello'; service: string }
  | { type: 'status'; state: string; detail?: string; username?: string }
  | PeerMessage
  | {
      type: 'display';
      display: number;
      width: number;
      height: number;
      x?: number;
      y?: number;
      cursorEmbedded?: boolean;
      name?: string;
    }
  | { type: 'clipboard'; text: string }
  | { type: 'cursor'; x: number; y: number }
  | { type: 'cursorShape'; shape: CursorShape | null }
  | { type: 'error'; message: string }
  | { type: 'rtt'; ms: number }
  | { type: 'pong'; seq: number; clientTs: number; serverTs: number };

export interface CodecSupport {
  vp8: boolean;
  vp9: boolean;
  h264: boolean;
  h265: boolean;
  av1: boolean;
  reported: boolean;
}

export type QualityLevel = 'low' | 'balanced' | 'best';

export type ClientMessage =
  | { type: 'privacy'; enabled: boolean }
  | { type: 'restart' }
  | { type: 'audio'; enabled: boolean }
  | { type: 'resolution'; display: number; width: number; height: number }
  | {
      type: 'connect';
      mode: 'direct' | 'id';
      host?: string;
      port?: number;
      password?: string;
      peerId?: string;
      idServer?: string;
      relayServer?: string;
      key?: string;
      forceRelay?: boolean;
      token?: string;
      codecs: CodecSupport;
      quality?: QualityLevel;
      fps?: number;
      codec?: string;
    }
  | {
      type: 'mouse';
      action: 'move' | 'down' | 'up' | 'wheel';
      button?: 'left' | 'right' | 'middle';
      x: number;
      y: number;
    }
  | {
      type: 'key';
      action: 'down' | 'up' | 'press';
      control?: string;
      chr?: string;
      text?: string;
      modifiers?: string[];
    }
  | { type: 'text'; text: string }
  | { type: 'clipboard'; text: string }
  | { type: 'followWindow'; enabled: boolean }
  | { type: 'switchDisplay'; display: number }
  | { type: 'refresh' }
  | { type: 'quality'; quality: QualityLevel; fps: number; codec?: string }
  | { type: 'ping'; seq: number; clientTs: number }
  | { type: 'disconnect' };

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
    codecName: CODEC_NAMES[codec] ?? 'unknown',
    key: (view.getUint8(5) & 1) !== 0,
    display: view.getUint16(6, true),
    width: view.getUint32(8, true),
    height: view.getUint32(12, true),
    pts: Number(view.getBigInt64(16, true)),
    gatewayUnixMs: Number(view.getBigUint64(24, true)),
    data: new Uint8Array(buf, HEADER_LEN),
  };
}

export function wsUrl(): string {
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${proto}//${window.location.host}/ws`;
}

export function formatBitrate(bps: number): string {
  if (!Number.isFinite(bps) || bps <= 0) return '0 b/s';
  if (bps >= 1_000_000) return `${(bps / 1_000_000).toFixed(1)} Mb/s`;
  if (bps >= 1_000) return `${Math.round(bps / 1_000)} kb/s`;
  return `${Math.round(bps)} b/s`;
}
