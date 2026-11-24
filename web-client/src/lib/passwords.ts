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

const SESSION_KEY = 'rd_web_session_credentials_v1';

async function sessionEncryptionKey(): Promise<CryptoKey> {
  // A read/write transaction selects one non-exportable key even if two tabs initialize together.
  const candidate = await crypto.subtle.generateKey({ name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt']);
  return new Promise((resolve, reject) => {
    const open = indexedDB.open('rd_web_keys', 1);
    open.onupgradeneeded = () => open.result.createObjectStore('keys');
    open.onerror = () => reject(open.error);
    open.onsuccess = () => {
      const db = open.result;
      const tx = db.transaction('keys', 'readwrite');
      const store = tx.objectStore('keys');
      const read = store.get('tab-passwords');
      let key = candidate;
      read.onsuccess = () => {
        if (read.result) key = read.result;
        else store.put(candidate, 'tab-passwords');
      };
      tx.oncomplete = () => { db.close(); resolve(key); };
      tx.onabort = () => { db.close(); reject(tx.error); };
    };
  });
}

export function forgetTabPasswords() {
  try { sessionStorage.removeItem(SESSION_KEY); } catch { /* Browser storage unavailable. */ }
}

function bytesFromBase64(value: string) {
  return Uint8Array.from(atob(value), char => char.charCodeAt(0));
}

export async function restoreTabPasswords(): Promise<SavedPasswordMap | null> {
  const stored = sessionStorage.getItem(SESSION_KEY);
  if (!stored) return null;
  const { iv, data } = JSON.parse(stored);
  const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: bytesFromBase64(iv) },
    await sessionEncryptionKey(), bytesFromBase64(data));
  const passwords = JSON.parse(new TextDecoder().decode(plain));
  if (!passwords || Array.isArray(passwords) || typeof passwords !== 'object' ||
      !Object.values(passwords).every(value => typeof value === 'string')) throw new Error('Invalid saved credentials');
  return passwords;
}

export async function persistTabPasswords(passwords: SavedPasswordMap, current: () => boolean) {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const encrypted = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, await sessionEncryptionKey(),
    new TextEncoder().encode(JSON.stringify(passwords)));
  const base64 = (bytes: Uint8Array) => btoa(Array.from(bytes, byte => String.fromCharCode(byte)).join(''));
  if (current()) sessionStorage.setItem(SESSION_KEY, JSON.stringify({ iv: base64(iv), data: base64(new Uint8Array(encrypted)) }));
}
