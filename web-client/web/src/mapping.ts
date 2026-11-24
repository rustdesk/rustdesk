/** Screen pixels per remote pixel, and where the remote origin sits in the viewport. */
export interface ViewTransform {
  scale: number;
  panX: number;
  panY: number;
}

export interface VideoRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Largest scale that fits the remote picture inside the viewport. */
export function fitScale(boxW: number, boxH: number, remoteW: number, remoteH: number): number {
  if (remoteW <= 0 || remoteH <= 0 || boxW <= 0 || boxH <= 0) return 1;
  return Math.min(boxW / remoteW, boxH / remoteH);
}

/** Keep the picture centered when it is smaller than the viewport, and inside the edges when it is larger. */
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
  const x = imgW <= boxW ? (boxW - imgW) / 2 : Math.min(0, Math.max(boxW - imgW, panX));
  const y = imgH <= boxH ? (boxH - imgH) / 2 : Math.min(0, Math.max(boxH - imgH, panY));
  return { panX: x, panY: y };
}

export function centeredPan(
  boxW: number,
  boxH: number,
  remoteW: number,
  remoteH: number,
  scale: number,
): ViewTransform {
  const pan = clampPan(boxW, boxH, remoteW, remoteH, scale, 0, 0);
  return { scale, ...pan };
}

/** Keep the remote point under `anchor` fixed while the scale changes. */
export function zoomAround(
  anchorX: number,
  anchorY: number,
  oldScale: number,
  newScale: number,
  panX: number,
  panY: number,
): { panX: number; panY: number } {
  const safeOld = oldScale === 0 ? 1 : oldScale;
  const remoteX = (anchorX - panX) / safeOld;
  const remoteY = (anchorY - panY) / safeOld;
  return {
    panX: anchorX - remoteX * newScale,
    panY: anchorY - remoteY * newScale,
  };
}

export function videoRect(transform: ViewTransform, remoteW: number, remoteH: number): VideoRect {
  return {
    x: transform.panX,
    y: transform.panY,
    width: remoteW * transform.scale,
    height: remoteH * transform.scale,
  };
}

export function mapToRemote(
  localX: number,
  localY: number,
  remoteW: number,
  remoteH: number,
  transform: ViewTransform,
): { x: number; y: number } | null {
  if (remoteW <= 0 || remoteH <= 0 || transform.scale <= 0) return null;
  const x = (localX - transform.panX) / transform.scale;
  const y = (localY - transform.panY) / transform.scale;
  if (x < 0 || y < 0 || x > remoteW || y > remoteH) return null;
  return {
    x: Math.min(remoteW - 1, Math.max(0, Math.floor(x))),
    y: Math.min(remoteH - 1, Math.max(0, Math.floor(y))),
  };
}

export function mapFromRemote(
  remoteX: number,
  remoteY: number,
  transform: ViewTransform,
): { x: number; y: number } {
  return {
    x: transform.panX + (remoteX + 0.5) * transform.scale,
    y: transform.panY + (remoteY + 0.5) * transform.scale,
  };
}
