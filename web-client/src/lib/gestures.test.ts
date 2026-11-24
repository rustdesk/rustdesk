import test from 'node:test';
import assert from 'node:assert/strict';
import {
  accumulateWheelTicks,
  consumeThreeFingerScroll,
  createTouchGestureState,
  fingerDown,
  fingerUp,
  gestureLongPressTimeout,
  gesturePointerDown,
  gesturePointerMove,
  gesturePointerUpOrCancel,
  isPageZoomed,
  isTwoFingerTap,
  keyboardInset,
  resetTouchGestureState,
  shouldCancelLongPress,
} from './gestures';
import {
  buildSoftKeyChord,
  controlFromKeyOrCode,
  diffTextInput,
  isImeEvent,
  mapPhysicalKeyDown,
  mapPhysicalKeyUp,
} from './keys';

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

test('physical keyboard sends native chr down/up for ASCII (including repeat and Shift tracking) and ignores IME composition events', () => {
  const downA = mapPhysicalKeyDown({
    code: 'KeyA',
    key: 'a',
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    repeat: false,
  });
  assert.deepEqual(downA, { type: 'key', action: 'down', chr: 'a', modifiers: undefined });

  const repeatA = mapPhysicalKeyDown({
    code: 'KeyA',
    key: 'a',
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    repeat: true,
  });
  assert.deepEqual(repeatA, { type: 'key', action: 'down', chr: 'a', modifiers: undefined });

  // Shift released before KeyA: trackedChr ensures uppercase 'A' is released cleanly
  const upTracked = mapPhysicalKeyUp(
    {
      code: 'KeyA',
      key: 'a',
      ctrlKey: false,
      altKey: false,
      shiftKey: false,
      metaKey: false,
      repeat: false,
    },
    'A',
  );
  assert.deepEqual(upTracked, { type: 'key', action: 'up', chr: 'A', modifiers: undefined });

  // Chinese / CJK IME Process or isComposing events must pass through to native IME
  assert.equal(isImeEvent({ isComposing: true, key: 'n' }), true);
  assert.equal(isImeEvent({ key: 'Process', keyCode: 229 }), true);
  assert.equal(
    mapPhysicalKeyDown({
      code: 'KeyN',
      key: 'Process',
      ctrlKey: false,
      altKey: false,
      shiftKey: false,
      metaKey: false,
      repeat: false,
      isComposing: true,
      keyCode: 229,
    }),
    null,
  );

  // Mobile soft keyboards often emit empty code with key='Backspace'/'Enter'
  assert.equal(controlFromKeyOrCode('', 'Backspace'), 'Backspace');
  assert.equal(controlFromKeyOrCode('Unidentified', 'Enter'), 'Enter');
});

test('diffTextInput computes minimal Backspace + Unicode insertion deltas without splitting CJK or emoji', () => {
  assert.deepEqual(diffTextInput('', 'hello'), { backspaces: 0, insert: 'hello' });
  assert.deepEqual(diffTextInput('hello', 'hello'), { backspaces: 0, insert: '' });
  assert.deepEqual(diffTextInput('hello', 'hell'), { backspaces: 1, insert: '' });
  assert.deepEqual(diffTextInput('你好', '你好世界'), { backspaces: 0, insert: '世界' });
  assert.deepEqual(diffTextInput('你好世界', '你好中国'), { backspaces: 2, insert: '中国' });
  assert.deepEqual(diffTextInput('hi😀', 'hi'), { backspaces: 1, insert: '' });
});

/* -------------------------------------------------------------------------- */
/*  Touch Gesture FSM regression tests (anti-scramble & anti-lockup)          */
/* -------------------------------------------------------------------------- */

test('TouchGesture FSM ignores untracked bubbled pointerup from overlay chrome (no phantom clicks)', () => {
  let s = createTouchGestureState();
  // Legitimate tap at (100, 100)
  let r = gesturePointerDown(s, 1, 100, 100, { mode: 'touch', hand: false, now: 1000, isPrimary: true });
  s = r.state;
  r = gesturePointerUpOrCancel(s, 1, 100, 100, 'up', { now: 1080 });
  s = r.state;
  assert.deepEqual(r.effects.filter((e) => e.type === 'tap-click'), [{ type: 'tap-click', x: 100, y: 100 }]);

  // Bubbled pointerup from MobileDock / SessionHandle (pointerId 99 was never registered on stage)
  r = gesturePointerUpOrCancel(s, 99, 320, 780, 'up', { now: 1200 });
  assert.deepEqual(r.effects, []);
  assert.equal(r.state.phase, 'idle');
});

