import test from 'node:test';
import assert from 'node:assert/strict';
import {
  computeWindowRates,
  createFrameWindow,
  pruneFrameWindow,
  recordFrameSample,
} from './stats';

test('sliding window reports accurate 30fps and bitrate for steady video stream', () => {
  let win = createFrameWindow();
  // Simulate 30 frames arriving every ~33.3ms over 1 second, 5000 bytes each
  for (let i = 0; i < 30; i++) {
    const ts = 1000 + Math.round((i * 1000) / 30);
    win = recordFrameSample(win, ts, 5000);
  }
  const rates = computeWindowRates(win, 2000);
  assert.equal(rates.sampleCount, 30);
  assert.equal(rates.fps, 30);
  // 30 * 5000 bytes * 8 bits = 1,200,000 bps
  assert.equal(rates.bitrate, 1_200_000);
});

test('sliding window decays to 0 fps when remote desktop is static/idle', () => {
  let win = createFrameWindow();
  win = recordFrameSample(win, 1000, 4000);
  win = recordFrameSample(win, 1050, 4000);
  assert.ok(computeWindowRates(win, 1250).fps > 0);

  // After > 1000ms without new frames, window prunes empty and reports 0
  const idle = computeWindowRates(win, 2100);
  assert.equal(idle.fps, 0);
  assert.equal(idle.bitrate, 0);
  assert.equal(idle.sampleCount, 0);
});

test('burst after idle does not under-report FPS by dividing by full 1000ms prematurely', () => {
  let win = createFrameWindow();
  // 10 frames in 333ms (~30 fps burst waking from idle)
  for (let i = 0; i < 10; i++) {
    win = recordFrameSample(win, 5000 + i * 33, 2000);
  }
  const rates = computeWindowRates(win, 5330);
  assert.ok(rates.fps >= 28 && rates.fps <= 32, `expected ~30fps, got ${rates.fps}`);
});

test('pruneFrameWindow drops only samples older than cutoff', () => {
  let win = createFrameWindow();
  win = recordFrameSample(win, 100, 1000);
  win = recordFrameSample(win, 800, 1000);
  win = recordFrameSample(win, 1200, 1000);
  const pruned = pruneFrameWindow(win, 1500, 1000);
  assert.equal(pruned.samples.length, 2);
  assert.equal(pruned.samples[0].ts, 800);
});
