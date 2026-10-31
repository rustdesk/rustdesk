// Shared ArkTS helpers: JSON coercion, router params, path utilities.

import { router } from '@kit.ArkUI';

export function evStr(v: Object | undefined | null): string {
  if (v === undefined || v === null) {
    return '';
  }
  if (typeof v === 'string') {
    return v as string;
  }
  return JSON.stringify(v);
}

export function evNum(v: Object | undefined | null): number {
  if (v === undefined || v === null) {
    return 0;
  }
  if (typeof v === 'number') {
    return v as number;
  }
  if (typeof v === 'boolean') {
    return (v as boolean) ? 1 : 0;
  }
  const n = parseFloat(evStr(v));
  return isNaN(n) ? 0 : n;
}

export function evBool(v: Object | undefined | null): boolean {
  if (v === undefined || v === null) {
    return false;
  }
  if (typeof v === 'boolean') {
    return v as boolean;
  }
  const s = evStr(v);
  return s === 'true' || s === '1';
}

export function parseJsonObject(s: string): Record<string, Object> {
  const empty: Record<string, Object> = {};
  if (s.length === 0) {
    return empty;
  }
  try {
    const obj: Object = JSON.parse(s) as Object;
    return obj as Record<string, Object>;
  } catch (e) {
    return empty;
  }
}

export function parseJsonArray(s: string): Record<string, Object>[] {
  const out: Record<string, Object>[] = [];
  if (s.length === 0) {
    return out;
  }
  try {
    const raw: Object[] = JSON.parse(s) as Object[];
    for (const it of raw) {
      out.push(it as Record<string, Object>);
    }
  } catch (e) {
    // fallthrough: return what was parsed so far
  }
  return out;
}

export function arrayOfRecords(v: Object | undefined | null): Record<string, Object>[] {
  const out: Record<string, Object>[] = [];
  if (v === undefined || v === null) {
    return out;
  }
  try {
    const raw: Object[] = v as Object[];
    for (const it of raw) {
      out.push(it as Record<string, Object>);
    }
  } catch (e) {
    // ignore malformed payloads
  }
  return out;
}

// router.pushUrl params are passed through as plain objects; pages read them
// back through this helper so no cross-page class identity is required.
export function routerParams(): Record<string, Object> {
  const raw: Object = router.getParams();
  if (raw === undefined || raw === null) {
    const empty: Record<string, Object> = {};
    return empty;
  }
  return raw as Record<string, Object>;
}

export function joinPath(dir: string, name: string): string {
  if (dir.endsWith('/') || dir.endsWith('\\')) {
    return `${dir}${name}`;
  }
  return `${dir}/${name}`;
}

export function parentPath(p: string): string {
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  if (i <= 0) {
    return p;
  }
  return p.substring(0, i);
}

export function fileNameOf(p: string): string {
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  if (i < 0) {
    return p;
  }
  return p.substring(i + 1);
}

export function formatBytes(n: number): string {
  if (n < 1024) {
    return `${n} B`;
  }
  if (n < 1024 * 1024) {
    return `${(n / 1024).toFixed(1)} KB`;
  }
  if (n < 1024 * 1024 * 1024) {
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

// Chat messages that arrive while ChatPage is still opening: the page drains
// them in aboutToAppear, before its own event subscription starts.
const pendingChats: Map<number, string[]> = new Map();

export function pushPendingChat(id: number, text: string): void {
  const arr = pendingChats.get(id) ?? [];
  arr.push(text);
  pendingChats.set(id, arr);
}

export function drainPendingChats(id: number): string[] {
  const arr = pendingChats.get(id) ?? [];
  pendingChats.delete(id);
  return arr;
}