test('TouchGesture FSM recovers from dropped pointerup via stale timeout or authoritative liveTouchCount=1 (never locks up or scrambles into pinch)', () => {
  let s = createTouchGestureState();
  // Start a 2-finger pinch; finger 2 lifts, finger 1 NEVER receives pointerup (dropped by OS)
  s = gesturePointerDown(s, 1, 100, 100, { mode: 'touch', hand: false, now: 1000, isPrimary: true }).state;
  s = gesturePointerDown(s, 2, 200, 100, { mode: 'touch', hand: false, now: 1010, isPrimary: false }).state;
  assert.equal(s.phase, 'pinch');
  s = gesturePointerMove(s, 2, 260, 100, { mode: 'touch', now: 1040 }).state;
  s = gesturePointerUpOrCancel(s, 2, 260, 100, 'up', { now: 1080 }).state;
  // Finger 1 is now orphaned in cooldown!
  assert.equal(s.phase, 'cooldown');
  assert.equal(s.pointers.size, 1);

  // Case A: User touches down again quickly (200ms later) with 1 physical finger (native liveTouchCount === 1)
  const rFast = gesturePointerDown(s, 3, 150, 150, {
    mode: 'touch',
    hand: false,
    now: 1280,
    isPrimary: true,
    liveTouchCount: 1,
  });
  assert.equal(rFast.state.phase, 'single', 'must prune zombie finger 1 instead of scrambling into pinch');
  assert.equal(rFast.state.pointers.size, 1);
  const rFastUp = gesturePointerUpOrCancel(rFast.state, 3, 150, 150, 'up', { now: 1340 });
  assert.deepEqual(rFastUp.effects.filter((e) => e.type === 'tap-click'), [{ type: 'tap-click', x: 150, y: 150 }]);
  assert.equal(rFastUp.state.phase, 'idle');

  // Case B: Without liveTouchCount, after STALE_POINTER_MS (>650ms), primary pointerdown prunes zombie & resets cooldown
  const rStale = gesturePointerDown(s, 4, 180, 180, {
    mode: 'touch',
    hand: false,
    now: 1800,
    isPrimary: true,
    liveTouchCount: null,
  });
  assert.equal(rStale.state.phase, 'single', 'stale primary touch must reset cooldown and enter single phase');
  assert.equal(rStale.state.pointers.size, 1);
  const rStaleUp = gesturePointerUpOrCancel(rStale.state, 4, 180, 180, 'up', { now: 1860 });
  assert.deepEqual(rStaleUp.effects.filter((e) => e.type === 'tap-click'), [{ type: 'tap-click', x: 180, y: 180 }]);
});

test('TouchGesture FSM never fires false two-finger tap on pinch-out-then-in, slow hold, or pointercancel', () => {
  // 1) Pinch out by 50px then pinch back to exact origin -> maxDistDelta blocks two-finger tap
  let s = createTouchGestureState();
  s = gesturePointerDown(s, 1, 100, 200, { mode: 'touch', hand: false, now: 1000, isPrimary: true }).state;
  s = gesturePointerDown(s, 2, 200, 200, { mode: 'touch', hand: false, now: 1010, isPrimary: false }).state;
  s = gesturePointerMove(s, 2, 250, 200, { mode: 'touch', now: 1040 }).state;
  s = gesturePointerMove(s, 2, 200, 200, { mode: 'touch', now: 1080 }).state;
  let up = gesturePointerUpOrCancel(s, 2, 200, 200, 'up', { now: 1120 });
  assert.equal(up.effects.some((e) => e.type === 'two-finger-tap'), false);

  // 2) Pointercancel on stationary 2-finger touch -> NEVER fires right-click
  s = createTouchGestureState();
  s = gesturePointerDown(s, 1, 100, 200, { mode: 'touch', hand: false, now: 2000, isPrimary: true }).state;
  s = gesturePointerDown(s, 2, 200, 200, { mode: 'touch', hand: false, now: 2010, isPrimary: false }).state;
  up = gesturePointerUpOrCancel(s, 2, 200, 200, 'cancel', { now: 2060 });
  assert.equal(up.effects.some((e) => e.type === 'two-finger-tap'), false);

  // 3) Genuine crisp two-finger tap (<350ms, minimal move) -> fires two-finger-tap at midpoint
  s = createTouchGestureState();
  s = gesturePointerDown(s, 1, 100, 200, { mode: 'touch', hand: false, now: 3000, isPrimary: true }).state;
  s = gesturePointerDown(s, 2, 200, 200, { mode: 'touch', hand: false, now: 3010, isPrimary: false }).state;
  up = gesturePointerUpOrCancel(s, 2, 202, 201, 'up', { now: 3120 });
  assert.deepEqual(up.effects, [{ type: 'two-finger-tap', x: 150, y: 200 }]);
  assert.equal(up.state.phase, 'cooldown');
  // Lifting remaining finger enters idle without stray tap-click
  const up2 = gesturePointerUpOrCancel(up.state, 1, 100, 200, 'up', { now: 3140 });
  assert.deepEqual(up2.effects, []);
  assert.equal(up2.state.phase, 'idle');
});

