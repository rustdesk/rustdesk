import test from 'node:test';
import assert from 'node:assert/strict';
import {
  computeDiagnosticsSummary,
  createDiagnosticsState,
  recordPingSent,
  recordPongReceived,
  sweepTimeouts,
} from './diagnostics.ts';

test('diagnostics computes raw ping-pong latency, min/avg/max, and jitter accurately', () => {
  let st = createDiagnosticsState();
  st = recordPingSent(st, 1, 1000);
  st = recordPongReceived(st, 1, 1020); // 20ms
  st = recordPingSent(st, 2, 2000);
  st = recordPongReceived(st, 2, 2030); // 30ms
  st = recordPingSent(st, 3, 3000);
  st = recordPongReceived(st, 3, 3040); // 40ms

  const summary = computeDiagnosticsSummary(st, 3100);
  assert.equal(summary.currentRttMs, 40);
  assert.equal(summary.minRttMs, 20);
  assert.equal(summary.maxRttMs, 40);
  assert.equal(summary.avgRttMs, 30);
  assert.equal(summary.jitterMs, 10);
  assert.equal(summary.packetLossPct, 0);
  assert.equal(summary.windowReceived, 3);
  assert.equal(summary.windowLost, 0);
  assert.equal(summary.health, 'good');
});

test('diagnostics detects packet loss from both timeouts and skipped sequence numbers', () => {
  let st = createDiagnosticsState();
  // seq 1 answered in 25ms
  st = recordPingSent(st, 1, 1000);
  st = recordPongReceived(st, 1, 1025);
  // seq 2 dropped by proxy before seq 3 pong arrives
  st = recordPingSent(st, 2, 2000);
  st = recordPingSent(st, 3, 3000);
  st = recordPongReceived(st, 3, 3035);
  // seq 4 times out (>2500ms without pong)
  st = recordPingSent(st, 4, 4000);

  st = sweepTimeouts(st, 6600, 2500);
  const summary = computeDiagnosticsSummary(st, 6600, 2500);

  assert.equal(summary.windowReceived, 2);
  assert.equal(summary.windowLost, 2);
  assert.equal(summary.packetLossPct, 50);
  assert.equal(summary.totalLost, 2);
  assert.equal(summary.health, 'poor');
  assert.equal(summary.history.length, 4);
  assert.equal(summary.history[1].lost, true);
  assert.equal(summary.history[3].lost, true);
});

test('diagnostics ignores duplicate or late pongs and enforces rolling window size', () => {
  let st = createDiagnosticsState();
  for (let i = 1; i <= 5; i++) {
    st = recordPingSent(st, i, i * 1000, 3);
    st = recordPongReceived(st, i, i * 1000 + 10 * i);
  }
  // Duplicate pong for seq 5 should not change counts
  st = recordPongReceived(st, 5, 9999);
  // Unknown seq should be ignored
  st = recordPongReceived(st, 999, 9999);

  const summary = computeDiagnosticsSummary(st, 6000);
  assert.equal(summary.windowSent, 3);
  assert.equal(summary.windowReceived, 3);
  assert.equal(summary.totalSent, 5);
  assert.equal(summary.totalReceived, 5);
  assert.equal(summary.minRttMs, 30);
  assert.equal(summary.maxRttMs, 50);
  assert.equal(summary.currentRttMs, 50);
});
