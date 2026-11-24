export type OrbSide = 'left' | 'right';

export interface OrbState {
  side: OrbSide;
  /** Vertical position as ratio of viewport height [0.1, 0.86] */
  yRatio: number;
}

export const DEFAULT_ORB_STATE: OrbState = {
  side: 'right',
  yRatio: 0.72,
};

/**
 * Clamps the desktop top-edge toolbar/handle horizontal center ratio
 * so the collapsed tab never gets pushed into screen corners.
 */
export function clampToolbarX(ratio: number): number {
  if (!Number.isFinite(ratio)) return 0.5;
  return Math.min(0.88, Math.max(0.12, ratio));
}

/**
 * Computes a safe left-center pixel coordinate for an expanded floating toolbar
 * of width `barWidth` inside a viewport of width `viewportWidth`.
 */
export function clampToolbarPixelX(
  centerRatio: number,
  barWidth: number,
  viewportWidth: number,
  margin = 12,
): number {
  if (viewportWidth <= 0) return 0;
  const half = barWidth / 2;
  const minCenter = Math.min(viewportWidth / 2, half + margin);
  const maxCenter = Math.max(viewportWidth / 2, viewportWidth - half - margin);
  const desired = clampToolbarX(centerRatio) * viewportWidth;
  return Math.min(maxCenter, Math.max(minCenter, desired));
}

/**
 * Clamps a mobile floating orb's top-left pixel coordinates strictly inside the viewport.
 */
export function clampOrbPosition(
  x: number,
  y: number,
  orbSize: number,
  viewportW: number,
  viewportH: number,
  margin = 10,
): { x: number; y: number } {
  const maxX = Math.max(margin, viewportW - orbSize - margin);
  const maxY = Math.max(margin, viewportH - orbSize - margin);
  return {
    x: Math.min(maxX, Math.max(margin, x)),
    y: Math.min(maxY, Math.max(margin, y)),
  };
}

/**
 * Snaps a dragged mobile orb cleanly to the nearest left or right screen edge
 * and stores its vertical position as a viewport ratio so rotation preserves placement.
 */
export function snapOrbToEdge(
  x: number,
  y: number,
  orbSize: number,
  viewportW: number,
  viewportH: number,
  margin = 10,
): OrbState {
  const clamped = clampOrbPosition(x, y, orbSize, viewportW, viewportH, margin);
  const centerX = clamped.x + orbSize / 2;
  const side: OrbSide = centerX < viewportW / 2 ? 'left' : 'right';
  const safeH = Math.max(1, viewportH);
  const rawRatio = (clamped.y + orbSize / 2) / safeH;
  const yRatio = Math.min(0.86, Math.max(0.1, rawRatio));
  return { side, yRatio };
}

/**
 * Converts a persisted edge-snapped OrbState back into exact top-left pixel coordinates.
 */
export function orbCoordsFromState(
  state: OrbState,
  orbSize: number,
  viewportW: number,
  viewportH: number,
  margin = 10,
): { x: number; y: number } {
  const x = state.side === 'left' ? margin : Math.max(margin, viewportW - orbSize - margin);
  const rawY = state.yRatio * viewportH - orbSize / 2;
  return clampOrbPosition(x, rawY, orbSize, viewportW, viewportH, margin);
}
