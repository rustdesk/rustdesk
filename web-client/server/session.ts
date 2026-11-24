import type { WebSocket } from 'ws';
import { tokenMatches } from './auth.ts';
import {
  type ClientMessage,
  buildVideoReceivedMessage,
  hasAnyCodec,
  humanizeError,
  selectCodecs,
  toHostMessage,
} from './control.ts';
import { decodeMessage, encodeMessage } from './protos.ts';
import { type ConnectedHost, connectDirect, connectViaId } from './rustdeskClient.ts';
import {
  CODEC_AV1,
  CODEC_H264,
  CODEC_H265,
  CODEC_VP8,
  CODEC_VP9,
  encodeRdv1Frame,
  vp8KeyframeSize,
} from './video.ts';

export interface GatewayConfig {
  token: string;
}

const MAX_CLIPBOARD_BYTES = 256 * 1024;

function peerToJson(peer: Record<string, any>, encrypted: boolean): Record<string, unknown> {
  const displays = Array.isArray(peer.displays)
    ? peer.displays.map((d: any, index: number) => ({
        index,
        name: String(d.name ?? ''),
        x: Number(d.x ?? 0),
        y: Number(d.y ?? 0),
        width: Number(d.width ?? 0),
        height: Number(d.height ?? 0),
        online: Boolean(d.online ?? true),
      }))
    : [];

  const encoding = peer.encoding
    ? {
        vp8: Boolean(peer.encoding.vp8),
        h264: Boolean(peer.encoding.h264),
        h265: Boolean(peer.encoding.h265),
        av1: Boolean(peer.encoding.av1),
      }
    : null;

  return {
    type: 'peer',
    hostname: String(peer.hostname ?? ''),
    platform: String(peer.platform ?? ''),
    username: String(peer.username ?? ''),
    version: String(peer.version ?? ''),
    currentDisplay: Number(peer.current_display ?? 0),
    displays,
    encoding,
    encrypted,
  };
}

function initialDisplaySize(peer: Record<string, any>): { width: number; height: number } {
  const displays = Array.isArray(peer.displays) ? peer.displays : [];
  const idx = Number(peer.current_display ?? 0);
  const chosen = displays[idx] ?? displays[0];
  if (!chosen) return { width: 0, height: 0 };
  return {
    width: Math.max(0, Number(chosen.width ?? 0)),
    height: Math.max(0, Number(chosen.height ?? 0)),
  };
}

function isH264AnnexBKeyframe(data: Uint8Array): boolean {
  const limit = Math.min(data.length, 4096);
  for (let i = 0; i + 4 < limit; i++) {
    if (data[i] === 0 && data[i + 1] === 0) {
      let nalOffset = -1;
      if (data[i + 2] === 1) {
        nalOffset = i + 3;
      } else if (data[i + 2] === 0 && data[i + 3] === 1) {
        nalOffset = i + 4;
      }
      if (nalOffset !== -1 && nalOffset < data.length) {
        const nalType = data[nalOffset] & 0x1f;
        if (nalType === 5 || nalType === 7) return true; // IDR slice (5) or SPS (7)
      }
    }
  }
  return false;
}

function isVp9Keyframe(data: Uint8Array): boolean {
  if (data.length < 4) return false;
  const b0 = data[0];
  // VP9 uncompressed header: frame_marker (2 bits) == 0b10
  if ((b0 >> 6) !== 2) return false;
  const profileLow = (b0 >> 5) & 1;
  const profileHigh = (b0 >> 4) & 1;
  const profile = (profileHigh << 1) | profileLow;
  // For profile 0..2: bit 3 is show_existing_frame (must be 0), bit 2 is frame_type (0 = key)
  if (profile < 3) {
    if ((b0 & 0x08) !== 0 || (b0 & 0x04) !== 0) return false;
    return data[1] === 0x49 && data[2] === 0x83 && data[3] === 0x42;
  }
  return false;
}

function extractEncodedFrames(
  vf: Record<string, any>,
): { codec: number; frames: any[] } | null {
  if (vf.vp8s?.frames?.length) return { codec: CODEC_VP8, frames: vf.vp8s.frames };
  if (vf.vp9s?.frames?.length) return { codec: CODEC_VP9, frames: vf.vp9s.frames };
  if (vf.h264s?.frames?.length) return { codec: CODEC_H264, frames: vf.h264s.frames };
  if (vf.h265s?.frames?.length) return { codec: CODEC_H265, frames: vf.h265s.frames };
  if (vf.av1s?.frames?.length) return { codec: CODEC_AV1, frames: vf.av1s.frames };
  return null;
}

