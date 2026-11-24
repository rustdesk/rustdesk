import type { CodecSupport, VideoPacket } from './protocol';

export interface CodecProbeEntry {
  name: 'vp8' | 'vp9' | 'av1' | 'h264' | 'h265';
  candidates: readonly string[];
}

/**
 * M11 enhancement: expanded H.264 profile probing (Constrained Baseline, Main, High)
 * for broader Chromium, Edge, and Safari/iOS WebCodecs compatibility.
 */
export const WEB_CODECS: readonly CodecProbeEntry[] = [
  { name: 'vp8', candidates: ['vp8'] },
  { name: 'vp9', candidates: ['vp09.00.41.08', 'vp09.00.10.08'] },
  { name: 'av1', candidates: ['av01.0.08M.08'] },
  { name: 'h264', candidates: ['avc1.640033', 'avc1.4D4033', 'avc1.42E033', 'avc1.42E01F'] },
  { name: 'h265', candidates: ['hev1.1.6.L93.B0'] },
];

const resolvedCodecStrings: Partial<Record<CodecProbeEntry['name'], string>> = {
  vp8: 'vp8',
  vp9: 'vp09.00.41.08',
  av1: 'av01.0.08M.08',
  h264: 'avc1.640033',
  h265: 'hev1.1.6.L93.B0',
};

export function webCodecsStatus(): { ok: boolean; reason: string } {
  if (typeof window === 'undefined') {
    return { ok: false, reason: 'WebCodecs is only available in a browser environment.' };
  }
  if (!window.isSecureContext) {
    return {
      ok: false,
      reason:
        'WebCodecs is unavailable on this page. Open it as http://localhost or http://127.0.0.1, or put the gateway behind HTTPS. A plain http:// LAN address hides VideoDecoder.',
    };
  }
  if (typeof VideoDecoder === 'undefined') {
    return {
      ok: false,
      reason:
        'This browser has no WebCodecs VideoDecoder. Use a current Chromium, Edge, or Safari.',
    };
  }
  return { ok: true, reason: '' };
}

export async function probeCodecs(): Promise<CodecSupport> {
  const result: CodecSupport = {
    vp8: false,
    vp9: false,
    h264: false,
    h265: false,
    av1: false,
    reported: true,
  };

  if (typeof VideoDecoder === 'undefined') {
    return result;
  }

  await Promise.all(
    WEB_CODECS.map(async (entry) => {
      for (const codec of entry.candidates) {
        try {
          const check = await VideoDecoder.isConfigSupported({
            codec,
            optimizeForLatency: true,
          });
          if (check.supported) {
            result[entry.name] = true;
            resolvedCodecStrings[entry.name] = codec;
            return;
          }
        } catch {
          // Try next candidate profile
        }
      }
    }),
  );

  return result;
}

export interface RemoteDecoderCallbacks {
  onOutput: (width: number, height: number, decodedCount: number) => void;
  onError: (message: string) => void;
  onNeedRefresh: () => void;
}

export class RemoteDecoder {
  private decoder: VideoDecoder | null = null;
  private canvas: HTMLCanvasElement | null = null;
  private configured = false;
  private waitKey = true;
  private codec = '';
  private width = 0;
  private height = 0;
  private pending: VideoFrame | null = null;
  private rafId: number | null = null;
  private decodedCount = 0;
  private readonly cb: RemoteDecoderCallbacks;

  constructor(cb: RemoteDecoderCallbacks) {
    this.cb = cb;
  }

  push(packet: VideoPacket, canvas: HTMLCanvasElement | null): void {
    this.canvas = canvas;
    if (!canvas) return;

    const sizeChanged =
      packet.width > 0 &&
      packet.height > 0 &&
      (this.width !== packet.width || this.height !== packet.height);

    if (!this.configured || this.codec !== packet.codecName || sizeChanged) {
      if (!packet.key) {
        this.waitKey = true;
        return;
      }
      if (!this.configure(packet.codecName, packet.width, packet.height)) {
        return;
      }
    }

    if (!this.configured || !this.decoder || this.decoder.state !== 'configured') return;
    if (this.waitKey && !packet.key) return;
    this.waitKey = false;

    try {
      this.decoder.decode(
        new EncodedVideoChunk({
          type: packet.key ? 'key' : 'delta',
          timestamp: Math.round(performance.now() * 1000),
          data: packet.data,
        }),
      );
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      this.fail(msg);
    }
  }

  private configure(codecName: string, width: number, height: number): boolean {
    const codecStr =
      resolvedCodecStrings[codecName as CodecProbeEntry['name']] ??
      WEB_CODECS.find((c) => c.name === codecName)?.candidates[0];

    if (!codecStr) {
      this.fail(`Unsupported video codec: ${codecName}`);
      return false;
    }

    this.closeDecoderOnly();

    try {
      this.decoder = new VideoDecoder({
        output: (frame) => this.enqueue(frame),
        error: (err) => this.fail(err.message || 'VideoDecoder error'),
      });
      const cfg: VideoDecoderConfig = {
        codec: codecStr,
        optimizeForLatency: true,
      };
      if (width > 0 && height > 0) {
        cfg.codedWidth = width;
        cfg.codedHeight = height;
      }
      this.decoder.configure(cfg);
      this.configured = true;
      this.codec = codecName;
      this.width = width;
      this.height = height;
      this.waitKey = true;
      return true;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      this.fail(msg);
      return false;
    }
  }

  private enqueue(frame: VideoFrame): void {
    if (this.pending) {
      this.pending.close();
    }
    this.pending = frame;
    if (this.rafId === null) {
      this.rafId = requestAnimationFrame(() => {
        this.rafId = null;
        this.paint();
      });
    }
  }

  private paint(): void {
    const frame = this.pending;
    this.pending = null;
    if (!frame) return;

    const canvas = this.canvas;
    if (canvas) {
      const w = frame.displayWidth || frame.codedWidth;
      const h = frame.displayHeight || frame.codedHeight;
      if (w > 0 && h > 0) {
        if (canvas.width !== w || canvas.height !== h) {
          canvas.width = w;
          canvas.height = h;
        }
        const ctx = canvas.getContext('2d');
        if (ctx) {
          ctx.drawImage(frame, 0, 0, w, h);
          this.decodedCount += 1;
          this.cb.onOutput(w, h, this.decodedCount);
        }
      }
    }
    frame.close();
  }

  private fail(message: string): void {
    this.cb.onError(message);
    this.cb.onNeedRefresh();
    this.waitKey = true;
    this.configured = false;
    this.closeDecoderOnly();
  }

  private closeDecoderOnly(): void {
    if (this.decoder) {
      try {
        if (this.decoder.state !== 'closed') {
          this.decoder.close();
        }
      } catch {
        // Ignore close race
      }
      this.decoder = null;
    }
  }

  close(): void {
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
    if (this.pending) {
      this.pending.close();
      this.pending = null;
    }
    this.closeDecoderOnly();
    this.configured = false;
    this.waitKey = true;
    this.codec = '';
    this.width = 0;
    this.height = 0;
    this.decodedCount = 0;
  }
}
