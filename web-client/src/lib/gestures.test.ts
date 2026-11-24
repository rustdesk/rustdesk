import test from 'node:test';
import assert from 'node:assert/strict';
import {
  accumulateWheelTicks,
  consumeThreeFingerScroll,
  fingerDown,
  fingerUp,
  isPageZoomed,
  isTwoFingerTap,
  keyboardInset,
  shouldCancelLongPress,
} from './gestures';
import { buildSoftKeyChord, mapPhysicalKeyDown, mapPhysicalKeyUp } from './keys';

test('fingerUp classifies tap, long-press, drag, and moved cancellation', () => {
  assert.equal(fingerUp({ moved: 3, longFired: false, dragging: false }), 'left-click');
  assert.equal(fingerUp({ moved: 3, longFired: true, dragging: false }), 'none');
  assert.equal(fingerUp({ moved: 24, longFired: false, dragging: true }), 'end-drag');
  assert.equal(fingerUp({ moved: 24, longFired: false, dragging: false }), 'none');
});

test('fingerDown triggers drag-lock only on double-tap in mouse mode', () => {
  assert.equal(fingerDown('mouse', 279), 'drag-lock');
  assert.equal(fingerDown('mouse', 280), 'pending');
  assert.equal(fingerDown('touch', 40), 'pending');
  assert.equal(fingerDown('mouse', null), 'pending');
});

test('isTwoFingerTap validates midpoint shift and pinch distance delta', () => {
  assert.equal(isTwoFingerTap(2, 3), true);
  assert.equal(isTwoFingerTap(20, 0), false);
  assert.equal(isTwoFingerTap(0, 30), false);
});

test('keyboardInset computes visualViewport software keyboard height', () => {
  assert.equal(keyboardInset(844, 500, 0), 344);
  assert.equal(keyboardInset(844, 844, 0), 0);
  assert.equal(keyboardInset(844, 820, 24), 0);
});

test('isPageZoomed detects browser-level pinch zoom via visualViewport.scale', () => {
  assert.equal(isPageZoomed(1), false);
  assert.equal(isPageZoomed(1.01), false);
  assert.equal(isPageZoomed(1.15), true);
  assert.equal(isPageZoomed(Number.NaN), false);
});

test('shouldCancelLongPress prevents false right-click during mouse-mode movement', () => {
  assert.equal(shouldCancelLongPress(5), false);
  assert.equal(shouldCancelLongPress(11), true);
});

test('consumeThreeFingerScroll emits discrete wheel ticks and preserves remainder', () => {
  assert.deepEqual(consumeThreeFingerScroll(12), { ticks: 0, remainder: 12 });
  assert.deepEqual(consumeThreeFingerScroll(52), { ticks: 2, remainder: 4 });
  assert.deepEqual(consumeThreeFingerScroll(-30), { ticks: -1, remainder: -6 });
});

test('accumulateWheelTicks translates desktop mouse wheel detents, trackpads, and line/page modes cleanly', () => {
  // Standard desktop mouse wheel down (deltaY = +120px) => -3 RustDesk ticks (scroll down)
  const down = accumulateWheelTicks(0, 120, 0, 0, 0, false);
  assert.equal(down.ty, -3);
  assert.equal(down.tx, 0);

  // Reverse vertical (natural scroll) flips sign
  const downNatural = accumulateWheelTicks(0, 120, 0, 0, 0, true);
  assert.equal(downNatural.ty, 3);

  // DOM_DELTA_LINE (Firefox) preserves line count
  const lineUp = accumulateWheelTicks(0, -3, 1, 0, 0, false);
  assert.equal(lineUp.ty, 3);

  // DOM_DELTA_PAGE clamps to ±6
  const pageDown = accumulateWheelTicks(0, 1, 2, 0, 0, false);
  assert.equal(pageDown.ty, -6);

  // Small trackpad events accumulate across multiple frames
  const p1 = accumulateWheelTicks(0, 10, 0, 0, 0, false);
  assert.equal(p1.ty, 0);
  const p2 = accumulateWheelTicks(0, 10, 0, p1.nextAccumX, p1.nextAccumY, false);
  assert.equal(p2.ty, -1);
});

test('physical keyboard forwards Space and Ctrl+C chord with modifiers', () => {
  const space = mapPhysicalKeyDown({
    code: 'Space',
    key: ' ',
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    repeat: false,
  });
  assert.deepEqual(space, { type: 'key', action: 'down', control: 'Space', modifiers: undefined });

  const ctrlC = mapPhysicalKeyDown({
    code: 'KeyC',
    key: 'c',
    ctrlKey: true,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    repeat: false,
  });
  assert.deepEqual(ctrlC, {
    type: 'key',
    action: 'down',
    chr: 'c',
    modifiers: ['Control'],
  });

  const ctrlCUp = mapPhysicalKeyUp({
    code: 'KeyC',
    key: 'c',
    ctrlKey: true,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    repeat: false,
  });
  assert.deepEqual(ctrlCUp, {
    type: 'key',
    action: 'up',
    chr: 'c',
    modifiers: ['Control'],
  });
});

test('buildSoftKeyChord combines latched modifiers with target key or character', () => {
  assert.deepEqual(buildSoftKeyChord(['Control'], { chr: 'c' }), [
    { control: 'Control' },
    { chr: 'c' },
  ]);
  assert.deepEqual(buildSoftKeyChord(['Control', 'Shift', 'Invalid'], { control: 'Tab' }), [
    { control: 'Control' },
    { control: 'Shift' },
    { control: 'Tab' },
  ]);
  assert.deepEqual(buildSoftKeyChord([], { control: 'Escape' }), [{ control: 'Escape' }]);
});


