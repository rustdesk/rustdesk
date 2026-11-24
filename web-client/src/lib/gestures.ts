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
