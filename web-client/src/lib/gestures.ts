export const TAP_SLOP_PX = 10;
export const LONG_PRESS_MS = 500;
export const DOUBLE_TAP_MS = 280;
export const THREE_FINGER_SCROLL_STEP_PX = 24;
export const TWO_FINGER_TAP_MAX_MS = 350;
export const TWO_FINGER_TAP_MAX_MOVE_PX = 12;
export const STALE_POINTER_MS = 650;

export type InputPointerMode = 'touch' | 'mouse';

export function keyboardInset(
  innerHeight: number,
  viewportHeight: number,
  offsetTop: number,
): number {
  const inset = innerHeight - viewportHeight - offsetTop;
  if (!Number.isFinite(inset) || inset <= 0) return 0;
  return inset;
}

export function fingerDown(
  mode: InputPointerMode,
  msSinceLastTap: number | null,
): 'drag-lock' | 'pending' {
  if (
    mode === 'mouse' &&
    msSinceLastTap !== null &&
    msSinceLastTap >= 0 &&
    msSinceLastTap < DOUBLE_TAP_MS
  ) {
    return 'drag-lock';
  }
  return 'pending';
}

export function fingerUp(state: {
  moved: number;
  longFired: boolean;
  dragging: boolean;
}): 'none' | 'end-drag' | 'left-click' {
  if (state.longFired) return 'none';
  if (state.dragging) return 'end-drag';
  if (state.moved > TAP_SLOP_PX) return 'none';
  return 'left-click';
}

export function isTwoFingerTap(midShift: number, distDelta: number): boolean {
  return midShift < TWO_FINGER_TAP_MAX_MOVE_PX && Math.abs(distDelta) < TWO_FINGER_TAP_MAX_MOVE_PX;
}

/**
 * Cancel pending long-press in BOTH touch and mouse modes
 * once finger movement exceeds TAP_SLOP_PX so trackpad dragging never triggers a false right-click.
 */
export function shouldCancelLongPress(movedPx: number): boolean {
  return movedPx > TAP_SLOP_PX;
}

export const PAGE_ZOOM_THRESHOLD = 1.02;

/**
 * Fallback detector for iOS Safari / mobile browsers that ignore user-scalable=no.
 */
export function isPageZoomed(scale: number): boolean {
  return Number.isFinite(scale) && scale > PAGE_ZOOM_THRESHOLD;
}

/**
 * Converts accumulated vertical three-finger delta into discrete wheel ticks
 * and returns the remaining unconsumed pixel delta.
 */
export function consumeThreeFingerScroll(accumulatedDy: number): {
  ticks: number;
  remainder: number;
} {
  if (Math.abs(accumulatedDy) < THREE_FINGER_SCROLL_STEP_PX) {
    return { ticks: 0, remainder: accumulatedDy };
  }
  const steps = Math.trunc(accumulatedDy / THREE_FINGER_SCROLL_STEP_PX);
  return {
    ticks: steps,
    remainder: accumulatedDy - steps * THREE_FINGER_SCROLL_STEP_PX,
  };
}

export const WHEEL_PIXELS_PER_TICK = 36;
export const MAX_WHEEL_TICKS_PER_EVENT = 6;

/**
 * Converts browser WheelEvent deltas (DOM_DELTA_PIXEL / LINE / PAGE) into RustDesk wheel ticks.
 * A standard desktop mouse wheel detent (deltaY ≈ 100..120px) emits ±3 ticks for responsive scrolling,
 * while smooth trackpad gestures accumulate sub-tick deltas cleanly without being dropped.
 */
