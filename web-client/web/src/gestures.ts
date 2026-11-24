/** Phone gesture thresholds, aligned with the RustDesk mobile client. */
export const TAP_SLOP_PX = 10;
export const LONG_PRESS_MS = 500;
export const DOUBLE_TAP_MS = 280;

export type PointerMode = "touch" | "mouse";

/** How much of the layout viewport a soft keyboard covers. */
export function keyboardInset(innerHeight: number, viewportHeight: number, offsetTop: number): number {
  const inset = innerHeight - viewportHeight - offsetTop;
  if (!Number.isFinite(inset) || inset <= 0) return 0;
  return inset;
}

/** A second tap in mouse mode, close to the previous one, locks a drag. */
export function fingerDown(mode: PointerMode, msSinceLastTap: number | null): "drag-lock" | "pending" {
  if (mode === "mouse" && msSinceLastTap !== null && msSinceLastTap >= 0 && msSinceLastTap < DOUBLE_TAP_MS) {
    return "drag-lock";
  }
  return "pending";
}

/**
 * What lifting one finger should do.
 * A still finger is a left click. A long-press already sent the right click.
 * Mouse mode reserves a quick second tap for drag lock; the drag itself starts on the way down.
 */
export function fingerUp(input: {
  moved: number;
  longFired: boolean;
  dragging: boolean;
}): "left-click" | "end-drag" | "none" {
  if (input.longFired) return "none";
  if (input.dragging) return "end-drag";
  if (input.moved > TAP_SLOP_PX) return "none";
  return "left-click";
}

/** Two fingers that barely moved are a right click, not a pinch or a pan. */
export function isTwoFingerTap(midShift: number, distDelta: number): boolean {
  return midShift < 12 && Math.abs(distDelta) < 12;
}