test('TouchGesture FSM handles 3-finger scroll -> lift -> immediate single tap cleanly without stray zoom or click', () => {
  let s = createTouchGestureState();
  s = gesturePointerDown(s, 1, 100, 300, { mode: 'touch', hand: false, now: 1000, isPrimary: true }).state;
  s = gesturePointerDown(s, 2, 160, 300, { mode: 'touch', hand: false, now: 1005 }).state;
  s = gesturePointerDown(s, 3, 220, 300, { mode: 'touch', hand: false, now: 1010 }).state;
  assert.equal(s.phase, 'three-finger');

  // Move all 3 fingers down by 36px -> emits wheel ticks as average Y crosses 24px step
  const m1 = gesturePointerMove(s, 1, 100, 336, { mode: 'touch', now: 1030 });
  const m2 = gesturePointerMove(m1.state, 2, 160, 336, { mode: 'touch', now: 1032 });
  const m3 = gesturePointerMove(m2.state, 3, 220, 336, { mode: 'touch', now: 1034 });
  s = m3.state;
  const scrollEffects = [...m1.effects, ...m2.effects, ...m3.effects];
  assert.ok(scrollEffects.some((e) => e.type === 'three-finger-scroll' && e.ticks > 0));

  // Lift fingers asynchronously: 3 -> 2 -> 1 -> 0 must stay in cooldown until all lifted (no accidental pinch or right-click)
  const u3 = gesturePointerUpOrCancel(s, 3, 220, 336, 'up', { now: 1080 });
  assert.equal(u3.state.phase, 'cooldown');
  assert.deepEqual(u3.effects, []);
  const cooldownMove2 = gesturePointerMove(u3.state, 2, 180, 340, { mode: 'touch', now: 1090 });
  assert.deepEqual(cooldownMove2.effects, []);
  const u2 = gesturePointerUpOrCancel(cooldownMove2.state, 2, 180, 340, 'up', { now: 1100 });
  assert.equal(u2.state.phase, 'cooldown');
  const u1 = gesturePointerUpOrCancel(u2.state, 1, 100, 336, 'up', { now: 1110 });
  assert.equal(u1.state.phase, 'idle');
  assert.deepEqual(u1.effects, []);

  // Immediate next single-finger tap works immediately
  const tapDown = gesturePointerDown(u1.state, 4, 240, 240, { mode: 'touch', hand: false, now: 1150, isPrimary: true });
  assert.equal(tapDown.state.phase, 'single');
  const tapUp = gesturePointerUpOrCancel(tapDown.state, 4, 240, 240, 'up', { now: 1210 });
  assert.deepEqual(tapUp.effects.filter((e) => e.type === 'tap-click'), [{ type: 'tap-click', x: 240, y: 240 }]);
});

test('TouchGesture FSM handles hand-pan -> 2nd finger pinch -> lift and reset mid-drag cleanly', () => {
  let s = createTouchGestureState();
  // Hand mode pan
  s = gesturePointerDown(s, 1, 100, 100, { mode: 'touch', hand: true, now: 1000, isPrimary: true }).state;
  assert.equal(s.phase, 'pan');
  const pMove = gesturePointerMove(s, 1, 140, 130, { mode: 'touch', now: 1020 });
  s = pMove.state;
  assert.deepEqual(pMove.effects, [{ type: 'pan', dx: 40, dy: 30 }]);

  // 2nd finger lands -> seamlessly switches to pinch without pan jump
  s = gesturePointerDown(s, 2, 240, 130, { mode: 'touch', hand: true, now: 1040 }).state;
  assert.equal(s.phase, 'pinch');
  assert.equal(s.pan, null);
  const zMove = gesturePointerMove(s, 2, 300, 130, { mode: 'touch', now: 1060 });
  s = zMove.state;
  assert.equal(zMove.effects[0]?.type, 'pinch');

  // Lift both fingers -> back to idle
  s = gesturePointerUpOrCancel(s, 2, 300, 130, 'up', { now: 1090 }).state;
  s = gesturePointerUpOrCancel(s, 1, 140, 130, 'up', { now: 1100 }).state;
  assert.equal(s.phase, 'idle');

  // Mid-drag reset (e.g. menu opens or tab blurs) emits release-stuck-drag & returns to idle
  s = gesturePointerDown(s, 5, 50, 50, { mode: 'touch', hand: false, now: 2000, isPrimary: true }).state;
  const dMove = gesturePointerMove(s, 5, 120, 90, { mode: 'touch', now: 2040 });
  s = dMove.state;
  assert.equal(s.single?.dragging, true);
  const reset = resetTouchGestureState(s);
  assert.equal(reset.state.phase, 'idle');
  assert.equal(reset.state.pointers.size, 0);
  assert.ok(reset.effects.some((e) => e.type === 'release-stuck-drag' && e.x === 120 && e.y === 90));

  // Long press timeout only fires for active stationary single finger
  s = gesturePointerDown(reset.state, 6, 80, 80, { mode: 'touch', hand: false, now: 3000, isPrimary: true }).state;
  const lp = gestureLongPressTimeout(s, 6);
  assert.deepEqual(lp.effects, [{ type: 'long-press', x: 80, y: 80 }]);
  // Subsequent pointerup after long-press must NOT fire a left-click
  const lpUp = gesturePointerUpOrCancel(lp.state, 6, 80, 80, 'up', { now: 3600 });
  assert.equal(lpUp.effects.some((e) => e.type === 'tap-click'), false);
});
