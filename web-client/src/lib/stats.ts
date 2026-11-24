export interface FrameSample {
  ts: number;
  bytes: number;
}

export interface FrameWindow {
  samples: FrameSample[];
}

export const DEFAULT_FPS_WINDOW_MS = 1000;

export function createFrameWindow(): FrameWindow {
  return { samples: [] };
}

export function pruneFrameWindow(
  win: FrameWindow,
  nowMs: number,
  windowMs = DEFAULT_FPS_WINDOW_MS,
): FrameWindow {
  const cutoff = nowMs - windowMs;
  let firstValid = 0;
  while (firstValid < win.samples.length && win.samples[firstValid].ts < cutoff) {
    firstValid += 1;
  }
  if (firstValid === 0) return win;
  return { samples: win.samples.slice(firstValid) };
}

export function recordFrameSample(
  win: FrameWindow,
  nowMs: number,
  bytes: number,
  windowMs = DEFAULT_FPS_WINDOW_MS,
): FrameWindow {
  const pruned = pruneFrameWindow(win, nowMs, windowMs);
  return {
    samples: [...pruned.samples, { ts: nowMs, bytes: Math.max(0, bytes) }],
  };
}

/**
 * Computes smoothed sliding-window FPS and bitrate (bits/sec) over the recent window.
 * RustDesk hosts are damage-driven: when the remote desktop is static, 0 new frames arrive
 * and the sliding window decays accurately to 0 instead of sticking on a stale reading.
 */
export function computeWindowRates(
  win: FrameWindow,
  nowMs: number,
  windowMs = DEFAULT_FPS_WINDOW_MS,
): { fps: number; bitrate: number; sampleCount: number } {
  const pruned = pruneFrameWindow(win, nowMs, windowMs);
  const count = pruned.samples.length;
  if (count === 0) {
    return { fps: 0, bitrate: 0, sampleCount: 0 };
  }

  let totalBytes = 0;
  for (const s of pruned.samples) {
    totalBytes += s.bytes;
  }

  // Use the full window duration once enough time has elapsed; during the first second of a
  // burst (e.g. first 300ms after waking from idle), divide by the actual span floored at 250ms
  // so active motion reports realistic FPS immediately instead of under-reporting by 3x.
  const oldestTs = pruned.samples[0].ts;
  const elapsed = Math.min(windowMs, Math.max(250, nowMs - oldestTs));
  const fps = Math.round((count * 1000) / elapsed);
  const bitrate = Math.round((totalBytes * 8 * 1000) / elapsed);
  return { fps, bitrate, sampleCount: count };
}
