import test from 'node:test';
import assert from 'node:assert/strict';
import {
  clampPan,
  computeNextScaleState,
  computePinchStep,
  computeViewTransform,
  fitScale,
  mapFromRemote,
  mapToRemote,
  mapToRemoteClamped,
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

test('mapToRemoteClamped clamps edge coordinates instead of dropping drag/pointerup events', () => {
  const t = { scale: 0.5, panX: 100, panY: 50 };
  assert.deepEqual(mapToRemoteClamped(0, 0, 1280, 720, t), { x: 0, y: 0 });
  assert.deepEqual(mapToRemoteClamped(2000, 2000, 1280, 720, t), { x: 1279, y: 719 });
});

test('computePinchStep accumulates small 60/120Hz pinch increments smoothly without deadzone loss', () => {
  const boxW = 390;
  const boxH = 740;
  const remoteW = 1920;
  const remoteH = 1080;
  let cur = computeViewTransform(boxW, boxH, remoteW, remoteH, 'fit', 1, { panX: 0, panY: 0 });
  const startScale = cur.scale;

  // Simulate 50 smooth frames where finger distance grows 1.4% per frame (below old 2% deadzone!)
  let dist = 100;
  for (let i = 0; i < 50; i++) {
    const nextDist = dist * 1.014;
    const step = computePinchStep(
      boxW,
      boxH,
      remoteW,
      remoteH,
      cur,
      195,
      370,
      dist,
      195,
      370,
      nextDist,
    );
    cur = { scale: step.scale, panX: step.pan.panX, panY: step.pan.panY };
    dist = nextDist;
  }

  // Expected ~2x scale growth (1.014^50 ≈ 2.004)
  assert.ok(
    cur.scale > startScale * 1.9 && cur.scale < startScale * 2.1,
    `expected scale ~${(startScale * 2).toFixed(3)}, got ${cur.scale.toFixed(3)}`,
  );
});

test('allows dragging initial fit desktop all the way to the very top and very bottom', () => {
  // Portrait phone (390x780) viewing 1920x1080 desktop at fit scale (~0.203, imgH ≈ 219.375)
  const boxW = 390;
  const boxH = 780;
  const remoteW = 1920;
  const remoteH = 1080;
  const fit = fitScale(boxW, boxH, remoteW, remoteH);
  const imgH = remoteH * fit;

  const toTop = clampPan(boxW, boxH, remoteW, remoteH, fit, 0, -500, { allowEdgePan: true });
  assert.equal(toTop.panY, 0, 'should reach the very top edge (panY = 0)');

  const toBottom = clampPan(boxW, boxH, remoteW, remoteH, fit, 0, 1000, { allowEdgePan: true });
  assert.ok(
    Math.abs(toBottom.panY - (boxH - imgH)) < 0.01,
    'should reach the very bottom edge (panY = boxH - imgH)',
  );

  // Two-finger pan at fit scale should switch out of locked fit so top/bottom drag persists
  const initial = computeViewTransform(boxW, boxH, remoteW, remoteH, 'fit', 1, { panX: 0, panY: 0 });
  const pannedUp = computePinchStep(
    boxW,
    boxH,
    remoteW,
    remoteH,
    initial,
    195,
    400,
    100,
    195,
    100,
    100,
  );
  assert.equal(pannedUp.viewMode, 'custom');
  assert.equal(pannedUp.pan.panY, 0);
});

test('supports zooming up to 400% native scale on phones and gives top/bottom clearance when zoomed', () => {
  const boxW = 390;
  const boxH = 780;
  const remoteW = 1920;
  const remoteH = 1080;
  const cur = computeViewTransform(boxW, boxH, remoteW, remoteH, 'fit', 1, { panX: 0, panY: 0 });
  const zoomed = computeNextScaleState(boxW, boxH, remoteW, remoteH, cur, 4.0, 195, 390, {
    edgePadding: 48,
    bottomInset: 112,
  });
  assert.equal(zoomed.scale, 4, 'must allow zooming up to 400% native pixels for small buttons');

  const topClear = clampPan(boxW, boxH, remoteW, remoteH, 4, 0, 200, {
    allowEdgePan: true,
    edgePadding: 48,
    bottomInset: 112,
  });
  assert.equal(topClear.panY, 48, 'zoomed top edge clears floating handle by edgePadding');

  const bottomClear = clampPan(boxW, boxH, remoteW, remoteH, 4, 0, -99999, {
    allowEdgePan: true,
    edgePadding: 48,
    bottomInset: 112,
  });
  assert.equal(
    bottomClear.panY,
    boxH - remoteH * 4 - 48 - 112,
    'zoomed bottom edge clears floating bar + soft keyboard inset',
  );
});


