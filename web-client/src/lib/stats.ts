export interface RxFrameSample {
  ts: number;
  bytes: number;
}

export interface RenderFrameSample {
  ts: number;
}

export type StreamActivityState = 'waiting' | 'active' | 'idle';

export interface FrameStatsState {
  rxSamples: RxFrameSample[];
  renderSamples: RenderFrameSample[];
  /** Exponentially weighted moving average of active inter-frame interval (ms). */
  emaIntervalMs: number | null;
  lastRxTs: number | null;
  lastRenderTs: number | null;
  totalRx: number;
  totalRendered: number;
}

export interface FrameRatesSnapshot {
  /** Primary real-time FPS (uses decoded/rendered sampling rate when available, else incoming rate). */
  fps: number;
  /** Incoming encoded packet sampling rate (fps). */
  rxFps: number;
  /** Decoded & painted canvas frame sampling rate (fps). */
  renderFps: number;
  /** Real-time video bitrate in bits/sec. */
  bitrate: number;
  /** Current stream activity: waiting for first frame, actively updating, or static/idle. */
  streamState: StreamActivityState;
  /** Number of samples currently inside the active window. */
  sampleCount: number;
}

export const DEFAULT_STATS_WINDOW_MS = 1500;
export const IDLE_SILENCE_MS = 1000;
export const MAX_ACTIVE_INTERVAL_MS = 500;
export const EMA_ALPHA = 0.28;
export const MAX_REPORTED_FPS = 120;

export function createFrameStats(): FrameStatsState {
  return {
    rxSamples: [],
    renderSamples: [],
    emaIntervalMs: null,
    lastRxTs: null,
    lastRenderTs: null,
    totalRx: 0,
    totalRendered: 0,
  };
}

function firstSampleInWindow(
  samples: readonly { ts: number }[],
  nowMs: number,
  windowMs: number,
): number {
  const cutoff = nowMs - windowMs;
  let firstValid = 0;
  while (firstValid < samples.length && samples[firstValid].ts < cutoff) {
    firstValid += 1;
  }
  return firstValid;
}

function pruneSamplesInPlace(samples: { ts: number }[], nowMs: number, windowMs: number): void {
  const firstValid = firstSampleInWindow(samples, nowMs, windowMs);
  if (firstValid > 0) {
    const remaining = samples.length - firstValid;
    for (let i = 0; i < remaining; i += 1) {
      samples[i] = samples[i + firstValid];
    }
    samples.length = remaining;
  }
}

function updateEmaInterval(prevEma: number | null, prevTs: number | null, nowMs: number): number | null {
  if (prevTs === null) return prevEma;
  const dt = nowMs - prevTs;
  if (dt <= 0) return prevEma;
  if (dt > MAX_ACTIVE_INTERVAL_MS) {
    // Gap was too long (returning from static/idle screen); reset EMA so stale cadence does not bias the new burst.
    return null;
  }
  if (prevEma === null) return dt;
  return prevEma * (1 - EMA_ALPHA) + dt * EMA_ALPHA;
}

export function recordRxFrame(
  state: FrameStatsState,
  nowMs: number,
  bytes: number,
  windowMs = DEFAULT_STATS_WINDOW_MS,
): void {
  pruneSamplesInPlace(state.rxSamples, nowMs, windowMs);
  state.rxSamples.push({ ts: nowMs, bytes: Math.max(0, bytes) });
  if (state.totalRendered === 0) {
    state.emaIntervalMs = updateEmaInterval(state.emaIntervalMs, state.lastRxTs, nowMs);
  }
  state.lastRxTs = nowMs;
  state.totalRx += 1;
}

export function recordRenderedFrame(
  state: FrameStatsState,
  nowMs: number,
  windowMs = DEFAULT_STATS_WINDOW_MS,
): void {
  pruneSamplesInPlace(state.renderSamples, nowMs, windowMs);
  state.renderSamples.push({ ts: nowMs });
  state.emaIntervalMs = updateEmaInterval(state.emaIntervalMs, state.lastRenderTs, nowMs);
  state.lastRenderTs = nowMs;
  state.totalRendered += 1;
}

/**
 * Computes an unbiased sampling-rate FPS from timestamped frames inside the sliding window:
 * - Idle silence (>= IDLE_SILENCE_MS since last frame): returns 0 (screen is static).
 * - Single isolated frame (e.g. blinking cursor on a static desktop): returns 1 instead of
 *   dividing by an artificial sub-second floor (which previously fabricated 4-5 fps).
 * - Active multi-frame stream (count >= 2): computes interval-based rate `(N - 1) / span`,
 *   accounting for trailing silence when motion tapers off, and lightly blends with EMA
 *   inter-frame cadence during steady motion to eliminate WebSocket batch jitter.
 */
