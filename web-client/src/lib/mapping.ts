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

export interface PanClampOptions {
  /**
   * When true, allows dragging a smaller-than-viewport canvas freely from the very top/left edge
   * to the very bottom/right edge instead of locking it rigidly in the center.
   */
  allowEdgePan?: boolean;
  /**
   * Extra top/bottom/side clearance (in CSS px) when zoomed larger than the viewport so remote
   * top menu bars, close buttons, and bottom taskbars can be dragged clear of floating controls.
   */
  edgePadding?: number;
  /**
   * Extra bottom viewport inset (in CSS px, e.g., when SoftKeyboardBar is visible) so the
   * bottom of the remote desktop can be dragged above the on-screen keyboard bar.
   */
  bottomInset?: number;
}

export const DEFAULT_EDGE_PADDING = 48;
export const MAX_ZOOM_MULTIPLIER = 8;
export const MIN_MAX_NATIVE_SCALE = 4;

export function fitScale(
  boxW: number,
  boxH: number,
  remoteW: number,
  remoteH: number,
): number {
  if (boxW <= 0 || boxH <= 0 || remoteW <= 0 || remoteH <= 0) return 1;
  return Math.min(boxW / remoteW, boxH / remoteH);
}

/**
 * Returns consistent [minScale, maxScale] bounds across fit, original, zoom buttons, and pinch.
 * Guarantees at least 400% native pixel zoom (4.0) or 8x fit scale so small remote buttons are
 * always easy to tap/click even on phones or HiDPI remote monitors.
 */
export function computeScaleBounds(fit: number): { minScale: number; maxScale: number } {
  return {
    minScale: Math.min(fit, 1),
    maxScale: Math.max(fit * MAX_ZOOM_MULTIPLIER, MIN_MAX_NATIVE_SCALE),
  };
}

export function clampPan(
  boxW: number,
  boxH: number,
  remoteW: number,
  remoteH: number,
  scale: number,
  panX: number,
  panY: number,
  options?: PanClampOptions,
): { panX: number; panY: number } {
  const imgW = remoteW * scale;
  const imgH = remoteH * scale;
  const allowEdgePan = Boolean(options?.allowEdgePan);
  const edgePad = Math.max(0, options?.edgePadding ?? 0);
  const padX = Math.min(edgePad, 32);
  const bottomInset = Math.max(0, options?.bottomInset ?? 0);

  let nextX: number;
  if (imgW <= boxW) {
    if (allowEdgePan) {
      nextX = Math.min(boxW - imgW, Math.max(0, panX));
    } else {
      nextX = (boxW - imgW) / 2;
    }
  } else {
    nextX = Math.min(padX, Math.max(boxW - imgW - padX, panX));
  }

  let nextY: number;
  if (imgH <= boxH) {
    if (allowEdgePan) {
      const usableH = Math.max(1, boxH - bottomInset);
      const minY = Math.min(0, usableH - imgH);
      const maxY = Math.max(0, boxH - imgH);
      nextY = Math.min(maxY, Math.max(minY, panY));
    } else {
      nextY = (boxH - imgH) / 2;
    }
  } else {
    nextY = Math.min(edgePad, Math.max(boxH - imgH - edgePad - bottomInset, panY));
  }

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
  panOptions?: PanClampOptions,
): ViewTransform {
  const fit = fitScale(boxW, boxH, remoteW, remoteH);
  const { minScale, maxScale } = computeScaleBounds(fit);

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

  const effectiveOptions: PanClampOptions | undefined =
    viewMode === 'fit'
      ? undefined
      : {
          allowEdgePan: true,
          edgePadding: panOptions?.edgePadding ?? 0,
          bottomInset: panOptions?.bottomInset ?? 0,
        };

  return {
    scale,
    ...clampPan(boxW, boxH, remoteW, remoteH, scale, basePan.panX, basePan.panY, effectiveOptions),
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
  panOptions?: PanClampOptions,
): { viewMode: ViewMode; zoom: number; pan: { panX: number; panY: number }; scale: number } {
  const fit = fitScale(boxW, boxH, remoteW, remoteH);
  const { minScale, maxScale } = computeScaleBounds(fit);
  const clampedScale = Math.min(maxScale, Math.max(minScale, nextScale));
  const shifted = zoomAround(
    anchorX,
    anchorY,
    current.scale,
    clampedScale,
    current.panX,
    current.panY,
  );
  const isAtFitScale = Math.abs(clampedScale - fit) < 0.01;
  const pan = clampPan(
    boxW,
    boxH,
    remoteW,
    remoteH,
    clampedScale,
    shifted.panX,
    shifted.panY,
    isAtFitScale
      ? undefined
      : {
          allowEdgePan: true,
          edgePadding: panOptions?.edgePadding ?? 0,
          bottomInset: panOptions?.bottomInset ?? 0,
        },
  );
  const viewMode: ViewMode = isAtFitScale
    ? 'fit'
    : Math.abs(clampedScale - 1) < 0.01
      ? 'original'
      : 'custom';
  const zoom = fit === 0 ? 1 : clampedScale / fit;
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
  panOptions?: PanClampOptions,
): { viewMode: ViewMode; zoom: number; pan: { panX: number; panY: number }; scale: number } {
  const factor = prevDist > 4 && nextDist > 4 ? nextDist / prevDist : 1;
  const dx = nextMidX - prevMidX;
  const dy = nextMidY - prevMidY;
  const fit = fitScale(boxW, boxH, remoteW, remoteH);
  const { minScale, maxScale } = computeScaleBounds(fit);
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
    {
      allowEdgePan: true,
      edgePadding: panOptions?.edgePadding ?? 0,
      bottomInset: panOptions?.bottomInset ?? 0,
    },
  );
  const centeredX = (boxW - remoteW * nextScale) / 2;
  const centeredY = (boxH - remoteH * nextScale) / 2;
  const isCentered = Math.abs(pan.panX - centeredX) < 1 && Math.abs(pan.panY - centeredY) < 1;
  const viewMode: ViewMode =
    Math.abs(nextScale - fit) < 0.01 && isCentered
      ? 'fit'
      : Math.abs(nextScale - 1) < 0.01
        ? 'original'
        : 'custom';
  const zoom = fit === 0 ? 1 : nextScale / fit;
  return { viewMode, zoom, pan, scale: nextScale };
}
