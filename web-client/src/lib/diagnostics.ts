export const PING_INTERVAL_MS = 1000;
export const PING_TIMEOUT_MS = 2500;
export const MAX_DIAGNOSTIC_SAMPLES = 30;

export interface PingProbe {
  seq: number;
  sentAt: number;
  rttMs: number | null;
  lost: boolean;
}

export interface DiagnosticsState {
  probes: PingProbe[];
  totalSent: number;
  totalReceived: number;
  totalLost: number;
  lastPongAt: number | null;
}

export type LinkHealth = 'waiting' | 'good' | 'fair' | 'poor';

export interface DiagnosticsSummary {
  currentRttMs: number | null;
  avgRttMs: number | null;
  minRttMs: number | null;
  maxRttMs: number | null;
  jitterMs: number | null;
  windowSent: number;
  windowReceived: number;
  windowLost: number;
  inFlight: number;
  packetLossPct: number;
  totalSent: number;
  totalReceived: number;
  totalLost: number;
  health: LinkHealth;
  history: Array<{ seq: number; rttMs: number | null; lost: boolean }>;
}

export function createDiagnosticsState(): DiagnosticsState {
  return {
    probes: [],
    totalSent: 0,
    totalReceived: 0,
    totalLost: 0,
    lastPongAt: null,
  };
}

export function recordPingSent(
  state: DiagnosticsState,
  seq: number,
  nowMs: number,
  maxSamples = MAX_DIAGNOSTIC_SAMPLES,
): DiagnosticsState {
  const nextProbes = [
    ...state.probes,
    { seq, sentAt: nowMs, rttMs: null, lost: false },
  ];
  const trimmed =
    nextProbes.length > maxSamples
      ? nextProbes.slice(nextProbes.length - maxSamples)
      : nextProbes;

  return {
    ...state,
    probes: trimmed,
    totalSent: state.totalSent + 1,
  };
}

export function recordPongReceived(
  state: DiagnosticsState,
  seq: number,
  nowMs: number,
): DiagnosticsState {
  let matched = false;
  let newlyLost = 0;

  const nextProbes = state.probes.map((probe) => {
    if (probe.seq === seq) {
      if (probe.rttMs !== null || probe.lost) {
        return probe;
      }
      matched = true;
      const rtt = Math.max(0, Math.round((nowMs - probe.sentAt) * 10) / 10);
      return { ...probe, rttMs: rtt, lost: false };
    }
    // Over an ordered WebSocket stream, any unanswered earlier probe skipped by a newer pong is lost
    if (probe.seq < seq && probe.rttMs === null && !probe.lost) {
      newlyLost += 1;
      return { ...probe, lost: true };
    }
    return probe;
  });

  if (!matched && newlyLost === 0) {
    return state;
  }

  return {
    ...state,
    probes: nextProbes,
    totalReceived: matched ? state.totalReceived + 1 : state.totalReceived,
    totalLost: state.totalLost + newlyLost,
    lastPongAt: matched ? nowMs : state.lastPongAt,
  };
}

export function sweepTimeouts(
  state: DiagnosticsState,
  nowMs: number,
  timeoutMs = PING_TIMEOUT_MS,
): DiagnosticsState {
  let newlyLost = 0;
  const nextProbes = state.probes.map((probe) => {
    if (probe.rttMs === null && !probe.lost && nowMs - probe.sentAt >= timeoutMs) {
      newlyLost += 1;
      return { ...probe, lost: true };
    }
    return probe;
  });

  if (newlyLost === 0) return state;

  return {
    ...state,
    probes: nextProbes,
    totalLost: state.totalLost + newlyLost,
  };
}

export function classifyLinkHealth(
  avgRttMs: number | null,
  packetLossPct: number,
  jitterMs: number | null,
): LinkHealth {
  if (avgRttMs === null && packetLossPct === 0) return 'waiting';
  if (packetLossPct >= 5 || (avgRttMs !== null && avgRttMs >= 200) || (jitterMs !== null && jitterMs >= 50)) {
    return 'poor';
  }
  if (packetLossPct > 0 || (avgRttMs !== null && avgRttMs >= 80) || (jitterMs !== null && jitterMs >= 20)) {
    return 'fair';
  }
  return 'good';
}

export function computeDiagnosticsSummary(
  state: DiagnosticsState,
  nowMs: number,
  timeoutMs = PING_TIMEOUT_MS,
): DiagnosticsSummary {
  const swept = sweepTimeouts(state, nowMs, timeoutMs);
  const rtts: number[] = [];
  let windowReceived = 0;
  let windowLost = 0;
  let inFlight = 0;

  for (const probe of swept.probes) {
    if (probe.rttMs !== null) {
      rtts.push(probe.rttMs);
      windowReceived += 1;
    } else if (probe.lost) {
      windowLost += 1;
    } else {
      inFlight += 1;
    }
  }

  const settled = windowReceived + windowLost;
  const packetLossPct =
    settled > 0 ? Math.round((windowLost / settled) * 1000) / 10 : 0;

  const currentRttMs = rtts.length > 0 ? rtts[rtts.length - 1] : null;
  const avgRttMs =
    rtts.length > 0
      ? Math.round((rtts.reduce((acc, v) => acc + v, 0) / rtts.length) * 10) / 10
      : null;
  const minRttMs = rtts.length > 0 ? Math.min(...rtts) : null;
  const maxRttMs = rtts.length > 0 ? Math.max(...rtts) : null;

  let jitterMs: number | null = null;
  if (rtts.length >= 2) {
    let deltaSum = 0;
    for (let i = 1; i < rtts.length; i++) {
      deltaSum += Math.abs(rtts[i] - rtts[i - 1]);
    }
    jitterMs = Math.round((deltaSum / (rtts.length - 1)) * 10) / 10;
  }

  const history = swept.probes
    .filter((p) => p.rttMs !== null || p.lost)
    .slice(-24)
    .map((p) => ({ seq: p.seq, rttMs: p.rttMs, lost: p.lost }));

  return {
    currentRttMs,
    avgRttMs,
    minRttMs,
    maxRttMs,
    jitterMs,
    windowSent: swept.probes.length,
    windowReceived,
    windowLost,
    inFlight,
    packetLossPct,
    totalSent: swept.totalSent,
    totalReceived: swept.totalReceived,
    totalLost: swept.totalLost,
    health: classifyLinkHealth(avgRttMs, packetLossPct, jitterMs),
    history,
  };
}
