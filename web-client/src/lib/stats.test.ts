import test from 'node:test';
import assert from 'node:assert/strict';
import {
  computeFrameRates,
  createFrameStats,
  recordRenderedFrame,
  recordRxFrame,
} from './stats';

test('reports waiting state before any frame arrives', () => {
  const s = createFrameStats();
  const r = computeFrameRates(s, 1000);
  assert.equal(r.streamState, 'waiting');
  assert.equal(r.fps, 0);
  assert.equal(r.bitrate, 0);
});

test('sampling-rate calculation reports exact 30fps and 60fps without fencepost bias', () => {
  const s30 = createFrameStats();
  for (let i = 0; i < 31; i++) {
    const ts = 1000 + (i * 1000) / 30;
    recordRxFrame(s30, ts, 5000);
    recordRenderedFrame(s30, ts + 2);
  }
  const r30 = computeFrameRates(s30, 2005);
  assert.equal(r30.streamState, 'active');
  assert.equal(r30.fps, 30);
  assert.equal(r30.renderFps, 30);
  assert.equal(r30.rxFps, 30);
  assert.ok(r30.bitrate > 1_000_000, `expected >1Mbps bitrate, got ${r30.bitrate}`);

  const s60 = createFrameStats();
  for (let i = 0; i < 30; i++) {
    const ts = 5000 + (i * 1000) / 60;
    recordRxFrame(s60, ts, 3000);
    recordRenderedFrame(s60, ts + 1);
  }
  const r60 = computeFrameRates(s60, 5500);
  assert.equal(r60.streamState, 'active');
  assert.ok(r60.fps >= 59 && r60.fps <= 61, `expected ~60fps, got ${r60.fps}`);
});

test('waking from idle reports true sampling rate immediately after 4 frames without ramp-up lag', () => {
  const s = createFrameStats();
  // Earlier active frame long ago (now idle)
  recordRxFrame(s, 1000, 4000);
  recordRenderedFrame(s, 1005);
  assert.equal(computeFrameRates(s, 3000).streamState, 'idle');

  // New 30fps burst (4 frames over 100ms)
  for (let i = 0; i < 4; i++) {
    const ts = 4000 + i * 33.33;
    recordRxFrame(s, ts, 4000);
    recordRenderedFrame(s, ts + 1);
  }
  const r = computeFrameRates(s, 4110);
  assert.equal(r.streamState, 'active');
  assert.ok(r.fps >= 28 && r.fps <= 32, `expected ~30fps immediately on burst, got ${r.fps}`);
});

test('single isolated frame on a static screen reports 1 fps (not inflated 4-5 fps) and decays to idle', () => {
  const s = createFrameStats();
  recordRxFrame(s, 2000, 2000);
  recordRenderedFrame(s, 2002);

  // Immediately after 1 frame arrives: must be 1 fps, NOT 4-5 fps!
  const immediate = computeFrameRates(s, 2010);
  assert.equal(immediate.streamState, 'active');
  assert.equal(immediate.fps, 1);

  // 500ms later, still 1 isolated frame
  const mid = computeFrameRates(s, 2500);
  assert.equal(mid.streamState, 'active');
  assert.equal(mid.fps, 1);

  // >= 1000ms silence -> static/idle, 0 fps, 0 bitrate
  const idle = computeFrameRates(s, 3050);
  assert.equal(idle.streamState, 'idle');
  assert.equal(idle.fps, 0);
  assert.equal(idle.bitrate, 0);
});

test('prefers actual decoded/rendered frame sampling rate over raw incoming packet rate', () => {
  const s = createFrameStats();
  // Incoming packets arrive at 30fps, but decoder only paints 15fps
  for (let i = 0; i < 30; i++) {
    const ts = 1000 + (i * 1000) / 30;
    recordRxFrame(s, ts, 4000);
    if (i % 2 === 0) {
      recordRenderedFrame(s, ts + 4);
    }
  }
  const r = computeFrameRates(s, 2000);
  assert.equal(r.rxFps, 30);
  assert.ok(r.renderFps >= 14 && r.renderFps <= 16, `expected ~15 renderFps, got ${r.renderFps}`);
  assert.equal(r.fps, r.renderFps);
});

test('tapering motion smoothly decays FPS during trailing silence before hitting idle', () => {
  const s = createFrameStats();
  // Short 30fps burst lasting 300ms (10 frames)
  for (let i = 0; i < 10; i++) {
    const ts = 1000 + i * 33.33;
    recordRxFrame(s, ts, 4000);
    recordRenderedFrame(s, ts + 2);
  }
  const peak = computeFrameRates(s, 1310).fps;
  assert.ok(peak >= 28 && peak <= 32);

  // 600ms after last frame (screen went still): FPS should smoothly taper down
  const tapering = computeFrameRates(s, 1900).fps;
  assert.ok(tapering < peak && tapering > 0, `expected tapering fps < ${peak}, got ${tapering}`);

  // 1050ms after last frame: idle 0 fps
  const idle = computeFrameRates(s, 2360);
  assert.equal(idle.streamState, 'idle');
  assert.equal(idle.fps, 0);
});
