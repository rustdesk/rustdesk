export const TAP_SLOP_PX = 10;
export const LONG_PRESS_MS = 500;
export const DOUBLE_TAP_MS = 280;
export const THREE_FINGER_SCROLL_STEP_PX = 24;

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
  return midShift < 12 && Math.abs(distDelta) < 12;
}

/**
 * Appendix B fix #1: cancel pending long-press in BOTH touch and mouse modes
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
 * M11 enhancement: converts accumulated vertical three-finger delta into discrete wheel ticks
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
