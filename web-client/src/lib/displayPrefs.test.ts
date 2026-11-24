import test from 'node:test';
import assert from 'node:assert/strict';
import {
  DEFAULT_DISPLAY_PREFS,
  applyMouseButtonSwap,
  applyWheelDirection,
  filterCodecSupport,
  normalizeDisplayPrefs,
  resolveQualityLevel,
  resolveTargetFps,
} from './displayPrefs';

test('normalizeDisplayPrefs restores safe defaults and clamps bounds', () => {
  const empty = normalizeDisplayPrefs(null);
  assert.deepEqual(empty, DEFAULT_DISPLAY_PREFS);

  const normalized = normalizeDisplayPrefs({
    viewStyle: 'original',
    scrollStyle: 'scrollbar',
    qualityPreset: 'custom',
    customBitrate: 999,
    customFps: 3,
    codec: 'vp9',
    viewOnly: true,
    showRemoteCursor: false,
    disableClipboard: true,
    lockAfterSessionEnd: true,
    reverseMouseWheel: true,
    swapMouseButtons: true,
  });

  assert.equal(normalized.viewStyle, 'original');
  assert.equal(normalized.scrollStyle, 'scrollbar');
  assert.equal(normalized.qualityPreset, 'custom');
  assert.equal(normalized.customBitrate, 200);
  assert.equal(normalized.customFps, 10);
  assert.equal(normalized.codec, 'vp9');
  assert.equal(normalized.viewOnly, true);
  assert.equal(normalized.showRemoteCursor, false);
  assert.equal(normalized.disableClipboard, true);
  assert.equal(normalized.lockAfterSessionEnd, true);
  assert.equal(normalized.reverseMouseWheel, true);
  assert.equal(normalized.swapMouseButtons, true);
});

test('resolveQualityLevel and resolveTargetFps map presets and custom values', () => {
  assert.equal(resolveQualityLevel('best', 50), 'best');
  assert.equal(resolveQualityLevel('balanced', 50), 'balanced');
  assert.equal(resolveQualityLevel('low', 50), 'low');
  assert.equal(resolveQualityLevel('custom', 25), 'low');
  assert.equal(resolveQualityLevel('custom', 50), 'balanced');
  assert.equal(resolveQualityLevel('custom', 90), 'best');

  assert.equal(resolveTargetFps('best', 30), 60);
  assert.equal(resolveTargetFps('balanced', 30), 30);
  assert.equal(resolveTargetFps('low', 30), 15);
  assert.equal(resolveTargetFps('custom', 45), 45);
  assert.equal(resolveTargetFps('custom', 120), 60);
});

test('filterCodecSupport disables AV1 in auto mode when faster codecs exist, isolates explicit codecs, and falls back gracefully', () => {
  const support = {
    vp8: true,
    vp9: true,
    h264: true,
    h265: false,
    av1: true,
    reported: true,
  };

  assert.deepEqual(filterCodecSupport(support, 'auto'), {
    vp8: true,
    vp9: true,
    h264: true,
    h265: false,
    av1: false,
    reported: true,
  });
  assert.deepEqual(filterCodecSupport(support, 'av1'), {
    vp8: false,
    vp9: false,
    h264: false,
    h265: false,
    av1: true,
    reported: true,
  });
  assert.deepEqual(filterCodecSupport(support, 'vp9'), {
    vp8: false,
    vp9: true,
    h264: false,
    h265: false,
    av1: false,
    reported: true,
  });
  // Unsupported preferred codec falls back to auto (AV1 disabled when faster codecs exist)
  assert.deepEqual(filterCodecSupport(support, 'h265'), {
    vp8: true,
    vp9: true,
    h264: true,
    h265: false,
    av1: false,
    reported: true,
  });
});

test('mouse button swap and wheel inversion behave deterministically', () => {
  assert.equal(applyMouseButtonSwap('left', false), 'left');
  assert.equal(applyMouseButtonSwap('left', true), 'right');
  assert.equal(applyMouseButtonSwap('right', true), 'left');
  assert.equal(applyMouseButtonSwap('middle', true), 'middle');

  assert.equal(applyWheelDirection(1, false), 1);
  assert.equal(applyWheelDirection(1, true), -1);
  assert.equal(applyWheelDirection(-1, true), 1);
  assert.equal(applyWheelDirection(0, true), 0);
});