export function accumulateWheelTicks(
  deltaX: number,
  deltaY: number,
  deltaMode: number,
  accumX: number,
  accumY: number,
  reverseVertical: boolean,
): { tx: number; ty: number; nextAccumX: number; nextAccumY: number } {
  const clampTicks = (n: number) =>
    Math.max(-MAX_WHEEL_TICKS_PER_EVENT, Math.min(MAX_WHEEL_TICKS_PER_EVENT, n));

  let rawTx = 0;
  let rawTy = 0;
  let nextAccumX = accumX;
  let nextAccumY = accumY;

  if (deltaMode === 1) {
    // DOM_DELTA_LINE (e.g. Firefox mouse wheel typically sends ±3 lines)
    rawTx = deltaX !== 0 ? Math.trunc(deltaX) || Math.sign(deltaX) : 0;
    rawTy = deltaY !== 0 ? Math.trunc(deltaY) || Math.sign(deltaY) : 0;
    nextAccumX = 0;
    nextAccumY = 0;
  } else if (deltaMode === 2) {
    // DOM_DELTA_PAGE
    rawTx = Math.sign(deltaX) * 6;
    rawTy = Math.sign(deltaY) * 6;
    nextAccumX = 0;
    nextAccumY = 0;
  } else {
    // DOM_DELTA_PIXEL: reset accumulator if scroll direction reverses
    if (deltaX !== 0 && nextAccumX !== 0 && Math.sign(deltaX) !== Math.sign(nextAccumX)) {
      nextAccumX = 0;
    }
    if (deltaY !== 0 && nextAccumY !== 0 && Math.sign(deltaY) !== Math.sign(nextAccumY)) {
      nextAccumY = 0;
    }
    nextAccumX += deltaX;
    nextAccumY += deltaY;

    if (Math.abs(nextAccumX) >= WHEEL_PIXELS_PER_TICK) {
      rawTx = Math.trunc(nextAccumX / WHEEL_PIXELS_PER_TICK);
      nextAccumX -= rawTx * WHEEL_PIXELS_PER_TICK;
    } else if (Math.abs(deltaX) >= 8 && Math.abs(nextAccumX) >= 16) {
      rawTx = Math.sign(nextAccumX);
      nextAccumX = 0;
    }

    if (Math.abs(nextAccumY) >= WHEEL_PIXELS_PER_TICK) {
      rawTy = Math.trunc(nextAccumY / WHEEL_PIXELS_PER_TICK);
      nextAccumY -= rawTy * WHEEL_PIXELS_PER_TICK;
    } else if (Math.abs(deltaY) >= 8 && Math.abs(nextAccumY) >= 16) {
      rawTy = Math.sign(nextAccumY);
      nextAccumY = 0;
    }
  }

  // RustDesk positive Y scrolls up, negative Y scrolls down (opposite of DOM deltaY)
  const rawClampedX = clampTicks(rawTx);
  const verticalDir = -rawTy;
  const rawClampedY = clampTicks(reverseVertical ? -verticalDir : verticalDir);
  const tx = rawClampedX === 0 ? 0 : rawClampedX;
  const ty = rawClampedY === 0 ? 0 : rawClampedY;
  return { tx, ty, nextAccumX, nextAccumY };
}

/* -------------------------------------------------------------------------- */
/*  Deterministic Touch Gesture State Machine                                 */
/* -------------------------------------------------------------------------- */

export type TouchGesturePhase =
  | 'idle'
  | 'single'
  | 'pan'
  | 'pinch'
  | 'three-finger'
  | 'cooldown';

export interface TrackedPointer {
  id: number;
  x: number;
  y: number;
  updatedAt: number;
}

export interface PinchGestureData {
  ids: [number, number];
  startedAt: number;
  originX: number;
  originY: number;
  originDist: number;
  prevX: number;
  prevY: number;
  prevDist: number;
  maxMidShift: number;
  maxDistDelta: number;
}

export interface SingleTouchData {
  pointerId: number;
  startX: number;
  startY: number;
  lastX: number;
  lastY: number;
  startedAt: number;
  dragging: boolean;
  longFired: boolean;
  maxMoved: number;
}

export interface PanTouchData {
  pointerId: number;
  lastX: number;
  lastY: number;
}

export interface ThreeFingerData {
  lastAvgY: number;
  accumY: number;
}

export interface TouchGestureState {
  phase: TouchGesturePhase;
  pointers: Map<number, TrackedPointer>;
  single: SingleTouchData | null;
  pan: PanTouchData | null;
  pinch: PinchGestureData | null;
  threeFinger: ThreeFingerData | null;
  lastTapUpAt: number | null;
}

