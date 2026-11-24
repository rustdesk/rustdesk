export const SAVED_PASSWORDS_STORAGE_KEY = 'rd_web_saved_passwords';
export const REMEMBER_PASSWORD_PREF_KEY = 'rd_web_remember_password';

export type SavedPasswordMap = Record<string, string>;

export function normalizeSavedPasswords(raw: unknown): SavedPasswordMap {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) {
    return {};
  }
  const result: SavedPasswordMap = {};
  for (const [key, value] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof key === 'string' && key.trim() && typeof value === 'string' && value.length > 0) {
      result[key] = value;
    }
  }
  return result;
}

export function getSavedPassword(map: SavedPasswordMap, deviceKey: string): string {
  if (!deviceKey) return '';
  return map[deviceKey] ?? '';
}

export function setSavedPassword(
  map: SavedPasswordMap,
  deviceKey: string,
  password: string,
): SavedPasswordMap {
  if (!deviceKey) return map;
  if (!password) {
    return removeSavedPassword(map, deviceKey);
  }
  if (map[deviceKey] === password) return map;
  return { ...map, [deviceKey]: password };
}

export function removeSavedPassword(
  map: SavedPasswordMap,
  deviceKey: string,
): SavedPasswordMap {
  if (!deviceKey || !(deviceKey in map)) return map;
  const next = { ...map };
  delete next[deviceKey];
  return next;
}

export function loadSavedPasswords(): SavedPasswordMap {
  try {
    const raw = window.localStorage.getItem(SAVED_PASSWORDS_STORAGE_KEY);
    if (!raw) return {};
    return normalizeSavedPasswords(JSON.parse(raw));
  } catch {
    return {};
  }
}

export function persistSavedPasswords(map: SavedPasswordMap): void {
  try {
    window.localStorage.setItem(SAVED_PASSWORDS_STORAGE_KEY, JSON.stringify(map));
  } catch {
    // Ignore storage quota or private-mode errors
  }
}

export function loadRememberPasswordPref(): boolean {
  try {
    const raw = window.localStorage.getItem(REMEMBER_PASSWORD_PREF_KEY);
    if (raw === '0' || raw === 'false') return false;
    return true;
  } catch {
    return true;
  }
}

export function persistRememberPasswordPref(enabled: boolean): void {
  try {
    window.localStorage.setItem(REMEMBER_PASSWORD_PREF_KEY, enabled ? '1' : '0');
  } catch {
    // Ignore storage quota or private-mode errors
  }
}
