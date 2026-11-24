import test from 'node:test';
import assert from 'node:assert/strict';
import {
  clampPan,
  computeNextScaleState,
  computeViewTransform,
  fitScale,
  mapFromRemote,
  mapToRemote,
  zoomAround,
} from './mapping';

test('fits 1280x720 into 900x900, maps center accurately, and rejects out-of-bounds clicks', () => {
  const scale = fitScale(900, 900, 1280, 720);
  assert.ok(Math.abs(scale - 900 / 1280) < 1e-6);

  const pan = clampPan(900, 900, 1280, 720, scale, 0, 0);
  const t = { scale, ...pan };
  const center = mapToRemote(450, 450, 1280, 720, t);
  assert.ok(center);
  assert.ok(Math.abs(center.x - 640) <= 1);
  assert.ok(Math.abs(center.y - 360) <= 1);

  // Above the letterboxed canvas must return null
  assert.equal(mapToRemote(450, 10, 1280, 720, t), null);
});

test('clamps 1:1 panning within viewport bounds', () => {
  const clamped = clampPan(800, 600, 1280, 720, 1, 40, -900);
  assert.deepEqual(clamped, { panX: 0, panY: -120 });
});

test('zoomAround keeps the remote coordinate under the anchor unchanged', () => {
  const t0 = { scale: 0.75, panX: 50, panY: 80 };
  const anchorX = 320;
  const anchorY = 240;
  const before = mapToRemote(anchorX, anchorY, 1280, 720, t0);
  assert.ok(before);

  const nextPan = zoomAround(anchorX, anchorY, t0.scale, 1.5, t0.panX, t0.panY);
  const t1 = { scale: 1.5, ...nextPan };
  const after = mapToRemote(anchorX, anchorY, 1280, 720, t1);
  assert.ok(after);
  assert.ok(Math.abs(after.x - before.x) <= 1);
  assert.ok(Math.abs(after.y - before.y) <= 1);
});

test('mapFromRemote and mapToRemote roundtrip cleanly when zoomed and panned', () => {
  const t = { scale: 1.6, panX: -240, panY: -110 };
  const screen = mapFromRemote(640, 486, t);
  const back = mapToRemote(screen.x, screen.y, 1280, 720, t);
  assert.deepEqual(back, { x: 640, y: 486 });
});

test('computeNextScaleState and computeViewTransform share consistent scale bounds', () => {
  const cur = computeViewTransform(1440, 900, 1280, 720, 'fit', 1, { panX: 0, panY: 0 });
  const zoomedOut = computeNextScaleState(1440, 900, 1280, 720, cur, 1.0, 720, 450);
  const rendered = computeViewTransform(
    1440,
    900,
    1280,
    720,
    zoomedOut.viewMode,
    zoomedOut.zoom,
    zoomedOut.pan,
  );
  assert.equal(zoomedOut.viewMode, 'original');
  assert.equal(rendered.scale, 1);
});