export type TouchGestureEffect =
  | { type: 'arm-long-press'; pointerId: number; x: number; y: number }
  | { type: 'cancel-long-press' }
  | { type: 'start-drag-lock' }
  | { type: 'start-touch-drag'; x: number; y: number }
  | { type: 'move-touch-drag'; x: number; y: number }
  | { type: 'end-drag'; x: number; y: number }
  | { type: 'release-stuck-drag'; x: number; y: number }
  | { type: 'cursor-move'; dx: number; dy: number }
  | { type: 'tap-click'; x: number; y: number }
  | { type: 'two-finger-tap'; x: number; y: number }
  | { type: 'long-press'; x: number; y: number }
  | { type: 'pan'; dx: number; dy: number }
  | {
      type: 'pinch';
      prevMidX: number;
      prevMidY: number;
      prevDist: number;
      nextMidX: number;
      nextMidY: number;
      nextDist: number;
    }
  | { type: 'three-finger-scroll'; ticks: number };

export interface TouchGestureTransition {
  state: TouchGestureState;
  effects: TouchGestureEffect[];
}

export interface TouchGestureDownOptions {
  mode: InputPointerMode;
  hand: boolean;
  now: number;
  isPrimary?: boolean;
  /** Physical touch count reported by native TouchEvent if recently observed, else null. */
  liveTouchCount?: number | null;
}

export interface TouchGestureMoveOptions {
  mode: InputPointerMode;
  now: number;
}

export interface TouchGestureUpOptions {
  now: number;
}

export function createTouchGestureState(lastTapUpAt: number | null = null): TouchGestureState {
  return {
    phase: 'idle',
    pointers: new Map(),
    single: null,
    pan: null,
    pinch: null,
    threeFinger: null,
    lastTapUpAt,
  };
}

function makePinchSnapshot(p0: TrackedPointer, p1: TrackedPointer, now: number): PinchGestureData {
  const midX = (p0.x + p1.x) / 2;
  const midY = (p0.y + p1.y) / 2;
  const dist = Math.hypot(p1.x - p0.x, p1.y - p0.y);
  return {
    ids: [p0.id, p1.id],
    startedAt: now,
    originX: midX,
    originY: midY,
    originDist: dist,
    prevX: midX,
    prevY: midY,
    prevDist: dist,
    maxMidShift: 0,
    maxDistDelta: 0,
  };
}

function averageY(pointers: Map<number, TrackedPointer>): number {
  let sum = 0;
  for (const p of pointers.values()) {
    sum += p.y;
  }
  return pointers.size > 0 ? sum / pointers.size : 0;
}

/**
 * Unconditionally resets the touch gesture machine to 'idle' (used on menu open,
 * blur/visibilitychange, mode switch, or authoritative touchend/touchcancel with 0 touches).
 * Always releases any active remote drag and cancels pending long-press timers.
 */
export function resetTouchGestureState(
  state: TouchGestureState,
  preserveLastTap = false,
): TouchGestureTransition {
  const effects: TouchGestureEffect[] = [];
  if (state.phase !== 'idle' || state.pointers.size > 0 || state.single !== null) {
    effects.push({ type: 'cancel-long-press' });
  }
  if (state.single?.dragging) {
    effects.push({
      type: 'release-stuck-drag',
      x: state.single.lastX,
      y: state.single.lastY,
    });
  }
  return {
    state: createTouchGestureState(preserveLastTap ? state.lastTapUpAt : null),
    effects,
  };
}