function computeSamplingFps(
  samples: readonly { ts: number }[],
  firstSample: number,
  lastTs: number | null,
  emaIntervalMs: number | null,
  nowMs: number,
): number {
  if (lastTs === null) return 0;
  const silenceMs = Math.max(0, nowMs - lastTs);
  if (silenceMs >= IDLE_SILENCE_MS) return 0;

  const count = samples.length - firstSample;
  if (count === 0) return 0;

  if (count === 1) {
    // One isolated frame inside the active window (e.g. a clock tick or cursor blink on a static screen)
    return 1;
  }

  const firstTs = samples[firstSample].ts;
  const newestTs = samples[samples.length - 1].ts;
  const spanMs = Math.max(1, newestTs - firstTs);
  const avgIntervalMs = spanMs / (count - 1);

  // If the stream is actively arriving (trailing silence within ~2.5 expected frame intervals),
  // use the exact inter-frame sampling span blended with EMA cadence for responsive, steady FPS.
  if (silenceMs <= Math.max(120, avgIntervalMs * 2.5)) {
    const windowRate = ((count - 1) * 1000) / spanMs;
    const blended =
      emaIntervalMs !== null && emaIntervalMs > 0
        ? windowRate * 0.65 + (1000 / emaIntervalMs) * 0.35
        : windowRate;
    return Math.min(MAX_REPORTED_FPS, Math.max(1, Math.round(blended)));
  }

  // Motion is tapering off toward idle: include trailing silence in the elapsed span so FPS decays smoothly.
  const effectiveSpanMs = Math.max(spanMs, nowMs - firstTs);
  const decayingRate = ((count - 1) * 1000) / effectiveSpanMs;
  return Math.min(MAX_REPORTED_FPS, Math.max(1, Math.round(decayingRate)));
}

export function computeFrameRates(
  state: FrameStatsState,
  nowMs: number,
  windowMs = DEFAULT_STATS_WINDOW_MS,
): FrameRatesSnapshot {
  const hasAnyFrame = state.totalRx > 0 || state.totalRendered > 0;
  if (!hasAnyFrame) {
    return {
      fps: 0,
      rxFps: 0,
      renderFps: 0,
      bitrate: 0,
      streamState: 'waiting',
      sampleCount: 0,
    };
  }

  const latestTs = Math.max(state.lastRxTs ?? -Infinity, state.lastRenderTs ?? -Infinity);
  const silenceMs = Math.max(0, nowMs - latestTs);

  if (silenceMs >= IDLE_SILENCE_MS) {
    return {
      fps: 0,
      rxFps: 0,
      renderFps: 0,
      bitrate: 0,
      streamState: 'idle',
      sampleCount: 0,
    };
  }

  const firstRx = firstSampleInWindow(state.rxSamples, nowMs, windowMs);
  const firstRender = firstSampleInWindow(state.renderSamples, nowMs, windowMs);
  const rxCount = state.rxSamples.length - firstRx;
  const renderCount = state.renderSamples.length - firstRender;

  const rxFps = computeSamplingFps(
    state.rxSamples,
    firstRx,
    state.lastRxTs,
    state.totalRendered === 0 ? state.emaIntervalMs : null,
    nowMs,
  );
  const renderFps = computeSamplingFps(
    state.renderSamples,
    firstRender,
    state.lastRenderTs,
    state.totalRendered > 0 ? state.emaIntervalMs : null,
    nowMs,
  );

  // Prefer actual decoded/rendered sampling FPS once WebCodecs is producing frames.
  const useRendered = state.totalRendered > 0;
  const fps = useRendered ? renderFps : rxFps;
  const sampleCount = useRendered ? renderCount : rxCount;

  // Bitrate (bits/sec) over the active rx window
  let bitrate = 0;
  if (rxCount > 0) {
    let totalBytes = 0;
    for (let i = firstRx; i < state.rxSamples.length; i += 1) {
      totalBytes += state.rxSamples[i].bytes;
    }
    const rxElapsed =
      rxCount === 1
        ? windowMs
        : Math.max(250, Math.min(windowMs, nowMs - state.rxSamples[firstRx].ts));
    bitrate = Math.round((totalBytes * 8 * 1000) / rxElapsed);
  }

  return {
    fps,
    rxFps,
    renderFps,
    bitrate,
    streamState: 'active',
    sampleCount,
  };
}
