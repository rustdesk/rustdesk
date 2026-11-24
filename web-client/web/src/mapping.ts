export interface VideoRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Letterboxed rectangle of the remote picture inside a CSS box (`object-fit: contain`). */
export function videoRect(boxW: number, boxH: number, remoteW: number, remoteH: number): VideoRect {
  if (remoteW <= 0 || remoteH <= 0 || boxW <= 0 || boxH <= 0) {
    return { x: 0, y: 0, width: Math.max(0, boxW), height: Math.max(0, boxH) };
  }
  const boxAspect = boxW / boxH;
  const remoteAspect = remoteW / remoteH;
  if (boxAspect > remoteAspect) {
    const height = boxH;
    const width = height * remoteAspect;
    return { x: (boxW - width) / 2, y: 0, width, height };
  }
  const width = boxW;
  const height = width / remoteAspect;
  return { x: 0, y: (boxH - height) / 2, width, height };
}

export function mapToRemote(
  localX: number,
  localY: number,
  boxW: number,
  boxH: number,
  remoteW: number,
  remoteH: number,
): { x: number; y: number } | null {
  if (remoteW <= 0 || remoteH <= 0) return null;
  const rect = videoRect(boxW, boxH, remoteW, remoteH);
  const lx = localX - rect.x;
  const ly = localY - rect.y;
  if (lx < 0 || ly < 0 || lx > rect.width || ly > rect.height) return null;
  const x = Math.min(remoteW - 1, Math.max(0, Math.round((lx / rect.width) * remoteW)));
  const y = Math.min(remoteH - 1, Math.max(0, Math.round((ly / rect.height) * remoteH)));
  return { x, y };
}

export function mapFromRemote(
  remoteX: number,
  remoteY: number,
  boxW: number,
  boxH: number,
  remoteW: number,
  remoteH: number,
): { x: number; y: number } | null {
  if (remoteW <= 0 || remoteH <= 0) return null;
  const rect = videoRect(boxW, boxH, remoteW, remoteH);
  return {
    x: rect.x + ((remoteX + 0.5) / remoteW) * rect.width,
    y: rect.y + ((remoteY + 0.5) / remoteH) * rect.height,
  };
}