export function gesturePointerDown(
  state: TouchGestureState,
  pointerId: number,
  x: number,
  y: number,
  opts: TouchGestureDownOptions,
): TouchGestureTransition {
  const effects: TouchGestureEffect[] = [];
  let pointers = new Map(state.pointers);
  let single = state.single;
  let lastTapUpAt = state.lastTapUpAt;

  // Orphan / zombie pointer recovery:
  // 1) If native TouchEvent authoritatively reports 1 physical finger on screen, any pre-existing
  //    tracked pointer IDs are provably orphaned from missed pointerup/pointercancel events.
  // 2) Otherwise, if this is a primary pointer and every existing pointer has been idle for > STALE_POINTER_MS,
  //    they are orphaned zombies and must be purged before classifying the new gesture.
  if (pointers.size > 0 && !pointers.has(pointerId)) {
    const authoritativeSingleFinger = opts.liveTouchCount === 1;
    let allStale = false;
    if (opts.isPrimary) {
      allStale = true;
      for (const p of pointers.values()) {
        if (opts.now - p.updatedAt <= STALE_POINTER_MS) {
          allStale = false;
          break;
        }
      }
    }

    if (authoritativeSingleFinger || allStale) {
      effects.push({ type: 'cancel-long-press' });
      if (single?.dragging) {
        effects.push({ type: 'release-stuck-drag', x: single.lastX, y: single.lastY });
      }
      pointers = new Map();
      single = null;
    }
  }

  pointers.set(pointerId, { id: pointerId, x, y, updatedAt: opts.now });

  // 3+ fingers -> three-finger vertical scroll
  if (pointers.size >= 3) {
    effects.push({ type: 'cancel-long-press' });
    if (single?.dragging) {
      effects.push({ type: 'release-stuck-drag', x: single.lastX, y: single.lastY });
    }
    return {
      state: {
        phase: 'three-finger',
        pointers,
        single: null,
        pan: null,
        pinch: null,
        threeFinger: { lastAvgY: averageY(pointers), accumY: 0 },
        lastTapUpAt,
      },
      effects,
    };
  }

  // 2 fingers -> pinch zoom + pan (or two-finger tap on quick lift)
  if (pointers.size === 2) {
    effects.push({ type: 'cancel-long-press' });
    if (single?.dragging) {
      effects.push({ type: 'release-stuck-drag', x: single.lastX, y: single.lastY });
    }
    const [p0, p1] = Array.from(pointers.values());
    return {
      state: {
        phase: 'pinch',
        pointers,
        single: null,
        pan: null,
        pinch: makePinchSnapshot(p0, p1, opts.now),
        threeFinger: null,
        lastTapUpAt,
      },
      effects,
    };
  }

  // 1 finger -> either viewport pan ('hand' mode) or single-finger cursor/tap/drag
  if (opts.hand) {
    return {
      state: {
        phase: 'pan',
        pointers,
        single: null,
        pan: { pointerId, lastX: x, lastY: y },
        pinch: null,
        threeFinger: null,
        lastTapUpAt,
      },
      effects,
    };
  }

  const sinceLast = lastTapUpAt !== null ? opts.now - lastTapUpAt : null;
  const start = fingerDown(opts.mode, sinceLast);
  const dragging = start === 'drag-lock';

  if (dragging) {
    effects.push({ type: 'cancel-long-press' });
    effects.push({ type: 'start-drag-lock' });
    lastTapUpAt = null;
  } else {
    effects.push({ type: 'arm-long-press', pointerId, x, y });
  }

  return {
    state: {
      phase: 'single',
      pointers,
      single: {
        pointerId,
        startX: x,
        startY: y,
        lastX: x,
        lastY: y,
        startedAt: opts.now,
        dragging,
        longFired: false,
        maxMoved: 0,
      },
      pan: null,
      pinch: null,
      threeFinger: null,
      lastTapUpAt,
    },
    effects,
  };
}

export function gestureLongPressTimeout(
  state: TouchGestureState,
  pointerId: number,
): TouchGestureTransition {
  if (
    state.phase !== 'single' ||
    state.pointers.size !== 1 ||
    !state.single ||
    state.single.pointerId !== pointerId ||
    state.single.dragging ||
    state.single.longFired ||
    state.single.maxMoved > TAP_SLOP_PX
  ) {
    return { state, effects: [] };
  }

  const nextSingle: SingleTouchData = { ...state.single, longFired: true };
  return {
    state: {
      ...state,
      single: nextSingle,
    },
    effects: [{ type: 'long-press', x: state.single.startX, y: state.single.startY }],
  };
}