export function serveSocket(ws: WebSocket, config: GatewayConfig): void {
  let state: 'awaiting-connect' | 'connecting' | 'live' | 'closed' = 'awaiting-connect';
  let hostConn: ConnectedHost | null = null;
  let warnedRawVideo = false;
  let currentSize = { width: 0, height: 0 };

  let isAlive = true;

  const sendJson = (payload: Record<string, unknown>, onSent?: () => void) => {
    if (ws.readyState === ws.OPEN) {
      ws.send(JSON.stringify(payload), () => {
        onSent?.();
      });
    } else {
      onSent?.();
    }
  };

  const sendBinary = (buf: Buffer) => {
    if (ws.readyState === ws.OPEN) {
      ws.send(buf, { binary: true });
    }
  };

  // WebSocket protocol-level heartbeat keeps Cloud Run / Nginx / Cloudflare proxies from dropping idle sessions
  const heartbeat = setInterval(() => {
    if (state === 'closed' || ws.readyState !== ws.OPEN) {
      clearInterval(heartbeat);
      return;
    }
    if (!isAlive) {
      cleanup(false);
      ws.terminate();
      return;
    }
    isAlive = false;
    try {
      ws.ping();
    } catch {
      cleanup(false);
      ws.terminate();
    }
  }, 25_000);
  heartbeat.unref?.();

  ws.on('pong', () => {
    isAlive = true;
  });

  const cleanup = (sendClosedStatus: boolean) => {
    if (state === 'closed') return;
    state = 'closed';
    clearInterval(heartbeat);
    if (hostConn) {
      hostConn.stream.close();
      hostConn = null;
    }
    if (sendClosedStatus) {
      sendJson({ type: 'status', state: 'closed', detail: 'disconnected' });
    }
  };

  // Initial hello message per spec §4.3
  sendJson({ type: 'hello', service: 'rustdesk-web-gateway' });

  const startHostPump = async (conn: ConnectedHost) => {
    currentSize = initialDisplaySize(conn.peerInfo);
    sendJson(peerToJson(conn.peerInfo, conn.encrypted));
    sendJson({
      type: 'status',
      state: 'live',
      detail: conn.encrypted ? 'encrypted' : 'direct',
      username: conn.username,
    });

    try {
      while (state === 'live') {
        const raw = await conn.stream.next();
        if (!raw) {
          if (state === 'live') {
            throw new Error('host closed the connection');
          }
          break;
        }
        const msg = decodeMessage(raw);

        // 1. TestDelay echo + RTT reporting
        if (msg.test_delay && !msg.test_delay.from_client) {
          await conn.stream.sendRaw(encodeMessage({ test_delay: msg.test_delay }));
          const lastDelay = Number(msg.test_delay.last_delay ?? 0);
          if (lastDelay > 0) {
            sendJson({ type: 'rtt', ms: lastDelay });
          }
        }

        // 2. Misc control events
        if (msg.misc) {
          if (msg.misc.switch_display) {
            const sd = msg.misc.switch_display;
            const w = Number(sd.width ?? 0);
            const h = Number(sd.height ?? 0);
            if (w > 0 && h > 0) {
              currentSize = { width: w, height: h };
            }
            sendJson({
              type: 'display',
              display: Number(sd.display ?? 0),
              width: w,
              height: h,
              name: '',
            });
          }
          if (msg.misc.close_reason) {
            const reason = String(msg.misc.close_reason);
            sendJson({ type: 'status', state: 'error', detail: humanizeError(reason) });
            // Appendix B fix #8: actively close the stream on close_reason
            cleanup(false);
            return;
          }
        }

        // 3. Clipboard (single or RustDesk 1.4/1.5 MultiClipboards, uncompressed UTF-8 <= 256 KiB)
        if (msg.clipboard && !msg.clipboard.compress && msg.clipboard.content) {
          const fmt = Number(msg.clipboard.format ?? 0);
          if (fmt === 0) {
            const buf = Buffer.from(msg.clipboard.content);
            if (buf.byteLength <= MAX_CLIPBOARD_BYTES) {
              sendJson({ type: 'clipboard', text: buf.toString('utf8') });
            }
          }
        } else if (Array.isArray(msg.multi_clipboards?.clipboards)) {
          for (const clip of msg.multi_clipboards.clipboards) {
            if (!clip.compress && clip.content && Number(clip.format ?? 0) === 0) {
              const buf = Buffer.from(clip.content);
              if (buf.byteLength <= MAX_CLIPBOARD_BYTES) {
                sendJson({ type: 'clipboard', text: buf.toString('utf8') });
                break;
              }
            }
          }
        }

        // 4. CursorPosition
        if (msg.cursor_position) {
          sendJson({
            type: 'cursor',
            x: Number(msg.cursor_position.x ?? 0),
            y: Number(msg.cursor_position.y ?? 0),
          });
        }

        // 4b. Mid-session PeerInfo update (e.g. display hotplug or RustDesk 1.5 refreshed peer_info)
        if (msg.peer_info) {
          const updatedSize = initialDisplaySize(msg.peer_info);
          if (updatedSize.width > 0 && updatedSize.height > 0) {
            currentSize = updatedSize;
          }
          sendJson(peerToJson(msg.peer_info, conn.encrypted));
        }

        // 5. VideoFrame
        if (msg.video_frame) {
          const vf = msg.video_frame;
          if ((vf.rgb || vf.yuv) && !warnedRawVideo) {
            warnedRawVideo = true;
            sendJson({
              type: 'error',
              message:
                'the host sent uncompressed video; this client only decodes vp8, vp9, h264, h265, and av1',
            });
          }

          const extracted = extractEncodedFrames(vf);
          if (extracted) {
            const display = Math.max(0, Number(vf.display ?? 0));
            const nowMs = BigInt(Date.now());
            for (const f of extracted.frames) {
              const payload = f.data ? new Uint8Array(f.data) : new Uint8Array(0);
              const isKey =
                Boolean(f.key) ||
                (extracted.codec === CODEC_H264 && isH264AnnexBKeyframe(payload)) ||
                (extracted.codec === CODEC_VP9 && isVp9Keyframe(payload));
              if (extracted.codec === CODEC_VP8 && isKey) {
                const parsedSize = vp8KeyframeSize(payload);
                if (parsedSize) {
                  currentSize = parsedSize;
                }
              }
              const pts = BigInt(f.pts ?? 0);
              const packet = encodeRdv1Frame({
                codec: extracted.codec,
                key: isKey,
                display,
                width: currentSize.width,
                height: currentSize.height,
                pts,
                gatewayUnixMs: nowMs,
                data: payload,
              });
              sendBinary(packet);
            }
            // RustDesk 1.5 QoS ACK once per message containing video frames
            await conn.stream.sendRaw(encodeMessage(buildVideoReceivedMessage()));
          }
        }
      }
    } catch (err) {
      if (state === 'live') {
        sendJson({ type: 'status', state: 'error', detail: humanizeError(err) });
        cleanup(false);
      }
    }
  };

  ws.on('message', async (rawData, isBinary) => {
    if (isBinary || state === 'closed') return;
    let parsed: ClientMessage;
    try {
      const text = typeof rawData === 'string' ? rawData : rawData.toString('utf8');
      parsed = JSON.parse(text) as ClientMessage;
      if (!parsed || typeof parsed !== 'object' || typeof parsed.type !== 'string') {
        throw new Error('missing type field');
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      sendJson({ type: 'error', message: `bad message: ${msg}` });
      return;
    }

    if (parsed.type === 'ping') {
      const seq = Number.isFinite(Number(parsed.seq)) ? Number(parsed.seq) : 0;
      const clientTs = Number.isFinite(Number(parsed.clientTs)) ? Number(parsed.clientTs) : 0;
      sendJson({
        type: 'pong',
        seq,
        clientTs,
        serverTs: Date.now(),
      });
      return;
    }

    if (parsed.type === 'disconnect') {
      cleanup(true);
      ws.close();
      return;
    }

    if (state === 'awaiting-connect') {
      if (parsed.type !== 'connect') {
        sendJson({ type: 'error', message: 'send a connect message first' });
        return;
      }

      if (!tokenMatches(config.token, parsed.token ?? '')) {
        sendJson({
          type: 'status',
          state: 'error',
          detail: 'The gateway access token is missing or incorrect.',
        });
        cleanup(false);
        ws.close();
        return;
      }

      const codecs = selectCodecs(parsed.codecs);
      if (!hasAnyCodec(codecs)) {
        sendJson({
          type: 'status',
          state: 'error',
          detail: 'This browser cannot decode VP8, VP9, H.264, HEVC, or AV1.',
        });
        cleanup(false);
        ws.close();
        return;
      }

      state = 'connecting';
      sendJson({ type: 'status', state: 'connecting', detail: 'dialing the host' });

      try {
        const conn =
          parsed.mode === 'id'
            ? await connectViaId(
                {
                  peerId: parsed.peerId ?? '',
                  password: parsed.password ?? '',
                  idServer: parsed.idServer ?? '',
                  relayServer: parsed.relayServer ?? '',
                  key: parsed.key ?? '',
                  forceRelay: Boolean(parsed.forceRelay),
                },
                codecs,
              )
            : await connectDirect(
                {
                  host: parsed.host ?? '',
                  port: Number(parsed.port ?? 0),
                  password: parsed.password ?? '',
                },
                codecs,
              );

        if ((state as string) !== 'connecting') {
          conn.stream.close();
          return;
        }
        hostConn = conn;
        state = 'live';
        void startHostPump(conn);
      } catch (err) {
        if ((state as string) !== 'closed') {
          sendJson({ type: 'status', state: 'error', detail: humanizeError(err) }, () => {
            if (ws.readyState === ws.OPEN) {
              ws.close(1000, 'connect error');
            }
          });
          cleanup(false);
        }
      }
      return;
    }

    if (parsed.type === 'connect') {
      sendJson({ type: 'error', message: 'already connected' });
      return;
    }

    if (state !== 'live') return;

    if (hostConn) {
      const hostMsg = toHostMessage(parsed);
      if (hostMsg) {
        try {
          await hostConn.stream.sendRaw(encodeMessage(hostMsg));
        } catch (err) {
          sendJson({ type: 'status', state: 'error', detail: humanizeError(err) });
          cleanup(false);
        }
      }
    }
  });

  ws.on('close', () => {
    cleanup(false);
  });

  ws.on('error', () => {
    cleanup(false);
  });
}
