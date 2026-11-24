import type { VideoPacket } from "./protocol";

/** Codec strings verified against the RustDesk web client. */
export const WEB_CODECS = [
  { name: "vp8", codec: "vp8" },
  { name: "vp9", codec: "vp09.00.10.08" },
  { name: "av1", codec: "av01.0.08M.08" },
  { name: "h264", codec: "avc1.640033" },
  { name: "h265", codec: "hev1.1.6.L93.B0" },
] as const;

export type CodecName = (typeof WEB_CODECS)[number]["name"];

export interface CodecSupport {
  vp8: boolean;
  vp9: boolean;
  h264: boolean;
  h265: boolean;
  av1: boolean;
  reported: boolean;
}

export function webCodecsStatus(): { ok: boolean; reason: string } {
  if (typeof window === "undefined") {
    return { ok: false, reason: "WebCodecs is only available in a browser." };
  }
  if (!window.isSecureContext) {
    return {
      ok: false,
      reason:
        "WebCodecs is unavailable on this page. Open it as http://localhost or http://127.0.0.1, or put the gateway behind HTTPS. A plain http:// LAN address hides VideoDecoder.",
    };
  }
  if (typeof VideoDecoder === "undefined") {
    return {
      ok: false,
      reason: "This browser has no WebCodecs VideoDecoder. Use a current Chromium, Edge, or Safari.",
    };
  }
  return { ok: true, reason: "" };
}

export async function probeCodecs(): Promise<CodecSupport> {
  const support: CodecSupport = {
    vp8: false,
    vp9: false,
    h264: false,
    h265: false,
    av1: false,
    reported: true,
  };
  if (typeof VideoDecoder === "undefined" || typeof VideoDecoder.isConfigSupported !== "function") {
    return support;
  }
  await Promise.all(
    WEB_CODECS.map(async (item) => {
      try {
        const result = await VideoDecoder.isConfigSupported({
          codec: item.codec,
          optimizeForLatency: true,
        });
        support[item.name] = Boolean(result.supported);
      } catch {
        support[item.name] = false;
      }
    }),
  );
  return support;
}

export class RemoteDecoder {
  private decoder: VideoDecoder | null = null;
  private canvas: HTMLCanvasElement | null = null;
  private codec = "";
  private width = 0;
  private height = 0;
  private configured = false;
  private waitKey = true;
  private pending: VideoFrame | null = null;
  private raf = 0;
  decoded = 0;
  onError: (message: string) => void = () => {};
  onNeedRefresh: () => void = () => {};
  onOutput: (width: number, height: number) => void = () => {};

  close() {
    if (this.raf) {
      cancelAnimationFrame(this.raf);
      this.raf = 0;
    }
    this.pending?.close();
    this.pending = null;
    try {
      this.decoder?.close();
    } catch {
      /* already closed */
    }
    this.decoder = null;
    this.configured = false;
    this.waitKey = true;
  }

  push(packet: VideoPacket, canvas: HTMLCanvasElement | null) {
    this.canvas = canvas;
    if (!canvas || packet.width === 0 || packet.height === 0) return;
    if (
      !this.configured ||
      this.codec !== packet.codecName ||
      this.width !== packet.width ||
      this.height !== packet.height
    ) {
      if (!packet.key) {
        this.waitKey = true;
        return;
      }
      if (!this.configure(packet.codecName, packet.width, packet.height)) return;
    }
    if (!this.configured || !this.decoder) return;
    if (this.waitKey && !packet.key) return;
    this.waitKey = false;
    try {
      this.decoder.decode(
        new EncodedVideoChunk({
          type: packet.key ? "key" : "delta",
          timestamp: Math.round(performance.now() * 1000),
          data: packet.data,
        }),
      );
    } catch (err) {
      this.fail(err instanceof Error ? err.message : String(err));
    }
  }

  private configure(name: string, width: number, height: number): boolean {
    this.close();
    const spec = WEB_CODECS.find((item) => item.name === name);
    if (!spec) {
      this.fail(`No WebCodecs string for ${name}.`);
      return false;
    }
    const canvas = this.canvas;
    this.decoder = new VideoDecoder({
      output: (frame) => this.enqueue(frame),
      error: (err) => this.fail(err.message),
    });
    try {
      this.decoder.configure({
        codec: spec.codec,
        codedWidth: width,
        codedHeight: height,
        optimizeForLatency: true,
      });
    } catch (err) {
      this.fail(
        err instanceof Error
          ? err.message
          : `Could not configure ${spec.codec} (${width}x${height}).`,
      );
      return false;
    }
    this.codec = name;
    this.width = width;
    this.height = height;
    this.configured = true;
    this.waitKey = true;
    this.canvas = canvas;
    return true;
  }

  private enqueue(frame: VideoFrame) {
    if (this.pending) {
      this.pending.close();
    }
    this.pending = frame;
    if (this.raf) return;
    this.raf = requestAnimationFrame(() => this.paint());
  }

  private paint() {
    this.raf = 0;
    const frame = this.pending;
    this.pending = null;
    if (!frame) return;
    try {
      const canvas = this.canvas;
      const ctx = canvas?.getContext("2d");
      if (ctx && canvas) {
        if (canvas.width !== frame.displayWidth || canvas.height !== frame.displayHeight) {
          canvas.width = frame.displayWidth;
          canvas.height = frame.displayHeight;
        }
        ctx.drawImage(frame, 0, 0);
        this.decoded += 1;
        this.onOutput(frame.displayWidth, frame.displayHeight);
      }
    } finally {
      frame.close();
    }
  }

  private fail(message: string) {
    this.onError(message);
    this.onNeedRefresh();
    this.waitKey = true;
    this.configured = false;
    try {
      this.decoder?.close();
    } catch {
      /* already closed */
    }
    this.decoder = null;
  }
}