export function gesturePointerMove(
  state: TouchGestureState,
  pointerId: number,
  x: number,
  y: number,
  opts: TouchGestureMoveOptions,
): TouchGestureTransition {
  // Ignore untracked pointer IDs (e.g. events bubbling from overlays/chrome)
  if (!state.pointers.has(pointerId)) {
    return { state, effects: [] };
  }

  const pointers = new Map(state.pointers);
  pointers.set(pointerId, { id: pointerId, x, y, updatedAt: opts.now });

  if (state.phase === 'idle' || state.phase === 'cooldown') {
    return {
      state: { ...state, pointers },
      effects: [],
    };
  }

  if (state.phase === 'three-finger' && state.threeFinger) {
    const avgY = averageY(pointers);
    const dy = avgY - state.threeFinger.lastAvgY;
    const { ticks, remainder } = consumeThreeFingerScroll(state.threeFinger.accumY + dy);
    const effects: TouchGestureEffect[] =
      ticks !== 0 ? [{ type: 'three-finger-scroll', ticks }] : [];
    return {
      state: {
        ...state,
        pointers,
        threeFinger: { lastAvgY: avgY, accumY: remainder },
      },
      effects,
    };
  }

  if (state.phase === 'pinch' && state.pinch) {
    const p0 = pointers.get(state.pinch.ids[0]);
    const p1 = pointers.get(state.pinch.ids[1]);
    if (!p0 || !p1) {
      return { state: { ...state, pointers }, effects: [] };
    }
    const midX = (p0.x + p1.x) / 2;
    const midY = (p0.y + p1.y) / 2;
    const dist = Math.hypot(p1.x - p0.x, p1.y - p0.y);
    const midShift = Math.hypot(midX - state.pinch.originX, midY - state.pinch.originY);
    const distDelta = Math.abs(dist - state.pinch.originDist);

    const nextPinch: PinchGestureData = {
      ...state.pinch,
      prevX: midX,
      prevY: midY,
      prevDist: dist,
      maxMidShift: Math.max(state.pinch.maxMidShift, midShift),
      maxDistDelta: Math.max(state.pinch.maxDistDelta, distDelta),
    };

    return {
      state: {
        ...state,
        pointers,
        pinch: nextPinch,
      },
      effects: [
        {
          type: 'pinch',
          prevMidX: state.pinch.prevX,
          prevMidY: state.pinch.prevY,
          prevDist: state.pinch.prevDist,
          nextMidX: midX,
          nextMidY: midY,
          nextDist: dist,
        },
      ],
    };
  }

  if (state.phase === 'pan' && state.pan) {
    if (state.pan.pointerId !== pointerId) {
      return { state: { ...state, pointers }, effects: [] };
    }
    const dx = x - state.pan.lastX;
    const dy = y - state.pan.lastY;
    const effects: TouchGestureEffect[] = dx !== 0 || dy !== 0 ? [{ type: 'pan', dx, dy }] : [];
    return {
      state: {
        ...state,
        pointers,
        pan: { pointerId, lastX: x, lastY: y },
      },
      effects,
    };
  }

  if (state.phase === 'single' && state.single) {
    if (state.single.pointerId !== pointerId) {
      return { state: { ...state, pointers }, effects: [] };
    }
    const dx = x - state.single.lastX;
    const dy = y - state.single.lastY;
    const movedFromStart = Math.hypot(x - state.single.startX, y - state.single.startY);
    const maxMoved = Math.max(state.single.maxMoved, movedFromStart);

    if (state.single.longFired) {
      return {
        state: {
          ...state,
          pointers,
          single: { ...state.single, lastX: x, lastY: y, maxMoved },
        },
        effects: [],
      };
    }

    const effects: TouchGestureEffect[] = [];

    if (opts.mode === 'mouse') {
      if (shouldCancelLongPress(maxMoved)) {
        effects.push({ type: 'cancel-long-press' });
      }
      if (dx !== 0 || dy !== 0) {
        effects.push({ type: 'cursor-move', dx, dy });
      }
      return {
        state: {
          ...state,
          pointers,
          single: { ...state.single, lastX: x, lastY: y, maxMoved },
        },
        effects,
      };
    }

    // Touch mode: drag past TAP_SLOP_PX initiates remote left-drag from touchdown point
    let dragging = state.single.dragging;
    if (!dragging && maxMoved > TAP_SLOP_PX) {
      dragging = true;
      effects.push({ type: 'cancel-long-press' });
      effects.push({
        type: 'start-touch-drag',
        x: state.single.startX,
        y: state.single.startY,
      });
    }
    if (dragging) {
      effects.push({ type: 'move-touch-drag', x, y });
    }
    return {
      state: {
        ...state,
        pointers,
        single: { ...state.single, lastX: x, lastY: y, maxMoved, dragging },
      },
      effects,
    };
  }

  return { state: { ...state, pointers }, effects: [] };
}

