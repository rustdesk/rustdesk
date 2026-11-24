import type { ConnectionMode } from '../components/RustDeskShell';

export const DEVICES_STORAGE_KEY = 'rd_web_devices';
export const MAX_SAVED_DEVICES = 20;

export interface SavedDevice {
  key: string;
  mode: ConnectionMode;
  peerId?: string;
  idServer?: string;
  relayServer?: string;
  serverKey?: string;
  forceRelay?: boolean;
  host?: string;
  port?: string;
  hostname: string;
  platform: string;
  favorite: boolean;
  lastConnectedAt: number;
}

/**
 * Formats 9-digit RustDesk IDs into spaced groups ("187685134" -> "187 685 134"),
 * matching the official RustDesk mobile & desktop apps.
 */
export function formatPeerId(raw: string): string {
  const clean = raw.replace(/\s+/g, '');
  if (/^\d{7,12}$/.test(clean)) {
    return clean.replace(/(\d{3})(?=\d)/g, '$1 ');
  }
  return raw.trim();
}

export function deviceKeyFor(
  input: Pick<SavedDevice, 'mode' | 'peerId' | 'host' | 'port'>,
): string {
  if (input.mode === 'id') {
    const cleanId = (input.peerId ?? '').replace(/\s+/g, '');
    return `id:${cleanId}`;
  }
  const cleanHost = (input.host ?? '').trim().toLowerCase();
  const cleanPort = (input.port ?? '21118').trim() || '21118';
  return `direct:${cleanHost}:${cleanPort}`;
}

export function upsertRecentDevice(
  list: SavedDevice[],
  entry: Omit<SavedDevice, 'key' | 'favorite' | 'lastConnectedAt'>,
  nowMs: number,
  maxItems = MAX_SAVED_DEVICES,
): SavedDevice[] {
  const key = deviceKeyFor(entry);
  const existing = list.find((item) => item.key === key);
  const updated: SavedDevice = {
    ...entry,
    peerId: entry.peerId ? entry.peerId.replace(/\s+/g, '') : undefined,
    key,
    favorite: existing?.favorite ?? false,
    lastConnectedAt: nowMs,
  };
  const rest = list.filter((item) => item.key !== key);
  const merged = [updated, ...rest];
  if (merged.length <= maxItems) return merged;
  // Preserve favorites first if trimming old non-favorites
  const favorites = merged.filter((d) => d.favorite);
  const others = merged.filter((d) => !d.favorite);
  const keepOthers = Math.max(0, maxItems - favorites.length);
  return [...favorites, ...others.slice(0, keepOthers)].sort(
    (a, b) => b.lastConnectedAt - a.lastConnectedAt,
  );
}

export function toggleFavoriteDevice(list: SavedDevice[], key: string): SavedDevice[] {
  return list.map((item) =>
    item.key === key ? { ...item, favorite: !item.favorite } : item,
  );
}

export function removeDevice(list: SavedDevice[], key: string): SavedDevice[] {
  return list.filter((item) => item.key !== key);
}

export function loadSavedDevices(): SavedDevice[] {
  try {
    const raw = window.localStorage.getItem(DEVICES_STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (item): item is SavedDevice =>
        item &&
        typeof item === 'object' &&
        typeof item.key === 'string' &&
        (item.mode === 'id' || item.mode === 'direct'),
    );
  } catch {
    return [];
  }
}

export function persistSavedDevices(list: SavedDevice[]): void {
  try {
    window.localStorage.setItem(DEVICES_STORAGE_KEY, JSON.stringify(list));
  } catch {
    // Ignore storage quota or private-mode errors
  }
}
