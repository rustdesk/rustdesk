export const SAVED_PASSWORDS_STORAGE_KEY = 'rd_web_saved_passwords';
export const REMEMBER_PASSWORD_PREF_KEY = 'rd_web_remember_password';

export type SavedPasswordMap = Record<string, string>;

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

export function clearLegacyPasswordStorage(): void {
  try {
    window.localStorage.removeItem(SAVED_PASSWORDS_STORAGE_KEY);
    window.localStorage.removeItem(REMEMBER_PASSWORD_PREF_KEY);
  } catch {
    // Some browsers block all access to local storage.
  }
}
