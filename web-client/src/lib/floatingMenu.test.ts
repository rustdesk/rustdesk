import assert from 'node:assert/strict';
import test from 'node:test';
import {
  clampOrbPosition,
  clampToolbarPixelX,
  clampToolbarX,
  orbCoordsFromState,
  snapOrbToEdge,
} from './floatingMenu';

test('clampToolbarX constrains desktop handle ratio within safe bounds', () => {
  assert.equal(clampToolbarX(0.5), 0.5);
  assert.equal(clampToolbarX(-0.2), 0.12);
  assert.equal(clampToolbarX(1.5), 0.88);
  assert.equal(clampToolbarX(Number.NaN), 0.5);
});

test('clampToolbarPixelX prevents expanded desktop toolbar from clipping off edges', () => {
  // 1440px viewport, 600px bar -> center must stay in [312, 1128]
  assert.equal(clampToolbarPixelX(0.12, 600, 1440, 12), 312);
  assert.equal(clampToolbarPixelX(0.88, 600, 1440, 12), 1128);
  assert.equal(clampToolbarPixelX(0.5, 600, 1440, 12), 720);
});

test('snapOrbToEdge snaps mobile orb to nearest left or right edge and clamps yRatio', () => {
  const left = snapOrbToEdge(40, 200, 46, 390, 844, 10);
  assert.equal(left.side, 'left');
  assert.ok(left.yRatio >= 0.1 && left.yRatio <= 0.86);

  const right = snapOrbToEdge(310, 780, 46, 390, 844, 10);
  assert.equal(right.side, 'right');
  assert.equal(right.yRatio, 0.86);

  const topClamped = snapOrbToEdge(10, -50, 46, 390, 844, 10);
  assert.equal(topClamped.side, 'left');
  assert.equal(topClamped.yRatio, 0.1);
});

test('orbCoordsFromState and clampOrbPosition keep orb strictly inside viewport after resize', () => {
  const coords = orbCoordsFromState({ side: 'right', yRatio: 0.72 }, 46, 390, 844, 10);
  assert.equal(coords.x, 390 - 46 - 10);
  assert.ok(coords.y >= 10 && coords.y <= 844 - 46 - 10);

  const clamped = clampOrbPosition(-20, 999, 46, 390, 844, 10);
  assert.equal(clamped.x, 10);
  assert.equal(clamped.y, 844 - 46 - 10);
});
