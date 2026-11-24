export type ViewMode = 'fit' | 'original' | 'custom';

export interface ViewTransform {
  scale: number;
  panX: number;
  panY: number;
}

export interface Point {
  x: number;
  y: number;
}

export function fitScale(
  boxW: number,
  boxH: number,
  remoteW: number,
  remoteH: number,
): number {
  if (boxW <= 0 || boxH <= 0 || remoteW <= 0 || remoteH <= 0) return 1;
  return Math.min(boxW / remoteW, boxH / remoteH);
}

export function clampPan(
  boxW: number,
  boxH: number,
  remoteW: number,
  remoteH: number,
  scale: number,
  panX: number,
  panY: number,
): { panX: number; panY: number } {
  const imgW = remoteW * scale;
  const imgH = remoteH * scale;

  const nextX =
    imgW <= boxW
      ? (boxW - imgW) / 2
      : Math.min(0, Math.max(boxW - imgW, panX));
  const nextY =
    imgH <= boxH
      ? (boxH - imgH) / 2
      : Math.min(0, Math.max(boxH - imgH, panY));

  return { panX: nextX, panY: nextY };
}

export function zoomAround(
  anchorX: number,
  anchorY: number,
  oldScale: number,
  newScale: number,
  panX: number,
  panY: number,
): { panX: number; panY: number } {
  const s0 = oldScale === 0 ? 1 : oldScale;
  const rx = (anchorX - panX) / s0;
  const ry = (anchorY - panY) / s0;
  return {
    panX: anchorX - rx * newScale,
    panY: anchorY - ry * newScale,
  };
}

export function mapToRemote(
  localX: number,
  localY: number,
  remoteW: number,
  remoteH: number,
  transform: ViewTransform,
): Point | null {
  if (remoteW <= 0 || remoteH <= 0 || transform.scale <= 0) return null;
  const x = (localX - transform.panX) / transform.scale;
  const y = (localY - transform.panY) / transform.scale;
  if (x < 0 || y < 0 || x > remoteW || y > remoteH) return null;
  return {
    x: Math.min(remoteW - 1, Math.max(0, Math.floor(x))),
    y: Math.min(remoteH - 1, Math.max(0, Math.floor(y))),
  };
}

export function mapToRemoteClamped(
  localX: number,
  localY: number,
  remoteW: number,
  remoteH: number,
  transform: ViewTransform,
): Point | null {
  if (remoteW <= 0 || remoteH <= 0 || transform.scale <= 0) return null;
  const x = (localX - transform.panX) / transform.scale;
  const y = (localY - transform.panY) / transform.scale;
  return {
    x: Math.min(remoteW - 1, Math.max(0, Math.floor(x))),
    y: Math.min(remoteH - 1, Math.max(0, Math.floor(y))),
  };
}

export function mapFromRemote(
  remoteX: number,
  remoteY: number,
  transform: ViewTransform,
): Point {
  return {
    x: transform.panX + (remoteX + 0.5) * transform.scale,
    y: transform.panY + (remoteY + 0.5) * transform.scale,
  };
}

/**
 * Computes the effective ViewTransform consistently across fit, original (1:1), and custom modes.
 * Fixes Appendix B #5 by allowing consistent scale bounds between setScale and currentView.
 */
export function computeViewTransform(
  boxW: number,
  boxH: number,
  remoteW: number,
  remoteH: number,
  viewMode: ViewMode,
  zoom: number,
  pan: { panX: number; panY: number },
): ViewTransform {
  const fit = fitScale(boxW, boxH, remoteW, remoteH);
  const minScale = Math.min(fit, 1);
  const maxScale = Math.max(fit * 4, 1);

  const scale =
    viewMode === 'original'
      ? 1
      : viewMode === 'custom'
        ? Math.min(maxScale, Math.max(minScale, fit * zoom))
        : fit;

  const basePan =
    viewMode === 'fit'
      ? { panX: (boxW - remoteW * scale) / 2, panY: (boxH - remoteH * scale) / 2 }
      : pan;

  return {
    scale,
    ...clampPan(boxW, boxH, remoteW, remoteH, scale, basePan.panX, basePan.panY),
  };
}

export function computeNextScaleState(
  boxW: number,
  boxH: number,
  remoteW: number,
  remoteH: number,
  current: ViewTransform,
  nextScale: number,
  anchorX: number,
  anchorY: number,
): { viewMode: ViewMode; zoom: number; pan: { panX: number; panY: number }; scale: number } {
  const fit = fitScale(boxW, boxH, remoteW, remoteH);
  const minScale = Math.min(fit, 1);
  const maxScale = Math.max(fit * 4, 1);
  const clampedScale = Math.min(maxScale, Math.max(minScale, nextScale));
  const shifted = zoomAround(
    anchorX,
    anchorY,
    current.scale,
    clampedScale,
    current.panX,
    current.panY,
  );
  const viewMode: ViewMode =
    Math.abs(clampedScale - fit) < 0.01
      ? 'fit'
      : Math.abs(clampedScale - 1) < 0.01
        ? 'original'
        : 'custom';
  const zoom = fit === 0 ? 1 : clampedScale / fit;
  const pan = clampPan(boxW, boxH, remoteW, remoteH, clampedScale, shifted.panX, shifted.panY);
  return { viewMode, zoom, pan, scale: clampedScale };
}

/**
 * Computes a single smooth pinch-zoom + two-finger pan step without destructive deadzones,
 * keeping the remote point under the finger midpoint anchored across 60/120Hz touch frames.
 */
export function computePinchStep(
  boxW: number,
  boxH: number,
  remoteW: number,
  remoteH: number,
  current: ViewTransform,
  prevMidX: number,
  prevMidY: number,
  prevDist: number,
  nextMidX: number,
  nextMidY: number,
  nextDist: number,
): { viewMode: ViewMode; zoom: number; pan: { panX: number; panY: number }; scale: number } {
  const factor = prevDist > 4 && nextDist > 4 ? nextDist / prevDist : 1;
  const dx = nextMidX - prevMidX;
  const dy = nextMidY - prevMidY;
  const fit = fitScale(boxW, boxH, remoteW, remoteH);
  const minScale = Math.min(fit, 1);
  const maxScale = Math.max(fit * 4, 1);
  const nextScale = Math.min(maxScale, Math.max(minScale, current.scale * factor));
  const shifted = zoomAround(
    prevMidX,
    prevMidY,
    current.scale,
    nextScale,
    current.panX,
    current.panY,
  );
  const pan = clampPan(
    boxW,
    boxH,
    remoteW,
    remoteH,
    nextScale,
    shifted.panX + dx,
    shifted.panY + dy,
  );
  const viewMode: ViewMode =
    Math.abs(nextScale - fit) < 0.01
      ? 'fit'
      : Math.abs(nextScale - 1) < 0.01
        ? 'original'
        : 'custom';
  const zoom = fit === 0 ? 1 : nextScale / fit;
  return { viewMode, zoom, pan, scale: nextScale };
}