export function gesturePointerUpOrCancel(
  state: TouchGestureState,
  pointerId: number,
  x: number,
  y: number,
  kind: 'up' | 'cancel',
  opts: TouchGestureUpOptions,
): TouchGestureTransition {
  // CRITICAL: Ignore pointerup/pointercancel for pointers that never started on the stage
  // (e.g., taps on MobileDock, SessionHandle, SoftKeyboardBar, or dialogs that bubble up).
  if (!state.pointers.has(pointerId)) {
    return { state, effects: [] };
  }

  const pointers = new Map(state.pointers);
  pointers.delete(pointerId);
  const effects: TouchGestureEffect[] = [];

  if (state.phase === 'pinch') {
    if (kind === 'up' && state.pinch) {
      const elapsed = opts.now - state.pinch.startedAt;
      if (
        elapsed <= TWO_FINGER_TAP_MAX_MS &&
        isTwoFingerTap(state.pinch.maxMidShift, state.pinch.maxDistDelta)
      ) {
        effects.push({
          type: 'two-finger-tap',
          x: state.pinch.originX,
          y: state.pinch.originY,
        });
      }
    }
    return {
      state: {
        phase: pointers.size === 0 ? 'idle' : 'cooldown',
        pointers,
        single: null,
        pan: null,
        pinch: null,
        threeFinger: null,
        lastTapUpAt: state.lastTapUpAt,
      },
      effects,
    };
  }

  if (state.phase === 'three-finger') {
    if (pointers.size >= 3) {
      return {
        state: {
          ...state,
          pointers,
          threeFinger: {
            lastAvgY: averageY(pointers),
            accumY: state.threeFinger?.accumY ?? 0,
          },
        },
        effects,
      };
    }
    return {
      state: {
        phase: pointers.size === 0 ? 'idle' : 'cooldown',
        pointers,
        single: null,
        pan: null,
        pinch: null,
        threeFinger: null,
        lastTapUpAt: state.lastTapUpAt,
      },
      effects,
    };
  }

  if (state.phase === 'pan') {
    return {
      state: {
        phase: pointers.size === 0 ? 'idle' : 'cooldown',
        pointers,
        single: null,
        pan: null,
        pinch: null,
        threeFinger: null,
        lastTapUpAt: state.lastTapUpAt,
      },
      effects,
    };
  }

  if (state.phase === 'single') {
    effects.push({ type: 'cancel-long-press' });
    let nextLastTapUpAt = state.lastTapUpAt;

    if (state.single && state.single.pointerId === pointerId) {
      const moved = Math.max(
        state.single.maxMoved,
        Math.hypot(x - state.single.startX, y - state.single.startY),
      );
      if (kind === 'cancel') {
        if (state.single.dragging) {
          effects.push({ type: 'release-stuck-drag', x, y });
        }
      } else {
        const action = fingerUp({
          moved,
          longFired: state.single.longFired,
          dragging: state.single.dragging,
        });
        if (action === 'end-drag') {
          effects.push({ type: 'end-drag', x, y });
        } else if (action === 'left-click') {
          nextLastTapUpAt = opts.now;
          effects.push({
            type: 'tap-click',
            x: state.single.startX,
            y: state.single.startY,
          });
        }
      }
    }

    return {
      state: {
        phase: pointers.size === 0 ? 'idle' : 'cooldown',
        pointers,
        single: null,
        pan: null,
        pinch: null,
        threeFinger: null,
        lastTapUpAt: nextLastTapUpAt,
      },
      effects,
    };
  }

  // 'cooldown' or 'idle': once all fingers lift, unconditionally return to 'idle'
  return {
    state: {
      phase: pointers.size === 0 ? 'idle' : 'cooldown',
      pointers,
      single: null,
      pan: null,
      pinch: null,
      threeFinger: null,
      lastTapUpAt: state.lastTapUpAt,
    },
    effects,
  };
}
